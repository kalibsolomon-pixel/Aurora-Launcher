//! External NeoForge metadata boundary: DTOs, endpoints, and validation.
//!
//! Everything in this module speaks the *external* shapes of the official
//! NeoForge distribution (the pinned `maven.neoforged.net` repository and
//! the installer artifacts it publishes). Raw documents never leave this
//! module; [`crate::neoforge::plan`] normalizes them into Aurora-owned
//! domain types, and installers consume only the normalized plan.
//!
//! The official model, verified against live artifacts (see
//! `.zcode-diag/phase-k-neoforge/research-notes.md`):
//!
//! - Version discovery is the repository's release listing for
//!   `net.neoforged:neoforge`. Versions are bare NeoForge versions; the
//!   Minecraft version is encoded by the leading components
//!   (`A.B.C` with `A` in `{20, 21}` → Minecraft `1.A.B`; `A.B.C.D` →
//!   Minecraft `A.B.C`, or `A.B` when `C` is zero).
//! - The installer jar is an ordinary repository artifact with official
//!   SHA-1 sidecars; it embeds `install_profile.json` (identity, processor
//!   definitions, installer libraries) and `version.json` (the launch
//!   profile: main class, arguments, libraries — each with SHA-1 and size).
//! - Every network artifact NeoForge publishes carries an official SHA-1,
//!   exactly like Mojang metadata; there are no digest-less artifacts in
//!   this pipeline.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use url::Url;

use crate::downloads::{self, DownloadError, DownloadOptions};
use crate::integrity::Sha1Digest;
use crate::minecraft::metadata::MinecraftVersionId;

/// The pinned official NeoForge repository root.
pub const OFFICIAL_NEOFORGE_MAVEN_URL: &str = "https://maven.neoforged.net/";

/// The largest metadata document this boundary will buffer.
pub const MAX_METADATA_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

/// The largest embedded installer document this boundary will parse.
pub const MAX_INSTALLER_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

/// The largest installer jar this boundary will open.
pub const MAX_INSTALLER_JAR_BYTES: u64 = 64 * 1024 * 1024;

/// Length bound for one bare NeoForge version string.
pub const MAX_NEOFORGE_VERSION_LENGTH: usize = 64;

/// The pinned official repository endpoints for NeoForge metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeMavenEndpoints {
    base_url: Url,
}

impl NeoForgeMavenEndpoints {
    pub fn official() -> Self {
        Self {
            base_url: Url::parse(OFFICIAL_NEOFORGE_MAVEN_URL)
                .expect("the official NeoForge maven URL is a valid absolute URL"),
        }
    }

    /// The explicit loopback variant used by deterministic offline tests.
    pub fn loopback_for_testing(base_url: &str) -> Self {
        let base = Url::parse(base_url).expect("test maven base URL must be valid");
        assert!(
            downloads::is_loopback_host(&base),
            "the test NeoForge maven base must be an explicit loopback host"
        );
        Self { base_url: base }
    }

    /// The release listing for `net.neoforged:neoforge` (JSON, oldest first).
    pub fn versions_api_url(&self) -> Url {
        self.join("api/maven/versions/releases/net/neoforged/neoforge")
    }

    /// The installer artifact URL for one exact version.
    pub fn installer_url(&self, version: &NeoForgeVersionId) -> Url {
        self.join(&format!(
            "releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
        ))
    }

    /// The universal artifact URL for one exact version.
    pub fn universal_url(&self, version: &NeoForgeVersionId) -> Url {
        self.join(&format!(
            "releases/net/neoforged/neoforge/{version}/neoforge-{version}-universal.jar"
        ))
    }

    fn join(&self, suffix: &str) -> Url {
        let url = self.base_url.clone();
        let base = url.as_str().trim_end_matches('/');
        let joined = format!("{base}/{suffix}");
        Url::parse(&joined).unwrap_or_else(|error| {
            panic!("a derived NeoForge maven URL must remain valid: {joined}: {error}")
        })
    }
}

/// A validated bare NeoForge version string (for example `26.2.0.88` or
/// `26.3.0.40-beta`), exactly as it appears in official repository
/// coordinates. This is the identity of the loader; it never embeds the
/// Minecraft version.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NeoForgeVersionId(String);

