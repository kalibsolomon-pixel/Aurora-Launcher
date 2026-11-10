//! Merge one exact Modrinth pack version into an existing, ready Fabric,
//! Forge or NeoForge instance. The instance keeps its identity,
//! configuration and (on Fabric) Aurora Client; the pack's files become
//! ordinary managed content. This is deliberately not pack ownership:
//! pack-owned instances cannot contain Aurora Client, and a merged instance
//! does not follow pack updates.
//!
//! Safety rules: the instance's mod loader must be the pack's, its Minecraft
//! version must equal the pack's, and its loader must be at least the pack's
//! (Fabric Loader, the Forge build, or the NeoForge version, each compared by
//! that loader's own rules). Those checks run as early as their evidence
//! exists: Modrinth's version metadata before the archive download, the
//! parsed index before any component lookup or download. Existing files are
//! never overwritten or adopted: byte-identical content is skipped, a
//! differing configuration override keeps the user's file, and a differing
//! mod (or a different version of an installed provider project) blocks the
//! merge. Planning is read-only; preview and install share it, and install
//! re-runs it and compares the fingerprint before any write.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::cache::{ArtifactCache, VerifiedArtifact};
use crate::instance_content::{ContentState, ProviderInstallPlan, ProviderRecord};
use crate::instance_mods::ModOwnership;
use crate::instances::platform::{InstalledConfiguration, PlatformPin};
use crate::instances::{InstanceId, InstanceRegistry, InstanceState};
use crate::integrity::{ArtifactDigest, Sha1Digest, verify_file, verify_file_sha1};
use crate::modpacks::{PackInstallError, PackPreview, Resolved};
use crate::mrpack::{OverrideFile, PackFile, PackLoader, PackLoaderKind, PackPlan};
use crate::paths::ManagedPaths;

/// Modrinth project of Fabric API, which Aurora Fabric instances already
/// provide as a protected bootstrap mod. On a Fabric instance a pack's copy
/// is skipped rather than duplicated; other loaders have no such mod.
const FABRIC_API_PROJECT: &str = "P7dR8mSH";

fn fail(code: &'static str, message: impl Into<String>) -> PackInstallError {
    PackInstallError {
        code,
        message: message.into(),
        instance_id: None,
    }
}

/// Shown, and returned as `pack_merge_instance_busy`, while the instance is
/// starting, running, or being prepared for launch.
const INSTANCE_BUSY: &str = "Quit Minecraft and wait for launch preparation to finish before adding a modpack to this instance.";

fn instance_busy() -> PackInstallError {
    fail("pack_merge_instance_busy", INSTANCE_BUSY)
}

fn launch_active(instance: &InstanceId) -> bool {
    crate::launch::process::snapshot(instance.as_str())
        .status
        .blocks_launch()
        || crate::launch::process::is_preparing(instance.as_str())
}

/// What merging this exact pack version into the instance would do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMergePreview {
    pub pack: PackPreview,
    pub instance_id: String,
    pub instance_minecraft_version: String,
    /// The instance's platform kind (`fabric`, `forge`, `neoForge`, ...).
    pub instance_loader: &'static str,
    /// The instance's exact loader pin (for Forge, `<minecraft>-<build>`).
    pub instance_loader_version: Option<String>,
    /// Files the merge adds (provider components and verified external files).
    pub added: Vec<String>,
    /// Pack files already present byte-for-byte (or provided by the instance).
    pub already_present: Vec<String>,
    /// Pack configuration overrides not applied because the user's file differs.
    pub kept_existing: Vec<String>,
    /// Configuration overrides the merge writes (none of these exist yet).
    pub overrides_written: Vec<String>,
    /// Mod files already in the instance that the merge leaves exactly as
    /// they are (a read-only listing; pack files already present excluded).
    pub untouched_mods: Vec<String>,
    /// Modrinth-recognized components this merge would add.
    pub component_count: usize,
    /// Most components one merge adds
    /// (`instance_content::MAX_PROVIDER_PLANS_PER_OPERATION`).
    pub component_limit: usize,
    /// Reasons the merge cannot proceed. Install refuses while any remain.
    pub blockers: Vec<String>,
    pub fingerprint: String,
}

/// Outcome of a completed merge.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMergeResult {
    pub instance_id: String,
    pub added: usize,
    pub already_present: usize,
    pub overrides_written: usize,
    pub kept_existing: Vec<String>,
}

struct MergePlan {
    preview: PackMergePreview,
    provider_plans: Vec<ProviderInstallPlan>,
    externals: Vec<PackFile>,
    overrides: Vec<OverrideFile>,
}

/// Compare dotted numeric versions (`0.19.5`); non-numeric parts compare as text.
fn version_at_least(installed: &str, required: &str) -> bool {
    let parts = |value: &str| -> Vec<String> {
        value
            .split(|c: char| c == '.' || c == '-' || c == '+')
            .map(str::to_owned)
            .collect()
    };
    let (a, b) = (parts(installed), parts(required));
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).map(String::as_str).unwrap_or("0");
        let right = b.get(index).map(String::as_str).unwrap_or("0");
        let ordering = match (left.parse::<u64>(), right.parse::<u64>()) {
            (Ok(l), Ok(r)) => l.cmp(&r),
            _ => left.cmp(right),
        };
        match ordering {
            std::cmp::Ordering::Greater => return true,
            std::cmp::Ordering::Less => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    true
}

/// How the instance's platform is named in refusals.
fn platform_label(platform: &PlatformPin) -> &'static str {
    match platform {
        PlatformPin::Vanilla {} => "no mod loader (Vanilla)",
        PlatformPin::Fabric { .. } => "Fabric",
        PlatformPin::Forge { .. } => "Forge",
        PlatformPin::NeoForge { .. } => "NeoForge",
        PlatformPin::Quilt { .. } => "Quilt",
    }
}

