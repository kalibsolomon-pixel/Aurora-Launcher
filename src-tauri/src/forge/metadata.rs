//! The external Forge metadata boundary: version discovery, the verified
//! installer's embedded documents, and bounded installer-entry extraction.
//!
//! Discovery consumes only official MinecraftForge sources: the pinned
//! `maven.minecraftforge.net` Maven metadata (the full artifact listing)
//! plus the official promotions document (which names each Minecraft
//! version's `recommended` build — Forge's own stability policy). A build
//! is offered as stable exactly when it is that Minecraft version's
//! recommended promotion; list order is never used as a policy.
//!
//! The installer jar is acquired through the verified SHA-1 store by the
//! resolver before this boundary reads it; the documents extracted here
//! (`install_profile.json`, `version.json`) are external DTOs that never
//! become launcher domain types directly.
//!
//! Trust facts, kept distinct: the installer's own digest comes from the
//! repository's `.sha1` sidecar (the same upstream channel as the
//! artifact, an integrity expectation — not an independent signature);
//! launch-library digests are published inline by the version document;
//! processor-output digests are published inline by the install profile
//! as quoted literal data values; and one launch library (the patched
//! client) intentionally carries no URL — it exists only as the processor
//! chain's generated output, verified against its published SHA-1.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use url::Url;

use crate::downloads::{self, DownloadError, DownloadOptions};
use crate::integrity::Sha1Digest;
use crate::minecraft::metadata::MinecraftVersionId;

/// The largest installer-embedded document this boundary will buffer.
pub const MAX_INSTALLER_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// The largest installer jar this boundary will open.
pub const MAX_INSTALLER_JAR_BYTES: u64 = 64 * 1024 * 1024;

/// Length bound for one bare Forge version string.
pub const MAX_FORGE_VERSION_LENGTH: usize = 128;

/// The largest Maven-metadata listing this boundary will buffer.
const MAX_METADATA_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// The pinned official MinecraftForge endpoints for Forge metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeMavenEndpoints {
    maven: Url,
    promotions: Url,
}

impl ForgeMavenEndpoints {
    /// The official production endpoints.
    pub fn official() -> Self {
        Self {
            maven: Url::parse("https://maven.minecraftforge.net/")
                .expect("official Forge Maven root"),
            promotions: Url::parse(
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
            )
            .expect("official Forge promotions document"),
        }
    }

    /// Test endpoints rooted at an explicit base (the launcher's
    /// documented loopback test-transport pattern).
    pub fn loopback_for_testing(
        maven_base: &str,
        promotions_url: &str,
    ) -> Result<Self, ForgeMetadataError> {
        Ok(Self {
            maven: parse_absolute(maven_base)?,
            promotions: parse_absolute(promotions_url)?,
        })
    }

    /// The full artifact-version listing document of the official Maven.
    pub fn version_metadata_url(&self) -> Url {
        self.maven
            .join("net/minecraftforge/forge/maven-metadata.xml")
            .expect("the Maven metadata URL is derivable from the root")
    }

    /// The promotions document naming each Minecraft version's
    /// recommended (stable) Forge build.
    pub fn promotions_url(&self) -> &Url {
        &self.promotions
    }

    /// The installer artifact URL of one exact Forge version.
    pub fn installer_url(&self, version: &ForgeVersionId) -> Url {
        self.maven
            .join(&format!(
                "net/minecraftforge/forge/{v}/forge-{v}-installer.jar",
                v = version.as_str()
            ))
            .expect("the installer URL is derivable from the root")
    }
}

fn parse_absolute(url: &str) -> Result<Url, ForgeMetadataError> {
    let url = Url::parse(url).map_err(|error| ForgeMetadataError::Malformed {
        reason: format!("'{url}' is not a usable absolute URL: {error}"),
    })?;
    Ok(url)
}

/// One exact Forge version identity: the full combined upstream id
/// (`<minecraft>-<build>`, for example `1.21.11-61.2.0`). The combined
/// form is the artifact identity the official repository publishes; the
/// Minecraft half must always agree with the requested game version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForgeVersionId(String);

impl ForgeVersionId {
    /// Exact, reviewed compatibility set. Discovery is broader than execution.
    /// Historical synthetic F1B fixtures remain available to unit tests only.
    pub fn is_reviewed(&self) -> bool {
        matches!(
            self.as_str(),
            "1.20.1-47.4.10" | "1.20.4-49.2.0" | "1.21.11-61.2.0"
        ) || (cfg!(test) && self.as_str() == "1.21.1-52.1.16")
    }

    pub fn require_reviewed(&self) -> Result<(), ForgeMetadataError> {
        if self.is_reviewed() {
            return Ok(());
        }
        Err(ForgeMetadataError::Unsupported {
            reason: format!(
                "Forge {} is discoverable but has no reviewed installer compatibility in Aurora",
                self
            ),
        })
    }

