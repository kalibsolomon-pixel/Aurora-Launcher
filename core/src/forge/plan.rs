//! The normalized Forge plan and the Forge family composer.
//!
//! This module converts a verified installer's embedded
//! `install_profile.json` and `version.json` into Aurora's own Forge
//! representation (`ForgePlan`), then composes that plan with a vanilla
//! `MinecraftInstallPlan` through the shared `game::plan` boundary.
//!
//! Everything here is planning: no library is downloaded, nothing is
//! installed or executed. The installer consumes the composed plan and
//! never re-parses Forge installer JSON.
//!
//! Forge-specific facts the plan owns (and NeoForge's plan deliberately
//! does not):
//!
//! - launch libraries come from `version.json` and include one artifact
//!   with no upstream URL — the processor-generated patched client,
//!   pinned only by its published SHA-1;
//! - the processor chain's data values are bracketed Maven references
//!   (with `@ext` extensions), quoted literals, or installer-embedded
//!   entry paths, and the published output expectations resolve through
//!   the data map;
//! - Forge's own JVM/game argument contributions (`-Djava.net…`,
//!   `--launchTarget forge_client`) ride the composed plan's effective
//!   argument groups;
//! - Forge publishes no Java floor beyond Minecraft's own requirement.

use std::collections::BTreeMap;
use std::fmt;

use url::Url;

use crate::forge::metadata::{
    ForgeMavenEndpoints, ForgeMetadataError, ForgeVersionId, InstallProfileDocument,
    ProfileLibrary, VersionProfileDocument,
};
use crate::game::plan::{
    CompositionError, GameInstallPlan, GameJavaRequirement, GameLibrary, LoaderPlan,
    installer_library_satisfied_by, merge_library_sets,
};
use crate::integrity::Sha1Digest;
use crate::minecraft::metadata::MinecraftVersionId;
use crate::minecraft::plan::MinecraftInstallPlan;

/// One Forge-published artifact requirement.
///
/// `url` is `None` for the one artifact that is never downloaded — the
/// processor-generated patched client — whose only upstream expectation
/// is its published SHA-1. Every other artifact carries the repository
/// URL it is acquired from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeArtifact {
    url: Option<Url>,
    sha1: Sha1Digest,
    size_bytes: Option<u64>,
}

impl ForgeArtifact {
    pub fn url(&self) -> Option<&Url> {
        self.url.as_ref()
    }

    pub fn sha1(&self) -> &Sha1Digest {
        &self.sha1
    }

    pub fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    /// Whether this artifact is downloadable (the generated patched
    /// client is not).
    pub fn is_downloadable(&self) -> bool {
        self.url.is_some()
    }
}

/// One Forge library the installer must acquire (or, for the generated
/// patched client, the processor chain must produce).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeLibrary {
    coordinate: ForgeCoordinate,
    path: String,
    artifact: ForgeArtifact,
    classpath_entry: bool,
}

impl ForgeLibrary {
    pub fn coordinate(&self) -> &ForgeCoordinate {
        &self.coordinate
    }

    /// The repository-relative Maven layout path of the artifact.
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn artifact(&self) -> &ForgeArtifact {
        &self.artifact
    }

    pub fn is_classpath_entry(&self) -> bool {
        self.classpath_entry
    }
}

/// A validated Forge Maven coordinate: `group:artifact:version` with an
/// optional `:classifier` and optional `@extension` suffix (official
/// Forge metadata uses both, for example `…:mcp_config:1.21.11-…@zip`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForgeCoordinate {
    group: String,
    artifact: String,
    version: String,
    classifier: Option<String>,
    extension: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidForgeCoordinate {
    pub coordinate: String,
    pub reason: String,
}

impl fmt::Display for InvalidForgeCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Forge coordinate '{}': {}",
            self.coordinate, self.reason
        )
    }
}

impl std::error::Error for InvalidForgeCoordinate {}

