//! The shared game-composition boundary: loader-neutral plan types.
//!
//! This module owns the *composed* half of installation planning — the
//! types every loader family contributes to and every consumer (installer,
//! validator, launch assembly) reads. It moved here from its historical
//! home in `fabric::plan` (F3): composition is a launcher-wide concern,
//! and a future Forge or Quilt adapter must not appear to require Fabric
//! merely because the shared types lived there. `fabric::plan` reexports
//! these types so existing paths compile unchanged during the transition.
//!
//! The boundary follows one rule: **share composition, not loader
//! behavior**. Each loader adapter (Fabric, NeoForge, and future families)
//! owns its own metadata parsing, normalization, and its family composer;
//! the shared model below represents only what is genuinely common:
//!
//! - explicit containment: the vanilla [`MinecraftInstallPlan`] and the
//!   family-specific [`LoaderPlan`] stay independently meaningful parts,
//!   so Mojang's and each loader's requirements remain distinguishable
//!   for diagnostics and repair;
//! - derived composed views — the ordered library set with provenance,
//!   the effective Java requirement, the final entry point, and the
//!   effective argument groups — are the only thing installers and launch
//!   assembly consume; they never re-branch on the loader family;
//! - collisions between requirements are deliberate policy, never silent:
//!   exact duplicates collapse to the earlier entry, a same identity at a
//!   different version is a hard conflict, and different classifiers of
//!   one library legitimately coexist.
//!
//! Launch arguments stay semantically unresolved templates here;
//! substitution of a real session and managed paths happens only in
//! `launch::resolve`.

use std::fmt;

use crate::minecraft::plan::{LibraryCoordinate, MinecraftInstallPlan, PlannedLibrary};

/// The effective Java requirement of a composed installation.
///
/// Minecraft's runtime component stays the runtime family a future installer
/// obtains; the effective minimum major version is the stricter of Mojang's
/// requirement and the loader's declared floor. `raised_by_loader` records
/// the (so far unobserved) case where a loader demands more than Mojang.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameJavaRequirement {
    component: String,
    major_version: u32,
    loader_min_major_version: u32,
    raised_by_loader: bool,
}

impl GameJavaRequirement {
    /// A requirement with no loader floor: Mojang's component and major
    /// stay authoritative (used by loaders that publish no Java floor of
    /// their own, currently NeoForge).
    pub fn without_loader_floor(component: &str, major_version: u32) -> Self {
        Self {
            component: component.to_owned(),
            major_version,
            loader_min_major_version: 0,
            raised_by_loader: false,
        }
    }

    /// A requirement where the loader declares a Java floor: the effective
    /// major is the stricter of Mojang's and the loader's, with
    /// `raised_by_loader` recording the (so far unobserved) case where the
    /// loader demands more than Mojang. Mojang's component always remains
    /// the runtime family.
    pub fn with_loader_floor(
        component: &str,
        mojang_major_version: u32,
        loader_min_major_version: u32,
    ) -> Self {
        Self {
            component: component.to_owned(),
            major_version: mojang_major_version.max(loader_min_major_version),
            loader_min_major_version,
            raised_by_loader: loader_min_major_version > mojang_major_version,
        }
    }

    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn major_version(&self) -> u32 {
        self.major_version
    }

    pub fn loader_min_major_version(&self) -> u32 {
        self.loader_min_major_version
    }

    pub fn raised_by_loader(&self) -> bool {
        self.raised_by_loader
    }
}

/// One entry of the composed library set, with its provenance.
///
/// Provenance is a fact about which metadata source contributed the
/// library, never about trust: every variant carries its own honest
/// artifact expectations (Mojang SHA-1, Fabric's published-or-absent
/// SHA-256, NeoForge SHA-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameLibrary {
    Minecraft(PlannedLibrary),
    Fabric(crate::fabric::plan::FabricLibrary),
    NeoForge(crate::neoforge::plan::NeoForgeLibrary),
    Forge(crate::forge::plan::ForgeLibrary),
}

/// Which metadata source contributed a composed library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibraryProvenance {
    Mojang,
    Fabric,
    NeoForge,
    Forge,
}