    /// Validates and wraps one combined version id.
    pub fn new(value: &str) -> Result<Self, InvalidForgeVersion> {
        let value = value.trim();
        if value.len() > MAX_FORGE_VERSION_LENGTH {
            return Err(InvalidForgeVersion {
                version: value.to_owned(),
                reason: "the version is unexpectedly long".to_owned(),
            });
        }
        let Some((minecraft, build)) = value.split_once('-') else {
            return Err(InvalidForgeVersion {
                version: value.to_owned(),
                reason: "a Forge version combines '<minecraft>-<build>'".to_owned(),
            });
        };
        for part in [minecraft, build] {
            if part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+'))
            {
                return Err(InvalidForgeVersion {
                    version: value.to_owned(),
                    reason: "the version contains unusable characters".to_owned(),
                });
            }
        }
        if build.starts_with('.') || build.ends_with('.') || build.contains("..") {
            return Err(InvalidForgeVersion {
                version: value.to_owned(),
                reason: "the build number is malformed".to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// Parses one candidate relative to the requested Minecraft version:
    /// the Minecraft half must match exactly.
    pub fn parse_for_minecraft(
        value: &str,
        game: &MinecraftVersionId,
    ) -> Result<Self, InvalidForgeVersion> {
        let version = Self::new(value)?;
        if version.minecraft_version() != game.as_str() {
            return Err(InvalidForgeVersion {
                version: value.to_owned(),
                reason: format!(
                    "the Forge build targets Minecraft {}, not {}",
                    version.minecraft_version(),
                    game.as_str()
                ),
            });
        }
        Ok(version)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The Minecraft half of the combined identity.
    pub fn minecraft_version(&self) -> &str {
        self.0.split('-').next().expect("validated on construction")
    }

    /// The build half of the combined identity.
    pub fn build(&self) -> &str {
        self.0.split_once('-').expect("validated on construction").1
    }
}

impl fmt::Display for ForgeVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Why one Forge version string is unusable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidForgeVersion {
    pub version: String,
    pub reason: String,
}

impl fmt::Display for InvalidForgeVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Forge version '{}': {}",
            self.version, self.reason
        )
    }
}

impl std::error::Error for InvalidForgeVersion {}

/// The failure vocabulary of the Forge metadata boundary.
#[derive(Debug)]
pub enum ForgeMetadataError {
    /// The metadata transport failed or returned an unusable response.
    Network(DownloadError),
    /// A non-success status from a pinned official source.
    HttpStatus { status: u16 },
    /// A document exceeded the size the launcher will buffer.
    ResponseTooLarge { limit_bytes: usize },
    /// A document could not be parsed or failed validation.
    Malformed { reason: String },
    /// A document uses semantics this launcher deliberately does not
    /// support.
    Unsupported { reason: String },
}

impl fmt::Display for ForgeMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(error) => {
                write!(
                    formatter,
                    "official Forge metadata could not be fetched: {error}"
                )
            }
            Self::HttpStatus { status } => {
                write!(formatter, "an official Forge source answered HTTP {status}")
            }
            Self::ResponseTooLarge { limit_bytes } => write!(
                formatter,
                "official Forge metadata exceeded the {limit_bytes}-byte limit the launcher will buffer"
            ),
            Self::Malformed { reason } => {
                write!(formatter, "official Forge metadata is unusable: {reason}")
            }
            Self::Unsupported { reason } => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for ForgeMetadataError {}

/// One attributable Forge version in the official listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVersionEntry {
    version: ForgeVersionId,
    /// Whether the official promotions document names this exact build as
    /// the recommended (stable) build for its Minecraft version.
    stable: bool,
}

impl ForgeVersionEntry {
    fn parse(value: &str, recommended: Option<&str>) -> Option<Self> {
        let version = ForgeVersionId::new(value).ok()?;
        let stable = recommended == Some(version.build());
        Some(Self { version, stable })
    }

    pub fn version(&self) -> &ForgeVersionId {
        &self.version
    }

    pub fn stable(&self) -> bool {
        self.stable
    }

    fn build_key(&self) -> Vec<u64> {
        dotted_number_key(self.version.build())
    }
}

/// The parsed official listing: every attributable Forge version with its
/// promotions-derived stability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeVersionList {
    entries: Vec<ForgeVersionEntry>,
}

impl ForgeVersionList {
    pub fn installable_for_minecraft(&self, game: &MinecraftVersionId) -> Vec<ForgeVersionEntry> {
        self.for_minecraft(game)
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.version().as_str(),
                    "1.20.1-47.4.10" | "1.20.4-49.2.0" | "1.21.11-61.2.0"
                )
            })
            .collect()
    }
    /// Builds the listing from the official Maven metadata XML and the
    /// promotions document. Unattributable version strings are filtered
    /// out (they remain real repository versions, but this launcher
    /// cannot honestly map them to a Minecraft version); a listing with
    /// no attributable versions is malformed.
    pub fn from_documents(
        metadata_xml: &str,
        promotions_json: &str,
    ) -> Result<Self, ForgeMetadataError> {
        let versions = extract_xml_versions(metadata_xml)?;
        let promotions = PromotionsDocument::from_json(promotions_json)?;
        let mut entries = Vec::with_capacity(versions.len());
        for version in versions {
            if let Some(entry) = ForgeVersionEntry::parse(
                version,
                promotions.recommended_for(version.split('-').next().unwrap_or("")),
            ) {
                entries.push(entry);
            }
        }
        if entries.is_empty() {
            return Err(ForgeMetadataError::Malformed {
                reason: "the official Forge listing contains no attributable versions".to_owned(),
            });
        }
        Ok(Self { entries })
    }

    /// All attributable versions for one exact Minecraft version, newest
    /// build first, with stability from the promotions policy.
    pub fn for_minecraft(&self, game: &MinecraftVersionId) -> Vec<ForgeVersionEntry> {
        let mut matching: Vec<ForgeVersionEntry> = self
            .entries
            .iter()
            .filter(|entry| entry.version().minecraft_version() == game.as_str())
            .cloned()
            .collect();
        matching.sort_by(|a, b| b.build_key().cmp(&a.build_key()));
        matching
    }

    pub fn find(&self, version: &ForgeVersionId) -> Option<&ForgeVersionEntry> {
        self.entries.iter().find(|entry| entry.version() == version)
    }
}

/// The official promotions document: each Minecraft version's named
/// `recommended` and `latest` builds.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PromotionsDocument {
    recommended: BTreeMap<String, String>,
}

impl PromotionsDocument {
    fn from_json(text: &str) -> Result<Self, ForgeMetadataError> {
        #[derive(serde::Deserialize)]
        struct Document {
            #[serde(rename = "promos")]
            promos: BTreeMap<String, String>,
        }
        let document: Document =
            serde_json::from_str(text).map_err(|error| ForgeMetadataError::Malformed {
                reason: format!("the Forge promotions document is unusable: {error}"),
            })?;
        let mut recommended = BTreeMap::new();
        for (name, build) in document.promos {
            if let Some(minecraft) = name.strip_suffix("-recommended") {
                recommended.insert(minecraft.to_owned(), build);
            }
            // "-latest" promotions are informational; the recommended
            // promotion is the stability authority and list order is
            // never a policy.
        }
        Ok(Self { recommended })
    }

    fn recommended_for(&self, minecraft: &str) -> Option<&str> {
        self.recommended.get(minecraft).map(String::as_str)
    }
}