fn pack_kind_of(platform: &PlatformPin) -> Option<PackLoaderKind> {
    match platform {
        PlatformPin::Fabric { .. } => Some(PackLoaderKind::Fabric),
        PlatformPin::Forge { .. } => Some(PackLoaderKind::Forge),
        PlatformPin::NeoForge { .. } => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

/// Incompatibilities visible in Modrinth's version metadata alone: the
/// instance's loader family must be listed, and its exact Minecraft version.
fn metadata_refusals(
    installed: &InstalledConfiguration,
    loaders: &[String],
    game_versions: &[String],
) -> Vec<String> {
    let label = platform_label(&installed.platform);
    let Ok(tag) = installed.platform.provider_loader() else {
        return vec![format!(
            "This instance uses {label}; a modpack can be added only to a Fabric, Forge or NeoForge instance."
        )];
    };
    let mut refusals = Vec::new();
    if !loaders
        .iter()
        .any(|loader| loader.eq_ignore_ascii_case(tag))
    {
        let listed = loaders
            .iter()
            .filter_map(|loader| match loader.to_ascii_lowercase().as_str() {
                "fabric" => Some("Fabric"),
                "forge" => Some("Forge"),
                "neoforge" => Some("NeoForge"),
                "quilt" => Some("Quilt"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" and ");
        refusals.push(format!(
            "This Modrinth version is a {listed} modpack; this instance uses {label}."
        ));
    }
    if !game_versions
        .iter()
        .any(|game| game == &installed.minecraft_version)
    {
        let games = game_versions
            .iter()
            .take(8)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        refusals.push(format!(
            "This Modrinth version is for Minecraft {games}; this instance has Minecraft {}.",
            installed.minecraft_version
        ));
    }
    refusals
}

/// Incompatibilities between the instance and the parsed pack index: the
/// loader kind, the exact Minecraft version, and then the loader version.
fn compatibility_refusals(installed: &InstalledConfiguration, pack: &PackPlan) -> Vec<String> {
    let mut refusals = Vec::new();
    if pack_kind_of(&installed.platform) != Some(pack.loader.kind) {
        refusals.push(format!(
            "This pack is a {} modpack; this instance uses {}.",
            pack.loader.kind.display_name(),
            platform_label(&installed.platform)
        ));
    }
    if installed.minecraft_version != pack.minecraft_version {
        refusals.push(format!(
            "This pack is for Minecraft {}; this instance has Minecraft {}.",
            pack.minecraft_version, installed.minecraft_version
        ));
    }
    if refusals.is_empty() {
        refusals.extend(loader_version_refusal(
            &installed.platform,
            &pack.loader,
            &pack.minecraft_version,
        ));
    }
    refusals
}

/// The instance's loader must be at least the pack's on the same Minecraft
/// version. Fabric keeps its dotted comparison; Forge builds and NeoForge
/// versions use the Maven ordering those loaders' own dependency ranges use
/// (`mod_compatibility::satisfies_for_family`), with the Forge build taken
/// from the instance pin by Forge's own identity parser.
fn loader_version_refusal(
    platform: &PlatformPin,
    loader: &PackLoader,
    minecraft: &str,
) -> Option<String> {
    let required = &loader.version;
    let at_least = |installed: &str, family: &str| {
        crate::mod_compatibility::satisfies_for_family(installed, &format!("[{required},)"), family)
    };
    match platform {
        PlatformPin::Fabric { version } => (!version_at_least(version, required)).then(|| {
            format!(
                "This pack needs Fabric Loader {required} or newer; this instance has Fabric Loader {version}."
            )
        }),
        PlatformPin::Forge { version } => {
            let Some(build) = crate::forge::metadata::ForgeVersionId::new(version)
                .ok()
                .filter(|pin| pin.minecraft_version() == minecraft)
                .map(|pin| pin.build().to_owned())
            else {
                return Some(format!(
                    "Aurora can't read this instance's Forge version {version} for Minecraft {minecraft}."
                ));
            };
            match at_least(&build, "forge") {
                Ok(true) => None,
                Ok(false) => Some(format!(
                    "This pack needs Forge {required} or newer for Minecraft {minecraft}; this instance has Forge {build}."
                )),
                Err(_) => Some(format!(
                    "Aurora can't compare this instance's Forge {build} with the pack's Forge {required}."
                )),
            }
        }
        PlatformPin::NeoForge { version } => match at_least(version, "neoForge") {
            Ok(true) => None,
            Ok(false) => Some(format!(
                "This pack needs NeoForge {required} or newer; this instance has NeoForge {version}."
            )),
            Err(_) => Some(format!(
                "Aurora can't compare this instance's NeoForge {version} with the pack's NeoForge {required}."
            )),
        },
        _ => None,
    }
}

const NOT_READY: &str = "Finish installing this instance before adding a modpack.";
const PACK_MANAGED: &str =
    "This instance already follows a Modrinth modpack; use its pack update instead.";

/// One registered instance as a possible destination for one exact pack
/// version, decided from Modrinth's version metadata and the registry alone
/// (nothing is downloaded). Review still re-checks everything, including the
/// exact loader version the pack index names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackDestination {
    pub instance_id: String,
    pub display_name: String,
    pub minecraft_version: String,
    /// The instance's platform kind (`fabric`, `forge`, `neoForge`, ...).
    pub loader: &'static str,
    pub loader_version: Option<String>,
    /// Whether the merge may be reviewed for this instance.
    pub compatible: bool,
    /// Why it may not (empty when compatible).
    pub reasons: Vec<String>,
}

/// The merge admission rules visible before the archive download, for one
/// instance: Ready, not following a pack, and the loader family and exact
/// Minecraft version listed by the version metadata.
fn destination_for(
    record: &crate::instances::InstanceRecord,
    loaders: &[String],
    game_versions: &[String],
) -> PackDestination {
    let installed = record.installed();
    let reasons = if record.state() != InstanceState::Ready {
        vec![NOT_READY.to_owned()]
    } else if record.pack().is_some() {
        vec![PACK_MANAGED.to_owned()]
    } else {
        metadata_refusals(installed, loaders, game_versions)
    };
    PackDestination {
        instance_id: record.id().to_string(),
        display_name: record.display_name().to_owned(),
        minecraft_version: installed.minecraft_version.clone(),
        loader: installed.platform.kind(),
        loader_version: installed.platform.version().map(str::to_owned),
        compatible: reasons.is_empty(),
        reasons,
    }
}

/// Every registered instance, annotated as a destination for this exact
/// pack version. A version Aurora cannot install at all is refused with the
/// same error resolution returns; no archive is downloaded.
pub async fn destinations(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    project_id: &str,
    version_id: &str,
) -> Result<Vec<PackDestination>, PackInstallError> {
    let artifact = provider
        .pack_artifact(project_id, version_id)
        .await
        .map_err(|e| fail(e.code(), e.to_string()))?;
    if let Some(refusal) = crate::modpacks::version_metadata_refusal(&artifact) {
        return Err(refusal);
    }
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    Ok(registry
        .instances()
        .iter()
        .map(|record| destination_for(record, &artifact.loaders, &artifact.game_versions))
        .collect())
}

/// The merge target's admission rule inside `modpacks::resolve_gated`.
struct MergeGate<'a> {
    installed: &'a InstalledConfiguration,
}

impl crate::modpacks::ResolveGate for MergeGate<'_> {
    fn before_download(
        &self,
        artifact: &crate::modrinth::PackArtifact,
    ) -> Result<(), PackInstallError> {
        incompatible(metadata_refusals(
            self.installed,
            &artifact.loaders,
            &artifact.game_versions,
        ))
    }

    fn after_parse(&self, pack: &PackPlan) -> Result<(), PackInstallError> {
        incompatible(compatibility_refusals(self.installed, pack))
    }
}

fn incompatible(refusals: Vec<String>) -> Result<(), PackInstallError> {
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(fail("pack_merge_incompatible", refusals.join(" ")))
    }
}

/// The instance's installed configuration, read from the registry.
fn installed_configuration(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<InstalledConfiguration, PackInstallError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    registry
        .find(instance)
        .map(|record| record.installed().clone())
        .ok_or_else(|| fail("instance_not_found", "The instance no longer exists."))
}

/// Resolve the exact pack version for this instance, refusing an
/// incompatible combination before the archive (metadata) or any component
/// (index) is fetched.
async fn resolve_for(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
) -> Result<Resolved, PackInstallError> {
    let installed = installed_configuration(managed, instance)?;
    crate::modpacks::resolve_gated(
        managed,
        provider,
        project_id,
        version_id,
        selected_optional,
        &MergeGate {
            installed: &installed,
        },
    )
    .await
}

/// Mod files in the instance now, read-only (the same inventory scan the
/// Mods tab uses). A missing `mods/` directory holds none.
fn existing_mod_files(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Vec<String>, PackInstallError> {
    if !managed.instance_paths(instance).mods().is_dir() {
        return Ok(Vec::new());
    }
    let inventory = crate::instance_mods::scan(managed, instance)
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    Ok(inventory
        .entries
        .into_iter()
        .filter(|entry| {
            matches!(
                entry.file_type,
                crate::instance_mods::ModFileType::EnabledJar
                    | crate::instance_mods::ModFileType::DisabledJar
            )
        })
        .map(|entry| entry.file_name)
        .collect())
}

/// Read-only destination lookup. Never creates directories; a redirected or
/// file-occupied parent is reported as unsafe.
fn existing_destination(root: &Path, relative: &str) -> Result<Option<PathBuf>, PackInstallError> {
    crate::mrpack::destination_path(relative)
        .map_err(|_| fail("pack_invalid_path", "A pack file destination is unsafe."))?;
    let mut target = root.to_path_buf();
    let mut segments = relative.split('/').peekable();
    while let Some(part) = segments.next() {
        target.push(part);
        match std::fs::symlink_metadata(&target) {
            Ok(meta) if segments.peek().is_some() => {
                if !meta.is_dir() || meta.file_type().is_symlink() || reparse(&meta) {
                    return Err(fail(
                        "pack_invalid_path",
                        format!("{relative}: a parent directory is redirected or occupied."),
                    ));
                }
            }
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) {
                    return Err(fail(
                        "pack_invalid_path",
                        format!("{relative}: the destination is not a regular file."),
                    ));
                }
                return Ok(Some(target));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => {
                return Err(fail(
                    "pack_io_error",
                    format!("{relative}: the destination could not be inspected."),
                ));
            }
        }
    }
    Ok(None)
}

/// Everything a preview reports besides the pack itself.
#[derive(Default)]
struct Findings {
    added: Vec<String>,
    already_present: Vec<String>,
    kept_existing: Vec<String>,
    overrides_written: Vec<String>,
    untouched_mods: Vec<String>,
    component_count: usize,
    blockers: Vec<String>,
}

fn finish(
    resolved: &Resolved,
    instance: &InstanceId,
    installed: &InstalledConfiguration,
    mut findings: Findings,
) -> Result<PackMergePreview, PackInstallError> {
    // Pack files already present are reported as such, not as untouched.
    findings.untouched_mods.retain(|name| {
        !findings
            .already_present
            .iter()
            .any(|path| path.eq_ignore_ascii_case(&format!("mods/{name}")))
    });
    for list in [
        &mut findings.added,
        &mut findings.already_present,
        &mut findings.kept_existing,
        &mut findings.overrides_written,
        &mut findings.untouched_mods,
        &mut findings.blockers,
    ] {
        list.sort();
    }
    let mut preview = PackMergePreview {
        pack: resolved.preview.clone(),
        instance_id: instance.to_string(),
        instance_minecraft_version: installed.minecraft_version.clone(),
        instance_loader: installed.platform.kind(),
        instance_loader_version: installed.platform.version().map(str::to_owned),
        added: findings.added,
        already_present: findings.already_present,
        kept_existing: findings.kept_existing,
        overrides_written: findings.overrides_written,
        untouched_mods: findings.untouched_mods,
        component_count: findings.component_count,
        component_limit: crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION,
        blockers: findings.blockers,
        fingerprint: String::new(),
    };
    preview.fingerprint = fingerprint_of(&preview)?;
    Ok(preview)
}