impl ForgeCoordinate {
    /// Parses a `group:artifact:version[:classifier][@extension]`
    /// coordinate with a conservative charset.
    pub fn parse(value: &str) -> Result<Self, InvalidForgeCoordinate> {
        let (base, extension) = match value.split_once('@') {
            Some((base, extension)) => (base, extension.to_owned()),
            None => (value, "jar".to_owned()),
        };
        let segments: Vec<&str> = base.split(':').collect();
        if !(3..=4).contains(&segments.len()) {
            return Err(InvalidForgeCoordinate {
                coordinate: value.to_owned(),
                reason: "expected group:artifact:version[:classifier][@extension]".to_owned(),
            });
        }
        for segment in &segments {
            if segment.is_empty() {
                return Err(InvalidForgeCoordinate {
                    coordinate: value.to_owned(),
                    reason: "coordinate segments must not be empty".to_owned(),
                });
            }
        }
        if extension.is_empty() {
            return Err(InvalidForgeCoordinate {
                coordinate: value.to_owned(),
                reason: "the extension must not be empty".to_owned(),
            });
        }
        for part in segments.iter().copied().chain([extension.as_str()]) {
            if part.len() > 128
                || !part.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+')
                })
            {
                return Err(InvalidForgeCoordinate {
                    coordinate: value.to_owned(),
                    reason: "a segment uses unusable characters".to_owned(),
                });
            }
        }
        Ok(Self {
            group: segments[0].to_owned(),
            artifact: segments[1].to_owned(),
            version: segments[2].to_owned(),
            classifier: segments.get(3).map(|segment| (*segment).to_owned()),
            extension,
        })
    }

    /// Parses the inside of a bracketed data reference
    /// (`[group:artifact:version[:classifier][@extension]]`).
    pub fn parse_bracket_reference(inner: &str) -> Result<(Self, String), InvalidForgeCoordinate> {
        let coordinate = Self::parse(inner)?;
        let extension = coordinate.extension.clone();
        Ok((coordinate, extension))
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

    pub fn extension(&self) -> &str {
        &self.extension
    }

    pub fn as_maven_string(&self) -> String {
        let mut value = format!("{}:{}:{}", self.group, self.artifact, self.version);
        if let Some(classifier) = &self.classifier {
            value.push(':');
            value.push_str(classifier);
        }
        if self.extension != "jar" {
            value.push('@');
            value.push_str(&self.extension);
        }
        value
    }

    /// The repository-relative Maven layout path for this coordinate.
    pub fn repository_path(&self) -> String {
        let group_path = self.group.replace('.', "/");
        let file = match &self.classifier {
            Some(classifier) => {
                format!(
                    "{}-{}-{}.{}",
                    self.artifact, self.version, classifier, self.extension
                )
            }
            None => format!("{}-{}.{}", self.artifact, self.version, self.extension),
        };
        format!("{group_path}/{}/{}/{}", self.artifact, self.version, file)
    }
}

/// One side-specific data-token value of the install profile, normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeDataValue {
    /// A bracketed Maven reference resolved under the libraries root
    /// (the `@extension`, when present, rides inside the coordinate).
    Artifact { coordinate: ForgeCoordinate },
    /// An installer-embedded entry path (leading slash).
    InstallerFile(String),
    /// A single-quoted literal value (for example a published SHA-1).
    Literal(String),
}

/// One client-side processor step, normalized to Aurora's execution
/// model: an exact tool jar, its classpath, a literal argument vector,
/// and the published output expectations for this step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeProcessorStep {
    jar: ForgeCoordinate,
    classpath: Vec<ForgeCoordinate>,
    args: Vec<String>,
    arguments: Vec<ForgeProcessorArgument>,
    outputs: Vec<ForgeProcessorOutputRule>,
}

/// Explicit forms accepted by the official whole-argument substitution grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeProcessorArgument {
    Literal(String),
    Token(String),
    Artifact(ForgeCoordinate),
}

impl ForgeProcessorArgument {
    fn parse(value: &str) -> Result<Self, ForgePlanError> {
        if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            return Ok(Self::Artifact(
                ForgeCoordinate::parse(inner).map_err(ForgePlanError::Coordinate)?,
            ));
        }
        if let Some(inner) = value.strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
            if !inner.is_empty()
                && inner
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Ok(Self::Token(inner.to_owned()));
            }
        }
        if value.contains(['{', '}', '[', ']', '\0']) {
            return Err(ForgePlanError::Data {
                token: value.to_owned(),
                reason: "unsupported processor argument expression".into(),
            });
        }
        Ok(Self::Literal(value.to_owned()))
    }
}

/// One published output expectation: the data token naming the artifact
/// the processor must produce, and its official SHA-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeProcessorOutputRule {
    token: String,
    sha1: Sha1Digest,
}

impl ForgeProcessorOutputRule {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn sha1(&self) -> &Sha1Digest {
        &self.sha1
    }
}

impl ForgeProcessorStep {
    pub fn jar(&self) -> &ForgeCoordinate {
        &self.jar
    }

    pub fn classpath(&self) -> &[ForgeCoordinate] {
        &self.classpath
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn arguments(&self) -> &[ForgeProcessorArgument] {
        &self.arguments
    }

    pub fn outputs(&self) -> &[ForgeProcessorOutputRule] {
        &self.outputs
    }
}

/// The complete, deterministic Forge half of one installation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgePlan {
    minecraft_version: String,
    forge_version: String,
    /// The installer's own version identity (`<mc>-forge-<build>`),
    /// which is also the effective launch version name.
    version_id: String,
    main_class: String,
    jvm_arguments: Vec<String>,
    game_arguments: Vec<String>,
    /// Launch libraries from the embedded `version.json`, in document
    /// order, including the generated patched client.
    libraries: Vec<ForgeLibrary>,
    /// Installer-tool and data libraries from `install_profile.json`,
    /// minus any coordinate already satisfied by the launch libraries.
    installer_libraries: Vec<ForgeLibrary>,
    /// The verified installer artifact the plan was derived from.
    installer: ForgeArtifact,
    /// The client-side processor steps, in profile order.
    processors: Vec<ForgeProcessorStep>,
    data: BTreeMap<String, ForgeDataValue>,
}