impl NeoForgeVersionId {
    pub fn new(version: &str) -> Result<Self, InvalidNeoForgeVersion> {
        let reason = if version.is_empty() {
            "a NeoForge version must not be empty"
        } else if version.len() > MAX_NEOFORGE_VERSION_LENGTH {
            "a NeoForge version is too long"
        } else if version.trim() != version {
            "a NeoForge version must not be padded with whitespace"
        } else if !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
        {
            "a NeoForge version may contain only ASCII letters, digits, '.', '-', '_', and '+'"
        } else {
            return Ok(Self(version.to_owned()));
        };
        Err(InvalidNeoForgeVersion {
            version: version.to_owned(),
            reason: reason.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NeoForgeVersionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One parsed entry of the official release listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeVersionEntry {
    version: String,
    minecraft_version: String,
    build: u64,
    stable: bool,
}

impl NeoForgeVersionEntry {
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    /// The publication ordering key within one Minecraft version: the
    /// trailing NeoForge build counter (repository builds increase
    /// monotonically; lexical ordering of the whole string is not
    /// authoritative).
    pub fn build(&self) -> u64 {
        self.build
    }

    /// Whether official metadata marks this a stable (non-prerelease)
    /// build: a bare version with no `-beta`/`-alpha` suffix.
    pub fn stable(&self) -> bool {
        self.stable
    }

    /// Parses one bare repository version into its identity parts.
    ///
    /// The mapping is the one the official artifacts themselves carry
    /// (verified through each installer's `install_profile.json`):
    /// three components `A.B.C` with `A` in `{20, 21}` address Minecraft
    /// `1.A.B`; four components `A.B.C.D` address Minecraft `A.B.C`, or
    /// `A.B` when `C` is zero. Anything else is not a version this
    /// launcher can attribute a Minecraft version to, and is rejected
    /// rather than guessed.
    pub fn parse(version: &str) -> Result<Self, InvalidNeoForgeVersion> {
        let (base, suffix) = match version.split_once('-') {
            Some((base, suffix)) => (base, Some(suffix)),
            None => (version, None),
        };
        let parts: Vec<&str> = base.split('.').collect();
        let minecraft_version = match parts.as_slice() {
            [major, minor, build] if matches!(*major, "20" | "21") => {
                build.parse::<u64>().map_err(|_| InvalidNeoForgeVersion {
                    version: version.to_owned(),
                    reason: "the NeoForge build component must be numeric".to_owned(),
                })?;
                format!("1.{major}.{minor}")
            }
            [major, minor, patch, _build] if major.parse::<u64>().is_ok_and(|v| v >= 22) => {
                let patch = patch.parse::<u64>().map_err(|_| InvalidNeoForgeVersion {
                    version: version.to_owned(),
                    reason: "the NeoForge patch component must be numeric".to_owned(),
                })?;
                if patch == 0 {
                    format!("{major}.{minor}")
                } else {
                    format!("{major}.{minor}.{patch}")
                }
            }
            _ => {
                return Err(InvalidNeoForgeVersion {
                    version: version.to_owned(),
                    reason: "the NeoForge version scheme is not one this launcher can ".to_owned()
                        + "attribute a Minecraft version to",
                });
            }
        };
        let stable = suffix.is_none();
        let build = parts[parts.len() - 1]
            .parse::<u64>()
            .map_err(|_| InvalidNeoForgeVersion {
                version: version.to_owned(),
                reason: "the NeoForge build component must be numeric".to_owned(),
            })?;
        Ok(Self {
            version: version.to_owned(),
            minecraft_version,
            build,
            stable,
        })
    }
}

/// A bare NeoForge version that failed validation or parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidNeoForgeVersion {
    pub version: String,
    pub reason: String,
}

impl fmt::Display for InvalidNeoForgeVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid NeoForge version '{}': {}",
            self.version, self.reason
        )
    }
}

impl std::error::Error for InvalidNeoForgeVersion {}

/// The failure vocabulary of the NeoForge metadata boundary.
#[derive(Debug)]
pub enum NeoForgeMetadataError {
    /// The metadata transport failed or returned an unusable response.
    Network(DownloadError),
    /// A non-success status from the pinned repository.
    HttpStatus { status: u16 },
    /// A document exceeded the size the launcher will buffer.
    ResponseTooLarge { limit_bytes: usize },
    /// A document could not be parsed or failed validation.
    Malformed { reason: String },
    /// A document or artifact uses semantics this launcher deliberately
    /// does not support.
    Unsupported { reason: String },
}

impl fmt::Display for NeoForgeMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(error) => {
                write!(
                    formatter,
                    "official NeoForge metadata could not be fetched: {error}"
                )
            }
            Self::HttpStatus { status } => write!(
                formatter,
                "the official NeoForge repository answered HTTP {status}"
            ),
            Self::ResponseTooLarge { limit_bytes } => write!(
                formatter,
                "official NeoForge metadata exceeded the {limit_bytes}-byte limit the launcher will buffer"
            ),
            Self::Malformed { reason } => write!(
                formatter,
                "official NeoForge metadata is unusable: {reason}"
            ),
            Self::Unsupported { reason } => write!(formatter, "{reason}"),
        }
    }
}

impl std::error::Error for NeoForgeMetadataError {}

/// The release listing response of the pinned repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeVersionList {
    entries: Vec<NeoForgeVersionEntry>,
}

impl NeoForgeVersionList {
    /// Parses the listing document. Unattributable version strings are
    /// filtered out of the listing (they are still real repository
    /// versions, but this launcher cannot honestly map them to a
    /// Minecraft version).
    pub fn from_json(text: &str) -> Result<Self, NeoForgeMetadataError> {
        #[derive(serde::Deserialize)]
        struct Document {
            #[serde(rename = "isSnapshot")]
            is_snapshot: bool,
            versions: Vec<String>,
        }
        let document: Document =
            serde_json::from_str(text).map_err(|error| NeoForgeMetadataError::Malformed {
                reason: format!("the NeoForge release listing is unusable: {error}"),
            })?;
        if document.is_snapshot {
            return Err(NeoForgeMetadataError::Malformed {
                reason: "the NeoForge release listing unexpectedly describes a snapshot "
                    .to_owned()
                    + "repository",
            });
        }
        let mut entries = Vec::with_capacity(document.versions.len());
        for version in document.versions {
            if let Ok(entry) = NeoForgeVersionEntry::parse(&version) {
                entries.push(entry);
            }
        }
        if entries.is_empty() {
            return Err(NeoForgeMetadataError::Malformed {
                reason: "the NeoForge release listing contains no attributable versions".to_owned(),
            });
        }
        Ok(Self { entries })
    }