async fn plan(
    managed: &ManagedPaths,
    instance: &InstanceId,
    resolved: &Resolved,
) -> Result<MergePlan, PackInstallError> {
    let registry = InstanceRegistry::load(&managed.instance_registry_file())
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let record = registry
        .find(instance)
        .ok_or_else(|| fail("instance_not_found", "The instance no longer exists."))?;
    let pack = &resolved.pack;
    let installed = record.installed();
    let mut blockers = Vec::new();
    if record.state() != InstanceState::Ready {
        blockers.push(NOT_READY.to_owned());
    }
    if record.pack().is_some() {
        blockers.push(PACK_MANAGED.to_owned());
    }
    // Resolution already refused an incompatible pack; the registry is read
    // again here, so the same rule is the authority for this plan.
    blockers.extend(compatibility_refusals(installed, pack));
    let blocked = |findings: Findings| -> Result<MergePlan, PackInstallError> {
        Ok(MergePlan {
            preview: finish(resolved, instance, installed, findings)?,
            provider_plans: Vec::new(),
            externals: Vec::new(),
            overrides: Vec::new(),
        })
    };
    if !blockers.is_empty() {
        // An incompatible or unfinished instance is reported as such; its
        // content is not examined.
        return blocked(Findings {
            blockers,
            ..Findings::default()
        });
    }
    let fabric = matches!(installed.platform, PlatformPin::Fabric { .. });
    let loader_name = pack.loader.kind.display_name();

    let root = managed.instance_paths(instance).root().to_path_buf();
    let state = ContentState::load(managed, instance)
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let untouched_mods = existing_mod_files(managed, instance)?;
    // Components not already installed (by project) must be added or block
    // the merge; a Fabric instance's own Fabric API may absorb the pack's
    // copy. Over the per-operation bound, refuse before any component
    // download.
    let pending = resolved
        .provider_plans
        .iter()
        .filter(|candidate| {
            !(fabric && candidate.project_id == FABRIC_API_PROJECT)
                && !state.entries.iter().any(|entry| {
                    entry.content_type == candidate.content_type
                        && entry.provider == candidate.provider
                        && entry.project_id == candidate.project_id
                })
        })
        .count();
    if pending > crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION {
        return blocked(Findings {
            untouched_mods,
            component_count: pending,
            blockers: vec![crate::modpacks::component_limit_message(pending, true)],
            ..Findings::default()
        });
    }
    let mut added = Vec::new();
    let mut already_present = Vec::new();
    let mut kept_existing = Vec::new();
    let mut overrides_written = Vec::new();
    let mut provider_plans = Vec::new();
    let mut seen = HashSet::new();

    for candidate in &resolved.provider_plans {
        let path = format!(
            "{}/{}",
            candidate.content_type.directory_name(),
            candidate.file_name
        );
        if !seen.insert(path.to_lowercase()) {
            blockers.push(format!("{path}: the pack lists this destination twice."));
            continue;
        }
        if let Some(existing) = state.entries.iter().find(|entry| {
            entry.content_type == candidate.content_type
                && entry.provider == candidate.provider
                && entry.project_id == candidate.project_id
        }) {
            if existing.version_id == candidate.version_id {
                already_present.push(path);
            } else {
                blockers.push(format!(
                    "{}: a different version of this project is already installed ({}). Remove or update it first.",
                    candidate.file_name, existing.file_name
                ));
            }
            continue;
        }
        let conflicts = match crate::instance_content::preview_provider_conflicts(
            managed,
            instance,
            std::slice::from_ref(candidate),
        )
        .await
        {
            Ok(conflicts) => conflicts,
            // A pack may author a component Modrinth doesn't list for this exact
            // game/loader. A new pack instance trusts the pack's composition; an
            // existing instance keeps its own compatibility rule, so say why.
            Err(crate::instance_content::ContentError::UnsupportedAction) => {
                blockers.push(format!(
                    "{}: Modrinth doesn't list this file for Minecraft {} with {loader_name}, so it can't be added to an existing instance.",
                    candidate.file_name, installed.minecraft_version
                ));
                continue;
            }
            Err(error) => return Err(fail("pack_merge_unavailable", error.to_string())),
        };
        if conflicts.is_empty() {
            added.push(path);
            provider_plans.push(candidate.clone());
        } else if fabric
            && candidate.project_id == FABRIC_API_PROJECT
            && conflicts.iter().all(|conflict| {
                matches!(
                    conflict.ownership,
                    ModOwnership::LauncherBootstrap
                        | ModOwnership::LauncherManagedRequired
                        | ModOwnership::LauncherManagedRetained
                )
            })
        {
            // The instance's own protected Fabric API stays authoritative.
            already_present.push(path);
        } else {
            for conflict in conflicts {
                blockers.push(format!("{}: {}", candidate.file_name, conflict.reason));
            }
        }
    }

    let mut externals = Vec::new();
    for file in &resolved.unresolved {
        if !seen.insert(file.path.to_lowercase()) {
            blockers.push(format!(
                "{}: the pack lists this destination twice.",
                file.path
            ));
            continue;
        }
        match existing_destination(&root, &file.path) {
            Ok(None) => {
                added.push(file.path.clone());
                externals.push(file.clone());
            }
            Ok(Some(target)) => {
                let identical = Sha1Digest::parse(&file.sha1).is_ok_and(|sha1| {
                    verify_file_sha1(&target, &sha1, Some(file.file_size)).is_ok()
                });
                if identical {
                    already_present.push(file.path.clone());
                } else {
                    blockers.push(format!(
                        "{}: a different file already exists here and will not be overwritten.",
                        file.path
                    ));
                }
            }
            Err(error) => blockers.push(error.message),
        }
    }

    let component_count = provider_plans.len();
    if component_count > crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION {
        blockers.push(crate::modpacks::component_limit_message(
            component_count,
            true,
        ));
    }
    // The per-component check above compares each mod with what is installed.
    // The pack's own mods must also agree with each other (and external jars
    // with the instance), or install would fail after review said it is fine.
    if blockers.is_empty() {
        blockers
            .extend(batch_identity_blockers(managed, instance, &provider_plans, &externals).await?);
    }
    if blockers.is_empty() {
        // Exactly the dependency/compatibility validation install will run.
        if let Some(problem) = crate::instance_content::projected_merge_problem(
            managed,
            instance,
            provider_plans.clone(),
        )
        .await
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?
        {
            blockers.push(problem);
        }
    }

    let mut overrides = Vec::new();
    for item in &pack.overrides {
        if !seen.insert(item.path.to_lowercase()) {
            // A component already claims this destination; keep the component.
            continue;
        }
        match existing_destination(&root, &item.path) {
            Ok(None) => {
                overrides_written.push(item.path.clone());
                overrides.push(item.clone());
            }
            Ok(Some(target)) => {
                let identical = ArtifactDigest::parse(&item.sha256)
                    .is_ok_and(|digest| verify_file(&target, &digest, Some(item.size)).is_ok());
                if identical {
                    already_present.push(item.path.clone());
                } else if item.path.starts_with("mods/") {
                    blockers.push(format!(
                        "{}: a different mod file already exists here and will not be overwritten.",
                        item.path
                    ));
                } else {
                    kept_existing.push(item.path.clone());
                }
            }
            Err(error) => blockers.push(error.message),
        }
    }

    let preview = finish(
        resolved,
        instance,
        installed,
        Findings {
            added,
            already_present,
            kept_existing,
            overrides_written,
            untouched_mods,
            component_count,
            blockers,
        },
    )?;
    Ok(MergePlan {
        preview,
        provider_plans,
        externals,
        overrides,
    })
}

/// Mod-identity agreement across the batch, using the same rule activation
/// enforces (`conflicting_mod_identity`). Artifacts come from the verified
/// cache (already populated by the per-component checks); nothing is written.
async fn batch_identity_blockers(
    managed: &ManagedPaths,
    instance: &InstanceId,
    provider_plans: &[ProviderInstallPlan],
    externals: &[PackFile],
) -> Result<Vec<String>, PackInstallError> {
    let cache = ArtifactCache::new(managed.clone());
    let kind = crate::instance_mods::platform_kind_of(managed, instance);
    let mut batch: Vec<(String, crate::instance_mods::ModMetadata)> = Vec::new();
    let mod_plans: Vec<&ProviderInstallPlan> = provider_plans
        .iter()
        .filter(|plan| plan.content_type == crate::instance_content::ContentType::Mod)
        .collect();
    let mod_artifacts = crate::modpacks::acquire_provider_artifacts(&cache, &mod_plans).await?;
    for (plan, artifact) in mod_plans.iter().zip(&mod_artifacts) {
        if let (Some(metadata), _) =
            crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, &kind)
        {
            batch.push((plan.file_name.clone(), metadata));
        }
    }
    let mut external_metadata = Vec::new();
    let external_mods: Vec<&PackFile> = externals
        .iter()
        .filter(|file| file.path.starts_with("mods/"))
        .collect();
    let external_artifacts = crate::modpacks::acquire_externals(&cache, &external_mods).await?;
    for (file, artifact) in external_mods.iter().zip(&external_artifacts) {
        if let (Some(metadata), _) =
            crate::instance_mods::inspect_mod_metadata(&artifact.path, artifact.bytes, &kind)
        {
            external_metadata.push((file.path.clone(), metadata));
        }
    }
    let mut blockers = Vec::new();
    // External jars against what is installed now.
    if !external_metadata.is_empty() {
        let inventory = crate::instance_mods::scan(managed, instance)
            .map_err(|e| fail("pack_merge_unavailable", e.to_string()))?;
        for (path, incoming) in &external_metadata {
            for entry in &inventory.entries {
                if let Some(existing) = &entry.metadata {
                    if let Some(id) =
                        crate::instance_content::conflicting_mod_identity(incoming, existing)
                    {
                        blockers.push(format!(
                            "{path}: mod {id} would duplicate {} already in this instance.",
                            entry.file_name
                        ));
                    }
                }
            }
        }
    }
    batch.extend(external_metadata);
    for (index, (name, incoming)) in batch.iter().enumerate() {
        for (other_name, existing) in batch.iter().skip(index + 1) {
            if let Some(id) = crate::instance_content::conflicting_mod_identity(incoming, existing)
                .or_else(|| crate::instance_content::conflicting_mod_identity(existing, incoming))
            {
                blockers.push(format!(
                    "{name} and {other_name} both include mod {id} at different versions. Aurora won't install two copies of one mod."
                ));
            }
        }
    }
    Ok(blockers)
}