impl ForgePlan {
    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    pub fn forge_version(&self) -> &str {
        &self.forge_version
    }

    pub fn version_id(&self) -> &str {
        &self.version_id
    }

    pub fn main_class(&self) -> &str {
        &self.main_class
    }

    pub fn jvm_arguments(&self) -> &[String] {
        &self.jvm_arguments
    }

    pub fn game_arguments(&self) -> &[String] {
        &self.game_arguments
    }

    pub fn libraries(&self) -> &[ForgeLibrary] {
        &self.libraries
    }

    pub fn installer_libraries(&self) -> &[ForgeLibrary] {
        &self.installer_libraries
    }

    pub fn installer(&self) -> &ForgeArtifact {
        &self.installer
    }

    pub fn processors(&self) -> &[ForgeProcessorStep] {
        &self.processors
    }

    pub fn data(&self) -> &BTreeMap<String, ForgeDataValue> {
        &self.data
    }

    /// Whether this plan requires the managed Java runtime at
    /// installation time (it does exactly when processors must run).
    pub fn requires_install_time_java(&self) -> bool {
        !self.processors.is_empty()
    }

    /// Derives the normalized plan from the two verified installer
    /// documents. Identity is exact: both documents must address the
    /// requested Minecraft version and Forge build, and the profile's
    /// server-only processor steps are dropped by side selection.
    #[allow(clippy::too_many_arguments)]
    pub fn from_documents(
        profile: &InstallProfileDocument,
        version: &VersionProfileDocument,
        game: &MinecraftVersionId,
        loader: &ForgeVersionId,
        endpoints: &ForgeMavenEndpoints,
        installer_sha1: Sha1Digest,
    ) -> Result<Self, ForgePlanError> {
        loader.require_reviewed()?;
        if !matches!(
            version.main_class.as_str(),
            "net.minecraftforge.bootstrap.ForgeBootstrap"
                | "cpw.mods.bootstraplauncher.BootstrapLauncher"
        ) || (version.main_class == "cpw.mods.bootstraplauncher.BootstrapLauncher"
            && loader.as_str() != "1.20.1-47.4.10")
        {
            return Err(ForgePlanError::Data {
                token: "mainClass".into(),
                reason: "unsupported Forge installer launch generation".into(),
            });
        }
        if profile.minecraft_version != game.as_str() {
            return Err(ForgePlanError::Identity {
                what: "the install profile addresses a different Minecraft version",
                expected: game.as_str().to_owned(),
                found: profile.minecraft_version.clone(),
            });
        }
        if version.inherits_from != game.as_str() {
            return Err(ForgePlanError::Identity {
                what: "the version profile inherits from a different Minecraft version",
                expected: game.as_str().to_owned(),
                found: version.inherits_from.clone(),
            });
        }
        let expected_version_id =
            format!("{}-forge-{}", loader.minecraft_version(), loader.build());
        if profile.version_id != expected_version_id || version.id != expected_version_id {
            return Err(ForgePlanError::Identity {
                what: "the installer documents do not identify the requested Forge version",
                expected: expected_version_id,
                found: format!(
                    "profile '{}' / version '{}'",
                    profile.version_id, version.id
                ),
            });
        }

        let libraries = version
            .libraries
            .iter()
            .map(library_from_profile)
            .collect::<Result<Vec<_>, _>>()?;
        let mut installer_libraries = Vec::new();
        for entry in &profile.libraries {
            let library = library_from_profile(entry)?;
            if libraries
                .iter()
                .any(|existing| existing.coordinate == library.coordinate)
            {
                continue;
            }
            installer_libraries.push(library);
        }

        let processors = profile
            .processors
            .iter()
            .filter(|processor| processor.applies_to_client())
            .map(|processor| {
                let jar = ForgeCoordinate::parse(&processor.jar).map_err(ForgePlanError::Coordinate)?;
                let mut classpath = Vec::with_capacity(processor.classpath.len());
                for entry in &processor.classpath {
                    classpath.push(
                        ForgeCoordinate::parse(entry).map_err(ForgePlanError::Coordinate)?,
                    );
                }
                let mut outputs = Vec::with_capacity(processor.outputs.len());
                for output in &processor.outputs {
                    if !profile.data.contains_key(&output.token) {
                        return Err(ForgePlanError::Output {
                            token: output.token.clone(),
                            reason:
                                "the published output expectation names a token the profile does not define"
                                    .to_owned(),
                        });
                    }
                    outputs.push(ForgeProcessorOutputRule {
                        token: output.token.clone(),
                        sha1: output.sha1,
                    });
                }
                let arguments = processor.args.iter().map(|arg| ForgeProcessorArgument::parse(arg)).collect::<Result<Vec<_>, _>>()?;
                // Some 1.20.1 steps omit an outputs map but still publish the
                // output SHA-1 in data. Preserve it rather than downgrading it.
                for pair in processor.args.windows(2) {
                    if matches!(pair[0].as_str(), "--output" | "--slim" | "--extra") {
                        if let Some(token) = pair[1].strip_prefix('{').and_then(|v| v.strip_suffix('}')) {
                            if !outputs.iter().any(|rule| rule.token == token) {
                                if let Some(value) = profile.data.get(&format!("{token}_SHA")) {
                                    let digest = value.client.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')).ok_or_else(|| ForgePlanError::Output { token: token.into(), reason: "expected a quoted published SHA-1".into() })?;
                                    outputs.push(ForgeProcessorOutputRule { token: token.into(), sha1: Sha1Digest::parse(digest).map_err(|e| ForgePlanError::Output { token: token.into(), reason: e.to_string() })? });
                                }
                            }
                        }
                    }
                }
                Ok(ForgeProcessorStep {
                    jar,
                    classpath,
                    args: processor.args.clone(),
                    arguments,
                    outputs,
                })
            })
            .collect::<Result<Vec<_>, ForgePlanError>>()?;

        let data = profile
            .data
            .iter()
            .map(|(token, value)| parse_data_value(token, value))
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        Ok(Self {
            minecraft_version: game.as_str().to_owned(),
            forge_version: loader.to_string(),
            version_id: expected_version_id,
            main_class: version.main_class.clone(),
            jvm_arguments: version.jvm_arguments.clone(),
            game_arguments: version.game_arguments.clone(),
            libraries,
            installer_libraries,
            installer: ForgeArtifact {
                url: Some(endpoints.installer_url(loader)),
                sha1: installer_sha1,
                size_bytes: None,
            },
            processors,
            data,
        })
    }
}

