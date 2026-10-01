//! Modrinth v2 adapter. API DTOs and URL authority stay inside this module.
//! Every version is checked against the instance before it becomes a plan.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::downloads::{DownloadOptions, Sha512ArtifactSource};
use crate::instance_content::{
    ContentCompatibility, ContentState, ContentType, DependencyKind, ProviderArtifactSource,
    ProviderDependency, ProviderIdentity, ProviderInstallPlan, ProviderRecord,
};

const BASE: &str = "https://api.modrinth.com/v2/";
const FABRIC_API_PROJECT: &str = "P7dR8mSH";
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
const MAX_GRAPH: usize = 64;
/// Recognition hash lookups are batched; the audit bound keeps one request
/// and one bounded response body per lookup round.
const MAX_HASH_BATCH: usize = 64;

#[derive(Debug, Clone)]
pub struct Context {
    pub minecraft_version: String,
    pub loader: String,
    pub fabric_api_protected: bool,
}

/// Provider content types a user can browse. A superset of the installable
/// `ContentType`: modpacks browse with strong provider identity but do not
/// install (Phase I will attach that pipeline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowseKind {
    Mod,
    Modpack,
    ResourcePack,
    ShaderPack,
}

impl BrowseKind {
    pub fn project_type(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::ResourcePack => "resourcepack",
            Self::ShaderPack => "shader",
        }
    }

    pub fn content_type(self) -> Option<ContentType> {
        match self {
            Self::Mod => Some(ContentType::Mod),
            Self::Modpack => None,
            Self::ResourcePack => Some(ContentType::ResourcePack),
            Self::ShaderPack => Some(ContentType::ShaderPack),
        }
    }

    pub fn from_content_type(kind: ContentType) -> Self {
        match kind {
            ContentType::Mod => Self::Mod,
            ContentType::ResourcePack => Self::ResourcePack,
            ContentType::ShaderPack => Self::ShaderPack,
        }
    }
}

/// Provider-supported browse sort orders. Relevance is the provider default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowseSort {
    Relevance,
    Downloads,
    Newest,
    Updated,
}

impl BrowseSort {
    fn query_value(self) -> Option<&'static str> {
        match self {
            Self::Relevance => None,
            Self::Downloads => Some("downloads"),
            Self::Newest => Some("newest"),
            Self::Updated => Some("updated"),
        }
    }
}

/// One live provider category tag, scoped by the provider's own project-type
/// attribution. Never a hardcoded taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCategory {
    pub name: String,
    pub project_type: String,
}

#[derive(Debug, Clone)]
pub struct Client {
    base: Url,
    http: reqwest::Client,
}

impl Client {
    pub fn official() -> Self {
        static OFFICIAL: OnceLock<Client> = OnceLock::new();
        OFFICIAL
            .get_or_init(|| Self {
                base: Url::parse(BASE).expect("fixed official Modrinth API URL"),
                http: crate::downloads::build_client(&DownloadOptions::default()),
            })
            .clone()
    }

    #[cfg(test)]
    pub(crate) fn for_testing(base: &str) -> Self {
        Self {
            base: Url::parse(base).unwrap(),
            http: crate::downloads::build_client(&DownloadOptions::default()),
        }
    }