    /// All attributable versions for one exact Minecraft version, newest
    /// first (stable builds before prereleases of the same build cannot
    /// occur; repository builds are unique).
    pub fn for_minecraft(&self, game: &MinecraftVersionId) -> Vec<NeoForgeVersionEntry> {
        let mut matching: Vec<NeoForgeVersionEntry> = self
            .entries
            .iter()
            .filter(|entry| entry.minecraft_version() == game.as_str())
            .cloned()
            .collect();
        matching.sort_by(|a, b| b.build().cmp(&a.build()));
        matching
    }

    pub fn find(&self, version: &NeoForgeVersionId) -> Option<&NeoForgeVersionEntry> {
        self.entries
            .iter()
            .find(|entry| entry.version() == version.as_str())
    }
}

/// Fetches the release listing of the pinned official repository.
pub async fn fetch_neoforge_versions(
    endpoints: &NeoForgeMavenEndpoints,
    options: &DownloadOptions,
) -> Result<NeoForgeVersionList, NeoForgeMetadataError> {
    let bytes = fetch_document(&endpoints.versions_api_url(), options).await?;
    let text = bytes_to_document_text(&bytes)?;
    NeoForgeVersionList::from_json(&text)
}

/// Fetches the official SHA-1 sidecar of one repository artifact.
///
/// Sidecars are the repository's own published digests; they turn the
/// installer and universal artifacts (which carry no inline digest in any
/// metadata document) into digest-verified acquisitions. A missing or
/// malformed sidecar is a hard failure — no digest is ever invented.
pub async fn fetch_sidecar_sha1(
    artifact_url: &Url,
    options: &DownloadOptions,
) -> Result<Sha1Digest, NeoForgeMetadataError> {
    let sidecar_url = Url::parse(&format!("{}.sha1", artifact_url.as_str())).map_err(|error| {
        NeoForgeMetadataError::Malformed {
            reason: format!("the artifact digest URL could not be derived: {error}"),
        }
    })?;
    let bytes = fetch_document(&sidecar_url, options).await?;
    if bytes.len() > 256 {
        return Err(NeoForgeMetadataError::Malformed {
            reason: "an artifact digest sidecar is unexpectedly large".to_owned(),
        });
    }
    let text = bytes_to_document_text(&bytes)?;
    let digest = text.trim();
    if digest.lines().count() > 1 {
        return Err(NeoForgeMetadataError::Malformed {
            reason: "an artifact digest sidecar must be a single line".to_owned(),
        });
    }
    Sha1Digest::parse(digest).map_err(|error| NeoForgeMetadataError::Malformed {
        reason: format!("the published artifact digest is not a valid SHA-1: {error}"),
    })
}

/// One Maven library requirement published by NeoForge metadata.
///
/// The shape mirrors Mojang's own library entries (name + downloads.artifact
/// with `path`, `url`, `sha1`, `size`), which is exactly how NeoForge
/// publishes them; digests are official SHA-1s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileLibrary {
    pub name: String,
    pub path: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
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

/// One side-specific token value of the installer's data map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDataValue {
    pub client: String,
    pub server: String,
}