/// One data-token value of the install profile, normalized to its kind.
fn parse_data_value(
    token: &str,
    value: &crate::forge::metadata::ProfileDataValue,
) -> Result<(String, ForgeDataValue), ForgePlanError> {
    let client = value.client.trim();
    let normalized = if let Some(inner) = client.strip_prefix('[') {
        let inner = inner
            .strip_suffix(']')
            .ok_or_else(|| ForgePlanError::Data {
                token: token.to_owned(),
                reason: format!("'{client}' is an unterminated bracket reference"),
            })?;
        let (coordinate, _extension) =
            ForgeCoordinate::parse_bracket_reference(inner).map_err(ForgePlanError::Coordinate)?;
        ForgeDataValue::Artifact { coordinate }
    } else if let Some(path) = client.strip_prefix('/') {
        if path.is_empty()
            || path.contains('\\')
            || path.contains("..")
            || path.contains(':')
            || path.starts_with('/')
        {
            return Err(ForgePlanError::Data {
                token: token.to_owned(),
                reason: format!("'{client}' is not a safe installer-embedded path"),
            });
        }
        ForgeDataValue::InstallerFile(format!("/{path}"))
    } else if let Some(literal) = client.strip_prefix('\'') {
        let literal = literal
            .strip_suffix('\'')
            .ok_or_else(|| ForgePlanError::Data {
                token: token.to_owned(),
                reason: format!("'{client}' is an unterminated literal"),
            })?;
        if literal.is_empty() || literal.len() > 256 || literal.contains(['\'', '{', '}', '[', ']'])
        {
            return Err(ForgePlanError::Data {
                token: token.to_owned(),
                reason: "the literal value is malformed".to_owned(),
            });
        }
        ForgeDataValue::Literal(literal.to_owned())
    } else {
        return Err(ForgePlanError::Data {
            token: token.to_owned(),
            reason: format!(
                "'{client}' uses a data-value form this launcher does not support \
                 (expected a bracketed reference, an installer entry path, or a quoted literal)"
            ),
        });
    };
    Ok((token.to_owned(), normalized))
}