    fn endpoint(&self, segments: &[&str]) -> Result<Url, Error> {
        let mut url = self.base.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| Error::InvalidResponse)?;
            path.pop_if_empty();
            for segment in segments {
                if segment.is_empty()
                    || segment.contains(['/', '\\'])
                    || segment == &"."
                    || segment == &".."
                {
                    return Err(Error::InvalidResponse);
                }
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, Error> {
        let mut response = self
            .http
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                concat!(
                    "kalibsolomon-pixel/aurora-launcher/",
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .send()
            .await
            .map_err(|_| Error::Network)?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let reset_seconds = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            return Err(Error::RateLimited(reset_seconds));
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NotFound);
        }
        if !response.status().is_success() {
            return Err(Error::Network);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(Error::InvalidResponse);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if body.len() + chunk.len() > MAX_RESPONSE {
                return Err(Error::InvalidResponse);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| Error::InvalidResponse)
    }

    /// POST with the same transport guards as GET: one attempt, rate-limit
    /// surfacing, bounded body. No automatic retry behavior.
    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        url: Url,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let payload = serde_json::to_vec(body).map_err(|_| Error::InvalidRequest)?;
        let mut response = self
            .http
            .post(url)
            .header(
                reqwest::header::USER_AGENT,
                concat!(
                    "kalibsolomon-pixel/aurora-launcher/",
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|_| Error::Network)?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let reset_seconds = response
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            return Err(Error::RateLimited(reset_seconds));
        }
        if !response.status().is_success() {
            return Err(Error::Network);
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE as u64)
        {
            return Err(Error::InvalidResponse);
        }
        let mut payload = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
            if payload.len() + chunk.len() > MAX_RESPONSE {
                return Err(Error::InvalidResponse);
            }
            payload.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&payload).map_err(|_| Error::InvalidResponse)
    }

    /// Provider-neutral browse search over the official facets API. The
    /// instance's Minecraft version stays the binding compatibility facet;
    /// categories, sort and search text are user filters. The response is
    /// re-validated against the requested project type and version.
    pub async fn search_browse(
        &self,
        context: &Context,
        kind: BrowseKind,
        query: &str,
        categories: &[String],
        sort: BrowseSort,
        offset: u32,
    ) -> Result<SearchPage, Error> {
        if context.loader != "fabric" {
            return Err(Error::NoCompatibleVersion);
        }
        if query.len() > 160 || offset > 10_000 || categories.len() > 8 {
            return Err(Error::InvalidRequest);
        }
        for category in categories {
            if category.is_empty()
                || category.len() > 48
                || !category
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(Error::InvalidRequest);
            }
        }
        let mut url = self.endpoint(&["search"])?;
        let project_type = kind.project_type();
        let mut facets = vec![vec![format!("project_type:{project_type}")]];
        if kind != BrowseKind::Modpack {
            facets.push(vec![format!("versions:{}", context.minecraft_version)]);
        }
        if kind == BrowseKind::Mod {
            facets.push(vec![format!("categories:{}", context.loader)]);
            facets.push(
                CLIENT_ENVIRONMENTS
                    .iter()
                    .map(|value| format!("environment:{value}"))
                    .collect(),
            );
        }
        if kind == BrowseKind::ResourcePack {
            facets.push(vec!["loaders:minecraft".to_owned()]);
        }
        // Thematic categories are separate facet groups (AND between groups);
        // loader remains its own facet, never mixed into a category group.
        for category in categories {
            facets.push(vec![format!("categories:{category}")]);
        }
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("query", query.trim());
            pairs.append_pair(
                "facets",
                &serde_json::to_string(&facets).expect("facets serialize"),
            );
            if let Some(sort_value) = sort.query_value() {
                pairs.append_pair("sort", sort_value);
            }
            pairs.append_pair("limit", "20");
            pairs.append_pair("offset", &offset.to_string());
        }
        let result: SearchDto = self.get(url).await?;
        Ok(SearchPage {
            offset: result.offset,
            total_hits: result.total_hits,
            hits: result
                .hits
                .into_iter()
                .filter(|hit| {
                    hit.project_type == project_type
                        && (kind == BrowseKind::Modpack
                            || hit
                                .versions
                                .iter()
                                .any(|version| version == &context.minecraft_version))
                        && (kind != BrowseKind::Mod
                            || hit.environment.iter().any(|env| client_environment(env)))
                })
                .map(|hit| ProjectSummary {
                    project_id: hit.project_id,
                    title: hit.title,
                    summary: hit.description,
                    author: hit.author,
                    downloads: hit.downloads,
                    icon_url: hit.icon_url.as_deref().and_then(safe_icon_url),
                    project_type: kind,
                    categories: hit.display_categories,
                })
                .collect(),
        })
    }

    /// Compatibility wrapper for the existing mod/pack search callers.
    pub async fn search(
        &self,
        context: &Context,
        kind: ContentType,
        query: &str,
        offset: u32,
    ) -> Result<SearchPage, Error> {
        self.search_browse(
            context,
            BrowseKind::from_content_type(kind),
            query,
            &[],
            BrowseSort::Relevance,
            offset,
        )
        .await
    }

    /// Live provider category tags, fetched once per provider host per
    /// process and reused. No polling; the provider's own project-type
    /// attribution scopes relevance.
    pub async fn categories(&self) -> Result<Vec<ProviderCategory>, Error> {
        static CATEGORIES: OnceLock<std::sync::Mutex<HashMap<String, Vec<ProviderCategory>>>> =
            OnceLock::new();
        let cache = CATEGORIES.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
        let host = format!(
            "{}:{}",
            self.base.host_str().unwrap_or_default(),
            self.base.port().unwrap_or(0)
        );
        if let Some(cached) = cache.lock().unwrap().get(&host) {
            return Ok(cached.clone());
        }
        let url = self.endpoint(&["tag", "category"])?;
        let tags: Vec<CategoryTagDto> = self.get(url).await?;
        let categories: Vec<ProviderCategory> = tags
            .into_iter()
            .filter(|tag| !tag.name.is_empty() && tag.name.len() <= 48)
            .map(|tag| ProviderCategory {
                name: tag.name,
                project_type: tag.project_type.unwrap_or_else(|| "mod".to_owned()),
            })
            .collect();
        cache.lock().unwrap().insert(host, categories.clone());
        Ok(categories)
    }

    async fn project(&self, id: &str) -> Result<ProjectDto, Error> {
        validate_id(id)?;
        self.get(self.endpoint(&["project", id])?).await
    }

    async fn version(&self, id: &str) -> Result<VersionDto, Error> {
        validate_id(id)?;
        self.get(self.endpoint(&["version", id])?).await
    }

    /// Crate-level access to one exact version document for acceptance
    /// evidence; never part of the public command surface.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn version_document(&self, id: &str) -> Result<VersionDto, Error> {
        self.version(id).await
    }

    /// Resolve an exact Modrinth modpack version from provider authority.
    /// The frontend supplies identity only; neither a URL nor a digest can
    /// cross into this source. The selected version is never substituted.
    pub async fn pack_artifact(
        &self,
        project_id: &str,
        version_id: &str,
    ) -> Result<PackArtifact, Error> {
        let project = self.project(project_id).await?;
        if project.project_type != "modpack" {
            return Err(Error::InvalidRequest);
        }
        let version = self.version(version_id).await?;
        if version.project_id != project.id {
            return Err(Error::InvalidRequest);
        }
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| {
                version
                    .files
                    .iter()
                    .find(|file| file.filename.ends_with(".mrpack"))
            })
            .filter(|file| file.filename.to_ascii_lowercase().ends_with(".mrpack") && file.size > 0)
            .ok_or(Error::InvalidResponse)?;
        let source = self.official_file_source(&file.url, &file.hashes.sha512, file.size)?;
        Ok(PackArtifact {
            project_id: project.id,
            version_id: version.id,
            title: project.title,
            version_number: version.version_number,
            game_versions: version.game_versions,
            loaders: version.loaders,
            sha512: file.hashes.sha512.to_ascii_lowercase(),
            source,
        })
    }

    /// Turn a hash-recognized pack entry into a normal provider plan, using
    /// Modrinth's file URL rather than the URL embedded in the pack.
    pub async fn pack_component_plan(
        &self,
        recognized: &RecognizedFile,
        destination: &str,
        expected_size: u64,
        _minecraft_version: &str,
    ) -> Result<ProviderInstallPlan, Error> {
        let (directory, file_name) = destination.split_once('/').ok_or(Error::InvalidRequest)?;
        let kind = match directory {
            "mods" => ContentType::Mod,
            "resourcepacks" => ContentType::ResourcePack,
            "shaderpacks" => ContentType::ShaderPack,
            _ => return Err(Error::InvalidRequest),
        };
        if file_name.contains('/')
            || crate::instance_content::validate_file_name(file_name).is_err()
        {
            return Err(Error::InvalidRequest);
        }
        let project = self.project(&recognized.project_id).await?;
        if content_type(&project.project_type)? != kind
            || recognized.file_size != expected_size
            || (kind == ContentType::Mod
                && (!recognized.loaders.iter().any(|loader| loader == "fabric")
                    || !pack_client_environment(&recognized.environment)))
        {
            return Err(Error::DependencyConflict);
        }
        let source = self.official_file_source(
            &recognized.official_url,
            &recognized.queried_sha512,
            recognized.file_size,
        )?;
        Ok(ProviderInstallPlan {
            content_type: kind,
            provider: "modrinth".into(),
            project_id: recognized.project_id.clone(),
            version_id: recognized.version_id.clone(),
            file_id: recognized.queried_sha512.clone(),
            file_name: file_name.into(),
            display_version: Some(recognized.version_number.clone()),
            compatibility: ContentCompatibility {
                minecraft_versions: recognized.game_versions.clone(),
                loader: (kind == ContentType::Mod).then(|| "fabric".into()),
                environment: Some(recognized.environment.clone()),
            },
            dependencies: recognized.dependencies.clone(),
            source: ProviderArtifactSource::Sha512(source),
        })
    }

    fn official_file_source(
        &self,
        raw: &str,
        sha512: &str,
        size: u64,
    ) -> Result<Sha512ArtifactSource, Error> {
        let url = Url::parse(raw).map_err(|_| Error::InvalidResponse)?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Error::InvalidResponse);
        }
        if url.scheme() == "https"
            && url.host_str() == Some("cdn.modrinth.com")
            && url.port().is_none()
        {
            return Sha512ArtifactSource::https(raw, sha512, Some(size))
                .map_err(|_| Error::InvalidResponse);
        }
        #[cfg(test)]
        if self.base.scheme() == "http"
            && crate::downloads::is_loopback_host(&self.base)
            && url.scheme() == "http"
            && crate::downloads::is_loopback_host(&url)
        {
            return Sha512ArtifactSource::loopback_http_for_testing(raw, sha512, Some(size))
                .map_err(|_| Error::InvalidResponse);
        }
        Err(Error::InvalidResponse)
    }

    async fn versions(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
    ) -> Result<Vec<VersionDto>, Error> {
        self.versions_browse(context, BrowseKind::from_content_type(kind), project_id)
            .await
    }

    async fn versions_browse(
        &self,
        context: &Context,
        kind: BrowseKind,
        project_id: &str,
    ) -> Result<Vec<VersionDto>, Error> {
        validate_id(project_id)?;
        let mut url = self.endpoint(&["project", project_id, "version"])?;
        {
            let mut pairs = url.query_pairs_mut();
            if kind != BrowseKind::Modpack {
                pairs.append_pair(
                    "game_versions",
                    &serde_json::to_string(&[&context.minecraft_version]).unwrap(),
                );
            }
            if kind == BrowseKind::Mod {
                pairs.append_pair(
                    "loaders",
                    &serde_json::to_string(&[&context.loader]).unwrap(),
                );
            } else if kind == BrowseKind::ResourcePack {
                pairs.append_pair("loaders", "[\"minecraft\"]");
            }
            pairs.append_pair("include_changelog", "false");
        }
        let versions: Vec<VersionDto> = self.get(url).await?;
        Ok(versions
            .into_iter()
            .filter(|version| compatible_browse(context, kind, version))
            .collect())
    }

    pub async fn details(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
    ) -> Result<ProjectDetails, Error> {
        self.details_browse(context, BrowseKind::from_content_type(kind), project_id)
            .await
    }

    /// Details for any browsable kind, including modpacks. Modpack versions
    /// carry strong provider identity only; nothing here plans an install.
    pub async fn details_browse(
        &self,
        context: &Context,
        kind: BrowseKind,
        project_id: &str,
    ) -> Result<ProjectDetails, Error> {
        let project = self.project(project_id).await?;
        if project.project_type != kind.project_type() {
            return Err(Error::NoCompatibleVersion);
        }
        let mut versions = self.versions_browse(context, kind, &project.id).await?;
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        let choices: Vec<_> = versions
            .iter()
            .map(|version| VersionChoice {
                id: version.id.clone(),
                name: version.name.clone(),
                version_number: version.version_number.clone(),
                version_type: version.version_type.clone(),
                date_published: version.date_published.clone(),
                environment: version.environment.clone(),
                loaders: version.loaders.clone(),
            })
            .collect();
        let default_version_id = choices
            .iter()
            .find(|item| item.version_type == "release")
            .or_else(|| choices.first())
            .map(|item| item.id.clone());
        Ok(ProjectDetails {
            project_id: project.id,
            title: project.title,
            summary: project.description,
            license: project.license.id,
            game_versions: project.game_versions,
            loaders: project.loaders,
            environments: project.environment,
            versions: choices,
            default_version_id,
            project_type: kind,
        })
    }

    /// Cosmetic metadata only, using the same project authority and CDN policy as Browse.
    pub async fn artwork(&self, project_id: &str) -> Result<Option<String>, Error> {
        Ok(self
            .project(project_id)
            .await?
            .icon_url
            .as_deref()
            .and_then(safe_icon_url))
    }

    /// The declared project type and title for recognition previews. The
    /// project document is the authority for which content type a project
    /// publishes; version documents do not carry it.
    pub async fn project_type_and_title(
        &self,
        project_id: &str,
    ) -> Result<(String, String), Error> {
        let project = self.project(project_id).await?;
        Ok((project.project_type, project.title))
    }

    pub async fn resolve(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
        version_id: &str,
        installed: &ContentState,
    ) -> Result<Resolved, Error> {
        self.resolve_inner(context, kind, project_id, version_id, installed, None)
            .await
    }

    pub async fn resolve_update(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
        version_id: &str,
        installed: &ContentState,
    ) -> Result<Resolved, Error> {
        self.resolve_updates(
            context,
            &[(kind, project_id.to_owned(), version_id.to_owned())],
            installed,
        )
        .await
    }

    /// Resolve one coherent multi-root update graph. Every target must be an
    /// installed managed record; within this graph a required dependency may
    /// advance to the version the updated roots require, which the state
    /// planner later validates against retention and outside dependents.
    /// Conflicting requirements for one project still fail resolution.
    pub async fn resolve_updates(
        &self,
        context: &Context,
        targets: &[(ContentType, String, String)],
        installed: &ContentState,
    ) -> Result<Resolved, Error> {
        if targets.is_empty() || targets.len() > MAX_GRAPH {
            return Err(Error::InvalidRequest);
        }
        let mut replacing = HashSet::new();
        for (kind, project_id, _) in targets {
            let identity = ProviderIdentity {
                content_type: *kind,
                provider: "modrinth".into(),
                project_id: project_id.clone(),
            };
            if installed.find(&identity).is_none() {
                return Err(Error::InvalidRequest);
            }
            if !replacing.insert(identity) {
                return Err(Error::InvalidRequest);
            }
        }
        self.resolve_multi_inner(context, targets, installed, replacing)
            .await
    }

    async fn resolve_inner(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
        version_id: &str,
        installed: &ContentState,
        replacing: Option<ProviderIdentity>,
    ) -> Result<Resolved, Error> {
        self.resolve_multi_inner(
            context,
            &[(kind, project_id.to_owned(), version_id.to_owned())],
            installed,
            replacing.into_iter().collect(),
        )
        .await
    }

    async fn resolve_multi_inner(
        &self,
        context: &Context,
        targets: &[(ContentType, String, String)],
        installed: &ContentState,
        replacing: HashSet<ProviderIdentity>,
    ) -> Result<Resolved, Error> {
        let mut graph = Graph {
            context,
            client: self,
            installed,
            plans: Vec::new(),
            items: Vec::new(),
            visiting: HashSet::new(),
            titles: HashMap::new(),
            seen: HashMap::new(),
            warnings: Vec::new(),
            replacing,
        };
        for (kind, project_id, version_id) in targets {
            graph
                .visit(
                    project_id.clone(),
                    Some(version_id.clone()),
                    Some(*kind),
                    true,
                )
                .await?;
        }
        if graph.plans.is_empty() && graph.items.is_empty() {
            return Err(Error::NoCompatibleVersion);
        }
        let (kind, project_id, version_id) = &targets[0];
        let preview = InstallPreview {
            inventory_revision: None,
            project_id: project_id.clone(),
            version_id: version_id.clone(),
            content_type: *kind,
            items: graph.items,
            warnings: graph.warnings,
        };
        Ok(Resolved {
            preview,
            plans: graph.plans,
        })
    }

    /// Select the same release-preferred compatible version shown by Details.
    /// Search hits are display data and never authorize a file or version.
    pub async fn resolve_latest(
        &self,
        context: &Context,
        kind: ContentType,
        project_id: &str,
        installed: &ContentState,
    ) -> Result<Resolved, Error> {
        let details = self.details(context, kind, project_id).await?;
        let version_id = details
            .default_version_id
            .ok_or(Error::NoCompatibleVersion)?;
        self.resolve(context, kind, project_id, &version_id, installed)
            .await
    }

    /// Publication time orders versions; the persisted release-channel policy
    /// selects which version types are eligible. Version strings are never
    /// compared. Returns the discovery evidence for one managed record.
    pub async fn update_discovery(
        &self,
        context: &Context,
        installed: &ProviderRecord,
    ) -> Result<UpdateDiscovery, Error> {
        if installed.provider != "modrinth" {
            return Err(Error::InvalidRequest);
        }
        let versions = self
            .versions(context, installed.content_type, &installed.project_id)
            .await?;
        // The installed exact version identity anchors ordering. It is
        // normally part of the compatibility-filtered list; the single-version
        // fallback covers a record whose version is not compatible with the
        // current instance context.
        let current = match versions
            .iter()
            .find(|version| version.id == installed.version_id)
        {
            Some(current) => current.clone(),
            None => self.version(&installed.version_id).await?,
        };
        if current.project_id != installed.project_id
            || choose_file(&current, installed.content_type)?.hashes.sha512 != installed.file_id
        {
            return Err(Error::InvalidResponse);
        }
        let channel = installed.update_channel;
        let mut newer: Vec<&VersionDto> = versions
            .iter()
            .filter(|candidate| {
                candidate.project_id == installed.project_id
                    && candidate.date_published > current.date_published
            })
            .collect();
        newer.sort_by(|a, b| {
            b.date_published
                .cmp(&a.date_published)
                .then_with(|| b.id.cmp(&a.id))
        });
        let newer_compatible_exists = !newer.is_empty();
        let candidate = newer
            .into_iter()
            .find(|version| channel.allows(&version.version_type))
            .map(|version| VersionChoice {
                id: version.id.clone(),
                name: version.name.clone(),
                version_number: version.version_number.clone(),
                version_type: version.version_type.clone(),
                date_published: version.date_published.clone(),
                environment: version.environment.clone(),
                loaders: version.loaders.clone(),
            });
        Ok(UpdateDiscovery {
            current_version_type: current.version_type,
            candidate,
            newer_compatible_exists,
        })
    }

    /// The newest policy-allowed compatible update candidate, if any.
    pub async fn update_candidate(
        &self,
        context: &Context,
        installed: &ProviderRecord,
    ) -> Result<Option<VersionChoice>, Error> {
        Ok(self.update_discovery(context, installed).await?.candidate)
    }

    /// Single-hash version-file lookup. `algorithm=sha512` is Aurora's
    /// preferred recognition digest; the returned version must actually
    /// publish the queried hash or the response is rejected.
    async fn version_file_by_hash(&self, sha512: &str) -> Result<VersionDto, Error> {
        let hash = validate_sha512(sha512)?;
        let mut url = self.endpoint(&["version_file", &hash])?;
        url.query_pairs_mut().append_pair("algorithm", "sha512");
        self.get(url).await
    }

    /// Batched version-file lookup, at most [`MAX_HASH_BATCH`] hashes per
    /// request. The response is verified hash-by-hash: every returned version
    /// must publish one of the queried digests among its files.
    async fn version_files_by_hashes(
        &self,
        hashes: &[String],
    ) -> Result<Vec<(String, VersionDto)>, Error> {
        let mut url = self.endpoint(&["version_files"])?;
        let body = serde_json::json!({
            "algorithm": "sha512",
            "hashes": hashes,
        });
        url.query_pairs_mut().append_pair("algorithm", "sha512");
        // The current API answers an object keyed by the queried hash whose
        // values are versions; an unrecognized key never appears. Each key
        // must be one of the queried digests or the response is rejected.
        let mapped: HashMap<String, VersionDto> = self.post_json(url, &body).await?;
        let mut versions = Vec::with_capacity(mapped.len());
        for (hash, version) in mapped {
            if !hashes
                .iter()
                .any(|queried| queried.eq_ignore_ascii_case(&hash))
            {
                return Err(Error::InvalidResponse);
            }
            versions.push((hash, version));
        }
        Ok(versions)
    }

    /// Recognize local content by SHA-512 against the version-file lookup
    /// APIs. A hash the provider does not know is simply absent from the
    /// result; a response claiming identity without publishing the queried
    /// bytes is rejected. The result is normalized adapter data, never raw
    /// provider JSON.
    pub async fn lookup_files(&self, hashes: &[String]) -> Result<Vec<RecognizedFile>, Error> {
        let mut unique: Vec<String> = Vec::new();
        for hash in hashes {
            let canonical = validate_sha512(hash)?;
            if !unique.contains(&canonical) {
                unique.push(canonical);
            }
        }
        let mut recognized = Vec::new();
        for chunk in unique.chunks(MAX_HASH_BATCH) {
            let versions = if chunk.len() == 1 {
                match self.version_file_by_hash(&chunk[0]).await {
                    Ok(version) => vec![(chunk[0].clone(), version)],
                    Err(Error::NotFound) => Vec::new(),
                    Err(error) => return Err(error),
                }
            } else {
                self.version_files_by_hashes(chunk).await?
            };
            for (hash, version) in versions {
                recognized.push(verified_recognition(&[hash], version)?);
            }
        }
        Ok(recognized)
    }
}