/// Extracts the `<version>` values inside the `<versions>` element of a
/// Maven metadata document. A small strict scanner rather than an XML
/// dependency: nesting is validated, values are returned verbatim, and
/// any structural surprise is malformed.
fn extract_xml_versions(text: &str) -> Result<Vec<&str>, ForgeMetadataError> {
    if text.len() > MAX_METADATA_DOCUMENT_BYTES {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_METADATA_DOCUMENT_BYTES,
        });
    }
    let start = text
        .find("<versions>")
        .ok_or_else(|| ForgeMetadataError::Malformed {
            reason: "the Maven metadata has no <versions> element".to_owned(),
        })?
        + "<versions>".len();
    let end = text[start..]
        .find("</versions>")
        .map(|offset| start + offset)
        .ok_or_else(|| ForgeMetadataError::Malformed {
            reason: "the Maven metadata <versions> element is unterminated".to_owned(),
        })?;
    let body = &text[start..end];
    let mut versions = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find("<version>") {
        let after = open + "<version>".len();
        let close = rest[after..]
            .find("</version>")
            .map(|offset| after + offset)
            .ok_or_else(|| ForgeMetadataError::Malformed {
                reason: "a Maven metadata <version> entry is unterminated".to_owned(),
            })?;
        versions.push(rest[after..close].trim());
        rest = &rest[close + "</version>".len()..];
    }
    if versions.is_empty() {
        return Err(ForgeMetadataError::Malformed {
            reason: "the Maven metadata lists no versions".to_owned(),
        });
    }
    Ok(versions)
}

/// Numeric comparison key for one dotted build number; unparseable
/// segments sort as zero so ordering stays total without inventing
/// semantics for them (such builds never become the automatic choice
/// because stability comes from promotions, not order).
fn dotted_number_key(build: &str) -> Vec<u64> {
    build
        .split('.')
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect()
}

/// Fetches the official listing and promotions documents together.
pub async fn fetch_forge_versions(
    endpoints: &ForgeMavenEndpoints,
    options: &DownloadOptions,
) -> Result<ForgeVersionList, ForgeMetadataError> {
    let metadata = fetch_document(&endpoints.version_metadata_url(), options).await?;
    let promotions = fetch_document(endpoints.promotions_url(), options).await?;
    let metadata_text = bytes_to_document_text(&metadata)?;
    let promotions_text = bytes_to_document_text(&promotions)?;
    ForgeVersionList::from_documents(&metadata_text, &promotions_text)
}

/// Fetches the official SHA-1 sidecar of one repository artifact.
///
/// Sidecars are the repository's own published digests; they turn the
/// installer artifact (which carries no inline digest in any metadata
/// document) into a digest-verified acquisition. A missing or malformed
/// sidecar is a hard failure — no digest is ever invented.
pub async fn fetch_sidecar_sha1(
    artifact_url: &Url,
    options: &DownloadOptions,
) -> Result<Sha1Digest, ForgeMetadataError> {
    let sidecar_url = Url::parse(&format!("{}.sha1", artifact_url.as_str())).map_err(|error| {
        ForgeMetadataError::Malformed {
            reason: format!("the artifact digest URL could not be derived: {error}"),
        }
    })?;
    let bytes = fetch_document(&sidecar_url, options).await?;
    if bytes.len() > 256 {
        return Err(ForgeMetadataError::Malformed {
            reason: "an artifact digest sidecar is unexpectedly large".to_owned(),
        });
    }
    let text = bytes_to_document_text(&bytes)?;
    let digest = text.trim();
    if digest.lines().count() > 1 {
        return Err(ForgeMetadataError::Malformed {
            reason: "an artifact digest sidecar must be a single line".to_owned(),
        });
    }
    Sha1Digest::parse(digest).map_err(|error| ForgeMetadataError::Malformed {
        reason: format!("the published artifact digest is not a valid SHA-1: {error}"),
    })
}

async fn fetch_document(
    url: &Url,
    options: &DownloadOptions,
) -> Result<Vec<u8>, ForgeMetadataError> {
    let client = downloads::build_client(options);
    let response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|error| ForgeMetadataError::Network(DownloadError::from_transport(error)))?;

    let status = response.status();
    if !status.is_success() {
        return Err(ForgeMetadataError::HttpStatus {
            status: status.as_u16(),
        });
    }
    if response
        .content_length()
        .is_some_and(|declared| declared as usize > MAX_METADATA_DOCUMENT_BYTES)
    {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_METADATA_DOCUMENT_BYTES,
        });
    }

    let mut document = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ForgeMetadataError::Network(DownloadError::from_transport(error)))?
    {
        if document.len() + chunk.len() > MAX_METADATA_DOCUMENT_BYTES {
            return Err(ForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_METADATA_DOCUMENT_BYTES,
            });
        }
        document.extend_from_slice(&chunk);
    }
    Ok(document)
}

fn bytes_to_document_text(bytes: &[u8]) -> Result<String, ForgeMetadataError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| ForgeMetadataError::Malformed {
        reason: "a Forge metadata document is not valid UTF-8".to_owned(),
    })
}

/// One Maven library requirement published by Forge metadata.
///
/// The shape mirrors Mojang's own library entries (name +
/// `downloads.artifact` with `path`, `url`, `sha1`, `size`), which is
/// exactly how Forge publishes them. One deliberate difference from the
/// Mojang shape: `url` may be empty, which marks an artifact that is
/// never downloaded — the processor chain's generated output (the
/// patched client), pinned by its published SHA-1 alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileLibrary {
    pub name: String,
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
}

/// One side-specific token value of the installer's data map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDataValue {
    pub client: String,
    pub server: String,
}

/// One published processor output expectation: the data token naming the
/// expected artifact and its official SHA-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProcessorOutput {
    pub token: String,
    pub sha1: Sha1Digest,
}

/// One processor definition: a tool jar, its classpath, its argument
/// vector, optionally restricted to installation sides, and any output
/// expectations the installer publishes for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProcessor {
    pub sides: Option<Vec<String>>,
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
    pub outputs: Vec<ProfileProcessorOutput>,
}

impl ProfileProcessor {
    /// Whether this processor applies to a client installation.
    pub fn applies_to_client(&self) -> bool {
        match &self.sides {
            None => true,
            Some(sides) => sides.iter().any(|side| side == "client"),
        }
    }
}

/// The `install_profile.json` document embedded in one installer jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProfileDocument {
    pub spec: u32,
    pub minecraft_version: String,
    pub version_id: String,
    pub data: BTreeMap<String, ProfileDataValue>,
    pub processors: Vec<ProfileProcessor>,
    pub libraries: Vec<ProfileLibrary>,
}

/// The `version.json` launch profile embedded in one installer jar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionProfileDocument {
    pub id: String,
    pub inherits_from: String,
    pub main_class: String,
    pub jvm_arguments: Vec<String>,
    pub game_arguments: Vec<String>,
    pub libraries: Vec<ProfileLibrary>,
}