fn library_from_profile(entry: &ProfileLibrary) -> Result<ForgeLibrary, ForgePlanError> {
    let coordinate = ForgeCoordinate::parse(&entry.name).map_err(ForgePlanError::Coordinate)?;
    // The declared repository path must equal the coordinate-derived Maven
    // layout, so no library can smuggle a path that disagrees with its
    // identity.
    if entry.path != coordinate.repository_path() {
        return Err(ForgePlanError::Coordinate(InvalidForgeCoordinate {
            coordinate: entry.name.clone(),
            reason: format!(
                "the declared path '{}' disagrees with the coordinate layout '{}'",
                entry.path,
                coordinate.repository_path()
            ),
        }));
    }
    let url = if entry.url.is_empty() {
        None
    } else {
        Some(Url::parse(&entry.url).map_err(|_| {
            ForgePlanError::Coordinate(InvalidForgeCoordinate {
                coordinate: entry.name.clone(),
                reason: "the library URL is not a valid absolute URL".to_owned(),
            })
        })?)
    };
    let sha1 = Sha1Digest::parse(&entry.sha1).map_err(|_| {
        ForgePlanError::Coordinate(InvalidForgeCoordinate {
            coordinate: entry.name.clone(),
            reason: "the library digest is not a valid SHA-1".to_owned(),
        })
    })?;
    if entry.size == 0 {
        return Err(ForgePlanError::Coordinate(InvalidForgeCoordinate {
            coordinate: entry.name.clone(),
            reason: "the declared size must be greater than zero".to_owned(),
        }));
    }
    Ok(ForgeLibrary {
        coordinate,
        path: entry.path.clone(),
        artifact: ForgeArtifact {
            url,
            sha1,
            size_bytes: Some(entry.size),
        },
        classpath_entry: true,
    })
}

/// Composes a vanilla Minecraft plan and a Forge plan into the complete
/// game install plan through the shared boundary.
///
/// This is the Forge family composer: it owns Forge's genuinely specific
/// derivation (no loader Java floor — Minecraft's requirement stays
/// authoritative; the Forge entry point wins) and delegates the shared
/// merge and plan assembly to `game::plan`. The inputs are not mutated.
pub fn compose_forge_game_plan(
    minecraft: MinecraftInstallPlan,
    mut forge: ForgePlan,
) -> Result<GameInstallPlan, CompositionError> {
    if minecraft.minecraft_version() != forge.minecraft_version() {
        return Err(CompositionError::VersionMismatch {
            minecraft_version: minecraft.minecraft_version().to_owned(),
            loader_version: forge.forge_version.clone(),
            loader_family: "Forge",
        });
    }

    // Unsanitized DOWNLOAD_MOJMAPS copies Mojang's raw file. The older
    // official tool does not check it; bind the step to Mojang's published
    // expectation before any dependent processor can consume the result.
    for step in &mut forge.processors {
        if step.args.iter().any(|arg| arg == "DOWNLOAD_MOJMAPS")
            && !step.args.iter().any(|arg| arg == "--sanitize")
        {
            let mappings =
                minecraft
                    .client_mappings()
                    .ok_or_else(|| CompositionError::LoaderRequirement {
                        loader_family: "Forge",
                        reason: "unsanitized mappings require Mojang's published digest".into(),
                    })?;
            step.outputs.push(ForgeProcessorOutputRule {
                token: "MOJMAPS".into(),
                sha1: *mappings.sha1(),
            });
        }
    }

    if forge.main_class == "cpw.mods.bootstraplauncher.BootstrapLauncher" {
        // Aurora's fixed vanilla filename differs from the official launcher's
        // <version>.jar. Keep that jar last on the classpath, but exclude it
        // from SecureJarHandler's module discovery just like upstream does.
        for argument in &mut forge.jvm_arguments {
            if argument.starts_with("-DignoreList=") {
                argument.push_str(",client.jar");
            }
        }
    }

    let loader_libraries: Vec<GameLibrary> = forge
        .libraries
        .iter()
        .cloned()
        .map(GameLibrary::Forge)
        .collect();
    let libraries = merge_library_sets(&minecraft, loader_libraries)?;

    // The installer-tool set overlaps the composed client set (1.20.4's
    // gson 2.10.1, 1.20.1's jopt-simple 5.0.4 — byte-identical official
    // publications). One managed destination carries exactly one verified
    // artifact: drop installer entries the composed set already satisfies,
    // refuse entries that claim the same destination with different bytes,
    // and keep genuinely distinct versions at their own destinations.
    let mut retained_installer = Vec::with_capacity(forge.installer_libraries.len());
    for library in std::mem::take(&mut forge.installer_libraries) {
        let satisfied = installer_library_satisfied_by(
            &libraries,
            library.path(),
            library.artifact().sha1(),
            &library.coordinate().as_maven_string(),
        )?;
        if !satisfied {
            retained_installer.push(library);
        }
    }
    forge.installer_libraries = retained_installer;

    let minecraft_java = minecraft.java();
    let java = GameJavaRequirement::without_loader_floor(
        minecraft_java.component(),
        minecraft_java.major_version(),
    );

    let main_class = forge.main_class().to_owned();
    Ok(GameInstallPlan::from_composed_parts(
        minecraft,
        LoaderPlan::Forge(forge),
        libraries,
        java,
        main_class,
    ))
}