/// One processor definition: a tool jar, its classpath, and its argument
/// vector, optionally restricted to installation sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProcessor {
    pub sides: Option<Vec<String>>,
    pub jar: String,
    pub classpath: Vec<String>,
    pub args: Vec<String>,
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
    pub fn from_json(text: &str) -> Result<Self, NeoForgeMetadataError> {
        if text.len() > MAX_INSTALLER_DOCUMENT_BYTES {
            return Err(NeoForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
            });
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document {
            spec: u32,
            #[serde(default)]
            profile: String,
            version: String,
            minecraft: String,
            #[serde(default)]
            json: String,
            data: BTreeMap<String, RawDataValue>,
            #[serde(default)]
            processors: Vec<RawProcessor>,
            #[serde(default)]
            libraries: Vec<RawLibrary>,
        }

        let document: Document =
            serde_json::from_str(text).map_err(|error| NeoForgeMetadataError::Malformed {
                reason: format!("the NeoForge install profile is unusable: {error}"),
            })?;
        if document.spec != 1 {
            return Err(NeoForgeMetadataError::Unsupported {
                reason: format!(
                    "the NeoForge install profile uses spec {} which this launcher does not support",
                    document.spec
                ),
            });
        }
        if document.version.is_empty() || document.minecraft.is_empty() {
            return Err(NeoForgeMetadataError::Malformed {
                reason: "the NeoForge install profile has no version identity".to_owned(),
            });
        }
        if !document.json.is_empty() && document.json != "/version.json" {
            return Err(NeoForgeMetadataError::Unsupported {
                reason: format!(
                    "the NeoForge install profile embeds its version document at '{}' instead of '/version.json'",
                    document.json
                ),
            });
        }
        let data = document
            .data
            .into_iter()
            .map(|(token, value)| {
                if token.is_empty() || token.contains(['{', '}', '[', ']']) {
                    return Err(NeoForgeMetadataError::Malformed {
                        reason: format!(
                            "the install profile defines a malformed data token '{token}'"
                        ),
                    });
                }
                if value.client.trim().is_empty() || value.server.trim().is_empty() {
                    return Err(NeoForgeMetadataError::Malformed {
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
            .map(|processor| {
                if processor.jar.trim().is_empty() || processor.args.is_empty() {
                    return Err(NeoForgeMetadataError::Malformed {
                        reason: "a NeoForge processor definition has no jar or arguments"
                            .to_owned(),
                    });
                }
                for argument in processor.args.iter().chain(processor.classpath.iter()) {
                    if argument.len() > 4096 || argument.contains('\0') {
                        return Err(NeoForgeMetadataError::Malformed {
                            reason: "a NeoForge processor argument is malformed".to_owned(),
                        });
                    }
                }
                Ok(ProfileProcessor {
                    sides: processor.sides,
                    jar: processor.jar,
                    classpath: processor.classpath,
                    args: processor.args,
                })
            })
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

impl VersionProfileDocument {
    pub fn from_json(text: &str) -> Result<Self, NeoForgeMetadataError> {
        if text.len() > MAX_INSTALLER_DOCUMENT_BYTES {
            return Err(NeoForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
            });
        }
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document {
            id: String,
            #[serde(rename = "inheritsFrom")]
            inherits_from: String,
            #[serde(rename = "mainClass")]
            main_class: String,
            arguments: RawArguments,
            #[serde(default)]
            libraries: Vec<RawLibrary>,
        }

        let document: Document =
            serde_json::from_str(text).map_err(|error| NeoForgeMetadataError::Malformed {
                reason: format!("the NeoForge version profile is unusable: {error}"),
            })?;
        if document.id.is_empty()
            || document.inherits_from.is_empty()
            || document.main_class.is_empty()
        {
            return Err(NeoForgeMetadataError::Malformed {
                reason: "the NeoForge version profile has no identity or entry point".to_owned(),
            });
        }
        if !is_plain_class_name(&document.main_class) {
            return Err(NeoForgeMetadataError::Malformed {
                reason: format!(
                    "the NeoForge version profile declares an unusual main class '{}'",
                    document.main_class
                ),
            });
        }
        for argument in document
            .arguments
            .jvm
            .iter()
            .chain(document.arguments.game.iter())
        {
            if argument.len() > 8192 || argument.contains('\0') {
                return Err(NeoForgeMetadataError::Malformed {
                    reason: "a NeoForge launch argument is malformed".to_owned(),
                });
            }
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

/// The raw serde shapes shared by both embedded installer documents.
#[derive(serde::Deserialize)]
struct RawDataValue {
    client: String,
    server: String,
}

#[derive(serde::Deserialize)]
struct RawProcessor {
    #[serde(default)]
    sides: Option<Vec<String>>,
    jar: String,
    #[serde(default)]
    classpath: Vec<String>,
    args: Vec<String>,
}

#[derive(serde::Deserialize)]
struct RawLibrary {
    name: String,
    downloads: RawDownloads,
}

#[derive(serde::Deserialize)]
struct RawDownloads {
    artifact: RawArtifact,
}

#[derive(serde::Deserialize)]
struct RawArtifact {
    path: String,
    url: String,
    sha1: String,
    size: u64,
}

#[derive(serde::Deserialize)]
struct RawArguments {
    #[serde(default)]
    jvm: Vec<String>,
    #[serde(default)]
    game: Vec<String>,
}

/// The minimal read access [`parse_profile_library`] needs, so the raw serde
/// shapes of both documents stay private while sharing one validator.
trait RawLibraryShape {
    fn name(&self) -> &str;
    fn artifact(&self) -> RawArtifactView<'_>;
}

struct RawArtifactView<'a> {
    path: &'a str,
    url: &'a str,
    sha1: &'a str,
    size: u64,
}

impl RawLibraryShape for RawLibrary {
    fn name(&self) -> &str {
        &self.name
    }
    fn artifact(&self) -> RawArtifactView<'_> {
        RawArtifactView {
            path: &self.downloads.artifact.path,
            url: &self.downloads.artifact.url,
            sha1: &self.downloads.artifact.sha1,
            size: self.downloads.artifact.size,
        }
    }
}

/// Validates one published library entry against the derivation rules this
/// launcher enforces for repository paths.
fn parse_profile_library(
    library: &impl RawLibraryShape,
) -> Result<ProfileLibrary, NeoForgeMetadataError> {
    let artifact = library.artifact();
    let name = library.name();
    if name.is_empty() {
        return Err(NeoForgeMetadataError::Malformed {
            reason: "a NeoForge library entry has no coordinate".to_owned(),
        });
    }
    let coordinate =
        NeoForgeCoordinate::parse(name).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("a NeoForge library coordinate is invalid: {error}"),
        })?;
    if artifact.path != coordinate.repository_path("jar") {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!(
                "the published path of '{}' disagrees with its coordinate-derived path",
                name
            ),
        });
    }
    Sha1Digest::parse(&artifact.sha1).map_err(|error| NeoForgeMetadataError::Malformed {
        reason: format!("a NeoForge library digest is not a valid SHA-1: {error}"),
    })?;
    if artifact.size == 0 {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!("the library '{name}' declares an empty artifact"),
        });
    }
    if !is_secure_artifact_url(&artifact.url) {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!("the library '{name}' is not served over secure HTTPS transport"),
        });
    }
    Ok(ProfileLibrary {
        name: name.to_owned(),
        path: artifact.path.to_owned(),
        url: artifact.url.to_owned(),
        sha1: artifact.sha1.to_owned(),
        size: artifact.size,
    })
}

