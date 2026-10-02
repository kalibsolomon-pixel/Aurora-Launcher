//! The normalized NeoForge half of an installation plan.
//!
//! [`NeoForgePlan`] is the Aurora-owned domain view of one exact Minecraft +
//! NeoForge combination, derived exclusively from the verified installer
//! artifact's embedded `install_profile.json` and `version.json`. External
//! DTOs stay inside [`crate::neoforge::metadata`]; installers consume only
//! this normalized plan plus the composed
//! [`crate::fabric::plan::GameInstallPlan`].
//!
//! The trust model mirrors Mojang's, because that is what NeoForge
//! publishes: every network artifact (launch libraries, installer-tool
//! libraries, the installer, the universal artifact) carries an official
//! SHA-1, so acquisition is digest-verified through the SHA-1 store. The
//! one artifact with no official digest is the processor-generated patched
//! client, which the installer records honestly as a locally generated
//! observation — never as an official verification.

use std::collections::BTreeMap;
use std::fmt;

use url::Url;

use crate::fabric::plan::{GameInstallPlan, GameJavaRequirement, GameLibrary, merge_library_sets};
use crate::integrity::Sha1Digest;
use crate::minecraft::metadata::MinecraftVersionId;
use crate::minecraft::plan::MinecraftInstallPlan;
use crate::neoforge::metadata::{
    InstallProfileDocument, InvalidNeoForgeCoordinate, NeoForgeCoordinate, NeoForgeMavenEndpoints,
    NeoForgeVersionId, ProfileDataValue, ProfileLibrary, VersionProfileDocument,
};

/// One NeoForge-published artifact with its official SHA-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeArtifact {
    url: Url,
    sha1: Sha1Digest,
    size_bytes: Option<u64>,
}

impl NeoForgeArtifact {
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn sha1(&self) -> &Sha1Digest {
        &self.sha1
    }

    pub fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }
}

/// One library the installation must materialize in the managed libraries
/// tree, in the repository's Maven layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeLibrary {
    coordinate: NeoForgeCoordinate,
    path: String,
    artifact: NeoForgeArtifact,
    /// Whether the launch classpath includes this library. Every launch
    /// library is; installer-tool libraries are not.
    classpath_entry: bool,
}

impl NeoForgeLibrary {
    pub fn coordinate(&self) -> &NeoForgeCoordinate {
        &self.coordinate
    }

    /// The repository-relative Maven layout path (forward slashes).
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn artifact(&self) -> &NeoForgeArtifact {
        &self.artifact
    }

    pub fn is_classpath_entry(&self) -> bool {
        self.classpath_entry
    }
}

/// One side-specific data token of the installer profile, normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeoForgeDataValue {
    /// A quoted literal (`'26.2-2'`).
    Literal(String),
    /// A file embedded in the installer jar (`/data/client.lzma`).
    InstallerFile(String),
    /// A Maven artifact reference (`[g:a:v[:c][@ext]]`) resolved inside the
    /// libraries tree.
    Artifact {
        coordinate: NeoForgeCoordinate,
        extension: String,
    },
}

/// One client-side processor step, normalized to Aurora's execution model:
/// an exact tool jar, its classpath, and a literal argument vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgeProcessorStep {
    jar: NeoForgeCoordinate,
    classpath: Vec<NeoForgeCoordinate>,
    args: Vec<String>,
}

impl NeoForgeProcessorStep {
    pub fn jar(&self) -> &NeoForgeCoordinate {
        &self.jar
    }