impl InstallProfileDocument {
    pub fn from_json(text: &str) -> Result<Self, ForgeMetadataError> {
        if text.len() > MAX_INSTALLER_DOCUMENT_BYTES {
            return Err(ForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
            });
        }
        /// Parsed for shape fidelity only: several fields exist so the
        /// strict `deny_unknown_fields` boundary accepts the documented
        /// document shape without consuming every value (they describe
        /// the upstream GUI installer, which Aurora never runs).
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        #[allow(dead_code)]
        struct Document {
            spec: u32,
            #[serde(default)]
            profile: String,
            version: String,
            minecraft: String,
            #[serde(default)]
            icon: Option<String>,
            #[serde(default)]
            json: String,
            #[serde(default)]
            logo: Option<String>,
            #[serde(default)]
            welcome: Option<String>,
            #[serde(default, rename = "mirrorList")]
            mirror_list: Option<String>,
            #[serde(default, rename = "hideExtract")]
            hide_extract: Option<bool>,
            #[serde(default)]
            path: Option<String>,
            #[serde(default, rename = "_comment", alias = "_comment_")]
            comment: Option<serde_json::Value>,
            #[serde(default, rename = "serverJarPath")]
            server_jar_path: Option<String>,
            data: BTreeMap<String, RawDataValue>,
            #[serde(default)]
            processors: Vec<RawProcessor>,
            #[serde(default)]
            libraries: Vec<RawLibrary>,
        }
        let document: Document =
            serde_json::from_str(text).map_err(|error| ForgeMetadataError::Malformed {
                reason: format!("the Forge install profile is unusable: {error}"),
            })?;
        if document.spec != 1 {
            return Err(ForgeMetadataError::Unsupported {
                reason: format!(
                    "the Forge install profile uses spec {} which this launcher does not support",
                    document.spec
                ),
            });
        }
        if document.version.is_empty() || document.minecraft.is_empty() {
            return Err(ForgeMetadataError::Malformed {
                reason: "the Forge install profile has no version identity".to_owned(),
            });
        }
        if !document.json.is_empty() && document.json != "/version.json" {
            return Err(ForgeMetadataError::Unsupported {
                reason: format!(
                    "the Forge install profile embeds its version document at '{}' instead of '/version.json'",
                    document.json
                ),
            });
        }
        let data = document
            .data
            .into_iter()
            .map(|(token, value)| {
                if token.is_empty() || token.contains(['{', '}', '[', ']']) {
                    return Err(ForgeMetadataError::Malformed {
                        reason: format!(
                            "the install profile defines a malformed data token '{token}'"
                        ),
                    });
                }
                if value.client.trim().is_empty() || value.server.trim().is_empty() {
                    return Err(ForgeMetadataError::Malformed {
                        reason: format!("the data token '{token}' has an empty side value"),
                    });
                }
                Ok((
                    token,
                    ProfileDataValue {
                        client: value.client,
                        server: value.server,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let processors = document
            .processors
            .into_iter()
            .map(|processor| normalize_processor(processor, &data))
            .collect::<Result<Vec<_>, _>>()?;
        let libraries = document
            .libraries
            .iter()
            .map(parse_profile_library)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            spec: document.spec,
            minecraft_version: document.minecraft,
            version_id: document.version,
            data,
            processors,
            libraries,
        })
    }
}

/// Normalizes one raw processor, resolving its output expectations
/// against the already-parsed data map.
fn normalize_processor(
    processor: RawProcessor,
    data: &BTreeMap<String, ProfileDataValue>,
) -> Result<ProfileProcessor, ForgeMetadataError> {
    if processor.sides.as_ref().is_some_and(|sides| {
        sides.is_empty()
            || sides
                .iter()
                .any(|side| !matches!(side.as_str(), "client" | "server"))
    }) {
        return Err(ForgeMetadataError::Unsupported {
            reason: "unrecognized Forge processor side".into(),
        });
    }
    if processor.jar.trim().is_empty() || processor.args.is_empty() {
        return Err(ForgeMetadataError::Malformed {
            reason: "a Forge processor definition has no jar or arguments".to_owned(),
        });
    }
    for argument in processor.args.iter().chain(processor.classpath.iter()) {
        if argument.len() > 4096 || argument.contains('\0') {
            return Err(ForgeMetadataError::Malformed {
                reason: "a Forge processor argument is malformed".to_owned(),
            });
        }
    }
    let published = processor.outputs.unwrap_or_default();
    let mut outputs = Vec::with_capacity(published.len());
    for (raw_token, digest) in published {
        // The upstream keys are brace-wrapped data-token references
        // ("{PATCHED}"); the token itself is the inner name.
        let token = raw_token
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
            .unwrap_or(&raw_token)
            .to_owned();
        if token.is_empty() || token.contains(['{', '}']) {
            return Err(ForgeMetadataError::Malformed {
                reason: format!(
                    "a Forge processor output expectation has a malformed token '{raw_token}'"
                ),
            });
        }
        let sha1_text: String = match digest
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        {
            // "{TOKEN}" references the data map; the referenced value
            // must be a quoted literal digest.
            Some(referenced) => {
                let Some(value) = data.get(referenced) else {
                    return Err(ForgeMetadataError::Malformed {
                        reason: format!(
                            "a Forge processor output expectation references the undefined token '{{{referenced}}}'"
                        ),
                    });
                };
                strip_single_quotes(&value.client)?.to_owned()
            }
            // A direct literal digest.
            None => digest.clone(),
        };
        let sha1 =
            Sha1Digest::parse(sha1_text.trim()).map_err(|_| ForgeMetadataError::Malformed {
                reason: format!(
                    "a Forge processor output expectation for '{token}' is not a valid SHA-1"
                ),
            })?;
        outputs.push(ProfileProcessorOutput { token, sha1 });
    }
    Ok(ProfileProcessor {
        sides: processor.sides,
        jar: processor.jar,
        classpath: processor.classpath,
        args: processor.args,
        outputs,
    })
}

fn strip_single_quotes(value: &str) -> Result<&str, ForgeMetadataError> {
    let inner = value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .ok_or_else(|| ForgeMetadataError::Malformed {
            reason: format!("a quoted Forge data value is malformed: '{value}'"),
        })?;
    Ok(inner)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProcessor {
    #[serde(default)]
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    args: Vec<String>,
    #[serde(default)]
    outputs: Option<BTreeMap<String, String>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDataValue {
    client: String,
    server: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLibrary {
    name: String,
    downloads: RawDownloads,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDownloads {
    artifact: RawArtifact,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifact {
    path: String,
    #[serde(default)]
    url: String,
    sha1: String,
    size: u64,
}

fn parse_profile_library(entry: &RawLibrary) -> Result<ProfileLibrary, ForgeMetadataError> {
    if entry.name.trim().is_empty() {
        return Err(ForgeMetadataError::Malformed {
            reason: "a Forge profile library has no name".to_owned(),
        });
    }
    if !is_safe_forward_path(&entry.downloads.artifact.path) {
        return Err(ForgeMetadataError::Malformed {
            reason: format!(
                "the Forge library '{}' declares an unsafe repository path",
                entry.name
            ),
        });
    }
    if !entry.downloads.artifact.url.is_empty()
        && Url::parse(&entry.downloads.artifact.url).is_err()
    {
        return Err(ForgeMetadataError::Malformed {
            reason: format!(
                "the Forge library '{}' declares a malformed URL",
                entry.name
            ),
        });
    }
    Sha1Digest::parse(&entry.downloads.artifact.sha1).map_err(|_| {
        ForgeMetadataError::Malformed {
            reason: format!(
                "the Forge library '{}' does not publish a valid SHA-1",
                entry.name
            ),
        }
    })?;
    Ok(ProfileLibrary {
        name: entry.name.clone(),
        path: entry.downloads.artifact.path.clone(),
        url: entry.downloads.artifact.url.clone(),
        sha1: entry.downloads.artifact.sha1.clone(),
        size: entry.downloads.artifact.size,
    })
}

fn is_safe_forward_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains("..")
        && !path.contains(':')
        && !path.starts_with('/')
}

impl VersionProfileDocument {
    pub fn from_json(text: &str) -> Result<Self, ForgeMetadataError> {
        if text.len() > MAX_INSTALLER_DOCUMENT_BYTES {
            return Err(ForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
            });
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        #[allow(dead_code)]
        struct Document {
            id: String,
            #[serde(default, rename = "_comment", alias = "_comment_")]
            comment: Option<serde_json::Value>,
            #[serde(rename = "inheritsFrom")]
            inherits_from: String,
            #[serde(rename = "mainClass")]
            main_class: String,
            #[serde(default)]
            logging: serde_json::Value,
            arguments: RawArguments,
            #[serde(default)]
            libraries: Vec<RawLibrary>,
            #[serde(default, rename = "releaseTime")]
            release_time: Option<String>,
            #[serde(default)]
            time: Option<String>,
            #[serde(default, rename = "type")]
            kind: Option<String>,
        }
        let document: Document =
            serde_json::from_str(text).map_err(|error| ForgeMetadataError::Malformed {
                reason: format!("the Forge version profile is unusable: {error}"),
            })?;
        if document
            .logging
            .as_object()
            .map(|value| !value.is_empty())
            .unwrap_or(true)
        {
            return Err(ForgeMetadataError::Unsupported {
                reason:
                    "the Forge version profile declares its own logging configuration, which this launcher does not support"
                        .to_owned(),
            });
        }
        let libraries = document
            .libraries
            .iter()
            .map(parse_profile_library)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            id: document.id,
            inherits_from: document.inherits_from,
            main_class: document.main_class,
            jvm_arguments: document.arguments.jvm,
            game_arguments: document.arguments.game,
            libraries,
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArguments {
    jvm: Vec<String>,
    game: Vec<String>,
}

/// Reads and parses the two documents embedded in one verified installer
/// jar. The jar is opened read-only; every entry is size-bounded before
/// buffering; duplicate document entries are refused.
pub fn read_installer_documents(
    installer_path: &Path,
) -> Result<(InstallProfileDocument, VersionProfileDocument), ForgeMetadataError> {
    let file =
        std::fs::File::open(installer_path).map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the verified Forge installer jar could not be opened: {error}"),
        })?;
    let jar_size = file
        .metadata()
        .map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the verified Forge installer jar is unreadable: {error}"),
        })?
        .len();
    if jar_size > MAX_INSTALLER_JAR_BYTES {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_JAR_BYTES as usize,
        });
    }
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the verified Forge installer jar is not a readable archive: {error}"),
        })?;
    let profile_text = read_bounded_entry(&mut archive, "install_profile.json")?;
    let version_text = read_bounded_entry(&mut archive, "version.json")?;
    let profile = InstallProfileDocument::from_json(&profile_text)?;
    let version = VersionProfileDocument::from_json(&version_text)?;
    Ok((profile, version))
}