/// A parsed Maven coordinate as NeoForge metadata spells it: three or four
/// colon-separated segments, optionally with an `@extension` suffix in
/// bracket references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeCoordinate {
    group: String,
    artifact: String,
    version: String,
    classifier: Option<String>,
}

impl NeoForgeCoordinate {
    /// Builds a coordinate from already-validated parts (plan boundaries
    /// deriving fixed identities, such as the universal artifact).
    pub fn new(group: &str, artifact: &str, version: &str, classifier: Option<&str>) -> Self {
        Self {
            group: group.to_owned(),
            artifact: artifact.to_owned(),
            version: version.to_owned(),
            classifier: classifier.map(str::to_owned),
        }
    }

    pub fn parse(coordinate: &str) -> Result<Self, InvalidNeoForgeCoordinate> {
        let segments: Vec<&str> = coordinate.split(':').collect();
        let invalid = |reason: &str| InvalidNeoForgeCoordinate {
            coordinate: coordinate.to_owned(),
            reason: reason.to_owned(),
        };
        let (group, artifact, version, classifier): (&str, &str, &str, Option<&str>) =
            match segments.as_slice() {
                [group, artifact, version] => (group, artifact, version, None),
                [group, artifact, version, classifier] => {
                    (group, artifact, version, Some(classifier))
                }
                _ => return Err(invalid("a coordinate must have three or four segments")),
            };
        for segment in [group, artifact, version] {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(invalid("a coordinate segment is empty or traversal-like"));
            }
            if !segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
            }) {
                return Err(invalid(
                    "a coordinate segment may contain only ASCII letters, digits, '.', '-', '_', and '+'",
                ));
            }
        }
        if let Some(classifier) = classifier {
            if classifier.is_empty()
                || !classifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+'))
            {
                return Err(invalid("a coordinate classifier is malformed"));
            }
        }
        Ok(Self {
            group: group.to_owned(),
            artifact: artifact.to_owned(),
            version: version.to_owned(),
            classifier: classifier.map(str::to_owned),
        })
    }

    /// Parses the bracket-reference form `[group:artifact:version[:classifier][@extension]]`
    /// used inside processor arguments and data-token values.
    pub fn parse_bracket_reference(
        reference: &str,
    ) -> Result<(Self, String), InvalidNeoForgeCoordinate> {
        let Some(inner) = reference
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            return Err(InvalidNeoForgeCoordinate {
                coordinate: reference.to_owned(),
                reason: "a bracket artifact reference must be enclosed in '[' ']'".to_owned(),
            });
        };
        let (coordinate, extension) = match inner.rsplit_once('@') {
            Some((coordinate, extension)) => (coordinate, extension.to_owned()),
            None => (inner, "jar".to_owned()),
        };
        if extension.is_empty()
            || !extension
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(InvalidNeoForgeCoordinate {
                coordinate: reference.to_owned(),
                reason: "a bracket artifact reference has a malformed extension".to_owned(),
            });
        }
        Ok((Self::parse(coordinate)?, extension))
    }

    /// The repository-relative Maven layout path with the given file
    /// extension.
    pub fn repository_path(&self, extension: &str) -> String {
        let file = match &self.classifier {
            Some(classifier) => format!(
                "{}-{}-{}.{}",
                self.artifact, self.version, classifier, extension
            ),
            None => format!("{}-{}.{}", self.artifact, self.version, extension),
        };
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            file
        )
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn classifier(&self) -> Option<&str> {
        self.classifier.as_deref()
    }

    pub fn as_maven_string(&self) -> String {
        match &self.classifier {
            Some(classifier) => format!(
                "{}:{}:{}:{}",
                self.group, self.artifact, self.version, classifier
            ),
            None => format!("{}:{}:{}", self.group, self.artifact, self.version),
        }
    }
}

/// A Maven coordinate that failed NeoForge validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidNeoForgeCoordinate {
    pub coordinate: String,
    pub reason: String,
}

impl fmt::Display for InvalidNeoForgeCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid NeoForge coordinate '{}': {}",
            self.coordinate, self.reason
        )
    }
}