/// The loader half of a composed plan. The container is loader-generic on
/// purpose — installation, validation, and launch consume the composed
/// views, not loader-specific branching — while each loader's plan stays an
/// independently meaningful boundary inside its variant. Adding a family
/// means adding a variant and updating every exhaustive match over this
/// enum; there is deliberately no fallback variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderPlan {
    Fabric(crate::fabric::plan::FabricPlan),
    NeoForge(crate::neoforge::plan::NeoForgePlan),
    Forge(crate::forge::plan::ForgePlan),
}

impl LoaderPlan {
    /// The exact loader version this contribution pins.
    pub fn loader_version(&self) -> &str {
        match self {
            Self::Fabric(plan) => plan.loader_version(),
            Self::NeoForge(plan) => plan.neoforge_version(),
            Self::Forge(plan) => plan.forge_version(),
        }
    }
}

impl GameLibrary {
    pub fn provenance(&self) -> LibraryProvenance {
        match self {
            Self::Minecraft(_) => LibraryProvenance::Mojang,
            Self::Fabric(_) => LibraryProvenance::Fabric,
            Self::NeoForge(_) => LibraryProvenance::NeoForge,
            Self::Forge(_) => LibraryProvenance::Forge,
        }
    }

    /// The canonical Maven coordinate string of this library.
    pub fn coordinate_string(&self) -> String {
        match self {
            Self::Minecraft(library) => library.coordinate().as_maven_string(),
            Self::Fabric(library) => library.coordinate().as_maven_string(),
            Self::NeoForge(library) => library.coordinate().as_maven_string(),
            Self::Forge(library) => library.coordinate().as_maven_string(),
        }
    }

    /// The repository-relative Maven layout path of this library.
    pub fn repository_path(&self) -> String {
        match self {
            Self::Minecraft(library) => library.path().to_owned(),
            Self::Fabric(library) => library.path(),
            Self::NeoForge(library) => library.path().to_owned(),
            Self::Forge(library) => library.path().to_owned(),
        }
    }

    /// Whether this artifact belongs on Java's classpath. Mojang native
    /// classifier jars are installation/extraction inputs, not classpath
    /// entries; every Fabric launcherMeta library is an ordinary classpath
    /// dependency, as are NeoForge's launch libraries (its universal and
    /// processor-generated artifacts are library-directory discoveries,
    /// planned separately and never placed on the classpath).
    pub fn is_classpath_entry(&self) -> bool {
        match self {
            Self::Minecraft(library) => {
                library.kind() == crate::minecraft::plan::LibraryKind::PlatformLibrary
            }
            Self::Fabric(_) => true,
            Self::NeoForge(library) => library.is_classpath_entry(),
            Self::Forge(library) => library.is_classpath_entry(),
        }
    }
}

/// The complete, deterministic plan for one Aurora installation.
///
/// Composition is explicit containment: the vanilla plan and the loader plan
/// remain independently meaningful parts (provenance is preserved for
/// diagnostics and repair), while the derived views — the ordered composed
/// library set, the effective Java requirement, and the final entry point —
/// are what an installer consumes. A plan with no loader is a complete
/// Vanilla plan, never a "missing loader" error case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInstallPlan {
    minecraft: MinecraftInstallPlan,
    loader: Option<LoaderPlan>,
    libraries: Vec<GameLibrary>,
    java: GameJavaRequirement,
    main_class: String,
}

impl GameInstallPlan {
    /// The vanilla Minecraft half, unchanged by composition.
    pub fn minecraft(&self) -> &MinecraftInstallPlan {
        &self.minecraft
    }

    /// The loader half, unchanged by composition.
    pub fn loader(&self) -> Option<&LoaderPlan> {
        self.loader.as_ref()
    }

    /// The Fabric half, when the contribution is Fabric.
    pub fn fabric_loader(&self) -> Option<&crate::fabric::plan::FabricPlan> {
        match &self.loader {
            Some(LoaderPlan::Fabric(plan)) => Some(plan),
            _ => None,
        }
    }