/// A provider response that establishes file identity. Every field comes from
/// the verified response for the exact queried hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognizedFile {
    pub queried_sha512: String,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
    pub version_name: String,
    pub version_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub environment: String,
    pub dependencies: Vec<ProviderDependency>,
    pub file_name: String,
    pub file_size: u64,
    #[serde(skip_serializing)]
    pub(crate) official_url: String,
}

fn validate_sha512(hash: &str) -> Result<String, Error> {
    if hash.len() == 128 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(hash.to_ascii_lowercase())
    } else {
        Err(Error::InvalidRequest)
    }
}

/// Response verification: identify the returned provider file whose published
/// SHA-512 equals the queried digest. Algorithm semantics are explicit — the
/// queried value is compared only against `sha512` fields, never a hash of
/// another algorithm.
fn verified_recognition(queried: &[String], version: VersionDto) -> Result<RecognizedFile, Error> {
    let file = version
        .files
        .iter()
        .find(|file| {
            queried
                .iter()
                .any(|hash| file.hashes.sha512.eq_ignore_ascii_case(hash))
        })
        .ok_or(Error::InvalidResponse)?;
    let queried_sha512 = file.hashes.sha512.to_ascii_lowercase();
    let mut dependencies = Vec::new();
    for dependency in &version.dependencies {
        if dependency.dependency_type == "embedded" {
            continue;
        }
        let Some(project_id) = &dependency.project_id else {
            continue;
        };
        if project_id.is_empty() {
            continue;
        }
        dependencies.push(ProviderDependency {
            kind: match dependency.dependency_type.as_str() {
                "required" => DependencyKind::Required,
                "optional" => DependencyKind::Optional,
                "incompatible" => DependencyKind::Incompatible,
                _ => return Err(Error::InvalidResponse),
            },
            provider: "modrinth".into(),
            project_id: project_id.clone(),
            version_id: dependency.version_id.clone(),
        });
    }
    Ok(RecognizedFile {
        queried_sha512,
        project_id: version.project_id,
        version_id: version.id,
        version_number: version.version_number,
        version_name: version.name,
        version_type: version.version_type,
        date_published: version.date_published,
        game_versions: version.game_versions,
        loaders: version.loaders,
        environment: version.environment,
        dependencies,
        file_name: file.filename.clone(),
        file_size: file.size,
        official_url: file.url.clone(),
    })
}

fn safe_icon_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("cdn.modrinth.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with("/data/")
    {
        return None;
    }
    Some(url.into())
}

const CLIENT_ENVIRONMENTS: &[&str] = &[
    "client_and_server",
    "client_only",
    "client_only_server_optional",
    "singleplayer_only",
    "client_or_server",
    "client_or_server_prefers_both",
];

fn client_environment(value: &str) -> bool {
    CLIENT_ENVIRONMENTS.contains(&value) || value == "unknown"
}

/// A pack's explicit client file requirement can include a provider version
/// whose client installation is optional (or useful for singleplayer). Only
/// dedicated-server-only files are incompatible with a physical client.
fn pack_client_environment(value: &str) -> bool {
    client_environment(value) || matches!(value, "server_only" | "server_only_client_optional")
}

fn content_type(project_type: &str) -> Result<ContentType, Error> {
    match project_type {
        "mod" => Ok(ContentType::Mod),
        "resourcepack" => Ok(ContentType::ResourcePack),
        "shader" => Ok(ContentType::ShaderPack),
        _ => Err(Error::DependencyUnresolved),
    }
}

fn compatible(context: &Context, kind: ContentType, version: &VersionDto) -> bool {
    compatible_browse(context, BrowseKind::from_content_type(kind), version)
}

fn compatible_browse(context: &Context, kind: BrowseKind, version: &VersionDto) -> bool {
    if kind == BrowseKind::Modpack {
        return version.loaders.iter().any(|loader| loader == "fabric");
    }
    if context.loader != "fabric" {
        return false;
    }
    version
        .game_versions
        .iter()
        .any(|value| value == &context.minecraft_version)
        && match kind {
            BrowseKind::Mod => {
                version.loaders.iter().any(|value| value == &context.loader)
                    && client_environment(&version.environment)
            }
            BrowseKind::ResourcePack => version.loaders.iter().any(|value| value == "minecraft"),
            BrowseKind::ShaderPack | BrowseKind::Modpack => true,
        }
}

fn validate_id(id: &str) -> Result<(), Error> {
    if id.len() == 8 && id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Ok(())
    } else {
        Err(Error::InvalidRequest)
    }
}

fn choose_file(version: &VersionDto, kind: ContentType) -> Result<&FileDto, Error> {
    let extension = if kind == ContentType::Mod {
        ".jar"
    } else {
        ".zip"
    };
    let eligible = |file: &&FileDto| {
        file.filename.to_ascii_lowercase().ends_with(extension)
            && !matches!(
                file.file_type.as_deref(),
                Some("sources-jar" | "dev-jar" | "javadoc-jar" | "signature")
            )
            && crate::instance_content::validate_file_name(&file.filename).is_ok()
            && file.size > 0
    };
    if let Some(primary) = version.files.iter().find(|file| file.primary) {
        return eligible(&primary)
            .then_some(primary)
            .ok_or(Error::InvalidResponse);
    }
    version
        .files
        .iter()
        .filter(eligible)
        .next()
        .ok_or(Error::InvalidResponse)
}

struct Graph<'a> {
    context: &'a Context,
    client: &'a Client,
    installed: &'a ContentState,
    plans: Vec<ProviderInstallPlan>,
    items: Vec<PreviewItem>,
    visiting: HashSet<String>,
    titles: HashMap<String, String>,
    seen: HashMap<String, String>,
    warnings: Vec<String>,
    /// Update roots whose installed records this graph may supersede. In an
    /// update graph a required dependency may also advance to the version the
    /// targets require; the state planner owns the retention/dependent
    /// safety checks. An empty set is a plain install graph.
    replacing: HashSet<ProviderIdentity>,
}

impl Graph<'_> {
    fn visit<'a>(
        &'a mut self,
        project_id: String,
        version_id: Option<String>,
        requested_kind: Option<ContentType>,
        root: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            validate_id(&project_id)?;
            if self.visiting.contains(&project_id) {
                return Err(Error::DependencyCycle);
            }
            if self.seen.contains_key(&project_id) {
                if version_id
                    .as_ref()
                    .is_some_and(|wanted| self.seen.get(&project_id) != Some(wanted))
                {
                    return Err(Error::DependencyUnresolved);
                }
                return Ok(());
            }
            if self.seen.len() >= MAX_GRAPH {
                return Err(Error::DependencyUnresolved);
            }
            if self.context.fabric_api_protected && project_id == FABRIC_API_PROJECT && !root {
                if version_id.is_some() {
                    return Err(Error::DependencyUnresolved);
                }
                self.warnings
                    .push("Fabric API is already present as verified bootstrap content; no provider ownership is assigned.".into());
                return Ok(());
            }
            let project = self.client.project(&project_id).await.map_err(|error| {
                if root {
                    error
                } else {
                    Error::DependencyUnresolved
                }
            })?;
            self.titles
                .insert(project.id.clone(), project.title.clone());
            let kind = content_type(&project.project_type)?;
            if requested_kind.is_some_and(|wanted| wanted != kind) {
                return Err(if root {
                    Error::NoCompatibleVersion
                } else {
                    Error::DependencyUnresolved
                });
            }
            let installed_version = (!root && version_id.is_none())
                .then(|| {
                    self.installed.entries.iter().find(|record| {
                        record.provider == "modrinth"
                            && record.project_id == project.id
                            && record.content_type == kind
                    })
                })
                .flatten()
                .map(|record| record.version_id.clone());
            let version = if let Some(id) = version_id.as_ref().or(installed_version.as_ref()) {
                let version = self.client.version(id).await?;
                if version.project_id != project.id || !compatible(self.context, kind, &version) {
                    return Err(if root {
                        Error::NoCompatibleVersion
                    } else {
                        Error::DependencyUnresolved
                    });
                }
                version
            } else {
                let mut versions = self
                    .client
                    .versions(self.context, kind, &project.id)
                    .await?;
                versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
                versions
                    .iter()
                    .find(|item| item.version_type == "release")
                    .or_else(|| versions.first())
                    .cloned()
                    .ok_or(Error::DependencyUnresolved)?
            };
            let file = choose_file(&version, kind)?;
            let source =
                Sha512ArtifactSource::https(&file.url, &file.hashes.sha512, Some(file.size))
                    .map_err(|_| Error::InvalidResponse)?;
            // Install graphs refuse to swap an installed dependency version.
            // Update graphs plan the swap instead; the state planner then
            // enforces retention and outside-dependent safety before any
            // transaction is proposed.
            if let Some(record) = self.installed.entries.iter().find(|record| {
                record.provider == "modrinth"
                    && record.project_id == project.id
                    && record.version_id != version.id
                    && record.content_type == kind
                    && self.replacing.is_empty()
            }) {
                return Err(Error::DependencyVersionConflict(format!(
                    "{} requires {} provider version {}. Installed: {} (version ID {}) in {}. Review an explicit provider update; no dependency is replaced or downgraded automatically.",
                    self.visiting
                        .iter()
                        .min()
                        .and_then(|id| self.titles.get(id))
                        .map(String::as_str)
                        .unwrap_or("Requested content"),
                    project.title,
                    version.version_number,
                    record.display_version.as_deref().unwrap_or("unknown"),
                    record.version_id,
                    record.file_name
                )));
            }
            let already = self.installed.entries.iter().any(|record| {
                record.provider == "modrinth"
                    && record.project_id == project.id
                    && record.version_id == version.id
                    && record.content_type == kind
                    && !self.replacing.contains(&record.identity())
            });
            self.visiting.insert(project.id.clone());
            let mut dependencies = Vec::new();
            for dependency in &version.dependencies {
                let dep_project = if let Some(id) = &dependency.project_id {
                    Some(id.clone())
                } else if let Some(id) = &dependency.version_id {
                    Some(self.client.version(id).await?.project_id)
                } else {
                    None
                };
                if let Some(id) = &dep_project {
                    if dependency.dependency_type != "embedded" {
                        dependencies.push(ProviderDependency {
                            kind: match dependency.dependency_type.as_str() {
                                "required" => DependencyKind::Required,
                                "optional" => DependencyKind::Optional,
                                "incompatible" => DependencyKind::Incompatible,
                                _ => return Err(Error::InvalidResponse),
                            },
                            provider: "modrinth".into(),
                            project_id: id.clone(),
                            version_id: dependency.version_id.clone(),
                        });
                    }
                }
                match dependency.dependency_type.as_str() {
                    "required" => {
                        let dep_project = dep_project.ok_or(Error::DependencyUnresolved)?;
                        self.visit(dep_project, dependency.version_id.clone(), None, false)
                            .await?;
                    }
                    "optional" => self.warnings.push(format!(
                        "Optional dependency {} is not installed automatically.",
                        dep_project.as_deref().unwrap_or("external")
                    )),
                    "incompatible" => {
                        if dep_project.as_ref().is_some_and(|id| {
                            self.installed.entries.iter().any(|record| {
                                record.provider == "modrinth" && record.project_id == *id
                            }) || self.seen.contains_key(id)
                        }) {
                            return Err(Error::DependencyConflict);
                        }
                        self.warnings.push("This version declares an incompatible project; inspect local manual mods before installing.".into());
                    }
                    "embedded" => {}
                    _ => return Err(Error::InvalidResponse),
                }
            }
            self.visiting.remove(&project.id);
            self.seen.insert(project.id.clone(), version.id.clone());
            self.items.push(PreviewItem {
                project_id: project.id.clone(),
                title: project.title.clone(),
                version_id: version.id.clone(),
                version_number: version.version_number.clone(),
                file_name: file.filename.clone(),
                already_installed: already,
                satisfied_by: None,
                changelog: version
                    .changelog
                    .clone()
                    .filter(|changelog| !changelog.trim().is_empty()),
            });
            if !already {
                self.plans.push(ProviderInstallPlan {
                    content_type: kind,
                    provider: "modrinth".into(),
                    project_id: project.id,
                    version_id: version.id.clone(),
                    file_id: file.hashes.sha512.clone(),
                    file_name: file.filename.clone(),
                    display_version: Some(version.version_number),
                    compatibility: ContentCompatibility {
                        minecraft_versions: version.game_versions,
                        loader: (kind == ContentType::Mod).then(|| self.context.loader.clone()),
                        environment: Some(version.environment),
                    },
                    dependencies,
                    source: ProviderArtifactSource::Sha512(source),
                });
            }
            Ok(())
        })
    }
}