impl std::error::Error for InvalidNeoForgeCoordinate {}

/// A Java binary class name in the canonical dotted form.
fn is_plain_class_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'$'))
        && !value.starts_with('.')
        && !value.ends_with('.')
}

/// HTTPS-only artifact URLs, with loopback HTTP allowed for tests.
fn is_secure_artifact_url(url: &str) -> bool {
    Url::parse(url)
        .map(|parsed| parsed.scheme() == "https" || downloads::is_loopback_host(&parsed))
        .unwrap_or(false)
}

/// Reads the two embedded installer documents from a verified installer jar.
///
/// The jar is a verified cache object by the time this runs; extraction is
/// bounded to the two named entries and never writes to disk.
pub fn read_installer_documents(
    installer_path: &Path,
) -> Result<(InstallProfileDocument, VersionProfileDocument), NeoForgeMetadataError> {
    let file =
        std::fs::File::open(installer_path).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the verified NeoForge installer jar could not be opened: {error}"),
        })?;
    let metadata = file
        .metadata()
        .map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the verified NeoForge installer jar could not be measured: {error}"),
        })?;
    if metadata.len() > MAX_INSTALLER_JAR_BYTES {
        return Err(NeoForgeMetadataError::Malformed {
            reason: "the NeoForge installer jar is unexpectedly large".to_owned(),
        });
    }
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the NeoForge installer jar is not a readable archive: {error}"),
        })?;
    let profile = read_bounded_entry(&mut archive, "install_profile.json")?;
    let version = read_bounded_entry(&mut archive, "version.json")?;
    let profile_text = bytes_to_document_text(&profile)?;
    let version_text = bytes_to_document_text(&version)?;
    Ok((
        InstallProfileDocument::from_json(&profile_text)?,
        VersionProfileDocument::from_json(&version_text)?,
    ))
}

/// Extracts one bounded entry of an installer jar into memory.
fn read_bounded_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<Vec<u8>, NeoForgeMetadataError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| NeoForgeMetadataError::Malformed {
            reason: format!("the NeoForge installer jar has no '{name}' entry"),
        })?;
    if entry.size() > MAX_INSTALLER_DOCUMENT_BYTES as u64 {
        return Err(NeoForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    std::io::Read::read_to_end(&mut entry, &mut bytes).map_err(|error| {
        NeoForgeMetadataError::Malformed {
            reason: format!("the '{name}' entry of the installer jar is unreadable: {error}"),
        }
    })?;
    if bytes.len() > MAX_INSTALLER_DOCUMENT_BYTES {
        return Err(NeoForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_INSTALLER_DOCUMENT_BYTES,
        });
    }
    Ok(bytes)
}

/// Extracts one bounded installer-embedded data file (for example
/// `data/client.lzma`) to a destination path.
pub fn extract_installer_entry(
    installer_path: &Path,
    entry_name: &str,
    destination: &Path,
) -> Result<u64, NeoForgeMetadataError> {
    // The entry name must be a strictly relative forward-slash path inside
    // the archive; this is the installer's own embedded-data convention.
    if entry_name.is_empty()
        || entry_name.contains('\\')
        || entry_name.contains("..")
        || entry_name.starts_with('/')
        || entry_name.contains(':')
        || entry_name
            .bytes()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'/' | b'.' | b'-' | b'_'))
    {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!("the installer entry name '{entry_name}' is not a safe relative path"),
        });
    }
    let file =
        std::fs::File::open(installer_path).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the verified NeoForge installer jar could not be opened: {error}"),
        })?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the NeoForge installer jar is not a readable archive: {error}"),
        })?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| NeoForgeMetadataError::Malformed {
            reason: format!("the NeoForge installer jar has no '{entry_name}' entry"),
        })?;
    if entry.is_dir() {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!("the installer entry '{entry_name}' is a directory"),
        });
    }
    if entry.size() > 256 * 1024 * 1024 {
        return Err(NeoForgeMetadataError::Malformed {
            reason: format!("the installer entry '{entry_name}' is unexpectedly large"),
        });
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the installer entry destination could not be prepared: {error}"),
        })?;
    }
    let mut output =
        std::fs::File::create(destination).map_err(|error| NeoForgeMetadataError::Malformed {
            reason: format!("the installer entry destination could not be created: {error}"),
        })?;
    std::io::copy(&mut entry, &mut output).map_err(|error| NeoForgeMetadataError::Malformed {
        reason: format!("the installer entry '{entry_name}' could not be extracted: {error}"),
    })
}