    /// The NeoForge half, when the contribution is NeoForge.
    pub fn neoforge_loader(&self) -> Option<&crate::neoforge::plan::NeoForgePlan> {
        match &self.loader {
            Some(LoaderPlan::NeoForge(plan)) => Some(plan),
            _ => None,
        }
    }

    /// The Forge half, when the contribution is Forge.
    pub fn forge_loader(&self) -> Option<&crate::forge::plan::ForgePlan> {
        match &self.loader {
            Some(LoaderPlan::Forge(plan)) => Some(plan),
            _ => None,
        }
    }

    /// The installed platform identity this plan produces: Vanilla with no
    /// loader, otherwise the exact family and version of the contribution.
    pub fn platform(&self) -> crate::instances::platform::PlatformPin {
        match &self.loader {
            Some(LoaderPlan::Fabric(plan)) => crate::instances::platform::PlatformPin::Fabric {
                version: plan.loader_version().to_owned(),
            },
            Some(LoaderPlan::NeoForge(plan)) => crate::instances::platform::PlatformPin::NeoForge {
                version: plan.neoforge_version().to_owned(),
            },
            Some(LoaderPlan::Forge(plan)) => crate::instances::platform::PlatformPin::Forge {
                version: plan.forge_version().to_owned(),
            },
            None => crate::instances::platform::PlatformPin::Vanilla {},
        }
    }

    /// A complete Vanilla plan contains only Mojang contributions.
    pub fn vanilla(minecraft: MinecraftInstallPlan) -> Self {
        let java = GameJavaRequirement {
            component: minecraft.java().component().to_owned(),
            major_version: minecraft.java().major_version(),
            loader_min_major_version: 0,
            raised_by_loader: false,
        };
        Self {
            libraries: minecraft
                .libraries()
                .iter()
                .cloned()
                .map(GameLibrary::Minecraft)
                .collect(),
            main_class: minecraft.launch().main_class().to_owned(),
            minecraft,
            loader: None,
            java,
        }
    }

    /// The composed library set in deterministic classpath order: Mojang
    /// libraries in official document order, then the loader's libraries in
    /// its own contribution order, with exact duplicates collapsed.
    pub fn libraries(&self) -> &[GameLibrary] {
        &self.libraries
    }

    /// The effective Java requirement.
    pub fn java(&self) -> &GameJavaRequirement {
        &self.java
    }

    /// The final launch entry point (the loader's client main class, or
    /// Mojang's for Vanilla).
    pub fn main_class(&self) -> &str {
        &self.main_class
    }

    pub fn vanilla_library_count(&self) -> usize {
        self.libraries
            .iter()
            .filter(|library| library.provenance() == LibraryProvenance::Mojang)
            .count()
    }

    pub fn fabric_library_count(&self) -> usize {
        self.libraries
            .iter()
            .filter(|library| library.provenance() == LibraryProvenance::Fabric)
            .count()
    }

    pub fn neoforge_library_count(&self) -> usize {
        self.libraries
            .iter()
            .filter(|library| library.provenance() == LibraryProvenance::NeoForge)
            .count()
    }

    pub fn forge_library_count(&self) -> usize {
        self.libraries
            .iter()
            .filter(|library| library.provenance() == LibraryProvenance::Forge)
            .count()
    }

    /// The effective JVM arguments in deterministic order: Mojang's planned
    /// arguments, then the loader's contributions appended (NeoForge is the
    /// first loader that contributes JVM arguments; Fabric contributes
    /// none). Returned as feature-unconditional groups — loader arguments
    /// carry no rule conditions.
    pub fn effective_jvm_arguments(&self) -> Vec<crate::minecraft::plan::LaunchArgument> {
        let mut arguments = self.minecraft.launch().jvm_arguments().to_vec();
        let loader_jvm: Option<&[String]> = match &self.loader {
            Some(LoaderPlan::NeoForge(plan)) => Some(plan.jvm_arguments()),
            Some(LoaderPlan::Forge(plan)) => Some(plan.jvm_arguments()),
            Some(LoaderPlan::Fabric(_)) | None => None,
        };
        if let Some(values) = loader_jvm.filter(|values| !values.is_empty()) {
            arguments.push(crate::minecraft::plan::LaunchArgument {
                features: Vec::new(),
                values: values.to_vec(),
            });
        }
        arguments
    }