/// A failure while normalizing installer documents into a Forge plan.
#[derive(Debug)]
pub enum ForgePlanError {
    /// The installer documents disagree with the requested identity.
    Identity {
        what: &'static str,
        expected: String,
        found: String,
    },
    /// A Maven coordinate in the profile is invalid.
    Coordinate(InvalidForgeCoordinate),
    /// A data-token value could not be normalized.
    Data { token: String, reason: String },
    /// A published processor output expectation is unresolvable.
    Output { token: String, reason: String },
}

impl fmt::Display for ForgePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity {
                what,
                expected,
                found,
            } => write!(formatter, "{what}: expected '{expected}', found '{found}'"),
            Self::Coordinate(error) => write!(formatter, "{error}"),
            Self::Data { token, reason } => {
                write!(formatter, "the data token '{token}' is unusable: {reason}")
            }
            Self::Output { token, reason } => {
                write!(
                    formatter,
                    "the processor output expectation '{token}' is unusable: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ForgePlanError {}

impl From<ForgeMetadataError> for ForgePlanError {
    fn from(error: ForgeMetadataError) -> Self {
        Self::Data {
            token: "<document>".to_owned(),
            reason: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_documents() -> (InstallProfileDocument, VersionProfileDocument) {
        let profile = InstallProfileDocument::from_json(
            r#"{
              "spec": 1, "profile": "forge", "version": "1.21.11-forge-61.2.0",
              "minecraft": "1.21.11", "json": "/version.json",
              "data": {
                "MOJMAPS": {"client": "[net.minecraft:client:1.21.11:mappings@tsrg]", "server": "[net.minecraft:server:1.21.11:mappings@tsrg]"},
                "MOJMAPS_SHA": {"client": "'f9240aaa50a06c04d8e285772ba16a340ddc5aef'", "server": "'cf70e98af2cbcce6cc03d7e6f29ee58dcd9d0e6f'"},
                "MC_OFF": {"client": "[net.minecraft:client:1.21.11:official]", "server": "[net.minecraft:server:1.21.11:official]"},
                "MC_OFF_SHA": {"client": "'30a7252f08508f83ddcafa39883fabd77596df72'", "server": "'a61ef82940c5e9ff8e3005bf3e00fc550cfda53a'"},
                "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"},
                "PATCHED": {"client": "[net.minecraftforge:forge:1.21.11-61.2.0:client]", "server": "[net.minecraftforge:forge:1.21.11-61.2.0:server]"},
                "PATCHED_SHA": {"client": "'632ace45e535291c7576764c1c8d6388b73c9a75'", "server": "'ce19cb32f24f3c39dac759da14fc84dc16adb9a9'"}
              },
              "processors": [
                {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.4.3", "classpath": [], "args": ["--task", "EXTRACT_FILES"], "outputs": {}},
                {"jar": "net.minecraftforge:installertools:1.4.3", "classpath": [], "args": ["--task", "DOWNLOAD_MOJMAPS", "--sanitize", "--version", "1.21.11", "--side", "{SIDE}", "--output", "{MOJMAPS}"], "outputs": {"{MOJMAPS}": "{MOJMAPS_SHA}"}},
                {"sides": ["client"], "jar": "net.minecraftforge:ForgeAutoRenamingTool:1.0.6", "classpath": [], "args": ["--input", "{MINECRAFT_JAR}", "--output", "{MC_OFF}", "--names", "{MOJMAPS}", "--reverse"], "outputs": {"{MC_OFF}": "{MC_OFF_SHA}"}},
                {"jar": "net.minecraftforge:binarypatcher:1.3.1", "classpath": [], "args": ["--clean", "{MC_OFF}", "--output", "{PATCHED}", "--apply", "{BINPATCH}", "--data", "--unpatched"], "outputs": {"{PATCHED}": "{PATCHED_SHA}"}}
              ],
              "libraries": [
                {"name": "net.minecraftforge:installertools:1.4.3", "downloads": {"artifact": {"path": "net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar", "sha1": "e37ea5710e86ff09c34ba1c4a8a3f03f24dba699", "size": 1234567}}},
                {"name": "net.minecraftforge:binarypatcher:1.3.1", "downloads": {"artifact": {"path": "net/minecraftforge/binarypatcher/1.3.1/binarypatcher-1.3.1.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/binarypatcher/1.3.1/binarypatcher-1.3.1.jar", "sha1": "171fd4a353689ba52a5166a2b9330f57f6a41bf5", "size": 76543}}}
              ]
            }"#,
        )
        .unwrap();
        let version = VersionProfileDocument::from_json(
            r#"{
              "id": "1.21.11-forge-61.2.0", "inheritsFrom": "1.21.11",
              "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
              "logging": {},
              "arguments": {"jvm": ["-Djava.net.preferIPv6Addresses=system"], "game": ["--launchTarget", "forge_client"]},
              "libraries": [
                {"name": "net.minecraftforge:forge:1.21.11-61.2.0:universal", "downloads": {"artifact": {"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "url": "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "sha1": "684d4cf385f66d2757c114cd513ad370f3465090", "size": 2908730}}},
                {"name": "net.minecraftforge:forge:1.21.11-61.2.0:client", "downloads": {"artifact": {"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-client.jar", "url": "", "sha1": "632ace45e535291c7576764c1c8d6388b73c9a75", "size": 32624066}}}
              ]
            }"#,
        )
        .unwrap();
        (profile, version)
    }

    fn plan() -> ForgePlan {
        let (profile, version) = fixture_documents();
        ForgePlan::from_documents(
            &profile,
            &version,
            &MinecraftVersionId::new("1.21.11").unwrap(),
            &ForgeVersionId::new("1.21.11-61.2.0").unwrap(),
            &ForgeMavenEndpoints::official(),
            Sha1Digest::parse("ca682f0d537c65e9a112e1e7ab337095d486298f").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn coordinates_parse_with_classifiers_and_extensions() {
        let plain = ForgeCoordinate::parse("net.minecraftforge:forge:1.21.11-61.2.0").unwrap();
        assert_eq!(
            plain.repository_path(),
            "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0.jar"
        );
        assert_eq!(
            plain.as_maven_string(),
            "net.minecraftforge:forge:1.21.11-61.2.0"
        );
        let classifier =
            ForgeCoordinate::parse("net.minecraftforge:forge:1.21.11-61.2.0:client").unwrap();
        assert_eq!(
            classifier.repository_path(),
            "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-client.jar"
        );
        let extension =
            ForgeCoordinate::parse("de.oceanlabs.mcp:mcp_config:1.21.11-20251223.124241@zip")
                .unwrap();
        assert_eq!(
            extension.repository_path(),
            "de/oceanlabs/mcp/mcp_config/1.21.11-20251223.124241/mcp_config-1.21.11-20251223.124241.zip"
        );
        let mappings =
            ForgeCoordinate::parse("net.minecraft:client:1.21.11:mappings@tsrg").unwrap();
        assert_eq!(
            mappings.repository_path(),
            "net/minecraft/client/1.21.11/client-1.21.11-mappings.tsrg"
        );
        assert!(ForgeCoordinate::parse("g:a").is_err());
        assert!(ForgeCoordinate::parse("g:a:v:").is_err());
        assert!(ForgeCoordinate::parse("g:a:v@").is_err());
        assert!(ForgeCoordinate::parse("g:a:v:../../evil").is_err());
    }

    #[test]
    fn the_plan_normalizes_identity_processors_and_data() {
        let plan = plan();
        assert_eq!(plan.minecraft_version(), "1.21.11");
        assert_eq!(plan.forge_version(), "1.21.11-61.2.0");
        assert_eq!(plan.version_id(), "1.21.11-forge-61.2.0");
        assert_eq!(
            plan.main_class(),
            "net.minecraftforge.bootstrap.ForgeBootstrap"
        );
        assert_eq!(
            plan.jvm_arguments(),
            &["-Djava.net.preferIPv6Addresses=system"]
        );
        assert_eq!(plan.game_arguments(), &["--launchTarget", "forge_client"]);
        // Server-only steps dropped; client chain in profile order.
        assert_eq!(plan.processors().len(), 3);
        assert_eq!(plan.processors()[0].jar().artifact(), "installertools");
        assert_eq!(plan.processors()[2].jar().artifact(), "binarypatcher");
        assert_eq!(
            plan.processors()[2].outputs()[0].sha1().as_hex(),
            "632ace45e535291c7576764c1c8d6388b73c9a75"
        );
        // Data kinds.
        assert!(matches!(
            plan.data().get("PATCHED"),
            Some(ForgeDataValue::Artifact { .. })
        ));
        assert!(matches!(
            plan.data().get("BINPATCH"),
            Some(ForgeDataValue::InstallerFile(path)) if path == "/data/client.lzma"
        ));
        assert!(matches!(
            plan.data().get("PATCHED_SHA"),
            Some(ForgeDataValue::Literal(_))
        ));
        // Launch libraries include the generated patched client.
        let patched = plan
            .libraries()
            .iter()
            .find(|library| library.coordinate().classifier() == Some("client"))
            .unwrap();
        assert!(!patched.artifact().is_downloadable());
        assert!(plan.installer_libraries().len() == 2);
        assert!(plan.requires_install_time_java());
    }

    #[test]
    fn identity_disagreements_are_rejected() {
        let (profile, version) = fixture_documents();
        let wrong_game = MinecraftVersionId::new("1.21.1").unwrap();
        let error = ForgePlan::from_documents(
            &profile,
            &version,
            &wrong_game,
            &ForgeVersionId::new("1.21.11-61.2.0").unwrap(),
            &ForgeMavenEndpoints::official(),
            Sha1Digest::parse("ca682f0d537c65e9a112e1e7ab337095d486298f").unwrap(),
        )
        .unwrap_err();
        assert!(matches!(error, ForgePlanError::Identity { .. }), "{error}");
    }

    #[test]
    fn unsupported_data_forms_are_refused() {
        let mut document = InstallProfileDocument::from_json(
            r#"{
              "spec": 1, "version": "1.21.11-forge-61.2.0", "minecraft": "1.21.11",
              "data": {"EVIL": {"client": "{ROOT}/fragment", "server": "{ROOT}/fragment"}},
              "processors": [], "libraries": []
            }"#,
        )
        .unwrap();
        let error = parse_data_value("EVIL", &document.data["EVIL"]).unwrap_err();
        assert!(matches!(error, ForgePlanError::Data { .. }), "{error}");
        // A quoted literal is fine.
        document.data.insert(
            "LIT".to_owned(),
            crate::forge::metadata::ProfileDataValue {
                client: "'value'".to_owned(),
                server: "'value'".to_owned(),
            },
        );
        assert!(matches!(
            parse_data_value("LIT", &document.data["LIT"]).unwrap(),
            (_, ForgeDataValue::Literal(value)) if value == "value"
        ));
    }

    #[test]
    fn composition_places_forge_after_mojang_and_derives_the_pin() {
        fn minecraft_plan() -> crate::minecraft::plan::MinecraftInstallPlan {
            let json = r#"{
                "id": "1.21.11",
                "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": { "component": "java-runtime-delta", "majorVersion": 21 },
                "assetIndex": {
                    "id": "31",
                    "sha1": "1832adbc4eee60faa097bb1409be305a0abbf3d2",
                    "size": 400253,
                    "totalSize": 470877946,
                    "url": "https://piston-meta.mojang.com/v1/packages/1832adbc4eee60faa097bb1409be305a0abbf3d2/31.json"
                },
                "downloads": {
                    "client": {
                        "sha1": "1832adbc4eee60faa097bb1409be305a0abbf3d2",
                        "size": 26000000,
                        "url": "https://piston-data.mojang.com/v1/objects/1832adbc4eee60faa097bb1409be305a0abbf3d2/client.jar"
                    }
                },
                "libraries": [
                    {
                        "downloads": { "artifact": {
                            "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                            "sha1": "1832adbc4eee60faa097bb1409be305a0abbf3d2",
                            "size": 1000,
                            "url": "https://libraries.minecraft.net/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                        } },
                        "name": "com.mojang:brigadier:1.0.18"
                    }
                ],
                "arguments": {
                    "game": ["--username", "${auth_player_name}"],
                    "jvm": ["-Djava.library.path=${natives_directory}"]
                }
            }"#;
            let document = crate::minecraft::metadata::VersionDocument::from_json(json).unwrap();
            crate::minecraft::plan::plan_version_document(
                &document,
                crate::minecraft::rules::PlatformProfile::new(
                    crate::minecraft::rules::OperatingSystem::Windows,
                    crate::minecraft::rules::Architecture::X86_64,
                ),
            )
            .unwrap()
        }

        let game = compose_forge_game_plan(minecraft_plan(), plan()).unwrap();
        assert!(game.forge_loader().is_some());
        assert!(
            game.platform()
                == crate::instances::platform::PlatformPin::Forge {
                    version: "1.21.11-61.2.0".to_owned()
                }
        );
        assert_eq!(
            game.main_class(),
            "net.minecraftforge.bootstrap.ForgeBootstrap"
        );
        // Mojang's Java requirement stays authoritative.
        assert_eq!(game.java().major_version(), 21);
        assert!(!game.java().raised_by_loader());
        // Effective arguments carry the Forge contributions.
        let jvm = game.effective_jvm_arguments();
        assert_eq!(
            jvm.last().map(|group| group.values.clone()),
            Some(vec!["-Djava.net.preferIPv6Addresses=system".to_owned()])
        );
        let game_arguments = game.effective_game_arguments();
        assert_eq!(
            game_arguments.last().map(|group| group.values.clone()),
            Some(vec!["--launchTarget".to_owned(), "forge_client".to_owned()])
        );
        // Composition is deterministic.
        let again = compose_forge_game_plan(minecraft_plan(), plan()).unwrap();
        assert_eq!(game, again);
    }
}