/// Streams one NeoForge metadata document into memory under a hard size
/// cap, mirroring the Fabric and Mojang metadata fetch boundaries.
async fn fetch_document(
    url: &Url,
    options: &DownloadOptions,
) -> Result<Vec<u8>, NeoForgeMetadataError> {
    let client = downloads::build_client(options);
    let response =
        client.get(url.clone()).send().await.map_err(|error| {
            NeoForgeMetadataError::Network(DownloadError::from_transport(error))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(NeoForgeMetadataError::HttpStatus {
            status: status.as_u16(),
        });
    }

    if response
        .content_length()
        .is_some_and(|declared| declared as usize > MAX_METADATA_DOCUMENT_BYTES)
    {
        return Err(NeoForgeMetadataError::ResponseTooLarge {
            limit_bytes: MAX_METADATA_DOCUMENT_BYTES,
        });
    }

    let mut document = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| NeoForgeMetadataError::Network(DownloadError::from_transport(error)))?
    {
        if document.len() + chunk.len() > MAX_METADATA_DOCUMENT_BYTES {
            return Err(NeoForgeMetadataError::ResponseTooLarge {
                limit_bytes: MAX_METADATA_DOCUMENT_BYTES,
            });
        }
        document.extend_from_slice(&chunk);
    }

    Ok(document)
}