    /// The effective game arguments in deterministic order: Mojang's, then
    /// the loader's appended.
    pub fn effective_game_arguments(&self) -> Vec<crate::minecraft::plan::LaunchArgument> {
        let mut arguments = self.minecraft.launch().game_arguments().to_vec();
        let loader_game: Option<&[String]> = match &self.loader {
            Some(LoaderPlan::NeoForge(plan)) => Some(plan.game_arguments()),
            Some(LoaderPlan::Forge(plan)) => Some(plan.game_arguments()),
            Some(LoaderPlan::Fabric(_)) | None => None,
        };
        if let Some(values) = loader_game.filter(|values| !values.is_empty()) {
            arguments.push(crate::minecraft::plan::LaunchArgument {
                features: Vec::new(),
                values: values.to_vec(),
            });
        }
        arguments
    }

    /// Assembles a composed plan from already-validated parts. Only loader
    /// composition boundaries call this; the parts must already agree.
    pub(crate) fn from_composed_parts(
        minecraft: MinecraftInstallPlan,
        loader: LoaderPlan,
        libraries: Vec<GameLibrary>,
        java: GameJavaRequirement,
        main_class: String,
    ) -> Self {
        Self {
            minecraft,
            loader: Some(loader),
            libraries,
            java,
            main_class,
        }
    }
}

/// A failure while composing the vanilla and loader plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionError {
    LoaderRequirement {
        loader_family: &'static str,
        reason: String,
    },
    /// The two plans describe different Minecraft versions.
    VersionMismatch {
        minecraft_version: String,
        loader_version: String,
        loader_family: &'static str,
    },
    /// Two requirements claim the same library identity at different
    /// versions; Aurora resolves no winner.
    LibraryConflict {
        existing: String,
        conflicting: String,
    },
}

impl fmt::Display for CompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoaderRequirement {
                loader_family,
                reason,
            } => write!(
                formatter,
                "{loader_family} composition requirement: {reason}"
            ),
            Self::VersionMismatch {
                minecraft_version,
                loader_version,
                loader_family,
            } => write!(
                formatter,
                "the Minecraft plan describes '{minecraft_version}' but the {loader_family} plan describes '{loader_version}'"
            ),
            Self::LibraryConflict {
                existing,
                conflicting,
            } => write!(
                formatter,
                "library identity conflict: '{conflicting}' conflicts with the already-planned '{existing}'"
            ),
        }
    }
}

impl std::error::Error for CompositionError {}

/// The identity two library requirements must share to be the *same*
/// artifact: group, artifact, and classifier (version decides duplicate
/// versus conflict). Different classifiers of one group and artifact are
/// genuinely different artifacts — that is how Mojang publishes natives.
pub(crate) type LibraryIdentity<'a> = (&'a str, &'a str, Option<&'a str>);

pub(crate) fn mojang_identity(coordinate: &LibraryCoordinate) -> LibraryIdentity<'_> {
    (
        coordinate.group(),
        coordinate.artifact(),
        coordinate.classifier(),
    )
}

pub(crate) fn fabric_identity(
    coordinate: &crate::fabric::maven::MavenCoordinate,
) -> LibraryIdentity<'_> {
    (
        coordinate.group(),
        coordinate.artifact(),
        coordinate.classifier(),
    )
}

pub(crate) fn neoforge_identity(
    coordinate: &crate::neoforge::metadata::NeoForgeCoordinate,
) -> LibraryIdentity<'_> {
    (
        coordinate.group(),
        coordinate.artifact(),
        coordinate.classifier(),
    )
}

pub(crate) fn forge_identity(
    coordinate: &crate::forge::plan::ForgeCoordinate,
) -> LibraryIdentity<'_> {
    (
        coordinate.group(),
        coordinate.artifact(),
        coordinate.classifier(),
    )
}