/// Extracts one installer-embedded file (the binpatch bundle) to an
/// explicit destination. The entry name must be a safe relative
/// forward-slash path; extraction is a plain file copy of verified
/// installer bytes, never an archive unpack.
pub fn extract_installer_entry(
    installer_path: &Path,
    entry_name: &str,
    destination: &Path,
) -> Result<(), ForgeMetadataError> {
    let name = entry_name
        .strip_prefix('/')
        .filter(|name| is_safe_forward_path(name))
        .ok_or_else(|| ForgeMetadataError::Malformed {
            reason: format!("'{entry_name}' is not a safe installer-embedded path"),
        })?;
    let file =
        std::fs::File::open(installer_path).map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the verified Forge installer jar could not be opened: {error}"),
        })?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the verified Forge installer jar is not a readable archive: {error}"),
        })?;
    if archive.file_names().filter(|entry| *entry == name).count() != 1 {
        return Err(ForgeMetadataError::Malformed {
            reason: "an installer entry is missing or duplicated".into(),
        });
    }
    let mut entry = archive
        .by_name(name)
        .map_err(|_| ForgeMetadataError::Malformed {
            reason: format!("the installer embeds no entry '{name}'"),
        })?;
    if entry.size() > MAX_INSTALLER_JAR_BYTES {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_JAR_BYTES as usize,
        });
    }
    if entry.is_dir()
        || entry
            .unix_mode()
            .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000))
    {
        return Err(ForgeMetadataError::Malformed {
            reason: "an installer entry is not a regular file".into(),
        });
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the installer entry destination is unusable: {error}"),
        })?;
    }
    let mut destination_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| ForgeMetadataError::Malformed {
            reason: format!("the installer entry could not be staged: {error}"),
        })?;
    let copied = std::io::copy(
        &mut std::io::Read::take(&mut entry, MAX_INSTALLER_JAR_BYTES + 1),
        &mut destination_file,
    )
    .map_err(|error| ForgeMetadataError::Malformed {
        reason: format!("the installer entry '{name}' is unreadable: {error}"),
    })?;
    if copied > MAX_INSTALLER_JAR_BYTES {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_JAR_BYTES as usize,
        });
    }
    Ok(())
}