fn bytes_to_document_text(bytes: &[u8]) -> Result<String, NeoForgeMetadataError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| NeoForgeMetadataError::Malformed {
        reason: "a NeoForge metadata document is not valid UTF-8".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_scheme_maps_to_minecraft_versions_verbatim() {
        let entry = NeoForgeVersionEntry::parse("21.1.252").unwrap();
        assert_eq!(entry.minecraft_version(), "1.21.1");
        assert_eq!(entry.build(), 252);
        assert!(entry.stable());
        let entry = NeoForgeVersionEntry::parse("26.2.0.88").unwrap();
        assert_eq!(entry.minecraft_version(), "26.2");
        assert_eq!(entry.build(), 88);
        let entry = NeoForgeVersionEntry::parse("26.1.2.112").unwrap();
        assert_eq!(entry.minecraft_version(), "26.1.2");
        let entry = NeoForgeVersionEntry::parse("26.3.0.40-beta").unwrap();
        assert_eq!(entry.minecraft_version(), "26.3");
        assert!(!entry.stable());
    }

    #[test]
    fn unattributable_versions_are_rejected_not_guessed() {
        for version in [
            "0.25w14craftmine.3-beta",
            "banana",
            "26.2.0",
            "21.1.252.7",
            "1.21.1-21.1.252",
        ] {
            assert!(NeoForgeVersionEntry::parse(version).is_err(), "{version}");
        }
    }

    #[test]
    fn a_release_listing_parses_and_filters_newest_first() {
        let list = NeoForgeVersionList::from_json(
            r#"{"isSnapshot":false,"versions":["0.25w14craftmine.3-beta","20.2.93","21.1.252","26.3.0.40-beta","26.2.0.88","26.2.0.80-beta","26.1.2.112"]}"#,
        )
        .unwrap();
        let game = MinecraftVersionId::new("26.2").unwrap();
        let matching = list.for_minecraft(&game);
        let versions: Vec<&str> = matching.iter().map(|e| e.version()).collect();
        assert_eq!(versions, vec!["26.2.0.88", "26.2.0.80-beta"]);
        assert!(
            list.find(&NeoForgeVersionId::new("21.1.252").unwrap())
                .is_some()
        );
        assert!(
            list.find(&NeoForgeVersionId::new("0.25w14craftmine.3-beta").unwrap())
                .is_none()
        );
    }

    #[test]
    fn malformed_listings_are_rejected() {
        assert!(NeoForgeVersionList::from_json("not json").is_err());
        assert!(
            NeoForgeVersionList::from_json(r#"{"isSnapshot":true,"versions":["26.2.0.88"]}"#)
                .is_err()
        );
        assert!(NeoForgeVersionList::from_json(r#"{"isSnapshot":false,"versions":[]}"#).is_err());
        assert!(NeoForgeVersionList::from_json(r#"{"isSnapshot":false}"#).is_err());
    }

    #[test]
    fn coordinates_validate_and_derive_repository_paths() {
        let coordinate =
            NeoForgeCoordinate::parse("net.neoforged.fancymodloader:loader:11.0.16").unwrap();
        assert_eq!(
            coordinate.repository_path("jar"),
            "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar"
        );
        let coordinate =
            NeoForgeCoordinate::parse("net.neoforged.installertools:installertools:4.0.17:fatjar")
                .unwrap();
        assert_eq!(
            coordinate.repository_path("jar"),
            "net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar"
        );
        let (coordinate, extension) = NeoForgeCoordinate::parse_bracket_reference(
            "[net.neoforged:neoform:1.21.1-20240808.144430:mappings@txt]",
        )
        .unwrap();
        assert_eq!(extension, "txt");
        assert_eq!(coordinate.classifier(), Some("mappings"));
        assert_eq!(
            coordinate.repository_path(&extension),
            "net/neoforged/neoform/1.21.1-20240808.144430/neoform-1.21.1-20240808.144430-mappings.txt"
        );
        let (_, extension) =
            NeoForgeCoordinate::parse_bracket_reference("[net.neoforged:neoforge:26.2.0.88]")
                .unwrap();
        assert_eq!(extension, "jar");
    }

    #[test]
    fn traversal_like_coordinates_are_rejected() {
        for coordinate in [
            "net.neoforged:..:26.2.0.88",
            "../evil:loader:1",
            "net.neoforged:loader:",
            "a:b",
            "a:b:c:d:e",
            "net/neoforged:loader:1",
            "net.neoforged:loader:1:natives-windows:extra",
        ] {
            assert!(
                NeoForgeCoordinate::parse(coordinate).is_err(),
                "{coordinate}"
            );
        }
        assert!(NeoForgeCoordinate::parse_bracket_reference("net.neoforged:loader:1").is_err());
    }

    #[test]
    fn a_representative_install_profile_parses() {
        let text = r#"{
          "spec": 1,
          "profile": "NeoForge",
          "version": "neoforge-26.2.0.88",
          "minecraft": "26.2",
          "json": "/version.json",
          "data": {
            "BINPATCH": {"client": "/data/client.lzma", "server": "/data/client.lzma"},
            "PATCHED": {"client": "[net.neoforged:minecraft-client-patched:26.2.0.88]", "server": "[net.neoforged:minecraft-server-patched:26.2.0.88]"},
            "MCP_VERSION": {"client": "'26.2-2'", "server": "'26.2-2'"}
          },
          "processors": [
            {"sides": ["server"], "jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--task", "EXTRACT_FILES"]},
            {"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--task", "PROCESS_MINECRAFT_JAR", "--input", "{MINECRAFT_JAR}", "--output", "{PATCHED}", "--apply-patches", "{BINPATCH}"]}
          ],
          "libraries": [
            {"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {"artifact": {"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "https://maven.neoforged.net/releases/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 669160}}}
          ]
        }"#;
        let document = InstallProfileDocument::from_json(text).unwrap();
        assert_eq!(document.minecraft_version, "26.2");
        assert_eq!(document.version_id, "neoforge-26.2.0.88");
        assert_eq!(document.libraries.len(), 1);
        assert_eq!(document.processors.len(), 2);
        assert!(!document.processors[0].applies_to_client());
        assert!(document.processors[1].applies_to_client());
        assert_eq!(
            document.data.get("PATCHED").unwrap().client,
            "[net.neoforged:minecraft-client-patched:26.2.0.88]"
        );
    }

    #[test]
    fn malformed_install_profiles_are_rejected() {
        let base = r#"{"spec":1,"version":"neoforge-26.2.0.88","minecraft":"26.2","data":{},"processors":[],"libraries":[]}"#;
        assert!(InstallProfileDocument::from_json(base).is_ok());
        assert!(InstallProfileDocument::from_json("not json").is_err());
        let future =
            r#"{"spec":2,"version":"v","minecraft":"m","data":{},"processors":[],"libraries":[]}"#;
        assert!(matches!(
            InstallProfileDocument::from_json(future).unwrap_err(),
            NeoForgeMetadataError::Unsupported { .. }
        ));
        let bad_library = r#"{"spec":1,"version":"v","minecraft":"m","data":{},"processors":[],"libraries":[{"name":"g:a:1","downloads":{"artifact":{"path":"evil.jar","url":"https://maven.neoforged.net/evil.jar","sha1":"42d3bfead0ba3aa89c7d45ac29115defc9967219","size":10}}}]}"#;
        assert!(InstallProfileDocument::from_json(bad_library).is_err());
        let http_library = r#"{"spec":1,"version":"v","minecraft":"m","data":{},"processors":[],"libraries":[{"name":"g:a:1","downloads":{"artifact":{"path":"g/a/1/a-1.jar","url":"http://maven.neoforged.net/g/a/1/a-1.jar","sha1":"42d3bfead0ba3aa89c7d45ac29115defc9967219","size":10}}}]}"#;
        assert!(InstallProfileDocument::from_json(http_library).is_err());
    }

    #[test]
    fn a_representative_version_profile_parses() {
        let text = r#"{
          "id": "neoforge-26.2.0.88",
          "inheritsFrom": "26.2",
          "mainClass": "net.neoforged.fml.startup.Client",
          "arguments": {
            "jvm": ["-DlibraryDirectory=${library_directory}", "--add-opens", "java.base/java.lang.invoke=ALL-UNNAMED"],
            "game": ["--fml.neoForgeVersion", "26.2.0.88", "--fml.mcVersion", "26.2", "--fml.neoFormVersion", "2"]
          },
          "libraries": [
            {"name": "cpw.mods:bootstraplauncher:2.0.2", "downloads": {"artifact": {"path": "cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "url": "https://maven.neoforged.net/releases/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 123456}}}
          ]
        }"#;
        let document = VersionProfileDocument::from_json(text).unwrap();
        assert_eq!(document.inherits_from, "26.2");
        assert_eq!(document.main_class, "net.neoforged.fml.startup.Client");
        assert_eq!(document.game_arguments.len(), 6);
        assert_eq!(document.libraries.len(), 1);
    }

    #[test]
    fn endpoints_pin_official_paths() {
        let endpoints = NeoForgeMavenEndpoints::official();
        assert_eq!(
            endpoints.versions_api_url().as_str(),
            "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge"
        );
        let version = NeoForgeVersionId::new("26.2.0.88").unwrap();
        assert_eq!(
            endpoints.installer_url(&version).as_str(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-installer.jar"
        );
        assert_eq!(
            endpoints.universal_url(&version).as_str(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-universal.jar"
        );
    }
}