/// Merges Mojang's libraries (document order) with a loader's libraries
/// (contribution order): exact duplicates collapse to the earlier (Mojang)
/// entry, and a same identity at a different version is a hard conflict.
pub(crate) fn merge_library_sets(
    minecraft: &MinecraftInstallPlan,
    loader_libraries: Vec<GameLibrary>,
) -> Result<Vec<GameLibrary>, CompositionError> {
    let mut libraries: Vec<GameLibrary> =
        Vec::with_capacity(minecraft.libraries().len() + loader_libraries.len());

    let mut push = |library: GameLibrary,
                    identity: LibraryIdentity<'_>,
                    version: &str|
     -> Result<(), CompositionError> {
        for planned in &libraries {
            let (existing_identity, existing_version) = match planned {
                GameLibrary::Minecraft(entry) => (
                    mojang_identity(entry.coordinate()),
                    entry.coordinate().version(),
                ),
                GameLibrary::Fabric(entry) => (
                    fabric_identity(entry.coordinate()),
                    entry.coordinate().version(),
                ),
                GameLibrary::NeoForge(entry) => (
                    neoforge_identity(entry.coordinate()),
                    entry.coordinate().version(),
                ),
                GameLibrary::Forge(entry) => (
                    forge_identity(entry.coordinate()),
                    entry.coordinate().version(),
                ),
            };
            if existing_identity == identity {
                if existing_version == version {
                    return Ok(()); // exact duplicate: one requirement suffices
                }
                return Err(CompositionError::LibraryConflict {
                    existing: planned.coordinate_string(),
                    conflicting: library.coordinate_string(),
                });
            }
        }
        libraries.push(library);
        Ok(())
    };

    for entry in minecraft.libraries() {
        let identity = mojang_identity(entry.coordinate());
        let version = entry.coordinate().version().to_owned();
        push(GameLibrary::Minecraft(entry.clone()), identity, &version)?;
    }
    for library in loader_libraries {
        let (identity, version) = match &library {
            GameLibrary::Minecraft(entry) => (
                mojang_identity(entry.coordinate()),
                entry.coordinate().version().to_owned(),
            ),
            GameLibrary::Fabric(entry) => (
                fabric_identity(entry.coordinate()),
                entry.coordinate().version().to_owned(),
            ),
            GameLibrary::NeoForge(entry) => (
                neoforge_identity(entry.coordinate()),
                entry.coordinate().version().to_owned(),
            ),
            GameLibrary::Forge(entry) => (
                forge_identity(entry.coordinate()),
                entry.coordinate().version().to_owned(),
            ),
        };
        let identity = (
            identity.0.to_owned(),
            identity.1.to_owned(),
            identity.2.map(str::to_owned),
        );
        push(
            library,
            (&identity.0, &identity.1, identity.2.as_deref()),
            &version,
        )?;
    }

    Ok(libraries)
}