fn read_bounded_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<String, ForgeMetadataError> {
    let duplicates = archive
        .file_names()
        .filter(|existing| *existing == name)
        .count();
    if duplicates != 1 {
        return Err(ForgeMetadataError::Malformed {
            reason: format!("the installer embeds {duplicates} '{name}' documents"),
        });
    }
    let mut entry = archive
        .by_name(name)
        .map_err(|_| ForgeMetadataError::Malformed {
            reason: format!("the installer embeds no '{name}' document"),
        })?;
    if entry.size() > MAX_INSTALLER_DOCUMENT_BYTES as u64 {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
        });
    }
    if entry.is_dir()
        || entry
            .unix_mode()
            .is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000))
    {
        return Err(ForgeMetadataError::Malformed {
            reason: "an installer document is not a regular file".into(),
        });
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    std::io::Read::read_to_end(
        &mut std::io::Read::take(&mut entry, (MAX_INSTALLER_DOCUMENT_BYTES + 1) as u64),
        &mut bytes,
    )
    .map_err(|error| ForgeMetadataError::Malformed {
        reason: format!("the '{name}' entry of the installer jar is unreadable: {error}"),
    })?;
    if bytes.len() > MAX_INSTALLER_DOCUMENT_BYTES {
        return Err(ForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
        });
    }
    String::from_utf8(bytes).map_err(|_| ForgeMetadataError::Malformed {
        reason: format!("the '{name}' document of the installer jar is not valid UTF-8"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The official Maven metadata listing, abridged to representative
    /// versions (shape verbatim from the live document).
    fn metadata_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>net.minecraftforge</groupId>
  <artifactId>forge</artifactId>
  <versioning>
    <release>1.21.11-61.2.1</release>
    <versions>
      <version>1.21.1-52.1.15</version>
      <version>1.21.1-52.1.16</version>
      <version>1.21.11-61.1.15</version>
      <version>1.21.11-61.1.16</version>
      <version>1.21.11-61.2.0</version>
      <version>1.21.11-61.2.1</version>
      <version>1.21.6-56.0.9</version>
      <version>not-a-pair</version>
    </versions>
  </versioning>
</metadata>"#
            .to_owned()
    }

    /// The official promotions document, abridged (shape verbatim).
    fn promotions_json() -> String {
        r#"{"homepage":"https://files.minecraftforge.net/net/minecraftforge/forge/","promos":{
            "1.21.1-latest":"52.1.16","1.21.1-recommended":"52.1.0",
            "1.21.11-latest":"61.2.1","1.21.11-recommended":"61.2.0",
            "1.21.6-latest":"56.0.9"}}"#
            .to_owned()
    }

    #[test]
    fn version_ids_require_the_combined_shape() {
        assert!(ForgeVersionId::new("1.21.11-61.2.0").is_ok());
        assert!(ForgeVersionId::new("61.2.0").is_err());
        assert!(ForgeVersionId::new("1.21.11-").is_err());
        assert!(ForgeVersionId::new("-61.2.0").is_err());
        assert!(ForgeVersionId::new("1.21.11-61..0").is_err());
        assert!(ForgeVersionId::new("1.21.11-61.2.0/../evil").is_err());
        let id = ForgeVersionId::new("1.21.11-61.2.0").unwrap();
        assert_eq!(id.minecraft_version(), "1.21.11");
        assert_eq!(id.build(), "61.2.0");
        assert_eq!(id.to_string(), "1.21.11-61.2.0");
        let game = MinecraftVersionId::new("1.21.1").unwrap();
        assert!(ForgeVersionId::parse_for_minecraft("1.21.11-61.2.0", &game).is_err());
        assert!(ForgeVersionId::parse_for_minecraft("1.21.1-52.1.16", &game).is_ok());
    }

    #[test]
    fn the_listing_parses_with_promotions_stability() {
        let listing =
            ForgeVersionList::from_documents(&metadata_xml(), &promotions_json()).unwrap();
        let game = MinecraftVersionId::new("1.21.11").unwrap();
        let for_game = listing.for_minecraft(&game);
        let ids: Vec<&str> = for_game
            .iter()
            .map(|entry| entry.version().as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "1.21.11-61.2.1",
                "1.21.11-61.2.0",
                "1.21.11-61.1.16",
                "1.21.11-61.1.15"
            ]
        );
        // Stability comes from the recommended promotion alone.
        let stable: Vec<bool> = for_game.iter().map(|entry| entry.stable()).collect();
        assert_eq!(stable, vec![false, true, false, false]);
        // Unattributable entries are filtered out entirely ("not-a-pair"
        // is not even a parsable combined id; unparsable strings never
        // reach the listing).
        assert!(ForgeVersionId::new("not-a-pair").is_err());
        assert_eq!(
            listing
                .find(&ForgeVersionId::new("1.21.11-61.2.1").unwrap())
                .map(|entry| entry.version().as_str()),
            Some("1.21.11-61.2.1")
        );
        // A Minecraft version with no recommended promotion has no stable
        // build: the automatic policy will refuse rather than guess.
        let other = MinecraftVersionId::new("1.21.6").unwrap();
        assert!(listing.for_minecraft(&other)[0].version().as_str() == "1.21.6-56.0.9");
        assert!(!listing.for_minecraft(&other)[0].stable());
        // An unknown Minecraft version has no builds at all.
        let unknown = MinecraftVersionId::new("1.21.2").unwrap();
        assert!(listing.for_minecraft(&unknown).is_empty());
    }

    #[test]
    fn malformed_listings_are_refused() {
        assert!(ForgeVersionList::from_documents("no xml here", &promotions_json()).is_err());
        assert!(
            ForgeVersionList::from_documents(
                "<metadata><versions><version>1.21.11-61.2.0</version>",
                &promotions_json()
            )
            .is_err()
        );
        assert!(
            ForgeVersionList::from_documents(
                "<metadata><versions></versions></metadata>",
                &promotions_json()
            )
            .is_err()
        );
        assert!(ForgeVersionList::from_documents(&metadata_xml(), "{ broken").is_err());
    }

    #[test]
    fn installer_urls_derive_from_the_official_root() {
        let endpoints = ForgeMavenEndpoints::official();
        assert_eq!(
            endpoints.version_metadata_url().as_str(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml"
        );
        let version = ForgeVersionId::new("1.21.11-61.2.0").unwrap();
        assert_eq!(
            endpoints.installer_url(&version).as_str(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-installer.jar"
        );
    }

    /// The real `install_profile.json` of Forge 1.21.11-61.2.0, abridged
    /// to its load-bearing content (data map, client processors with
    /// output expectations, representative libraries) with shapes and
    /// digests verbatim from the verified upstream document.
    fn real_profile_61_2_0() -> String {
        r#"{
          "spec": 1,
          "profile": "forge",
          "version": "1.21.11-forge-61.2.0",
          "minecraft": "1.21.11",
          "json": "/version.json",
          "path": null,
          "logo": "/logo.png",
          "welcome": "Welcome to the simple Forge installer.",
          "mirrorList": "https://maven.minecraftforge.net/mirrors/",
          "hideExtract": false,
          "_comment": ["abridged for tests"],
          "data": {
            "MOJMAPS": {"client": "[net.minecraft:client:1.21.11:mappings@tsrg]", "server": "[net.minecraft:server:1.21.11:mappings@tsrg]"},
            "MOJMAPS_SHA": {"client": "'f9240aaa50a06c04d8e285772ba16a340ddc5aef'", "server": "'cf70e98af2cbcce6cc03d7e6f29ee58dcd9d0e6f'"},
            "MC_UNPACKED": {"client": "[net.minecraft:client:1.21.11]", "server": "[net.minecraft:server:1.21.11:unpacked]"},
            "MC_UNPACKED_SHA": {"client": "'ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd'", "server": "'37f3f53895def38dcb4c9e4cd7f2efdecf201d9b'"},
            "MC_OFF": {"client": "[net.minecraft:client:1.21.11:official]", "server": "[net.minecraft:server:1.21.11:official]"},
            "MC_OFF_SHA": {"client": "'30a7252f08508f83ddcafa39883fabd77596df72'", "server": "'a61ef82940c5e9ff8e3005bf3e00fc550cfda53a'"},
            "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"},
            "PATCHED": {"client": "[net.minecraftforge:forge:1.21.11-61.2.0:client]", "server": "[net.minecraftforge:forge:1.21.11-61.2.0:server]"},
            "PATCHED_SHA": {"client": "'632ace45e535291c7576764c1c8d6388b73c9a75'", "server": "'ce19cb32f24f3c39dac759da14fc84dc16adb9a9'"}
          },
          "processors": [
            {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.4.3", "classpath": ["net.minecraftforge:installertools:1.4.3"], "args": ["--task", "EXTRACT_FILES", "--archive", "{INSTALLER}", "--from", "data/README.txt", "--to", "{ROOT}/README.txt"], "outputs": {}},
            {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.4.3", "classpath": ["net.minecraftforge:installertools:1.4.3"], "args": ["--task", "BUNDLER_EXTRACT", "--input", "{MINECRAFT_JAR}", "--output", "{ROOT}/libraries/", "--libraries"], "outputs": {}},
            {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.4.3", "classpath": ["net.minecraftforge:installertools:1.4.3"], "args": ["--task", "BUNDLER_EXTRACT", "--input", "{MINECRAFT_JAR}", "--output", "{MC_UNPACKED}", "--jar-only"], "outputs": {"{MC_UNPACKED}": "{MC_UNPACKED_SHA}"}},
            {"jar": "net.minecraftforge:installertools:1.4.3", "classpath": ["net.minecraftforge:installertools:1.4.3"], "args": ["--task", "DOWNLOAD_MOJMAPS", "--sanitize", "--version", "1.21.11", "--side", "{SIDE}", "--output", "{MOJMAPS}"], "outputs": {"{MOJMAPS}": "{MOJMAPS_SHA}"}},
            {"sides": ["server"], "jar": "net.minecraftforge:ForgeAutoRenamingTool:1.0.6", "classpath": ["net.minecraftforge:ForgeAutoRenamingTool:1.0.6"], "args": ["--input", "{MC_UNPACKED}", "--output", "{MC_OFF}", "--names", "{MOJMAPS}", "--reverse"], "outputs": {"{MC_OFF}": "{MC_OFF_SHA}"}},
            {"sides": ["client"], "jar": "net.minecraftforge:ForgeAutoRenamingTool:1.0.6", "classpath": ["net.minecraftforge:ForgeAutoRenamingTool:1.0.6"], "args": ["--input", "{MINECRAFT_JAR}", "--output", "{MC_OFF}", "--names", "{MOJMAPS}", "--reverse"], "outputs": {"{MC_OFF}": "{MC_OFF_SHA}"}},
            {"jar": "net.minecraftforge:binarypatcher:1.3.1", "classpath": ["net.minecraftforge:binarypatcher:1.3.1"], "args": ["--clean", "{MC_OFF}", "--output", "{PATCHED}", "--apply", "{BINPATCH}", "--data", "--unpatched"], "outputs": {"{PATCHED}": "{PATCHED_SHA}"}}
          ],
          "libraries": [
            {"name": "net.minecraftforge:installertools:1.4.3", "downloads": {"artifact": {"path": "net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar", "sha1": "e37ea5710e86ff09c34ba1c4a8a3f03f24dba699", "size": 1234567}}},
            {"name": "net.minecraftforge:binarypatcher:1.3.1", "downloads": {"artifact": {"path": "net/minecraftforge/binarypatcher/1.3.1/binarypatcher-1.3.1.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/binarypatcher/1.3.1/binarypatcher-1.3.1.jar", "sha1": "171fd4a353689ba52a5166a2b9330f57f6a41bf5", "size": 76543}}},
            {"name": "de.oceanlabs.mcp:mcp_config:1.21.11-20251223.124241@zip", "downloads": {"artifact": {"path": "de/oceanlabs/mcp/mcp_config/1.21.11-20251223.124241/mcp_config-1.21.11-20251223.124241.zip", "url": "https://maven.minecraftforge.net/de/oceanlabs/mcp/mcp_config/1.21.11-20251223.124241/mcp_config-1.21.11-20251223.124241.zip", "sha1": "22f0d9332cd0b64e5b7e2b09e9ddca94596384cf", "size": 999999}}}
          ]
        }"#
        .to_owned()
    }

    /// The real `version.json` of Forge 1.21.11-61.2.0, abridged with
    /// shapes and digests verbatim (including the URL-less generated
    /// patched-client library).
    fn real_version_61_2_0() -> String {
        r#"{
          "_comment": ["abridged for tests"],
          "id": "1.21.11-forge-61.2.0",
          "inheritsFrom": "1.21.11",
          "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
          "type": "release",
          "releaseTime": "2026-01-15T00:00:00Z",
          "time": "2026-01-15T00:00:00Z",
          "logging": {},
          "arguments": {
            "jvm": ["-Djava.net.preferIPv6Addresses=system"],
            "game": ["--launchTarget", "forge_client"]
          },
          "libraries": [
            {"name": "net.minecraftforge:forge:1.21.11-61.2.0:universal", "downloads": {"artifact": {"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "sha1": "684d4cf385f66d2757c114cd513ad370f3465090", "size": 2908730}}},
            {"name": "net.minecraftforge:forge:1.21.11-61.2.0:client", "downloads": {"artifact": {"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-client.jar", "url": "", "sha1": "632ace45e535291c7576764c1c8d6388b73c9a75", "size": 32624066}}},
            {"name": "cpw.mods:bootstraplauncher:2.0.2", "downloads": {"artifact": {"path": "cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "url": "https://maven.minecraftforge.net/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "sha1": "b3add478d438ff7e5c9dd1c4a2a2d0a1ff7e4d20", "size": 111222}}}
          ]
        }"#
        .to_owned()
    }

    #[test]
    fn the_real_profile_parses_with_verbatim_semantics() {
        let profile = InstallProfileDocument::from_json(&real_profile_61_2_0()).unwrap();
        assert_eq!(profile.spec, 1);
        assert_eq!(profile.minecraft_version, "1.21.11");
        assert_eq!(profile.version_id, "1.21.11-forge-61.2.0");
        assert_eq!(profile.data.len(), 9);
        assert_eq!(
            profile.data["MOJMAPS"].client,
            "[net.minecraft:client:1.21.11:mappings@tsrg]"
        );
        assert_eq!(profile.data["BINPATCH"].client, "/data/client.lzma");
        // Exactly three client-side processors, in profile order, with
        // their published output digests resolved from the data map.
        let client: Vec<&ProfileProcessor> = profile
            .processors
            .iter()
            .filter(|processor| processor.applies_to_client())
            .collect();
        assert_eq!(client.len(), 3);
        assert_eq!(client[0].jar, "net.minecraftforge:installertools:1.4.3");
        assert_eq!(client[0].outputs[0].token, "MOJMAPS");
        assert_eq!(
            client[0].outputs[0].sha1.as_hex(),
            "f9240aaa50a06c04d8e285772ba16a340ddc5aef"
        );
        assert_eq!(
            client[1].jar,
            "net.minecraftforge:ForgeAutoRenamingTool:1.0.6"
        );
        assert_eq!(client[2].jar, "net.minecraftforge:binarypatcher:1.3.1");
        assert_eq!(
            client[2].outputs[0].sha1.as_hex(),
            "632ace45e535291c7576764c1c8d6388b73c9a75"
        );
        // The @zip extension survives in the profile library name.
        assert!(
            profile
                .libraries
                .iter()
                .any(|library| library.name.ends_with("@zip"))
        );
    }

    #[test]
    fn the_real_version_profile_parses_with_the_generated_client() {
        let version = VersionProfileDocument::from_json(&real_version_61_2_0()).unwrap();
        assert_eq!(version.id, "1.21.11-forge-61.2.0");
        assert_eq!(version.inherits_from, "1.21.11");
        assert_eq!(
            version.main_class,
            "net.minecraftforge.bootstrap.ForgeBootstrap"
        );
        assert_eq!(
            version.jvm_arguments,
            vec!["-Djava.net.preferIPv6Addresses=system"]
        );
        assert_eq!(
            version.game_arguments,
            vec!["--launchTarget", "forge_client"]
        );
        let patched = version
            .libraries
            .iter()
            .find(|library| library.name.ends_with(":client"))
            .unwrap();
        assert_eq!(patched.url, "", "the patched client is never downloaded");
        assert_eq!(patched.sha1, "632ace45e535291c7576764c1c8d6388b73c9a75");
    }

    #[test]
    fn unsupported_and_malformed_profiles_fail_explicitly() {
        let spec2 = real_profile_61_2_0().replace("\"spec\": 1", "\"spec\": 2");
        assert!(matches!(
            InstallProfileDocument::from_json(&spec2),
            Err(ForgeMetadataError::Unsupported { .. })
        ));
        let unknown_output_token = real_profile_61_2_0().replace(
            "\"{MOJMAPS}\": \"{MOJMAPS_SHA}\"",
            "\"{MOJMAPS}\": \"{UNDEFINED_SHA}\"",
        );
        assert!(InstallProfileDocument::from_json(&unknown_output_token).is_err());
        let direct_digest = real_profile_61_2_0().replace(
            "\"{PATCHED}\": \"{PATCHED_SHA}\"",
            "\"{PATCHED}\": \"632ace45e535291c7576764c1c8d6388b73c9a75\"",
        );
        assert!(InstallProfileDocument::from_json(&direct_digest).is_ok());
        let bad_digest = real_profile_61_2_0().replace(
            "\"{PATCHED}\": \"{PATCHED_SHA}\"",
            "\"{PATCHED}\": \"deadbeef\"",
        );
        assert!(InstallProfileDocument::from_json(&bad_digest).is_err());
        let traversal_library = real_profile_61_2_0().replace(
            "net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar",
            "../../evil.jar",
        );
        assert!(InstallProfileDocument::from_json(&traversal_library).is_err());
        // A declared logging configuration is deliberately unsupported.
        let with_logging =
            real_version_61_2_0().replace("\"logging\": {}", "\"logging\": {\"client\": {}}");
        assert!(matches!(
            VersionProfileDocument::from_json(&with_logging),
            Err(ForgeMetadataError::Unsupported { .. })
        ));
    }

    #[test]
    fn installer_documents_read_from_a_real_shaped_jar() {
        use std::io::Write as _;
        let directory =
            std::env::temp_dir().join(format!("aurora-forge-metadata-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let jar = directory.join("installer.jar");
        let file = std::fs::File::create(&jar).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, bytes) in [
            ("install_profile.json", real_profile_61_2_0().into_bytes()),
            ("version.json", real_version_61_2_0().into_bytes()),
            ("data/client.lzma", b"synthetic binpatch bundle".to_vec()),
        ] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();
        let (profile, version) = read_installer_documents(&jar).unwrap();
        assert_eq!(profile.version_id, "1.21.11-forge-61.2.0");
        assert_eq!(
            version.main_class,
            "net.minecraftforge.bootstrap.ForgeBootstrap"
        );
        let destination = directory.join("client.lzma");
        extract_installer_entry(&jar, "/data/client.lzma", &destination).unwrap();
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"synthetic binpatch bundle"
        );
        assert!(extract_installer_entry(&jar, "/../evil", &destination).is_err());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