    pub fn classpath(&self) -> &[NeoForgeCoordinate] {
        &self.classpath
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// The complete, deterministic NeoForge half of one installation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeoForgePlan {
    minecraft_version: String,
    neoforge_version: String,
    /// The installer's own version identity (`neoforge-<version>`), which
    /// is also the effective launch version name.
    version_id: String,
    main_class: String,
    jvm_arguments: Vec<String>,
    game_arguments: Vec<String>,
    /// Launch libraries from the embedded `version.json`, in document order.
    libraries: Vec<NeoForgeLibrary>,
    /// Installer-tool and data libraries from `install_profile.json`, minus
    /// any coordinate already satisfied by the launch libraries.
    installer_libraries: Vec<NeoForgeLibrary>,
    /// The universal artifact (`net.neoforged:neoforge:<version>:universal`),
    /// which the runtime locates through the library directory rather than
    /// the classpath.
    universal: NeoForgeLibrary,
    /// The verified installer artifact the plan was derived from.
    installer: NeoForgeArtifact,
    /// The client-side processor steps, in profile order.
    processors: Vec<NeoForgeProcessorStep>,
    data: BTreeMap<String, NeoForgeDataValue>,
}

impl NeoForgePlan {
    #[allow(clippy::too_many_arguments)]
    pub fn from_documents(
        profile: &InstallProfileDocument,
        version: &VersionProfileDocument,
        game: &MinecraftVersionId,
        loader: &NeoForgeVersionId,
        endpoints: &NeoForgeMavenEndpoints,
        installer_sha1: Sha1Digest,
        universal_sha1: Sha1Digest,
    ) -> Result<Self, NeoForgePlanError> {
        if profile.minecraft_version != game.as_str() {
            return Err(NeoForgePlanError::Identity {
                what: "the install profile addresses a different Minecraft version",
                expected: game.as_str().to_owned(),
                found: profile.minecraft_version.clone(),
            });
        }
        if version.inherits_from != game.as_str() {
            return Err(NeoForgePlanError::Identity {
                what: "the version profile inherits from a different Minecraft version",
                expected: game.as_str().to_owned(),
                found: version.inherits_from.clone(),
            });
        }
        let expected_version_id = format!("neoforge-{loader}");
        if profile.version_id != expected_version_id || version.id != expected_version_id {
            return Err(NeoForgePlanError::Identity {
                what: "the installer documents do not identify the requested NeoForge version",
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

        let universal_coordinate = NeoForgeCoordinate::new(
            "net.neoforged",
            "neoforge",
            loader.as_str(),
            Some("universal"),
        );
        let universal = NeoForgeLibrary {
            path: universal_coordinate.repository_path("jar"),
            coordinate: universal_coordinate,
            artifact: NeoForgeArtifact {
                url: endpoints.universal_url(loader),
                sha1: universal_sha1,
                size_bytes: None,
            },
            classpath_entry: false,
        };

        let processors = profile
            .processors
            .iter()
            .filter(|processor| processor.applies_to_client())
            .map(|processor| {
                let jar = NeoForgeCoordinate::parse(&processor.jar)
                    .map_err(NeoForgePlanError::Coordinate)?;
                let mut classpath = Vec::with_capacity(processor.classpath.len());
                for entry in &processor.classpath {
                    classpath.push(
                        NeoForgeCoordinate::parse(entry).map_err(NeoForgePlanError::Coordinate)?,
                    );
                }
                Ok(NeoForgeProcessorStep {
                    jar,
                    classpath,
                    args: processor.args.clone(),
                })
            })
            .collect::<Result<Vec<_>, NeoForgePlanError>>()?;

        let data = profile
            .data
            .iter()
            .map(|(token, value)| parse_data_value(token, value))
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        Ok(Self {
            minecraft_version: game.as_str().to_owned(),
            neoforge_version: loader.to_string(),
            version_id: expected_version_id,
            main_class: version.main_class.clone(),
            jvm_arguments: version.jvm_arguments.clone(),
            game_arguments: version.game_arguments.clone(),
            libraries,
            installer_libraries,
            universal,
            installer: NeoForgeArtifact {
                url: endpoints.installer_url(loader),
                sha1: installer_sha1,
                size_bytes: None,
            },
            processors,
            data,
        })
    }

    pub fn minecraft_version(&self) -> &str {
        &self.minecraft_version
    }

    pub fn neoforge_version(&self) -> &str {
        &self.neoforge_version
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

    pub fn libraries(&self) -> &[NeoForgeLibrary] {
        &self.libraries
    }

    pub fn installer_libraries(&self) -> &[NeoForgeLibrary] {
        &self.installer_libraries
    }

    pub fn universal(&self) -> &NeoForgeLibrary {
        &self.universal
    }

    pub fn installer(&self) -> &NeoForgeArtifact {
        &self.installer
    }

    pub fn processors(&self) -> &[NeoForgeProcessorStep] {
        &self.processors
    }

    pub fn data(&self) -> &BTreeMap<String, NeoForgeDataValue> {
        &self.data
    }

    /// Whether this plan requires a managed Java executable during
    /// installation (it does whenever processor steps exist).
    pub fn requires_install_time_java(&self) -> bool {
        !self.processors.is_empty()
    }
}

/// A failure while normalizing installer documents into a plan.
#[derive(Debug)]
pub enum NeoForgePlanError {
    /// The installer documents disagree with the requested identity.
    Identity {
        what: &'static str,
        expected: String,
        found: String,
    },
    /// A Maven coordinate in the profile is invalid.
    Coordinate(InvalidNeoForgeCoordinate),
    /// A data-token value could not be normalized.
    Data { token: String, reason: String },
}

impl fmt::Display for NeoForgePlanError {
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
        }
    }
}

impl std::error::Error for NeoForgePlanError {}

fn library_from_profile(entry: &ProfileLibrary) -> Result<NeoForgeLibrary, NeoForgePlanError> {
    let coordinate =
        NeoForgeCoordinate::parse(&entry.name).map_err(NeoForgePlanError::Coordinate)?;
    let artifact = NeoForgeArtifact {
        url: Url::parse(&entry.url).map_err(|_| {
            NeoForgePlanError::Coordinate(InvalidNeoForgeCoordinate {
                coordinate: entry.name.clone(),
                reason: "the library URL is not a valid absolute URL".to_owned(),
            })
        })?,
        sha1: Sha1Digest::parse(&entry.sha1).map_err(|_| {
            NeoForgePlanError::Coordinate(InvalidNeoForgeCoordinate {
                coordinate: entry.name.clone(),
                reason: "the library digest is not a valid SHA-1".to_owned(),
            })
        })?,
        size_bytes: Some(entry.size),
    };
    Ok(NeoForgeLibrary {
        coordinate,
        path: entry.path.clone(),
        artifact,
        classpath_entry: true,
    })
}

/// Normalizes one data-token value of the install profile.
fn parse_data_value(
    token: &str,
    value: &ProfileDataValue,
) -> Result<(String, NeoForgeDataValue), NeoForgePlanError> {
    let client = value.client.trim();
    let normalized = if let Some(inner) = client.strip_prefix('[') {
        let (coordinate, extension) =
            NeoForgeCoordinate::parse_bracket_reference(&format!("[{inner}"))
                .map_err(NeoForgePlanError::Coordinate)?;
        NeoForgeDataValue::Artifact {
            coordinate,
            extension,
        }
    } else if let Some(path) = client.strip_prefix('/') {
        if path.is_empty()
            || path.contains('\\')
            || path.contains("..")
            || path.contains(':')
            || path.starts_with('/')
        {
            return Err(NeoForgePlanError::Data {
                token: token.to_owned(),
                reason: format!("'{client}' is not a safe installer-embedded path"),
            });
        }
        NeoForgeDataValue::InstallerFile(format!("/{path}"))
    } else if let Some(literal) = client.strip_prefix('\'') {
        let literal = literal
            .strip_suffix('\'')
            .ok_or_else(|| NeoForgePlanError::Data {
                token: token.to_owned(),
                reason: format!("'{client}' is an unterminated literal"),
            })?;
        NeoForgeDataValue::Literal(literal.to_owned())
    } else {
        NeoForgeDataValue::Literal(client.to_owned())
    };
    Ok((token.to_owned(), normalized))
}

/// Composes a vanilla Minecraft plan and a NeoForge plan into the complete
/// game install plan.
///
/// Composition mirrors the Fabric boundary: the two plans stay
/// independently meaningful, the library merge rejects identity conflicts,
/// Mojang's Java requirement stays authoritative (NeoForge publishes no
/// loader Java floor of its own), and the loader's entry point wins.
pub fn compose_neoforge_game_plan(
    minecraft: MinecraftInstallPlan,
    neoforge: NeoForgePlan,
) -> Result<GameInstallPlan, crate::fabric::plan::CompositionError> {
    if minecraft.minecraft_version() != neoforge.minecraft_version() {
        return Err(crate::fabric::plan::CompositionError::VersionMismatch {
            minecraft_version: minecraft.minecraft_version().to_owned(),
            fabric_version: format!("neoforge {}", neoforge.neoforge_version),
        });
    }

    let loader_libraries: Vec<GameLibrary> = neoforge
        .libraries
        .iter()
        .cloned()
        .map(|library| GameLibrary::NeoForge(library))
        .collect();
    let libraries = merge_library_sets(&minecraft, loader_libraries)?;

    let minecraft_java = minecraft.java();
    let java = GameJavaRequirement::without_loader_floor(
        minecraft_java.component(),
        minecraft_java.major_version(),
    );

    let main_class = neoforge.main_class().to_owned();
    Ok(GameInstallPlan::from_composed_parts(
        minecraft,
        crate::fabric::plan::LoaderPlan::NeoForge(neoforge),
        libraries,
        java,
        main_class,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_document() -> InstallProfileDocument {
        InstallProfileDocument::from_json(
            r#"{
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
                {"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {"artifact": {"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "https://maven.neoforged.net/releases/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 669160}}},
                {"name": "net.neoforged.installertools:installertools:4.0.17:fatjar", "downloads": {"artifact": {"path": "net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "url": "https://maven.neoforged.net/releases/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 100000}}}
              ]
            }"#,
        )
        .unwrap()
    }

    fn version_document() -> VersionProfileDocument {
        VersionProfileDocument::from_json(
            r#"{
              "id": "neoforge-26.2.0.88",
              "inheritsFrom": "26.2",
              "mainClass": "net.neoforged.fml.startup.Client",
              "arguments": {
                "jvm": ["-DlibraryDirectory=${library_directory}", "--add-opens", "java.base/java.lang.invoke=ALL-UNNAMED"],
                "game": ["--fml.neoForgeVersion", "26.2.0.88", "--fml.mcVersion", "26.2"]
              },
              "libraries": [
                {"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {"artifact": {"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "https://maven.neoforged.net/releases/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 669160}}},
                {"name": "cpw.mods:bootstraplauncher:2.0.2", "downloads": {"artifact": {"path": "cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "url": "https://maven.neoforged.net/releases/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "sha1": "42d3bfead0ba3aa89c7d45ac29115defc9967219", "size": 123456}}}
              ]
            }"#,
        )
        .unwrap()
    }

    fn plan() -> NeoForgePlan {
        NeoForgePlan::from_documents(
            &profile_document(),
            &version_document(),
            &MinecraftVersionId::new("26.2").unwrap(),
            &NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &NeoForgeMavenEndpoints::official(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn a_plan_records_identity_entry_point_and_libraries() {
        let plan = plan();
        assert_eq!(plan.minecraft_version(), "26.2");
        assert_eq!(plan.neoforge_version(), "26.2.0.88");
        assert_eq!(plan.version_id(), "neoforge-26.2.0.88");
        assert_eq!(plan.main_class(), "net.neoforged.fml.startup.Client");
        assert_eq!(plan.libraries().len(), 2);
        // The duplicated loader library is satisfied by the launch set.
        assert_eq!(plan.installer_libraries().len(), 1);
        assert_eq!(
            plan.installer_libraries()[0].coordinate().artifact(),
            "installertools"
        );
        assert_eq!(
            plan.universal().path(),
            "net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-universal.jar"
        );
        assert!(!plan.universal().is_classpath_entry());
    }

    #[test]
    fn only_client_processors_are_planned() {
        let plan = plan();
        assert_eq!(plan.processors().len(), 1);
        assert_eq!(plan.processors()[0].args()[1], "PROCESS_MINECRAFT_JAR");
        assert!(plan.requires_install_time_java());
        let data = plan.data();
        assert_eq!(
            data.get("BINPATCH"),
            Some(&NeoForgeDataValue::InstallerFile(
                "/data/client.lzma".to_owned()
            ))
        );
        assert_eq!(
            data.get("MCP_VERSION"),
            Some(&NeoForgeDataValue::Literal("26.2-2".to_owned()))
        );
        match data.get("PATCHED") {
            Some(NeoForgeDataValue::Artifact {
                coordinate,
                extension,
            }) => {
                assert_eq!(coordinate.artifact(), "minecraft-client-patched");
                assert_eq!(extension, "jar");
            }
            other => panic!("unexpected PATCHED value: {other:?}"),
        }
    }

    #[test]
    fn identity_disagreements_are_rejected() {
        let error = NeoForgePlan::from_documents(
            &profile_document(),
            &version_document(),
            &MinecraftVersionId::new("26.3").unwrap(),
            &NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &NeoForgeMavenEndpoints::official(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
        )
        .unwrap_err();
        assert!(matches!(error, NeoForgePlanError::Identity { .. }));
    }

    #[test]
    fn unsafe_installer_paths_are_rejected_as_data_values() {
        let mut profile = profile_document();
        profile.data.insert(
            "EVIL".to_owned(),
            ProfileDataValue {
                client: "/../escape.lzma".to_owned(),
                server: "/../escape.lzma".to_owned(),
            },
        );
        let error = NeoForgePlan::from_documents(
            &profile,
            &version_document(),
            &MinecraftVersionId::new("26.2").unwrap(),
            &NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &NeoForgeMavenEndpoints::official(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
            Sha1Digest::parse("42d3bfead0ba3aa89c7d45ac29115defc9967219").unwrap(),
        )
        .unwrap_err();
        assert!(matches!(error, NeoForgePlanError::Data { .. }));
    }
}