/// Compare validated normalized repository destinations, including extension.
/// At one destination agreeing official SHA-1 satisfies the installer-tool
/// requirement; disagreement is a hard conflict. Other destinations remain
/// separate regardless of basename, version, URL, or digest equality.
/// This does not change the established launch-library merge policy.
pub(crate) fn installer_library_satisfied_by(
    merged: &[GameLibrary],
    destination: &str,
    sha1: &crate::integrity::Sha1Digest,
    conflicting: &str,
) -> Result<bool, CompositionError> {
    for planned in merged {
        if planned.repository_path() != destination {
            continue;
        }
        let existing_sha1 = match planned {
            GameLibrary::Minecraft(entry) => entry.artifact().sha1(),
            GameLibrary::Forge(entry) => entry.artifact().sha1(),
            GameLibrary::NeoForge(entry) => entry.artifact().sha1(),
            // No official SHA-1: cannot establish equivalence at this path.
            GameLibrary::Fabric(_) => {
                return Err(CompositionError::LibraryConflict {
                    existing: planned.coordinate_string(),
                    conflicting: conflicting.to_owned(),
                });
            }
        };
        if existing_sha1 != sha1 {
            return Err(CompositionError::LibraryConflict {
                existing: planned.coordinate_string(),
                conflicting: conflicting.to_owned(),
            });
        }
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared model's Vanilla invariants, exercised without any loader:
    /// a Vanilla plan is complete (not a degenerate loader case), carries
    /// only Mojang provenance, Mojang's main class, and Mojang's exact Java
    /// requirement. Fixture construction reuses the Minecraft planner over
    /// a minimal modern document.
    fn minecraft_plan() -> MinecraftInstallPlan {
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

    #[test]
    fn a_vanilla_plan_is_complete_without_a_loader() {
        let plan = GameInstallPlan::vanilla(minecraft_plan());
        assert!(plan.loader().is_none());
        assert!(plan.fabric_loader().is_none());
        assert!(plan.neoforge_loader().is_none());
        assert_eq!(
            plan.platform(),
            crate::instances::platform::PlatformPin::Vanilla {}
        );
        assert_eq!(plan.main_class(), "net.minecraft.client.main.Main");
        assert_eq!(plan.vanilla_library_count(), plan.libraries().len());
        assert_eq!(plan.fabric_library_count(), 0);
        assert_eq!(plan.neoforge_library_count(), 0);
        // Mojang's requirement is authoritative with no loader floor.
        assert_eq!(plan.java().component(), "java-runtime-delta");
        assert_eq!(plan.java().major_version(), 21);
        assert_eq!(plan.java().loader_min_major_version(), 0);
        assert!(!plan.java().raised_by_loader());
        // Effective arguments are exactly Mojang's groups.
        assert_eq!(
            plan.effective_jvm_arguments(),
            plan.minecraft().launch().jvm_arguments()
        );
        assert_eq!(
            plan.effective_game_arguments(),
            plan.minecraft().launch().game_arguments()
        );
    }

    #[test]
    fn merging_without_loader_libraries_preserves_document_order() {
        let minecraft = minecraft_plan();
        let merged = merge_library_sets(&minecraft, Vec::new()).unwrap();
        assert_eq!(merged.len(), minecraft.libraries().len());
        assert_eq!(merged[0].provenance(), LibraryProvenance::Mojang);
        assert_eq!(merged[0].coordinate_string(), "com.mojang:brigadier:1.0.18");
    }

    #[test]
    fn a_mojang_duplicate_of_itself_collapses() {
        let minecraft = minecraft_plan();
        let duplicate = minecraft.libraries()[0].clone();
        let merged =
            merge_library_sets(&minecraft, vec![GameLibrary::Minecraft(duplicate)]).unwrap();
        assert_eq!(merged.len(), minecraft.libraries().len());
    }

    #[test]
    fn installer_libraries_compare_exact_destinations_and_published_sha1() {
        let minecraft = minecraft_plan();
        let merged = merge_library_sets(&minecraft, Vec::new()).unwrap();
        let library = &merged[0];
        let destination = library.repository_path();
        let GameLibrary::Minecraft(entry) = library else {
            unreachable!()
        };
        let sha1 = entry.artifact().sha1();
        assert!(installer_library_satisfied_by(&merged, &destination, sha1, "same").unwrap());
        let conflicting =
            crate::integrity::Sha1Digest::parse("0000000000000000000000000000000000000000")
                .unwrap();
        assert!(matches!(
            installer_library_satisfied_by(&merged, &destination, &conflicting, "conflicting"),
            Err(CompositionError::LibraryConflict { .. })
        ));
        for other in [
            destination.replace(".jar", ".zip"),
            format!("unrelated/{destination}"),
        ] {
            for digest in [sha1, &conflicting] {
                assert!(
                    !installer_library_satisfied_by(&merged, &other, digest, "distinct").unwrap()
                );
            }
        }
    }

    #[test]
    fn composition_errors_name_the_loader_family() {
        let error = CompositionError::VersionMismatch {
            minecraft_version: "26.2".to_owned(),
            loader_version: "1.20.1".to_owned(),
            loader_family: "NeoForge",
        };
        let message = error.to_string();
        assert!(message.contains("NeoForge"), "{message}");
        assert!(message.contains("26.2"), "{message}");
    }
}