/// Fingerprint over the whole preview (computed with an empty fingerprint
/// field), so install can prove it is applying exactly what was reviewed.
fn fingerprint_of(preview: &PackMergePreview) -> Result<String, PackInstallError> {
    let bytes = serde_json::to_vec(preview).map_err(|_| {
        fail(
            "pack_plan_invalid",
            "The merge preview could not be encoded.",
        )
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub async fn preview(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
) -> Result<PackMergePreview, PackInstallError> {
    let resolved = resolve_for(
        managed,
        provider,
        instance,
        project_id,
        version_id,
        selected_optional,
    )
    .await?;
    let mut preview = plan(managed, instance, &resolved).await?.preview;
    // Only review reports this: install holds the launch reservation itself
    // and refuses outright when it cannot take it.
    if launch_active(instance) {
        preview.blockers.push(INSTANCE_BUSY.to_owned());
        preview.blockers.sort();
        preview.fingerprint = String::new();
        preview.fingerprint = fingerprint_of(&preview)?;
    }
    Ok(preview)
}

fn merges_in_progress() -> &'static Mutex<HashSet<String>> {
    static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashSet::new()))
}

struct MergeGuard(String);
impl Drop for MergeGuard {
    fn drop(&mut self) {
        merges_in_progress()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.0);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn install(
    managed: &ManagedPaths,
    provider: &crate::modrinth::Client,
    instance: &InstanceId,
    project_id: &str,
    version_id: &str,
    selected_optional: &[String],
    fingerprint: &str,
    progress: &mut (dyn FnMut(&'static str) + Send),
) -> Result<PackMergeResult, PackInstallError> {
    if !merges_in_progress()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(instance.to_string())
    {
        return Err(fail(
            "pack_merge_in_progress",
            "A modpack is already being added to this instance.",
        ));
    }
    let _guard = MergeGuard(instance.to_string());
    // Launch takes this same per-instance Play reservation before it
    // prepares or spawns, and it is refused while Minecraft is starting or
    // running. Holding it for the whole merge (it may span awaits) keeps a
    // launch from starting while files change; taking it before any
    // download gives a fast, honest refusal.
    let _play = crate::launch::process::PreparationGuard::acquire(instance.as_str())
        .map_err(|_| instance_busy())?;

    progress(crate::modpacks::progress_phase::RESOLVING_PACK);
    let resolved = resolve_for(
        managed,
        provider,
        instance,
        project_id,
        version_id,
        selected_optional,
    )
    .await?;
    let revision = crate::instance_content::local_inventory_revision(managed, instance)
        .map_err(|error| fail("pack_merge_unavailable", error.to_string()))?;
    let merge = plan(managed, instance, &resolved).await?;
    if merge.preview.fingerprint != fingerprint {
        return Err(fail(
            "pack_preview_stale",
            "This instance or the pack changed since review. Review it again before adding it.",
        ));
    }
    if let Some(blocker) = merge.preview.blockers.first() {
        return Err(fail("pack_merge_blocked", blocker.clone()));
    }

    progress(crate::modpacks::progress_phase::DOWNLOADING);
    let cache = ArtifactCache::new(managed.clone());
    let external_refs: Vec<&PackFile> = merge.externals.iter().collect();
    let external: Vec<(PackFile, VerifiedArtifact)> = merge
        .externals
        .iter()
        .cloned()
        .zip(crate::modpacks::acquire_externals(&cache, &external_refs).await?)
        .collect();

    if crate::launch::process::snapshot(instance.as_str())
        .status
        .blocks_launch()
    {
        return Err(instance_busy());
    }
    let _commit = crate::content_jobs::commit().map_err(|_| {
        fail(
            "installation_cancelled",
            "Installation cancelled before activation.",
        )
    })?;
    progress(crate::modpacks::progress_phase::INSTALLING_COMPONENTS);
    let records: Vec<ProviderRecord> = if merge.provider_plans.is_empty() {
        Vec::new()
    } else {
        crate::instance_content::install_merged_provider_plans(
            managed,
            instance,
            merge.provider_plans,
            &revision,
        )
        .await
        .map_err(|error| fail("pack_component_install_failed", error.to_string()))?
    };

    progress(crate::modpacks::progress_phase::INSTALLING_OVERRIDES);
    let root = managed.instance_paths(instance).root().to_path_buf();
    let mut created: Vec<(PathBuf, String)> = Vec::new();
    let written = (|| -> Result<(), PackInstallError> {
        // Synchronous writes: no spawn/starting transition can interleave.
        // Released before any rollback runs.
        let _process =
            crate::launch::process::lock_stopped(instance.as_str()).map_err(|_| instance_busy())?;
        for (file, artifact) in &external {
            #[cfg(test)]
            crate::modpacks::materialize_fault::check(created.len())?;
            let digest = artifact.sha256.as_hex();
            let target = crate::modpacks::materialize(&root, &file.path, &artifact.path, &digest)?;
            created.push((target, digest));
        }
        for item in &merge.overrides {
            #[cfg(test)]
            crate::modpacks::materialize_fault::check(created.len())?;
            let bytes = crate::mrpack::read_override(&resolved.archive.path, item)
                .map_err(|e| fail(e.code(), e.to_string()))?;
            let target =
                crate::modpacks::materialize_bytes(&root, &item.path, &bytes, &item.sha256)?;
            created.push((target, item.sha256.clone()));
        }
        Ok(())
    })();
    if let Err(error) = written {
        return Err(rollback(
            managed, instance, &root, &created, &records, error,
        ));
    }

    progress(crate::modpacks::progress_phase::COMPLETE);
    Ok(PackMergeResult {
        instance_id: instance.to_string(),
        added: merge.preview.added.len(),
        already_present: merge.preview.already_present.len(),
        overrides_written: merge.preview.overrides_written.len(),
        kept_existing: merge.preview.kept_existing,
    })
}

/// Remove only what this merge created, each proven by its digest. The
/// instance record, game tree and all pre-existing files are never touched.
fn rollback(
    managed: &ManagedPaths,
    instance: &InstanceId,
    root: &Path,
    created: &[(PathBuf, String)],
    records: &[ProviderRecord],
    original: PackInstallError,
) -> PackInstallError {
    let mut clean = true;
    for (path, digest) in created.iter().rev() {
        let proven = path.starts_with(root)
            && path != root
            && ArtifactDigest::parse(digest)
                .is_ok_and(|expected| verify_file(path, &expected, None).is_ok());
        if !proven || std::fs::remove_file(path).is_err() {
            clean = false;
        }
    }
    if !records.is_empty()
        && crate::instance_content::remove_merged_provider_records(managed, instance, records)
            .is_err()
    {
        clean = false;
    }
    if clean {
        original
    } else {
        fail(
            "pack_merge_rollback_incomplete",
            format!(
                "{} Some files this merge added could not be removed safely; review the instance's Mods tab.",
                original.message
            ),
        )
    }
}

#[cfg(windows)]
fn reparse(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_comparison_accepts_equal_and_newer_only() {
        assert!(version_at_least("0.19.5", "0.19.5"));
        assert!(version_at_least("0.19.5", "0.16.14"));
        assert!(version_at_least("0.19.10", "0.19.9"));
        assert!(!version_at_least("0.16.14", "0.19.5"));
        assert!(!version_at_least("0.19", "0.19.1"));
        assert!(version_at_least("1.0", "0.99.99"));
    }

    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use sha2::Sha512;
    use std::io::{Cursor, Write};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MergeFixture {
        root: PathBuf,
        managed: ManagedPaths,
        instance: InstanceId,
        server: TestServer,
        requests: Arc<AtomicUsize>,
    }

    impl Drop for MergeFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A Ready Fabric instance and a loopback pack whose only content is one
    /// configuration override, so a merge needs no component downloads.
    fn merge_fixture(label: &str) -> MergeFixture {
        use crate::instances::platform::InstalledConfiguration;
        use crate::instances::settings::InstanceConfiguration;
        let root = std::env::temp_dir().join(format!("aurora-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap();
        let mut config = InstanceConfiguration::for_minecraft_version("1.21.1");
        config.set_aurora_enabled(false);
        let mut registry = InstanceRegistry::empty();
        registry.instances_mut().push(
            crate::instances::InstanceRecord::from_installed(
                instance.clone(),
                "Merge target",
                InstanceState::Ready,
                InstalledConfiguration {
                    minecraft_version: "1.21.1".into(),
                    platform: PlatformPin::Fabric {
                        version: "0.19.5".into(),
                    },
                    aurora: None,
                },
                config,
            )
            .unwrap(),
        );
        std::fs::create_dir_all(managed.launcher_dir()).unwrap();
        registry.save(&managed.instance_registry_file()).unwrap();
        std::fs::create_dir_all(managed.instance_paths(&instance).root()).unwrap();

        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("modrinth.index.json", options).unwrap();
            zip.write_all(
                serde_json::json!({
                    "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Merge Pack",
                    "files": [], "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.0"}
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();
            zip.start_file("overrides/config/merge.toml", options)
                .unwrap();
            zip.write_all(b"merged = true").unwrap();
            zip.finish().unwrap();
        }
        let bytes = buffer.into_inner();
        let requests = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&requests);
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            counted.fetch_add(1, Ordering::SeqCst);
            match request.path.split('?').next().unwrap_or_default() {
                "/pack.mrpack" => TestResponse::ok(&bytes),
                "/v2/project/PACK0001" => TestResponse::ok(serde_json::json!({
                    "id": "PACK0001", "project_type": "modpack", "title": "Merge Pack", "description": "Fixture",
                    "license": {"id":"mit"}, "game_versions": ["1.21.1"], "loaders": ["fabric"], "environment": ["client_only"]
                }).to_string().as_bytes()),
                "/v2/version/VERS0001" => TestResponse::ok(serde_json::json!({
                    "id": "VERS0001", "project_id": "PACK0001", "name": "One", "version_number": "1",
                    "version_type": "release", "date_published": "2026-01-01T00:00:00Z", "game_versions": ["1.21.1"],
                    "loaders": ["fabric"], "environment": "client_only", "dependencies": [],
                    "files": [{"hashes": {"sha512": format!("{:x}", Sha512::digest(&bytes))}, "url": format!("{}/pack.mrpack", request.base_url),
                        "filename": "pack.mrpack", "primary": true, "size": bytes.len()}]
                }).to_string().as_bytes()),
                _ => TestResponse::status(404),
            }
        }));
        MergeFixture {
            root,
            managed,
            instance,
            server,
            requests,
        }
    }

    #[tokio::test]
    async fn destinations_mark_each_instance_with_a_reason_and_download_nothing() {
        use crate::instances::platform::InstalledConfiguration;
        use crate::instances::settings::InstanceConfiguration;
        let fixture = merge_fixture("destinations");
        let path = fixture.managed.instance_registry_file();
        let mut registry = InstanceRegistry::load(&path).unwrap();
        let record = |name: &str, state, minecraft: &str, platform| {
            let mut config = InstanceConfiguration::for_minecraft_version(minecraft);
            config.set_aurora_enabled(false);
            crate::instances::InstanceRecord::from_installed(
                InstanceId::new(uuid::Uuid::new_v4().simple().to_string()).unwrap(),
                name,
                state,
                InstalledConfiguration {
                    minecraft_version: minecraft.into(),
                    platform,
                    aurora: None,
                },
                config,
            )
            .unwrap()
        };
        let fabric = || PlatformPin::Fabric {
            version: "0.19.5".into(),
        };
        let mut managed_pack = record("Pack managed", InstanceState::Ready, "1.21.1", fabric());
        managed_pack
            .set_pack(crate::instances::PackRegistryIdentity {
                provider: "modrinth".into(),
                project_id: "OTHERPCK".into(),
                version_id: "OTHERVER".into(),
                name: "Other".into(),
                pack_version: "1".into(),
            })
            .unwrap();
        registry.instances_mut().extend([
            record(
                "Forge one",
                InstanceState::Ready,
                "1.21.1",
                PlatformPin::Forge {
                    version: "1.21.1-52.0.1".into(),
                },
            ),
            record("Old Fabric", InstanceState::Ready, "1.20.1", fabric()),
            record("Installing", InstanceState::Installing, "1.21.1", fabric()),
            managed_pack,
            record(
                "Vanilla",
                InstanceState::Ready,
                "1.21.1",
                PlatformPin::Vanilla {},
            ),
        ]);
        registry.save(&path).unwrap();
        let client =
            crate::modrinth::Client::for_testing(&format!("{}/v2/", fixture.server.base_url()));

        let listed = destinations(&fixture.managed, &client, "PACK0001", "VERS0001")
            .await
            .unwrap();
        let find = |name: &str| listed.iter().find(|d| d.display_name == name).unwrap();
        let target = find("Merge target");
        assert!(target.compatible && target.reasons.is_empty());
        assert_eq!(
            (target.loader, target.loader_version.as_deref()),
            ("fabric", Some("0.19.5"))
        );
        assert_eq!(target.minecraft_version, "1.21.1");
        for (name, expected) in [
            ("Forge one", "this instance uses Forge"),
            ("Old Fabric", "this instance has Minecraft 1.20.1"),
            ("Installing", NOT_READY),
            ("Pack managed", PACK_MANAGED),
            ("Vanilla", "only to a Fabric, Forge or NeoForge instance"),
        ] {
            let destination = find(name);
            assert!(!destination.compatible, "{name}");
            assert!(
                destination.reasons.iter().any(|r| r.contains(expected)),
                "{name}: {:?}",
                destination.reasons
            );
        }
        assert_eq!(listed.len(), 6);
        // Project and version metadata only: the archive is never requested.
        assert_eq!(fixture.requests.load(Ordering::SeqCst), 2);
    }

    async fn merge(
        fixture: &MergeFixture,
        fingerprint: &str,
    ) -> Result<PackMergeResult, PackInstallError> {
        let client =
            crate::modrinth::Client::for_testing(&format!("{}/v2/", fixture.server.base_url()));
        install(
            &fixture.managed,
            &client,
            &fixture.instance,
            "PACK0001",
            "VERS0001",
            &[],
            fingerprint,
            &mut |_| {},
        )
        .await
    }

    async fn review(fixture: &MergeFixture) -> PackMergePreview {
        let client =
            crate::modrinth::Client::for_testing(&format!("{}/v2/", fixture.server.base_url()));
        preview(
            &fixture.managed,
            &client,
            &fixture.instance,
            "PACK0001",
            "VERS0001",
            &[],
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn a_merge_is_refused_while_the_instance_runs_or_prepares_to_launch() {
        let fixture = merge_fixture("merge-running");
        let target = fixture
            .managed
            .instance_paths(&fixture.instance)
            .root()
            .join("config/merge.toml");
        let clean = review(&fixture).await;
        assert!(clean.blockers.is_empty(), "{:?}", clean.blockers);
        assert_eq!(clean.overrides_written, ["config/merge.toml"]);

        let running = crate::launch::process::force_running(fixture.instance.as_str());
        let blocked = review(&fixture).await;
        assert_eq!(blocked.blockers, [INSTANCE_BUSY]);
        assert_ne!(blocked.fingerprint, clean.fingerprint);
        let before = fixture.requests.load(Ordering::SeqCst);
        let error = merge(&fixture, &clean.fingerprint).await.unwrap_err();
        assert_eq!(error.code, "pack_merge_instance_busy");
        // Refused before resolving: no metadata or archive request.
        assert_eq!(fixture.requests.load(Ordering::SeqCst), before);
        drop(running);

        let preparing =
            crate::launch::process::PreparationGuard::acquire(fixture.instance.as_str()).unwrap();
        assert_eq!(review(&fixture).await.blockers, [INSTANCE_BUSY]);
        let before = fixture.requests.load(Ordering::SeqCst);
        let error = merge(&fixture, &clean.fingerprint).await.unwrap_err();
        assert_eq!(error.code, "pack_merge_instance_busy");
        assert_eq!(fixture.requests.load(Ordering::SeqCst), before);
        drop(preparing);
        assert!(!target.exists());

        // Once the instance is idle the reviewed merge applies, and launch
        // can reserve the instance again afterwards.
        let result = merge(&fixture, &clean.fingerprint).await.unwrap();
        assert_eq!(result.overrides_written, 1);
        assert_eq!(std::fs::read(&target).unwrap(), b"merged = true");
        assert!(
            crate::launch::process::PreparationGuard::acquire(fixture.instance.as_str()).is_ok()
        );
    }

    #[tokio::test]
    async fn a_launch_cannot_be_prepared_while_a_merge_is_in_flight() {
        let fixture = merge_fixture("merge-holds-launch");
        let clean = review(&fixture).await;
        let instance = fixture.instance.clone();
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let client =
            crate::modrinth::Client::for_testing(&format!("{}/v2/", fixture.server.base_url()));
        install(
            &fixture.managed,
            &client,
            &fixture.instance,
            "PACK0001",
            "VERS0001",
            &[],
            &clean.fingerprint,
            &mut |phase| {
                if phase != crate::modpacks::progress_phase::COMPLETE {
                    seen.lock().unwrap().push(
                        crate::launch::process::PreparationGuard::acquire(instance.as_str())
                            .is_err(),
                    );
                }
            },
        )
        .await
        .unwrap();
        let observed = observed.lock().unwrap();
        assert!(!observed.is_empty());
        assert!(observed.iter().all(|refused| *refused), "{observed:?}");
    }

    #[test]
    fn destination_lookup_is_read_only_and_rejects_occupied_parents() {
        let root = std::env::temp_dir().join(format!("aurora-merge-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        // Missing parent: reported absent and never created.
        assert!(
            existing_destination(&root, "config/example.json")
                .unwrap()
                .is_none()
        );
        assert!(!root.join("config").exists());
        std::fs::write(root.join("options.txt"), b"user").unwrap();
        assert_eq!(
            existing_destination(&root, "options.txt").unwrap(),
            Some(root.join("options.txt"))
        );
        std::fs::write(root.join("config"), b"not a directory").unwrap();
        assert!(existing_destination(&root, "config/example.json").is_err());
        assert!(existing_destination(&root, "../escape.txt").is_err());
        std::fs::remove_file(root.join("options.txt")).unwrap();
        std::fs::remove_file(root.join("config")).unwrap();
        std::fs::remove_dir(&root).unwrap();
    }

    // ------------------------------------------------------------------
    // End-to-end merges into Ready Fabric, Forge and NeoForge instances
    // (loopback only; the Phase 2 pack/loader world).
    // ------------------------------------------------------------------

    use crate::instance_content::ProviderOrigin;
    use crate::instances::settings::{
        DEFAULT_MEMORY_MIB, InstanceConfiguration, LoaderConfiguration, LoaderPolicy,
    };
    use crate::modpacks::tests::{
        ComponentFixture, FABRIC_LOADER, FORGE_BUILD, LOADER_MC, LoaderWorld, NEOFORGE_VERSION,
        dependencies_for, loader_mod_jar, tag_for,
    };
    use std::collections::BTreeMap;

    const KINDS: [PackLoaderKind; 3] = [
        PackLoaderKind::Fabric,
        PackLoaderKind::Forge,
        PackLoaderKind::NeoForge,
    ];

    /// The exact loader pin a fresh instance of `kind` has in this world.
    fn instance_pin(kind: PackLoaderKind) -> String {
        match kind {
            PackLoaderKind::Fabric => FABRIC_LOADER.to_owned(),
            PackLoaderKind::Forge => format!("{LOADER_MC}-{FORGE_BUILD}"),
            PackLoaderKind::NeoForge => NEOFORGE_VERSION.to_owned(),
        }
    }

    /// A Ready, non-pack instance of `kind` created through the ordinary
    /// validated instance installer.
    async fn ready_instance(world: &LoaderWorld, kind: PackLoaderKind) -> InstanceId {
        let policy = LoaderPolicy::Pinned {
            version: instance_pin(kind),
        };
        let loader = match kind {
            PackLoaderKind::Fabric => LoaderConfiguration::Fabric { policy },
            PackLoaderKind::Forge => LoaderConfiguration::Forge { policy },
            PackLoaderKind::NeoForge => LoaderConfiguration::NeoForge { policy },
        };
        let mut configuration = InstanceConfiguration::from_parts(
            LOADER_MC,
            loader,
            DEFAULT_MEMORY_MIB,
            String::new(),
            None,
        );
        configuration.set_aurora_enabled(false);
        let record = crate::instances::lifecycle::create_instance(
            &world.managed,
            &world.managed.instance_registry_file(),
            &world.managed.config_file(),
            &world.endpoints,
            crate::instances::lifecycle::CreateInstanceRequest::new("Merge target", configuration),
            &mut |_| {},
            crate::instances::lifecycle::InstanceFaults::default(),
        )
        .await
        .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
        assert_eq!(record.state(), InstanceState::Ready);
        record.id().clone()
    }

    fn instance_root(world: &LoaderWorld, instance: &InstanceId) -> PathBuf {
        world.managed.instance_paths(instance).root().to_path_buf()
    }

    async fn review_in(
        world: &LoaderWorld,
        instance: &InstanceId,
        version_id: &str,
    ) -> Result<PackMergePreview, PackInstallError> {
        preview(
            &world.managed,
            &world.client,
            instance,
            "PACK0001",
            version_id,
            &[],
        )
        .await
    }

    async fn merge_in(
        world: &LoaderWorld,
        instance: &InstanceId,
        version_id: &str,
        fingerprint: &str,
    ) -> Result<PackMergeResult, PackInstallError> {
        install(
            &world.managed,
            &world.client,
            instance,
            "PACK0001",
            version_id,
            &[],
            fingerprint,
            &mut |_| {},
        )
        .await
    }

    /// Every file under the instance root with its bytes.
    fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    let relative = path.strip_prefix(root).unwrap().to_string_lossy();
                    out.insert(relative.replace('\\', "/"), std::fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// Rewrites the published Modrinth version document of `version_id`.
    fn edit_version(
        world: &LoaderWorld,
        version_id: &str,
        edit: impl FnOnce(&mut serde_json::Value),
    ) {
        let path = format!("/v2/version/{version_id}");
        let mut document: serde_json::Value = serde_json::from_slice(&world.take(&path)).unwrap();
        edit(&mut document);
        world.insert(path, document.to_string());
    }

    fn assert_ready_and_valid(world: &LoaderWorld, instance: &InstanceId, kind: PackLoaderKind) {
        let registry = InstanceRegistry::load(&world.managed.instance_registry_file()).unwrap();
        let record = registry.find(instance).unwrap();
        assert_eq!(record.state(), InstanceState::Ready, "{kind:?}");
        assert!(
            record.pack().is_none(),
            "{kind:?}: a merge never adopts a pack"
        );
        assert_eq!(
            record.installed().platform.version(),
            Some(instance_pin(kind).as_str())
        );
        let validation =
            crate::instances::lifecycle::validate_instance(&world.managed, &registry, instance)
                .unwrap();
        assert_eq!(
            validation.status,
            crate::instances::lifecycle::InstanceStatus::Ready,
            "{kind:?}: {:?}",
            validation.problems
        );
    }

    const USER_CONFIG: &[u8] = b"user = true\n";

    /// Two components, one external mod jar, an override the user already
    /// has a differing copy of, and a new override.
    fn publish_merge_pack(
        world: &LoaderWorld,
        version_id: &str,
        kind: PackLoaderKind,
    ) -> Vec<ComponentFixture> {
        let components = vec![
            ComponentFixture::new(1, kind, &[tag_for(kind)]),
            ComponentFixture::new(2, kind, &[tag_for(kind)]),
        ];
        world.publish(
            version_id,
            &[tag_for(kind)],
            dependencies_for(kind),
            &components,
            &[(
                "mods/external.jar",
                loader_mod_jar(kind, "external_mod", None),
            )],
            &[
                ("config/pack.toml", b"pack = true\n"),
                ("config/new.toml", b"new = true\n"),
            ],
        );
        components
    }

    #[tokio::test]
    async fn a_pack_merges_into_ready_fabric_forge_and_neoforge_instances_leaving_user_files_untouched()
     {
        for kind in KINDS {
            let world = LoaderWorld::new(&format!("merge-ok-{}", tag_for(kind)));
            let instance = ready_instance(&world, kind).await;
            let root = instance_root(&world, &instance);
            let user_mod = loader_mod_jar(kind, "user_mod", None);
            write(&root, "mods/user.jar", &user_mod);
            write(&root, "config/pack.toml", USER_CONFIG);
            let components = publish_merge_pack(&world, "MERGE001", kind);

            let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
            assert!(
                reviewed.blockers.is_empty(),
                "{kind:?}: {:?}",
                reviewed.blockers
            );
            assert_eq!(
                reviewed.instance_loader,
                match kind {
                    PackLoaderKind::Fabric => "fabric",
                    PackLoaderKind::Forge => "forge",
                    PackLoaderKind::NeoForge => "neoForge",
                }
            );
            assert_eq!(
                reviewed.instance_loader_version.as_deref(),
                Some(instance_pin(kind).as_str())
            );
            assert_eq!(reviewed.instance_minecraft_version, LOADER_MC);
            assert_eq!(
                reviewed.added,
                [
                    "mods/component-1.jar",
                    "mods/component-2.jar",
                    "mods/external.jar"
                ]
            );
            assert_eq!(reviewed.overrides_written, ["config/new.toml"]);
            assert_eq!(reviewed.kept_existing, ["config/pack.toml"]);
            assert_eq!(reviewed.untouched_mods, ["user.jar"]);
            assert!(reviewed.already_present.is_empty());
            assert_eq!(reviewed.component_count, 2);
            assert_eq!(
                reviewed.component_limit,
                crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION
            );
            assert_eq!(reviewed.component_limit, 64);

            let result = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
                .await
                .unwrap_or_else(|error| panic!("{kind:?}: {} ({})", error.message, error.code));
            assert_eq!(result.added, 3);
            assert_eq!(result.overrides_written, 1);
            assert_eq!(result.kept_existing, ["config/pack.toml"]);
            for component in &components {
                assert_eq!(
                    std::fs::read(root.join("mods").join(&component.file_name)).unwrap(),
                    component.bytes
                );
            }
            assert_eq!(
                std::fs::read(root.join("mods/external.jar")).unwrap(),
                loader_mod_jar(kind, "external_mod", None)
            );
            assert_eq!(
                std::fs::read(root.join("config/new.toml")).unwrap(),
                b"new = true\n"
            );
            // The user's mod and differing configuration are byte-for-byte intact.
            assert_eq!(std::fs::read(root.join("mods/user.jar")).unwrap(), user_mod);
            assert_eq!(
                std::fs::read(root.join("config/pack.toml")).unwrap(),
                USER_CONFIG
            );

            let state = ContentState::load(&world.managed, &instance).unwrap();
            assert_eq!(state.entries.len(), 2, "{kind:?}");
            for entry in &state.entries {
                assert_eq!(entry.origin, ProviderOrigin::Direct);
                assert!(entry.explicitly_retained);
                assert_eq!(entry.compatibility.loader.as_deref(), Some(tag_for(kind)));
            }
            assert_ready_and_valid(&world, &instance, kind);

            // Reviewing again finds everything present and adds nothing.
            let again = review_in(&world, &instance, "MERGE001").await.unwrap();
            assert!(again.blockers.is_empty(), "{:?}", again.blockers);
            assert!(again.added.is_empty() && again.overrides_written.is_empty());
            assert_eq!(
                again.already_present,
                [
                    "config/new.toml",
                    "mods/component-1.jar",
                    "mods/component-2.jar",
                    "mods/external.jar"
                ]
            );
            assert_eq!(again.kept_existing, ["config/pack.toml"]);
            assert_eq!(again.untouched_mods, ["user.jar"]);
            assert_eq!(again.component_count, 0);
        }
    }

    #[tokio::test]
    async fn a_component_duplicating_a_user_mod_blocks_the_merge_and_writes_nothing() {
        for kind in KINDS {
            let world = LoaderWorld::new(&format!("merge-conflict-{}", tag_for(kind)));
            let instance = ready_instance(&world, kind).await;
            let root = instance_root(&world, &instance);
            // The user's own copy of the pack's first component (same mod id,
            // different file name).
            write(
                &root,
                "mods/user-copy.jar",
                &loader_mod_jar(kind, "component_1", None),
            );
            write(&root, "config/pack.toml", USER_CONFIG);
            publish_merge_pack(&world, "MERGE001", kind);

            let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
            assert_eq!(
                reviewed.blockers,
                [
                    "component-1.jar: A top-level mod identity would duplicate an installed mod. Neither file will be replaced or adopted."
                ],
                "{kind:?}"
            );
            assert_eq!(reviewed.untouched_mods, ["user-copy.jar"]);
            let before = tree(&root);
            let error = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
                .await
                .unwrap_err();
            assert_eq!(error.code, "pack_merge_blocked");
            assert_eq!(error.message, reviewed.blockers[0]);
            assert_eq!(tree(&root), before, "{kind:?}: nothing may be written");
            assert!(
                ContentState::load(&world.managed, &instance)
                    .unwrap()
                    .entries
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn a_review_is_stale_once_the_instance_changes_and_nothing_is_written() {
        let kind = PackLoaderKind::NeoForge;
        let world = LoaderWorld::new("merge-stale");
        let instance = ready_instance(&world, kind).await;
        let root = instance_root(&world, &instance);
        write(&root, "config/pack.toml", USER_CONFIG);
        publish_merge_pack(&world, "MERGE001", kind);

        // A user mod added after review.
        let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert!(reviewed.blockers.is_empty(), "{:?}", reviewed.blockers);
        let later = loader_mod_jar(kind, "later_mod", None);
        write(&root, "mods/later.jar", &later);
        let before = tree(&root);
        let error = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_preview_stale");
        assert_eq!(tree(&root), before);

        // A user file appearing at an override destination after review.
        let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert_eq!(reviewed.untouched_mods, ["later.jar"]);
        write(&root, "config/new.toml", b"mine\n");
        let before = tree(&root);
        let error = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_preview_stale");
        assert_eq!(tree(&root), before);
        assert!(
            ContentState::load(&world.managed, &instance)
                .unwrap()
                .entries
                .is_empty()
        );

        // A fresh review of the changed instance keeps both user files.
        let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert_eq!(
            reviewed.kept_existing,
            ["config/new.toml", "config/pack.toml"]
        );
        merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(root.join("config/new.toml")).unwrap(),
            b"mine\n"
        );
        assert_eq!(std::fs::read(root.join("mods/later.jar")).unwrap(), later);
    }

    #[tokio::test]
    async fn a_failing_merge_removes_only_what_it_created_and_the_instance_stays_ready() {
        let kind = PackLoaderKind::Forge;
        let world = LoaderWorld::new("merge-rollback");
        let instance = ready_instance(&world, kind).await;
        let root = instance_root(&world, &instance);
        let user_mod = loader_mod_jar(kind, "user_mod", None);
        write(&root, "mods/user.jar", &user_mod);
        write(&root, "config/pack.toml", USER_CONFIG);
        let components = publish_merge_pack(&world, "MERGE001", kind);
        let reviewed = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert!(reviewed.blockers.is_empty(), "{:?}", reviewed.blockers);
        let before = tree(&root);
        let without_state = |mut files: BTreeMap<String, Vec<u8>>| {
            files.remove("content-managed.json");
            files
        };

        // Component activation fails after one component file was created.
        crate::instance_content::activation_fault::fail_after(Some(1));
        let error = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap_err();
        crate::instance_content::activation_fault::fail_after(None);
        assert_eq!(error.code, "pack_component_install_failed", "{error}");
        assert!(
            error.message.contains("deterministic activation fault"),
            "{error}"
        );
        assert_eq!(without_state(tree(&root)), without_state(before.clone()));
        assert!(
            ContentState::load(&world.managed, &instance)
                .unwrap()
                .entries
                .is_empty()
        );
        assert_ready_and_valid(&world, &instance, kind);

        // Components activated and the external jar written; the override
        // write then fails. Everything the merge created leaves again.
        crate::modpacks::materialize_fault::fail_at(Some(1));
        let error = merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap_err();
        crate::modpacks::materialize_fault::fail_at(None);
        assert_eq!(error.code, "pack_io_error", "{error}");
        assert_eq!(error.message, "deterministic materialization fault");
        assert_eq!(without_state(tree(&root)), without_state(before));
        assert!(
            ContentState::load(&world.managed, &instance)
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(std::fs::read(root.join("mods/user.jar")).unwrap(), user_mod);
        assert_eq!(
            std::fs::read(root.join("config/pack.toml")).unwrap(),
            USER_CONFIG
        );
        assert_ready_and_valid(&world, &instance, kind);

        // The same review then applies cleanly.
        merge_in(&world, &instance, "MERGE001", &reviewed.fingerprint)
            .await
            .unwrap();
        for component in &components {
            assert!(root.join("mods").join(&component.file_name).is_file());
        }
        assert_ready_and_valid(&world, &instance, kind);
    }

    #[tokio::test]
    async fn a_neoforge_merge_is_refused_while_the_instance_runs_or_prepares_to_launch() {
        let kind = PackLoaderKind::NeoForge;
        let world = LoaderWorld::new("merge-running-neoforge");
        let instance = ready_instance(&world, kind).await;
        publish_merge_pack(&world, "MERGE001", kind);
        let clean = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert!(clean.blockers.is_empty(), "{:?}", clean.blockers);
        let root = instance_root(&world, &instance);
        let before = tree(&root);

        let running = crate::launch::process::force_running(instance.as_str());
        let blocked = review_in(&world, &instance, "MERGE001").await.unwrap();
        assert_eq!(blocked.blockers, [INSTANCE_BUSY]);
        assert_ne!(blocked.fingerprint, clean.fingerprint);
        let requests = world.total_requests();
        let error = merge_in(&world, &instance, "MERGE001", &clean.fingerprint)
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_merge_instance_busy");
        assert_eq!(world.total_requests(), requests);
        drop(running);

        let preparing =
            crate::launch::process::PreparationGuard::acquire(instance.as_str()).unwrap();
        let error = merge_in(&world, &instance, "MERGE001", &clean.fingerprint)
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_merge_instance_busy");
        assert_eq!(world.total_requests(), requests);
        drop(preparing);
        assert_eq!(tree(&root), before);

        merge_in(&world, &instance, "MERGE001", &clean.fingerprint)
            .await
            .unwrap();
        assert!(root.join("mods/component-1.jar").is_file());
    }

    #[tokio::test]
    async fn more_components_than_one_merge_adds_are_refused_before_any_component_download() {
        let kind = PackLoaderKind::Fabric;
        let limit = crate::instance_content::MAX_PROVIDER_PLANS_PER_OPERATION;
        let world = LoaderWorld::new("merge-cap");
        let instance = ready_instance(&world, kind).await;
        let over: Vec<_> = (0..=limit)
            .map(|index| ComponentFixture::new(index, kind, &[tag_for(kind)]))
            .collect();
        world.publish(
            "MERGEBIG",
            &[tag_for(kind)],
            dependencies_for(kind),
            &over,
            &[],
            &[],
        );
        let downloads = |components: &[ComponentFixture]| {
            components
                .iter()
                .map(|c| world.requested(&format!("/files/{}/{}", c.project, c.file_name)))
                .sum::<usize>()
        };

        let reviewed = review_in(&world, &instance, "MERGEBIG").await.unwrap();
        let message = crate::modpacks::component_limit_message(limit + 1, true);
        assert_eq!(reviewed.blockers, [message.as_str()]);
        assert!(
            message.contains("65") && message.contains("at most 64"),
            "{message}"
        );
        assert_eq!(
            (reviewed.component_count, reviewed.component_limit),
            (65, 64)
        );
        assert!(reviewed.added.is_empty());
        assert_eq!(downloads(&over), 0, "no component jar may be downloaded");
        let error = merge_in(&world, &instance, "MERGEBIG", &reviewed.fingerprint)
            .await
            .unwrap_err();
        assert_eq!(error.code, "pack_merge_blocked");
        assert_eq!(error.message, message);
        assert_eq!(downloads(&over), 0);

        // Exactly at the bound the merge is allowed and says so.
        world.publish(
            "MERGE064",
            &[tag_for(kind)],
            dependencies_for(kind),
            &over[..limit],
            &[],
            &[],
        );
        let reviewed = review_in(&world, &instance, "MERGE064").await.unwrap();
        assert!(reviewed.blockers.is_empty(), "{:?}", reviewed.blockers);
        assert_eq!(
            (reviewed.component_count, reviewed.component_limit),
            (64, 64)
        );
        assert_eq!(reviewed.added.len(), 64);
    }

    /// The refusal of `version_id` from Modrinth version metadata alone:
    /// review and install both refuse, and neither requests the archive.
    async fn refused_from_metadata(
        world: &LoaderWorld,
        instance: &InstanceId,
        version_id: &str,
    ) -> String {
        let archive = format!("/packs/{version_id}.mrpack");
        let error = review_in(world, instance, version_id).await.unwrap_err();
        assert_eq!(
            world.requested(&archive),
            0,
            "{version_id}: archive requested"
        );
        let installed = merge_in(world, instance, version_id, "reviewed")
            .await
            .unwrap_err();
        assert_eq!(
            world.requested(&archive),
            0,
            "{version_id}: archive requested"
        );
        assert_eq!(
            (installed.code, &installed.message),
            (error.code, &error.message)
        );
        assert_eq!(error.code, "pack_merge_incompatible");
        error.message
    }

    /// The refusal of `version_id` from its parsed index: the archive is
    /// read, but no component is looked up or downloaded.
    async fn refused_from_index(
        world: &LoaderWorld,
        instance: &InstanceId,
        version_id: &str,
    ) -> String {
        let lookups = world.requested("/v2/version_files");
        let error = review_in(world, instance, version_id).await.unwrap_err();
        assert_eq!(world.requested(&format!("/packs/{version_id}.mrpack")), 1);
        assert_eq!(
            world.requested("/v2/version_files"),
            lookups,
            "{version_id}"
        );
        assert_eq!(error.code, "pack_merge_incompatible");
        error.message
    }

    #[tokio::test]
    async fn incompatible_instances_are_refused_precisely_and_metadata_mismatches_download_nothing()
    {
        let world = LoaderWorld::new("merge-incompatible-fabric");
        let fabric = ready_instance(&world, PackLoaderKind::Fabric).await;
        let fabric_kind = PackLoaderKind::Fabric;
        world.publish(
            "FORGEPAK",
            &["forge"],
            dependencies_for(PackLoaderKind::Forge),
            &[],
            &[],
            &[],
        );
        assert_eq!(
            refused_from_metadata(&world, &fabric, "FORGEPAK").await,
            "This Modrinth version is a Forge modpack; this instance uses Fabric."
        );
        world.publish(
            "OLDGAME1",
            &["fabric"],
            dependencies_for(fabric_kind),
            &[],
            &[],
            &[],
        );
        edit_version(&world, "OLDGAME1", |document| {
            document["game_versions"] = serde_json::json!(["1.21.1", "1.21"]);
        });
        assert_eq!(
            refused_from_metadata(&world, &fabric, "OLDGAME1").await,
            "This Modrinth version is for Minecraft 1.21.1, 1.21; this instance has Minecraft 26.2."
        );

        // Metadata listing the instance's loader and game, but an index that
        // disagrees: the index decides, before any component lookup.
        let neoforge = PackLoaderKind::NeoForge;
        world.publish(
            "NEOINDEX",
            &["fabric", "neoforge"],
            dependencies_for(neoforge),
            &[ComponentFixture::new(1, neoforge, &["neoforge"])],
            &[],
            &[],
        );
        assert_eq!(
            refused_from_index(&world, &fabric, "NEOINDEX").await,
            "This pack is a NeoForge modpack; this instance uses Fabric."
        );
        world.publish(
            "MCINDEX1",
            &["fabric"],
            serde_json::json!({"minecraft": "1.21.1", "fabric-loader": FABRIC_LOADER}),
            &[ComponentFixture::new(2, fabric_kind, &["fabric"])],
            &[],
            &[],
        );
        edit_version(&world, "MCINDEX1", |document| {
            document["game_versions"] = serde_json::json!([LOADER_MC, "1.21.1"]);
        });
        assert_eq!(
            refused_from_index(&world, &fabric, "MCINDEX1").await,
            "This pack is for Minecraft 1.21.1; this instance has Minecraft 26.2."
        );
        world.publish(
            "NEWFAB01",
            &["fabric"],
            serde_json::json!({"minecraft": LOADER_MC, "fabric-loader": "0.20.0"}),
            &[ComponentFixture::new(3, fabric_kind, &["fabric"])],
            &[],
            &[],
        );
        assert_eq!(
            refused_from_index(&world, &fabric, "NEWFAB01").await,
            "This pack needs Fabric Loader 0.20.0 or newer; this instance has Fabric Loader 0.19.5."
        );

        // Forge and NeoForge: a newer pack loader is refused by the loader's
        // own ordering, an older one accepted, and a Fabric pack refused
        // from metadata.
        for (kind, newer, older, message) in [
            (
                PackLoaderKind::Forge,
                serde_json::json!({"minecraft": LOADER_MC, "forge": "63.0.5"}),
                serde_json::json!({"minecraft": LOADER_MC, "forge": "63.0.0"}),
                "This pack needs Forge 63.0.5 or newer for Minecraft 26.2; this instance has Forge 63.0.1.",
            ),
            (
                PackLoaderKind::NeoForge,
                serde_json::json!({"minecraft": LOADER_MC, "neoforge": "26.2.0.90"}),
                serde_json::json!({"minecraft": LOADER_MC, "neoforge": "26.2.0.80"}),
                "This pack needs NeoForge 26.2.0.90 or newer; this instance has NeoForge 26.2.0.88.",
            ),
        ] {
            let world = LoaderWorld::new(&format!("merge-incompatible-{}", tag_for(kind)));
            let instance = ready_instance(&world, kind).await;
            let tag = tag_for(kind);
            world.publish(
                "NEWER001",
                &[tag],
                newer,
                &[ComponentFixture::new(1, kind, &[tag])],
                &[],
                &[],
            );
            assert_eq!(
                refused_from_index(&world, &instance, "NEWER001").await,
                message
            );
            world.publish(
                "OLDER001",
                &[tag],
                older,
                &[ComponentFixture::new(2, kind, &[tag])],
                &[],
                &[],
            );
            let reviewed = review_in(&world, &instance, "OLDER001").await.unwrap();
            assert!(
                reviewed.blockers.is_empty(),
                "{kind:?}: {:?}",
                reviewed.blockers
            );
            assert_eq!(reviewed.added, ["mods/component-2.jar"]);
            world.publish(
                "FABRIC01",
                &["fabric"],
                dependencies_for(PackLoaderKind::Fabric),
                &[],
                &[],
                &[],
            );
            assert_eq!(
                refused_from_metadata(&world, &instance, "FABRIC01").await,
                format!(
                    "This Modrinth version is a Fabric modpack; this instance uses {}.",
                    kind.display_name()
                )
            );
        }
    }

    #[test]
    fn instances_without_a_supported_mod_loader_are_named_in_refusals() {
        let pack = PackPlan {
            name: "Pack".into(),
            pack_version: "1".into(),
            minecraft_version: "26.2".into(),
            loader: PackLoader {
                kind: PackLoaderKind::Forge,
                version: "63.0.1".into(),
            },
            required_files: Vec::new(),
            optional_files: Vec::new(),
            excluded_paths: Vec::new(),
            overrides: Vec::new(),
            skipped_overrides: Vec::new(),
        };
        let vanilla = InstalledConfiguration {
            minecraft_version: "26.1".into(),
            platform: PlatformPin::Vanilla {},
            aurora: None,
        };
        assert_eq!(
            compatibility_refusals(&vanilla, &pack),
            [
                "This pack is a Forge modpack; this instance uses no mod loader (Vanilla).",
                "This pack is for Minecraft 26.2; this instance has Minecraft 26.1."
            ]
        );
        // The Forge build half is compared numerically, not as text.
        let forge = |build: &str| InstalledConfiguration {
            minecraft_version: "26.2".into(),
            platform: PlatformPin::Forge {
                version: format!("26.2-{build}"),
            },
            aurora: None,
        };
        assert!(compatibility_refusals(&forge("63.0.10"), &pack).is_empty());
        assert_eq!(
            compatibility_refusals(&forge("63.0"), &pack),
            [
                "This pack needs Forge 63.0.1 or newer for Minecraft 26.2; this instance has Forge 63.0."
            ]
        );
    }
}