pub struct Resolved {
    pub preview: InstallPreview,
    pub plans: Vec<ProviderInstallPlan>,
}

/// Verified-acquisition input for exactly one provider-owned pack version.
/// The source is Rust-only; UI previews expose only its identity and digest.
pub struct PackArtifact {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub sha512: String,
    pub source: Sha512ArtifactSource,
}

/// Candidate-discovery evidence for one managed record. Ordering is provider
/// publication chronology (`date_published`, ties broken by version id);
/// equal publication times are never treated as newer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDiscovery {
    pub current_version_type: String,
    pub candidate: Option<VersionChoice>,
    /// Newer compatible versions exist but none is allowed by the persisted
    /// release-channel policy. Distinguishes "up to date" from "no newer
    /// version under current policy".
    pub newer_compatible_exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub offset: u32,
    pub total_hits: u32,
    pub hits: Vec<ProjectSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub project_id: String,
    pub title: String,
    pub summary: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub project_type: BrowseKind,
    /// Concise provider-curated categories for display chips.
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub project_id: String,
    pub title: String,
    pub summary: String,
    pub license: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub environments: Vec<String>,
    pub versions: Vec<VersionChoice>,
    pub default_version_id: Option<String>,
    pub project_type: BrowseKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionChoice {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub environment: String,
    pub loaders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPreview {
    pub inventory_revision: Option<String>,
    pub project_id: String,
    pub version_id: String,
    pub content_type: ContentType,
    pub items: Vec<PreviewItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub project_id: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub already_installed: bool,
    pub satisfied_by: Option<crate::instance_content::DependencySatisfaction>,
    /// Provider-authored release notes for this version, when the provider
    /// supplied them. Untrusted display text.
    pub changelog: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchDto {
    hits: Vec<SearchHitDto>,
    offset: u32,
    total_hits: u32,
}
#[derive(Debug, Deserialize)]
struct SearchHitDto {
    project_id: String,
    project_type: String,
    title: String,
    description: String,
    author: String,
    downloads: u64,
    #[serde(default)]
    icon_url: Option<String>,
    versions: Vec<String>,
    environment: Vec<String>,
    #[serde(default)]
    display_categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CategoryTagDto {
    name: String,
    project_type: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ProjectDto {
    id: String,
    project_type: String,
    title: String,
    description: String,
    license: LicenseDto,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    environment: Vec<String>,
    #[serde(default)]
    icon_url: Option<String>,
}
#[derive(Debug, Deserialize)]
struct LicenseDto {
    id: String,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct VersionDto {
    pub(crate) id: String,
    pub(crate) project_id: String,
    name: String,
    pub(crate) version_number: String,
    pub(crate) version_type: String,
    pub(crate) date_published: String,
    pub(crate) game_versions: Vec<String>,
    pub(crate) loaders: Vec<String>,
    pub(crate) environment: String,
    pub(crate) files: Vec<FileDto>,
    pub(crate) dependencies: Vec<DependencyDto>,
    /// Provider-authored release notes. Absent on list responses that exclude
    /// changelogs; display-only untrusted text.
    #[serde(default)]
    changelog: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FileDto {
    pub(crate) hashes: HashesDto,
    pub(crate) url: String,
    pub(crate) filename: String,
    pub(crate) primary: bool,
    pub(crate) size: u64,
    file_type: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct HashesDto {
    pub(crate) sha512: String,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DependencyDto {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}

#[derive(Debug)]
pub enum Error {
    InvalidRequest,
    Network,
    RateLimited(Option<u64>),
    InvalidResponse,
    NotFound,
    NoCompatibleVersion,
    DependencyUnresolved,
    DependencyCycle,
    DependencyConflict,
    DependencyVersionConflict(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest | Self::InvalidResponse => "provider_invalid_response",
            Self::Network => "provider_network_error",
            Self::RateLimited(_) => "provider_rate_limited",
            Self::NotFound => "provider_project_not_found",
            Self::NoCompatibleVersion => "provider_no_compatible_version",
            Self::DependencyUnresolved => "provider_dependency_unresolved",
            Self::DependencyCycle => "provider_dependency_cycle",
            Self::DependencyConflict | Self::DependencyVersionConflict(_) => {
                "provider_content_collision"
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => write!(formatter, "The Modrinth request is invalid."),
            Self::Network => write!(
                formatter,
                "Modrinth is unavailable. Installed content remains usable."
            ),
            Self::RateLimited(reset) => write!(
                formatter,
                "Modrinth's rate limit was reached. Try again in {} seconds.",
                reset.unwrap_or(60)
            ),
            Self::InvalidResponse => write!(
                formatter,
                "Modrinth returned metadata Aurora cannot use safely."
            ),
            Self::NotFound => write!(formatter, "This Modrinth project or version was not found."),
            Self::NoCompatibleVersion => write!(
                formatter,
                "No version supports this instance's Minecraft and loader."
            ),
            Self::DependencyUnresolved => write!(
                formatter,
                "A required Modrinth dependency cannot be resolved safely."
            ),
            Self::DependencyCycle => {
                write!(formatter, "The required dependency graph contains a cycle.")
            }
            Self::DependencyConflict => write!(
                formatter,
                "An installed project conflicts with a required dependency."
            ),
            Self::DependencyVersionConflict(message) => formatter.write_str(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance_content::ProviderOrigin;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    fn context() -> Context {
        Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: true,
        }
    }

    fn project(id: &str, kind: &str) -> Value {
        json!({
            "id": id, "project_type": kind, "title": format!("Project {id}"),
            "description": "A test project", "license": {"id": "MIT"},
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": ["client_and_server"]
        })
    }

    fn version(id: &str, project_id: &str, deps: Value) -> Value {
        json!({
            "id": id, "project_id": project_id, "name": "Test release",
            "version_number": "1.0.0", "version_type": "release",
            "date_published": "2026-01-01T00:00:00Z",
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": "client_and_server",
            "files": [{
                "hashes": {"sha512": "a".repeat(128)},
                "url": "https://cdn.modrinth.com/data/test/file.jar",
                "filename": format!("{id}.jar"), "primary": true,
                "size": 4, "file_type": null
            }],
            "dependencies": deps
        })
    }

    fn server(routes: HashMap<String, Value>) -> TestServer {
        TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            routes
                .get(path)
                .map(|value| TestResponse::ok(&serde_json::to_vec(value).unwrap()))
                .unwrap_or_else(|| {
                    eprintln!("missing mock route: {path}");
                    TestResponse::status(404)
                })
        }))
    }

    #[tokio::test]
    async fn search_uses_instance_owned_filters_and_pagination() {
        let paths = Arc::new(Mutex::new(Vec::new()));
        let captured = paths.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            captured.lock().unwrap().push(request.path.clone());
            TestResponse::ok(br#"{"hits":[],"offset":20,"total_hits":30}"#)
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        for kind in [
            ContentType::Mod,
            ContentType::ResourcePack,
            ContentType::ShaderPack,
        ] {
            let page = client.search(&context(), kind, "test", 20).await.unwrap();
            assert_eq!(page.offset, 20);
        }
        let requests = paths.lock().unwrap();
        let facets: Vec<Vec<Vec<String>>> = requests
            .iter()
            .map(|path| {
                let url = Url::parse(&format!("http://localhost{path}")).unwrap();
                let text = url
                    .query_pairs()
                    .find(|(key, _)| key == "facets")
                    .unwrap()
                    .1;
                serde_json::from_str(&text).unwrap()
            })
            .collect();
        assert!(
            facets[0]
                .iter()
                .flatten()
                .any(|value| value == "categories:fabric")
        );
        assert!(
            facets[0]
                .iter()
                .flatten()
                .any(|value| value == "versions:1.21.11")
        );
        assert!(
            facets[1]
                .iter()
                .flatten()
                .any(|value| value == "project_type:resourcepack")
        );
        assert!(
            !facets[1]
                .iter()
                .flatten()
                .any(|value| value == "categories:fabric")
        );
        assert!(
            facets[2]
                .iter()
                .flatten()
                .any(|value| value == "project_type:shader")
        );
    }

    #[tokio::test]
    async fn empty_browse_query_keeps_safe_icons_and_drops_untrusted_ones() {
        let server = TestServer::spawn(Arc::new(|request: &TestRequest| {
            let url = Url::parse(&format!("http://localhost{}", request.path)).unwrap();
            assert_eq!(
                url.query_pairs().find(|(key, _)| key == "query").unwrap().1,
                ""
            );
            assert_eq!(
                url.query_pairs()
                    .find(|(key, _)| key == "offset")
                    .unwrap()
                    .1,
                "0"
            );
            TestResponse::ok(&serde_json::to_vec(&json!({
                "offset": 0, "total_hits": 3,
                "hits": [
                    {"project_id":"AAAABBBB","project_type":"mod","title":"Icon","description":"a","author":"a","downloads":1,"versions":["1.21.11"],"environment":["client_and_server"],"icon_url":"https://cdn.modrinth.com/data/AAAABBBB/icon.png"},
                    {"project_id":"BBBBCCCC","project_type":"mod","title":"None","description":"b","author":"b","downloads":1,"versions":["1.21.11"],"environment":["client_and_server"],"icon_url":null},
                    {"project_id":"CCCCDDDD","project_type":"mod","title":"Bad","description":"c","author":"c","downloads":1,"versions":["1.21.11"],"environment":["client_and_server"],"icon_url":"https://evil.example/data/icon.png"}
                ]
            })).unwrap())
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let page = client
            .search(&context(), ContentType::Mod, "", 0)
            .await
            .unwrap();
        assert_eq!(page.hits.len(), 3);
        assert_eq!(
            page.hits[0].icon_url.as_deref(),
            Some("https://cdn.modrinth.com/data/AAAABBBB/icon.png")
        );
        assert!(page.hits[1].icon_url.is_none());
        assert!(page.hits[2].icon_url.is_none());
        assert!(safe_icon_url("javascript:alert(1)").is_none());
        assert!(safe_icon_url("https://cdn.modrinth.com.evil.example/data/a.png").is_none());
    }

    #[tokio::test]
    async fn installed_artwork_uses_authoritative_project_and_same_cdn_policy() {
        let server = TestServer::spawn(Arc::new(|request: &TestRequest| {
            let icon = if request.path.ends_with("AAAABBBB") {
                "https://cdn.modrinth.com/data/AAAABBBB/icon.png"
            } else {
                "https://untrusted.example/icon.png"
            };
            TestResponse::ok(&serde_json::to_vec(&serde_json::json!({"id":if request.path.ends_with("AAAABBBB"){"AAAABBBB"}else{"BBBBCCCC"},"project_type":"mod","title":"Fixture","description":"","license":{"id":"MIT"},"game_versions":["1.21.11"],"loaders":["fabric"],"environment":["client_only"],"icon_url":icon})).unwrap())
        }));
        let client = Client::for_testing(&server.base_url());
        assert_eq!(
            client.artwork("AAAABBBB").await.unwrap().as_deref(),
            Some("https://cdn.modrinth.com/data/AAAABBBB/icon.png")
        );
        assert!(client.artwork("BBBBCCCC").await.unwrap().is_none());
        assert!(client.artwork("../unsafe").await.is_err());
    }

    #[tokio::test]
    async fn quick_resolution_uses_latest_compatible_for_each_content_type() {
        for (kind, project_type, loader, extension) in [
            (ContentType::Mod, "mod", "fabric", "jar"),
            (
                ContentType::ResourcePack,
                "resourcepack",
                "minecraft",
                "zip",
            ),
            (ContentType::ShaderPack, "shader", "iris", "zip"),
        ] {
            let mut routes = HashMap::new();
            routes.insert(
                "/v2/project/AAAABBBB".into(),
                project("AAAABBBB", project_type),
            );
            let mut wrong = version("11112222", "AAAABBBB", json!([]));
            wrong["date_published"] = json!("2026-09-01T00:00:00Z");
            wrong["game_versions"] = json!(["1.20.1"]);
            let mut good = version("22223333", "AAAABBBB", json!([]));
            good["loaders"] = json!([loader]);
            good["files"][0]["filename"] = json!(format!("content.{extension}"));
            routes.insert(
                "/v2/project/AAAABBBB/version".into(),
                json!([wrong, good.clone()]),
            );
            routes.insert("/v2/version/22223333".into(), good);
            let server = server(routes);
            let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
            let resolved = client
                .resolve_latest(&context(), kind, "AAAABBBB", &ContentState::empty())
                .await
                .unwrap();
            assert_eq!(resolved.preview.version_id, "22223333");
            assert_eq!(resolved.plans.len(), 1);
        }
    }

    #[tokio::test]
    async fn quick_resolution_no_match_never_reaches_artifact_planning() {
        for (kind, project_type) in [
            (ContentType::Mod, "mod"),
            (ContentType::ResourcePack, "resourcepack"),
            (ContentType::ShaderPack, "shader"),
        ] {
            let mut routes = HashMap::new();
            routes.insert(
                "/v2/project/AAAABBBB".into(),
                project("AAAABBBB", project_type),
            );
            let mut wrong = version("11112222", "AAAABBBB", json!([]));
            wrong["game_versions"] = json!(["1.20.1"]);
            routes.insert("/v2/project/AAAABBBB/version".into(), json!([wrong]));
            let server = server(routes);
            let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
            assert!(matches!(
                client
                    .resolve_latest(&context(), kind, "AAAABBBB", &ContentState::empty())
                    .await,
                Err(Error::NoCompatibleVersion)
            ));
        }
    }

    #[tokio::test]
    async fn update_discovery_respects_publication_compatibility_and_release_channel() {
        let mut current = version("11112222", "AAAABBBB", json!([]));
        current["date_published"] = json!("2026-01-01T00:00:00Z");
        let mut stable = version("22223333", "AAAABBBB", json!([]));
        stable["date_published"] = json!("2026-02-01T00:00:00Z");
        let mut beta = version("33334444", "AAAABBBB", json!([]));
        beta["version_type"] = json!("beta");
        beta["date_published"] = json!("2026-03-01T00:00:00Z");
        let mut wrong_loader = version("44445555", "AAAABBBB", json!([]));
        wrong_loader["date_published"] = json!("2026-04-01T00:00:00Z");
        wrong_loader["loaders"] = json!(["forge"]);
        let mut wrong_game = version("55556666", "AAAABBBB", json!([]));
        wrong_game["date_published"] = json!("2026-05-01T00:00:00Z");
        wrong_game["game_versions"] = json!(["1.20.1"]);
        let mut routes = HashMap::new();
        routes.insert("/v2/version/11112222".into(), current.clone());
        routes.insert(
            "/v2/project/AAAABBBB/version".into(),
            json!([
                wrong_game,
                wrong_loader,
                beta.clone(),
                stable.clone(),
                current.clone()
            ]),
        );
        let first_server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", first_server.base_url()));
        let mut record = ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "AAAABBBB".into(),
            version_id: "11112222".into(),
            file_id: "a".repeat(128),
            file_name: "11112222.jar".into(),
            sha256: "b".repeat(64),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: crate::instance_content::UpdateChannel::Stable,
        };
        assert_eq!(
            client
                .update_candidate(&context(), &record)
                .await
                .unwrap()
                .unwrap()
                .id,
            "22223333"
        );
        // The persisted release-channel policy, not the installed version's
        // type or any human-readable version string, decides eligibility.
        // Under Stable the newer beta stays hidden and the discovery reports
        // that newer versions exist outside the policy.
        let mut beta_routes = HashMap::new();
        beta_routes.insert("/v2/version/11112222".into(), current.clone());
        beta_routes.insert("/v2/project/AAAABBBB/version".into(), json!([beta, stable]));
        let beta_server = server(beta_routes);
        let beta_client = Client::for_testing(&format!("{}/v2/", beta_server.base_url()));
        let stable_only = beta_client
            .update_discovery(&context(), &record)
            .await
            .unwrap();
        assert_eq!(stable_only.candidate.as_ref().unwrap().id, "22223333");
        assert!(stable_only.newer_compatible_exists);
        let mut beta_policy = record.clone();
        beta_policy.update_channel = crate::instance_content::UpdateChannel::Beta;
        assert_eq!(
            beta_client
                .update_discovery(&context(), &beta_policy)
                .await
                .unwrap()
                .candidate
                .unwrap()
                .id,
            "33334444"
        );
        let mut alpha_only = beta.clone();
        alpha_only["id"] = json!("66667777");
        alpha_only["version_type"] = json!("alpha");
        alpha_only["date_published"] = json!("2026-06-01T00:00:00Z");
        let mut alpha_routes = HashMap::new();
        alpha_routes.insert("/v2/version/11112222".into(), current);
        alpha_routes.insert(
            "/v2/project/AAAABBBB/version".into(),
            json!([alpha_only, beta, stable]),
        );
        let alpha_server = server(alpha_routes);
        let alpha_client = Client::for_testing(&format!("{}/v2/", alpha_server.base_url()));
        let beta_view = alpha_client
            .update_discovery(&context(), &beta_policy)
            .await
            .unwrap();
        // Alpha remains excluded under Beta, and the installed version type
        // is reported as provider evidence.
        assert_eq!(beta_view.candidate.as_ref().unwrap().id, "33334444");
        assert_eq!(beta_view.current_version_type, "release");
        beta_policy.update_channel = crate::instance_content::UpdateChannel::Alpha;
        assert_eq!(
            alpha_client
                .update_discovery(&context(), &beta_policy)
                .await
                .unwrap()
                .candidate
                .unwrap()
                .id,
            "66667777"
        );
        // The installed exact provider identity anchors the check: a version
        // document that no longer publishes the recorded file is rejected.
        record.file_id = "c".repeat(128);
        assert!(matches!(
            beta_client.update_discovery(&context(), &record).await,
            Err(Error::InvalidResponse)
        ));
    }

    #[tokio::test]
    async fn update_check_distinguishes_no_candidate_and_provider_failures() {
        let mut record = ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: "AAAABBBB".into(),
            version_id: "11112222".into(),
            file_id: "a".repeat(128),
            file_name: "11112222.jar".into(),
            sha256: "b".repeat(64),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: crate::instance_content::UpdateChannel::Stable,
        };
        let mut routes = HashMap::new();
        routes.insert(
            "/v2/version/11112222".into(),
            version("11112222", "AAAABBBB", json!([])),
        );
        routes.insert(
            "/v2/project/AAAABBBB/version".into(),
            json!([version("11112222", "AAAABBBB", json!([]))]),
        );
        let no_update = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", no_update.base_url()));
        assert!(
            client
                .update_candidate(&context(), &record)
                .await
                .unwrap()
                .is_none()
        );

        record.version_id = "99998888".into();
        assert!(matches!(
            client.update_candidate(&context(), &record).await,
            Err(Error::NotFound)
        ));
        record.version_id = "11112222".into();

        let rate = TestServer::spawn(Arc::new(|_| {
            TestResponse::status(429).with_header("X-Ratelimit-Reset", "12")
        }));
        let client = Client::for_testing(&format!("{}/v2/", rate.base_url()));
        assert!(matches!(
            client.update_candidate(&context(), &record).await,
            Err(Error::RateLimited(Some(12)))
        ));

        let malformed =
            TestServer::spawn(Arc::new(|_| TestResponse::ok(br#"{"unexpected":true}"#)));
        let client = Client::for_testing(&format!("{}/v2/", malformed.base_url()));
        assert!(matches!(
            client.update_candidate(&context(), &record).await,
            Err(Error::InvalidResponse)
        ));

        let client = Client::for_testing("http://127.0.0.1:0/v2/");
        assert!(matches!(
            client.update_candidate(&context(), &record).await,
            Err(Error::Network)
        ));
    }

    #[tokio::test]
    async fn version_choice_prefers_release_and_rejects_wrong_loader() {
        let mut routes = HashMap::new();
        routes.insert("/v2/project/AAAABBBB".into(), project("AAAABBBB", "mod"));
        let mut beta = version("BBBBCCCC", "AAAABBBB", json!([]));
        beta["version_type"] = json!("beta");
        beta["date_published"] = json!("2026-03-01T00:00:00Z");
        let stable = version("CCCCDDDD", "AAAABBBB", json!([]));
        let mut wrong = version("DDDDEEEE", "AAAABBBB", json!([]));
        wrong["loaders"] = json!(["forge"]);
        routes.insert(
            "/v2/project/AAAABBBB/version".into(),
            json!([beta, stable, wrong]),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let details = client
            .details(&context(), ContentType::Mod, "AAAABBBB")
            .await
            .unwrap();
        assert_eq!(details.versions.len(), 2);
        assert_eq!(details.default_version_id.as_deref(), Some("CCCCDDDD"));
    }

    #[tokio::test]
    async fn required_graph_is_transitive_and_optional_is_not_installed() {
        let mut routes = HashMap::new();
        for id in ["AAAABBBB", "BBBBCCCC", "CCCCDDDD"] {
            routes.insert(format!("/v2/project/{id}"), project(id, "mod"));
        }
        let root = version(
            "11112222",
            "AAAABBBB",
            json!([
                {"project_id": "BBBBCCCC", "version_id": null, "dependency_type": "required"},
                {"project_id": "BBBBCCCC", "version_id": null, "dependency_type": "required"},
                {"project_id": "CCCCDDDD", "version_id": null, "dependency_type": "optional"}
            ]),
        );
        let child = version(
            "22223333",
            "BBBBCCCC",
            json!([
                {"project_id": "CCCCDDDD", "version_id": null, "dependency_type": "required"}
            ]),
        );
        let leaf = version("33334444", "CCCCDDDD", json!([]));
        routes.insert("/v2/version/11112222".into(), root);
        routes.insert("/v2/project/BBBBCCCC/version".into(), json!([child]));
        routes.insert("/v2/project/CCCCDDDD/version".into(), json!([leaf]));
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let graph = client
            .resolve(
                &context(),
                ContentType::Mod,
                "AAAABBBB",
                "11112222",
                &ContentState::empty(),
            )
            .await
            .unwrap();
        assert_eq!(graph.plans.len(), 3);
        assert_eq!(graph.preview.items[0].project_id, "CCCCDDDD");
        assert_eq!(graph.preview.items[2].project_id, "AAAABBBB");
        assert!(
            graph
                .preview
                .warnings
                .iter()
                .any(|item| item.contains("Optional"))
        );
    }

    #[tokio::test]
    async fn cycle_and_rate_limit_fail_with_distinct_codes() {
        let mut routes = HashMap::new();
        for id in ["AAAABBBB", "BBBBCCCC"] {
            routes.insert(format!("/v2/project/{id}"), project(id, "mod"));
        }
        routes.insert(
            "/v2/version/11112222".into(),
            version(
                "11112222",
                "AAAABBBB",
                json!([
                    {"project_id": "BBBBCCCC", "version_id": null, "dependency_type": "required"}
                ]),
            ),
        );
        routes.insert(
            "/v2/project/BBBBCCCC/version".into(),
            json!([version(
                "22223333",
                "BBBBCCCC",
                json!([
                    {"project_id": "AAAABBBB", "version_id": null, "dependency_type": "required"}
                ])
            )]),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(matches!(
            client
                .resolve(
                    &context(),
                    ContentType::Mod,
                    "AAAABBBB",
                    "11112222",
                    &ContentState::empty()
                )
                .await,
            Err(Error::DependencyCycle)
        ));
        let rate = TestServer::spawn(Arc::new(|_| {
            TestResponse::status(429).with_header("X-Ratelimit-Reset", "12")
        }));
        let client = Client::for_testing(&format!("{}/v2/", rate.base_url()));
        assert!(matches!(
            client.search(&context(), ContentType::Mod, "", 0).await,
            Err(Error::RateLimited(Some(12)))
        ));
    }

    #[test]
    fn primary_file_is_deterministic_and_unsafe_primary_is_rejected() {
        let mut value = version("11112222", "AAAABBBB", json!([]));
        value["files"] = json!([
            {"hashes":{"sha512":"a".repeat(128)},"url":"https://cdn.modrinth.com/a","filename":"source.jar","primary":false,"size":4,"file_type":"sources-jar"},
            {"hashes":{"sha512":"b".repeat(128)},"url":"https://cdn.modrinth.com/b","filename":"release.jar","primary":true,"size":4,"file_type":null}
        ]);
        let parsed: VersionDto = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            choose_file(&parsed, ContentType::Mod).unwrap().filename,
            "release.jar"
        );
        value["files"][1]["filename"] = json!("../escape.jar");
        let parsed: VersionDto = serde_json::from_value(value).unwrap();
        assert!(choose_file(&parsed, ContentType::Mod).is_err());
    }

    #[test]
    fn pack_and_shader_version_rules_are_distinct() {
        let mut pack: VersionDto =
            serde_json::from_value(version("11112222", "AAAABBBB", json!([]))).unwrap();
        pack.loaders = vec!["minecraft".into()];
        pack.environment = "unknown".into();
        assert!(compatible(&context(), ContentType::ResourcePack, &pack));
        assert!(!compatible(&context(), ContentType::Mod, &pack));
        pack.loaders = vec!["iris".into()];
        assert!(compatible(&context(), ContentType::ShaderPack, &pack));
        assert!(!compatible(&context(), ContentType::ResourcePack, &pack));
        pack.game_versions = vec!["1.20.1".into()];
        assert!(!compatible(&context(), ContentType::ShaderPack, &pack));
    }

    #[tokio::test]
    async fn bootstrap_api_does_not_reserve_a_provider_root() {
        let mut routes = HashMap::new();
        routes.insert(
            format!("/v2/project/{FABRIC_API_PROJECT}"),
            project(FABRIC_API_PROJECT, "mod"),
        );
        routes.insert(
            "/v2/version/11112222".into(),
            version("11112222", FABRIC_API_PROJECT, json!([])),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let resolved = client
            .resolve(
                &context(),
                ContentType::Mod,
                FABRIC_API_PROJECT,
                "11112222",
                &ContentState::empty(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.plans.len(), 1);
        assert_eq!(resolved.plans[0].project_id, FABRIC_API_PROJECT);
        // Physical collisions are still checked by native verified activation.
    }

    #[tokio::test]
    #[ignore = "uses the live public Modrinth API and downloads real provider artifacts"]
    async fn live_mod_menu_resolves_and_installs_into_a_disposable_root() {
        let client = Client::official();
        let context = context();
        let page = client
            .search(&context, ContentType::Mod, "Mod Menu", 0)
            .await
            .unwrap();
        assert!(page.hits.iter().any(|hit| hit.project_id == "mOgUt4GM"));
        let details = client
            .details(&context, ContentType::Mod, "mOgUt4GM")
            .await
            .unwrap();
        let version = details.default_version_id.unwrap();
        let root = std::env::temp_dir().join(format!(
            "aurora-modrinth-acceptance-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance =
            crate::instances::InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        std::fs::create_dir(root.join("instances").join(instance.as_str())).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let resolved = client
            .resolve(
                &context,
                ContentType::Mod,
                "mOgUt4GM",
                &version,
                &ContentState::empty(),
            )
            .await
            .unwrap();
        assert!(resolved.preview.items.len() >= 2);
        assert!(
            resolved
                .preview
                .warnings
                .iter()
                .any(|warning| warning.contains("Fabric API"))
        );
        let installed =
            crate::instance_content::install_provider_plans(&managed, &instance, resolved.plans)
                .await
                .unwrap();
        assert!(
            installed
                .iter()
                .any(|record| record.project_id == "mOgUt4GM")
        );
        let inventory = crate::instance_mods::scan(&managed, &instance).unwrap();
        assert!(inventory.entries.iter().any(|entry| entry.ownership == crate::instance_mods::ModOwnership::ProviderManaged));
        eprintln!("live Modrinth acceptance root: {}", root.display());
    }
    #[tokio::test]
    async fn unprotected_fabric_api_resolves_as_ordinary_provider_content() {
        let mut routes = HashMap::new();
        routes.insert(
            format!("/v2/project/{FABRIC_API_PROJECT}"),
            project(FABRIC_API_PROJECT, "mod"),
        );
        routes.insert(
            "/v2/version/11112222".into(),
            version("11112222", FABRIC_API_PROJECT, json!([])),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let mut context = context();
        context.fabric_api_protected = false;
        let resolved = client
            .resolve(
                &context,
                ContentType::Mod,
                FABRIC_API_PROJECT,
                "11112222",
                &ContentState::empty(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.plans.len(), 1);
        assert_eq!(resolved.plans[0].project_id, FABRIC_API_PROJECT);
    }
    #[tokio::test]
    async fn unsupported_provider_platform_fails_closed_before_network() {
        let mut context = context();
        let client = Client::for_testing("http://127.0.0.1:1/v2/");
        for loader in ["forge", "vanilla"] {
            context.loader = loader.into();
            assert!(matches!(
                client.search(&context, ContentType::Mod, "", 0).await,
                Err(Error::NoCompatibleVersion)
            ));
            let version: VersionDto =
                serde_json::from_value(version("11112222", FABRIC_API_PROJECT, json!([]))).unwrap();
            assert!(!compatible(&context, ContentType::Mod, &version));
        }
    }

    #[tokio::test]
    async fn protected_api_dependency_is_external_and_cannot_create_provider_ownership() {
        let dependency = json!([{"dependency_type":"required","project_id":FABRIC_API_PROJECT,"version_id":null}]);
        let mut routes = HashMap::new();
        routes.insert("/v2/project/AAAABBBB".into(), project("AAAABBBB", "mod"));
        routes.insert(
            "/v2/version/11112222".into(),
            version("11112222", "AAAABBBB", dependency),
        );
        // No Fabric API provider routes: the validated launcher requirement satisfies it.
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let resolved = client
            .resolve(
                &context(),
                ContentType::Mod,
                "AAAABBBB",
                "11112222",
                &ContentState::empty(),
            )
            .await
            .unwrap();
        assert_eq!(resolved.plans.len(), 1);
        assert_eq!(resolved.plans[0].project_id, "AAAABBBB");
        assert_eq!(
            resolved.plans[0].dependencies[0].project_id,
            FABRIC_API_PROJECT
        );
        assert!(
            resolved
                .preview
                .warnings
                .iter()
                .any(|warning| warning.contains("verified bootstrap content"))
        );
    }

    #[tokio::test]
    async fn protected_api_with_a_provider_version_pin_is_not_assumed_satisfied() {
        let dependency = json!([{"dependency_type":"required","project_id":FABRIC_API_PROJECT,"version_id":"22223333"}]);
        let mut routes = HashMap::new();
        routes.insert("/v2/project/AAAABBBB".into(), project("AAAABBBB", "mod"));
        routes.insert(
            "/v2/version/11112222".into(),
            version("11112222", "AAAABBBB", dependency),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(matches!(
            client
                .resolve(
                    &context(),
                    ContentType::Mod,
                    "AAAABBBB",
                    "11112222",
                    &ContentState::empty()
                )
                .await,
            Err(Error::DependencyUnresolved)
        ));
    }

    // ---- Phase G: recognition hash lookups --------------------------------

    fn lookup_version(id: &str, project_id: &str, sha512: &str) -> Value {
        json!({
            "id": id, "project_id": project_id, "name": "Recognized release",
            "version_number": "1.4.2", "version_type": "release",
            "date_published": "2026-06-01T00:00:00Z",
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": "client_and_server",
            "files": [{
                "hashes": {"sha512": sha512, "sha1": "f".repeat(40)},
                "url": "https://cdn.modrinth.com/data/test/file.jar",
                "filename": format!("{id}.jar"), "primary": true,
                "size": 4096, "file_type": null
            }],
            "dependencies": [
                {"project_id": "BBBBCCCC", "version_id": null, "dependency_type": "required"},
                {"project_id": "CCCCDDDD", "version_id": null, "dependency_type": "optional"},
                {"project_id": "DDDD0000", "version_id": "eeee1111", "dependency_type": "embedded"}
            ]
        })
    }

    fn hash_of(seed: u8) -> String {
        // A distinct 128-hex-character digest per seed, by construction.
        use sha2::Digest as _;
        let digest = sha2::Sha512::digest([seed]);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[tokio::test]
    async fn single_hash_lookup_verifies_the_published_digest() {
        let hash = hash_of(1);
        let expected = hash.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            assert_eq!(request.method, "GET");
            let url = Url::parse(&format!("http://localhost{}", request.path)).unwrap();
            assert!(url.path().ends_with(&expected));
            assert_eq!(
                url.query_pairs()
                    .find(|(key, _)| key == "algorithm")
                    .unwrap()
                    .1,
                "sha512"
            );
            TestResponse::ok(
                &serde_json::to_vec(&lookup_version("11112222", "AAAABBBB", &expected)).unwrap(),
            )
        }));
        let client = Client::for_testing(&server.base_url());
        let found = client.lookup_files(&[hash.clone()]).await.unwrap();
        assert_eq!(found.len(), 1);
        let file = &found[0];
        assert_eq!(file.queried_sha512, hash_of(1));
        assert_eq!(file.project_id, "AAAABBBB");
        assert_eq!(file.version_id, "11112222");
        assert_eq!(file.version_number, "1.4.2");
        assert_eq!(file.version_type, "release");
        assert_eq!(file.version_name, "Recognized release");
        assert_eq!(file.file_name, "11112222.jar");
        assert_eq!(file.file_size, 4096);
        // Normalized dependency declarations: required and optional survive,
        // embedded dependencies add no download.
        assert_eq!(file.dependencies.len(), 2);
        assert_eq!(file.dependencies[0].project_id, "BBBBCCCC");
        assert_eq!(file.dependencies[0].kind, DependencyKind::Required);
        assert_eq!(file.dependencies[1].kind, DependencyKind::Optional);
    }

    #[tokio::test]
    async fn single_hash_lookup_not_found_is_unrecognized() {
        let server = TestServer::spawn(Arc::new(|_: &TestRequest| TestResponse::status(404)));
        let client = Client::for_testing(&server.base_url());
        let found = client.lookup_files(&[hash_of(2)]).await.unwrap();
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn a_response_without_the_queried_digest_is_rejected() {
        // The provider answered a different file's identity: the published
        // SHA-512 (never SHA-1 or another algorithm) must equal the queried
        // digest, or the response establishes nothing.
        let server = TestServer::spawn(Arc::new(|_: &TestRequest| {
            TestResponse::ok(
                &serde_json::to_vec(&lookup_version("11112222", "AAAABBBB", &hash_of(99))).unwrap(),
            )
        }));
        let client = Client::for_testing(&server.base_url());
        assert!(matches!(
            client.lookup_files(&[hash_of(3)]).await,
            Err(Error::InvalidResponse)
        ));
    }

    #[tokio::test]
    async fn invalid_hash_shapes_are_refused_before_any_request() {
        let requests = Arc::new(Mutex::new(0usize));
        let seen = requests.clone();
        let server = TestServer::spawn(Arc::new(move |_: &TestRequest| {
            *seen.lock().unwrap() += 1;
            TestResponse::ok(b"[]")
        }));
        let client = Client::for_testing(&server.base_url());
        assert!(matches!(
            client.lookup_files(&["z".repeat(128)]).await,
            Err(Error::InvalidRequest)
        ));
        assert_eq!(*requests.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn batch_lookup_maps_hashes_to_verified_versions_and_splits_at_bound() {
        let hashes: Vec<String> = (0..65u8).map(hash_of).collect();
        let bodies = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let captured = bodies.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let url = Url::parse(&format!("http://localhost{}", request.path)).unwrap();
            let batch: Vec<String> = if request.method == "POST" {
                assert!(url.path() == "/v2/version_files");
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["algorithm"], "sha512");
                body["hashes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect()
            } else {
                assert_eq!(request.method, "GET");
                assert!(url.path().starts_with("/v2/version_file/"));
                assert_eq!(
                    url.query_pairs()
                        .find(|(key, _)| key == "algorithm")
                        .unwrap()
                        .1,
                    "sha512"
                );
                vec![url.path().rsplit('/').next().unwrap().to_owned()]
            };
            captured.lock().unwrap().push(batch.clone());
            if request.method == "GET" {
                // The single-hash endpoint answers one version object.
                return TestResponse::ok(
                    &serde_json::to_vec(&lookup_version("00000000", "AAAABBBB", &batch[0]))
                        .unwrap(),
                );
            }
            let mapped: serde_json::Map<String, Value> = batch
                .iter()
                .enumerate()
                .map(|(index, hash)| {
                    (
                        hash.clone(),
                        lookup_version(&format!("{index:08x}"), "AAAABBBB", hash),
                    )
                })
                .collect();
            TestResponse::ok(&serde_json::to_vec(&mapped).unwrap())
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let found = client.lookup_files(&hashes).await.unwrap();
        assert_eq!(found.len(), 65);
        let batches = bodies.lock().unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), MAX_HASH_BATCH);
        assert_eq!(batches[1].len(), 1);
        assert!(found.iter().all(|file| file.queried_sha512.len() == 128));
    }

    #[tokio::test]
    async fn batch_lookup_preserves_each_file_hash_within_one_version() {
        let first = hash_of(71);
        let second = hash_of(72);
        let version = lookup_version("00000071", "AAAABBBB", &first);
        let mut version = version;
        let mut other = version["files"][0].clone();
        other["hashes"]["sha512"] = serde_json::json!(second);
        other["filename"] = serde_json::json!("second.jar");
        version["files"].as_array_mut().unwrap().push(other);
        let server = TestServer::spawn(Arc::new(move |_: &TestRequest| {
            let mapped = serde_json::Map::from_iter([
                (first.clone(), version.clone()),
                (second.clone(), version.clone()),
            ]);
            TestResponse::ok(&serde_json::to_vec(&mapped).unwrap())
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let found = client
            .lookup_files(&[hash_of(71), hash_of(72)])
            .await
            .unwrap();
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .any(|item| item.queried_sha512 == hash_of(71) && item.file_name != "second.jar")
        );
        assert!(
            found
                .iter()
                .any(|item| item.queried_sha512 == hash_of(72) && item.file_name == "second.jar")
        );
    }

    #[test]
    fn pack_environment_accepts_singleplayer_and_rejects_dedicated_server_only() {
        for value in [
            "client_only",
            "client_and_server",
            "client_only_server_optional",
            "server_only_client_optional",
            "server_only",
            "singleplayer_only",
            "client_or_server",
            "client_or_server_prefers_both",
            "unknown",
        ] {
            assert!(pack_client_environment(value), "{value}");
        }
        for value in ["dedicated_server_only", "future_value"] {
            assert!(!pack_client_environment(value), "{value}");
        }
    }

    #[tokio::test]
    async fn batch_lookup_rejects_an_unmatched_version() {
        let hashes = vec![hash_of(10), hash_of(11)];
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let _body: Value = serde_json::from_slice(&request.body).unwrap();
            // A response keyed by a digest Aurora never queried is invalid.
            let mapped = serde_json::json!({
                hash_of(200): lookup_version("99998888", "EEEEFFFF", &hash_of(200)),
            });
            TestResponse::ok(&serde_json::to_vec(&mapped).unwrap())
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        // One returned version publishes none of the queried digests; a
        // response claiming identity without matching bytes is invalid.
        assert!(matches!(
            client.lookup_files(&hashes).await,
            Err(Error::InvalidResponse)
        ));
    }

    #[tokio::test]
    async fn batch_lookup_surfaces_rate_limit_with_reset_delay() {
        let server = TestServer::spawn(Arc::new(|_: &TestRequest| {
            TestResponse::status(429).with_header("x-ratelimit-reset", "42")
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(matches!(
            client.lookup_files(&[hash_of(20), hash_of(21)]).await,
            Err(Error::RateLimited(Some(42)))
        ));
    }

    #[tokio::test]
    async fn batch_lookup_rejects_malformed_response_bodies() {
        let server = TestServer::spawn(Arc::new(|_: &TestRequest| {
            TestResponse::ok(b"not json at all")
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(matches!(
            client.lookup_files(&[hash_of(30), hash_of(31)]).await,
            Err(Error::InvalidResponse)
        ));
    }

    fn sha(seed: &str, width: usize) -> String {
        const HEX: &[u8] = b"0123456789abcdef";
        let bytes = seed.as_bytes();
        (0..width)
            .map(|index| HEX[(usize::from(bytes[index % bytes.len()]) + index) % HEX.len()] as char)
            .collect()
    }

    fn versioned(id: &str, project_id: &str, kind: &str, published: &str, deps: Value) -> Value {
        let mut document = version(id, project_id, deps);
        document["version_type"] = json!(kind);
        document["date_published"] = json!(published);
        document["files"][0]["hashes"]["sha512"] = json!(sha(&format!("f{id}"), 128));
        document
    }

    fn state_record(project: &str, version_id: &str, file_seed: &str) -> ProviderRecord {
        ProviderRecord {
            content_type: ContentType::Mod,
            provider: "modrinth".into(),
            project_id: project.into(),
            version_id: version_id.into(),
            file_id: sha(file_seed, 128),
            file_name: format!("{version_id}.jar").into(),
            sha256: "b".repeat(64),
            display_version: Some("1.0.0".into()),
            compatibility: ContentCompatibility {
                minecraft_versions: vec!["1.21.11".into()],
                loader: Some("fabric".into()),
                environment: Some("client_and_server".into()),
            },
            dependencies: vec![],
            explicitly_retained: true,
            requires: vec![],
            origin: ProviderOrigin::Direct,
            installed_at_unix_seconds: None,
            pinned: false,
            update_channel: crate::instance_content::UpdateChannel::Stable,
        }
    }

    fn installed_state(entries: Vec<ProviderRecord>) -> ContentState {
        let mut state = ContentState::empty();
        state.entries = entries;
        state
    }

    #[tokio::test]
    async fn update_discovery_falls_back_to_the_installed_version_document() {
        // The compatibility-filtered list does not contain the installed
        // version (it was built for another game version), but the exact
        // version document anchors the ordering and identity check.
        let current = versioned(
            "11112222",
            "AAAABBBB",
            "release",
            "2026-01-01T00:00:00Z",
            json!([]),
        );
        let mut newer = versioned(
            "22223333",
            "AAAABBBB",
            "release",
            "2026-02-01T00:00:00Z",
            json!([]),
        );
        newer["game_versions"] = json!(["1.21.11"]);
        let mut routes = HashMap::new();
        routes.insert(
            "/v2/project/AAAABBBB/version".into(),
            json!([newer.clone()]),
        );
        routes.insert("/v2/version/11112222".into(), current.clone());
        let anchored = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", anchored.base_url()));
        let record = state_record("AAAABBBB", "11112222", "f11112222");
        let discovery = client.update_discovery(&context(), &record).await.unwrap();
        assert_eq!(discovery.candidate.as_ref().unwrap().id, "22223333");
        assert_eq!(discovery.current_version_type, "release");

        // Both the project list and the exact version document missing means
        // the provider no longer knows this identity.
        let mut gone = HashMap::new();
        gone.insert("/v2/project/AAAABBBB/version".into(), json!([]));
        let missing = server(gone);
        let client = Client::for_testing(&format!("{}/v2/", missing.base_url()));
        assert!(matches!(
            client.update_discovery(&context(), &record).await,
            Err(Error::NotFound)
        ));
    }

    #[tokio::test]
    async fn resolve_updates_plans_multi_root_graphs_with_shared_dependency_replacement() {
        // Two update roots both advance a shared installed dependency.
        let root_a_old = state_record("AAAA0001", "AOLD0001", "fAOLD0001");
        let root_b_old = state_record("CCCC0003", "BOLD0002", "fBOLD0002");
        let mut dep_old = state_record("BBBB0002", "DOLD0003", "fDOLD0003");
        dep_old.explicitly_retained = false;
        let mut state = installed_state(vec![
            root_a_old.clone(),
            root_b_old.clone(),
            dep_old.clone(),
        ]);
        state.entries[0].requires = vec![dep_old.identity()];
        state.entries[1].requires = vec![dep_old.identity()];

        let dep_new = versioned(
            "DNEW0006",
            "BBBB0002",
            "release",
            "2026-03-01T00:00:00Z",
            json!([]),
        );
        let mut root_a_new = versioned(
            "ANEW0004",
            "AAAA0001",
            "release",
            "2026-03-02T00:00:00Z",
            json!([{"project_id": "BBBB0002", "version_id": "DNEW0006", "dependency_type": "required"}]),
        );
        root_a_new["changelog"] = json!("Adds widgets and fixes rendering.");
        let root_b_new = versioned(
            "BNEW0005",
            "CCCC0003",
            "release",
            "2026-03-03T00:00:00Z",
            json!([{"project_id": "BBBB0002", "version_id": "DNEW0006", "dependency_type": "required"}]),
        );
        let mut routes = HashMap::new();
        for (project_id, document) in [
            ("AAAA0001", root_a_new.clone()),
            ("BBBB0002", dep_new.clone()),
            ("CCCC0003", root_b_new.clone()),
        ] {
            routes.insert(
                format!("/v2/project/{project_id}"),
                project(project_id, "mod"),
            );
            routes.insert(
                format!("/v2/project/{project_id}/version"),
                json!([document]),
            );
            routes.insert(
                format!("/v2/version/{}", document["id"].as_str().unwrap()),
                document,
            );
        }
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));

        let resolved = client
            .resolve_updates(
                &context(),
                &[
                    (ContentType::Mod, "AAAA0001".into(), "ANEW0004".into()),
                    (ContentType::Mod, "CCCC0003".into(), "BNEW0005".into()),
                ],
                &state,
            )
            .await
            .unwrap();
        let mut planned: Vec<(String, String)> = resolved
            .plans
            .iter()
            .map(|plan| (plan.project_id.clone(), plan.version_id.clone()))
            .collect();
        planned.sort();
        assert_eq!(
            planned,
            vec![
                ("AAAA0001".to_owned(), "ANEW0004".to_owned()),
                ("BBBB0002".to_owned(), "DNEW0006".to_owned()),
                ("CCCC0003".to_owned(), "BNEW0005".to_owned()),
            ]
        );
        // Provider changelogs ride along as untrusted display text.
        let changelog_item = resolved
            .preview
            .items
            .iter()
            .find(|item| item.project_id == "AAAA0001")
            .unwrap();
        assert_eq!(
            changelog_item.changelog.as_deref(),
            Some("Adds widgets and fixes rendering.")
        );
    }

    #[tokio::test]
    async fn resolve_updates_conflicting_dependency_versions_fail_closed() {
        let root_a_old = state_record("AAAA0001", "AOLD0001", "fAOLD0001");
        let root_b_old = state_record("CCCC0003", "BOLD0002", "fBOLD0002");
        let state = installed_state(vec![root_a_old.clone(), root_b_old.clone()]);
        let dep_v2 = versioned(
            "DNEW0006",
            "BBBB0002",
            "release",
            "2026-03-01T00:00:00Z",
            json!([]),
        );
        let dep_v3 = versioned(
            "DNEW0007",
            "BBBB0002",
            "release",
            "2026-03-02T00:00:00Z",
            json!([]),
        );
        let root_a_new = versioned(
            "ANEW0004",
            "AAAA0001",
            "release",
            "2026-03-03T00:00:00Z",
            json!([{"project_id": "BBBB0002", "version_id": "DNEW0006", "dependency_type": "required"}]),
        );
        let root_b_new = versioned(
            "BNEW0005",
            "CCCC0003",
            "release",
            "2026-03-04T00:00:00Z",
            json!([{"project_id": "BBBB0002", "version_id": "DNEW0007", "dependency_type": "required"}]),
        );
        let mut routes = HashMap::new();
        for (project_id, document) in [
            ("AAAA0001", root_a_new.clone()),
            ("CCCC0003", root_b_new.clone()),
        ] {
            routes.insert(
                format!("/v2/project/{project_id}"),
                project(project_id, "mod"),
            );
            routes.insert(
                format!("/v2/project/{project_id}/version"),
                json!([document]),
            );
            routes.insert(
                format!("/v2/version/{}", document["id"].as_str().unwrap()),
                document,
            );
        }
        routes.insert("/v2/project/BBBB0002".into(), project("BBBB0002", "mod"));
        routes.insert(
            "/v2/project/BBBB0002/version".into(),
            json!([dep_v2.clone(), dep_v3.clone()]),
        );
        routes.insert("/v2/version/DNEW0006".into(), dep_v2.clone());
        routes.insert("/v2/version/DNEW0007".into(), dep_v3.clone());
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));

        assert!(matches!(
            client
                .resolve_updates(
                    &context(),
                    &[
                        (ContentType::Mod, "AAAA0001".into(), "ANEW0004".into()),
                        (ContentType::Mod, "CCCC0003".into(), "BNEW0005".into()),
                    ],
                    &state,
                )
                .await,
            Err(Error::DependencyUnresolved)
        ));
        // A target that is not an installed managed record is refused.
        assert!(matches!(
            client
                .resolve_updates(
                    &context(),
                    &[(ContentType::Mod, "DDDD0004".into(), "ANEW0004".into())],
                    &state,
                )
                .await,
            Err(Error::InvalidRequest)
        ));
    }

    #[tokio::test]
    async fn install_resolution_still_refuses_replacing_installed_dependency_versions() {
        // Outside an update graph, a dependency pinned to a version the
        // project does not have installed stays a hard conflict.
        let mut dep_old = state_record("BBBB0002", "DOLD0003", "fDOLD0003");
        dep_old.explicitly_retained = false;
        let state = installed_state(vec![dep_old.clone()]);
        let dep_new = versioned(
            "DNEW0006",
            "BBBB0002",
            "release",
            "2026-03-01T00:00:00Z",
            json!([]),
        );
        let root = versioned(
            "ANEW0004",
            "AAAA0001",
            "release",
            "2026-03-02T00:00:00Z",
            json!([{"project_id": "BBBB0002", "version_id": "DNEW0006", "dependency_type": "required"}]),
        );
        let mut routes = HashMap::new();
        routes.insert("/v2/project/AAAA0001".into(), project("AAAA0001", "mod"));
        routes.insert("/v2/version/ANEW0004".into(), root.clone());
        routes.insert("/v2/project/BBBB0002".into(), project("BBBB0002", "mod"));
        routes.insert("/v2/version/DNEW0006".into(), dep_new.clone());
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        assert!(matches!(
            client
                .resolve(&context(), ContentType::Mod, "AAAA0001", "ANEW0004", &state,)
                .await,
            Err(Error::DependencyVersionConflict(_))
        ));
    }

    #[tokio::test]
    async fn browse_search_maps_kinds_categories_and_sort_into_provider_facets() {
        let paths = Arc::new(Mutex::new(Vec::new()));
        let captured = paths.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            captured.lock().unwrap().push(request.path.clone());
            TestResponse::ok(br#"{"hits":[],"offset":0,"total_hits":0}"#)
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));

        let cases: Vec<(BrowseKind, Vec<&str>, BrowseSort)> = vec![
            (BrowseKind::Mod, vec![], BrowseSort::Relevance),
            (
                BrowseKind::Modpack,
                vec!["adventure", "technology"],
                BrowseSort::Downloads,
            ),
            (BrowseKind::ResourcePack, vec![], BrowseSort::Newest),
            (BrowseKind::ShaderPack, vec!["cartoon"], BrowseSort::Updated),
        ];
        for (kind, categories, sort) in cases {
            client
                .search_browse(
                    &context(),
                    kind,
                    "",
                    &categories
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>(),
                    sort,
                    0,
                )
                .await
                .unwrap();
        }
        let requests = paths.lock().unwrap();
        let parsed: Vec<(String, Vec<Vec<String>>, Option<String>)> = requests
            .iter()
            .map(|path| {
                let url = Url::parse(&format!("http://localhost{path}")).unwrap();
                let facets: Vec<Vec<String>> = serde_json::from_str(
                    &url.query_pairs()
                        .find(|(key, _)| key == "facets")
                        .unwrap()
                        .1,
                )
                .unwrap();
                let sort = url
                    .query_pairs()
                    .find(|(key, _)| key == "sort")
                    .map(|(_, value)| value.to_string());
                (url.path().to_owned(), facets, sort)
            })
            .collect();
        assert!(parsed.iter().all(|(path, _, _)| path == "/v2/search"));
        let (mod_facets, mod_sort) = (&parsed[0].1, &parsed[0].2);
        assert!(
            mod_facets
                .iter()
                .flatten()
                .any(|value| value == "project_type:mod")
        );
        assert!(
            mod_facets
                .iter()
                .flatten()
                .any(|value| value == "categories:fabric")
        );
        assert!(
            mod_facets
                .iter()
                .flatten()
                .any(|value| value == "environment:client_and_server")
        );
        assert_eq!(mod_sort.as_deref(), None);
        let (pack_facets, pack_sort) = (&parsed[1].1, &parsed[1].2);
        assert!(
            pack_facets
                .iter()
                .flatten()
                .any(|value| value == "project_type:modpack")
        );
        assert!(
            !pack_facets
                .iter()
                .flatten()
                .any(|value| value == "categories:fabric")
        );
        // Each selected category is its own facet group (AND semantics).
        assert_eq!(
            pack_facets
                .iter()
                .filter(|group| group.len() == 1 && group[0] == "categories:adventure")
                .count(),
            1
        );
        assert_eq!(
            pack_facets
                .iter()
                .filter(|group| group.len() == 1 && group[0] == "categories:technology")
                .count(),
            1
        );
        assert_eq!(pack_sort.as_deref(), Some("downloads"));
        let (rp_facets, _) = (&parsed[2].1, &parsed[2].2);
        assert!(
            rp_facets
                .iter()
                .flatten()
                .any(|value| value == "loaders:minecraft")
        );
        assert!(
            rp_facets
                .iter()
                .flatten()
                .any(|value| value == "project_type:resourcepack")
        );
        assert_eq!(parsed[3].2.as_deref(), Some("updated"));
        assert!(
            parsed[3]
                .1
                .iter()
                .flatten()
                .any(|value| value == "project_type:shader")
        );
        assert!(
            !parsed[3]
                .1
                .iter()
                .flatten()
                .any(|value| value.starts_with("loaders:"))
        );
    }

    #[tokio::test]
    async fn browse_search_rejects_invalid_categories_and_bounds_without_requests() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            captured.lock().unwrap().push(request.path.clone());
            TestResponse::ok(br#"{"hits":[],"offset":0,"total_hits":0}"#)
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        for categories in [
            vec!["has space".to_owned()],
            vec!["bad/slash".to_owned()],
            vec!["x".repeat(49)],
            (0..9).map(|index| format!("c{index}")).collect::<Vec<_>>(),
        ] {
            assert!(matches!(
                client
                    .search_browse(
                        &context(),
                        BrowseKind::Mod,
                        "",
                        &categories,
                        BrowseSort::Relevance,
                        0
                    )
                    .await,
                Err(Error::InvalidRequest)
            ));
        }
        assert!(requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn browse_categories_use_live_provider_tags_with_project_types() {
        let routes = Arc::new(Mutex::new(HashMap::new()));
        routes.lock().unwrap().insert(
            "/v2/tag/category".to_owned(),
            json!([
                {"name": "adventure", "project_type": "modpack"},
                {"name": "technology", "project_type": "mod"},
                {"name": "decorative", "project_type": "resourcepack"},
                {"name": "", "project_type": "mod"}
            ]),
        );
        let shared = routes.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            shared
                .lock()
                .unwrap()
                .get(path)
                .map(|value| TestResponse::ok(&serde_json::to_vec(value).unwrap()))
                .unwrap_or_else(|| TestResponse::status(404))
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let categories = client.categories().await.unwrap();
        assert_eq!(categories.len(), 3);
        assert!(
            categories
                .iter()
                .any(|category| category.name == "adventure" && category.project_type == "modpack")
        );

        let rate = TestServer::spawn(Arc::new(|_| TestResponse::status(429)));
        let client = Client::for_testing(&format!("{}/v2/", rate.base_url()));
        assert!(matches!(
            client.categories().await,
            Err(Error::RateLimited(_))
        ));
    }

    #[tokio::test]
    async fn modpack_details_browse_with_strong_identity_and_no_install_planning() {
        let mut routes: HashMap<String, Value> = HashMap::new();
        routes.insert(
            "/v2/project/PACK0001".into(),
            project("PACK0001", "modpack"),
        );
        routes.insert(
            "/v2/project/PACK0001/version".into(),
            json!([version("pack0002", "PACK0001", json!([]))]),
        );
        let server = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let details = client
            .details_browse(&context(), BrowseKind::Modpack, "PACK0001")
            .await
            .unwrap();
        assert_eq!(details.project_id, "PACK0001");
        assert_eq!(details.project_type, BrowseKind::Modpack);
        assert_eq!(details.default_version_id.as_deref(), Some("pack0002"));
        // A modpack is not an installable ContentType: no plan can exist.
        assert_eq!(BrowseKind::Modpack.content_type(), None);
        // Browsing another kind against a modpack project is refused by the
        // provider's own project type.
        assert!(matches!(
            client
                .details_browse(&context(), BrowseKind::Mod, "PACK0001")
                .await,
            Err(Error::NoCompatibleVersion)
        ));
    }

    #[tokio::test]
    async fn exact_pack_artifact_uses_provider_identity_and_published_sha512() {
        let mut routes = HashMap::new();
        routes.insert(
            "/v2/project/PACK0001".into(),
            project("PACK0001", "modpack"),
        );
        routes.insert("/v2/version/VERS0001".into(), json!({
            "id": "VERS0001", "project_id": "PACK0001", "name": "Pack 1", "version_number": "1.0",
            "version_type": "release", "date_published": "2026-01-01T00:00:00Z",
            "game_versions": ["1.21.1"], "loaders": ["fabric"], "environment": "client_only",
            "files": [{ "hashes": {"sha512": "a".repeat(128)}, "url": "https://cdn.modrinth.com/data/PACK0001/versions/VERS0001/pack.mrpack", "filename": "pack.mrpack", "primary": true, "size": 12, "file_type": null }],
            "dependencies": []
        }));
        let source = server(routes);
        let client = Client::for_testing(&format!("{}/v2/", source.base_url()));
        // The provider version id is exact; another project or version is
        // rejected rather than silently falling back to the newest release.
        let pack = client.pack_artifact("PACK0001", "VERS0001").await.unwrap();
        assert_eq!(pack.version_id, "VERS0001");
        assert_eq!(pack.sha512, "a".repeat(128));
        assert!(matches!(
            client.pack_artifact("PACK0001", "VERS0002").await,
            Err(Error::NotFound)
        ));
    }
}
