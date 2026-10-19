//! Isolated game installation execution.
//!
//! The installer consumes one fully resolved [`GameInstallPlan`] plus a
//! validated [`InstanceId`] and materializes the complete, isolated game
//! into launcher-managed instance storage. It never parses raw Mojang or
//! Fabric metadata, never decides versions, and never re-evaluates rules:
//! the plan is authoritative, and the only external document the installer
//! touches is the asset index — acquired and verified like any artifact
//! first, then parsed by the DTO boundary in `minecraft::metadata`.
//!
//! ## Execution model
//!
//! ```text
//! GameInstallPlan
//!   ↓ acquire per trust policy (sha256 / sha1 / transport-observed stores)
//! verified cache objects
//!   ↓ copy + extract into instance staging  (instances/<id>/.install-staging/game)
//! staged installation
//!   ↓ validate staged files against their recorded trust
//!   ↓ write installed-state manifest LAST inside staging
//!   ↓ promote the staged tree onto the final game directory by rename
//! InstalledGame (instances/<id>/game + installed-game.json)
//! ```
//!
//! ## Atomicity
//!
//! Nothing is ever written into the final `game/` directory directly. The
//! complete tree — including the manifest that marks completion — is built
//! under `.install-staging/` and becomes visible through one directory
//! rename. An interrupted installation therefore always leaves the instance
//! recoverable through its durable operation journal; a
//! staging directory is never mistaken for an installed game. A durable
//! journal protects the previous tree outside staging and controls recovery.
//!
//! ## User-data boundary
//!
//! Only `instances/<id>/game/` is launcher-managed and reconstructable.
//! `mods/`, `config/`, `logs/`, and any future user content at the instance
//! root are never read, written, or removed by installation. Replacing an
//! existing installation only ever moves a manifest-proven `game/` directory
//! into staging before deletion — never a recursive wipe of the instance.
//!
//! ## Concurrency
//!
//! One installation may run per instance at a time; a second attempt fails
//! immediately with `installation_already_in_progress`. There is no global
//! queue; kernel file locks coordinate independent launcher processes.

pub mod assets;
pub(crate) mod coordination;
pub mod journal;
pub mod natives;
pub mod processors;
pub mod receipt;
pub mod retention;
pub mod state;

use futures_util::future::join_all;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::cache::{
    AcquisitionError, ArtifactCache, ArtifactOrigin, ObservedArtifact, VerifiedArtifact,
    VerifiedSha1Artifact,
};
use crate::downloads::{
    ArtifactSource, DownloadOptions, InvalidArtifactSource, ObservedArtifactSource,
    Sha1ArtifactSource,
};
use crate::fabric::plan::{GameInstallPlan, GameLibrary};
use crate::instances::InstanceId;
use crate::integrity::{
    ArtifactDigest, ArtifactTrust, DigestAlgorithm, Sha1Digest, verify_file, verify_file_sha1,
};
use crate::minecraft::metadata::AssetIndexObjectsDocument;
use crate::minecraft::plan::{ArtifactRequirement, AssetIndexRequirement, LibraryKind};
use crate::neoforge::processors::run_client_processors;
use crate::paths::ManagedPaths;

use assets::AssetObjectEndpoints;
use natives::NativeExtractionError;
use state::{
    INSTALLED_GAME_FILE_NAME, InstalledFile, InstalledFileRole, InstalledGameManifest,
    InstalledStateError, NativesRecord, load_installed_state,
};

/// The fixed staging directory name beside the game directory.
const STAGING_DIR_NAME: &str = ".install-staging";

/// Where a replaced (proven-managed) previous game directory waits inside
/// staging until the new installation has been promoted.
const REPLACED_GAME_DIR_NAME: &str = "replaced-game";

/// The maximum asset-index document size the installer will parse (real
/// indexes are well under one megabyte).
const MAX_ASSET_INDEX_BYTES: usize = 8 * 1024 * 1024;
const SMALL_ARTIFACT_CONCURRENCY: usize = 16;
const STAGED_VALIDATION_CONCURRENCY: usize = 8;

/// Everything the installer needs beyond the plan itself.
#[derive(Debug, Clone)]
pub struct InstallContext {
    download_options: DownloadOptions,
    asset_endpoints: AssetObjectEndpoints,
    small_artifact_concurrency: usize,
    /// The managed Java diagnostic executable used to run installer
    /// processors. Required by NeoForge plans; `None` for Vanilla and
    /// Fabric, whose installations execute no build steps.
    processor_java: Option<PathBuf>,
    /// The per-step wall-clock bound for installer processors.
    processor_timeout: std::time::Duration,
    /// Cooperative cancellation for installer processors. The default
    /// never fires; wiring a live signal is a caller decision.
    processor_cancel: processors::CancelSignal,
}

impl InstallContext {
    /// Production context: the launcher's transport defaults and Mojang's
    /// official asset host.
    pub fn official() -> Self {
        Self {
            download_options: DownloadOptions::default(),
            asset_endpoints: AssetObjectEndpoints::official(),
            small_artifact_concurrency: SMALL_ARTIFACT_CONCURRENCY,
            processor_java: None,
            processor_timeout: processors::SupervisorLimits::default().timeout,
            processor_cancel: processors::CancelSignal::default(),
        }
    }

    /// Test context with explicit transport limits and a loopback asset
    /// root (the launcher's documented test-transport pattern).
    pub fn loopback_for_testing(
        download_options: DownloadOptions,
        asset_endpoints: AssetObjectEndpoints,
    ) -> Self {
        Self {
            download_options,
            asset_endpoints,
            small_artifact_concurrency: SMALL_ARTIFACT_CONCURRENCY,
            processor_java: None,
            processor_timeout: processors::SupervisorLimits::default().timeout,
            processor_cancel: processors::CancelSignal::default(),
        }
    }

    /// Sets the managed Java executable used for installer processors.
    pub fn with_processor_java(mut self, executable: PathBuf) -> Self {
        self.processor_java = Some(executable);
        self
    }

    /// Overrides the per-step processor timeout (tests use short values).
    pub fn with_processor_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.processor_timeout = timeout;
        self
    }

    /// Attaches cooperative processor cancellation.
    pub fn with_processor_cancel(mut self, cancel: processors::CancelSignal) -> Self {
        self.processor_cancel = cancel;
        self
    }

    pub fn download_options(&self) -> &DownloadOptions {
        &self.download_options
    }

    pub fn asset_endpoints(&self) -> &AssetObjectEndpoints {
        &self.asset_endpoints
    }

    #[cfg(test)]
    fn with_concurrency_for_testing(mut self, concurrency: usize) -> Self {
        assert!(concurrency > 0);
        self.small_artifact_concurrency = concurrency;
        self
    }
}

/// One coarse installation phase, reported as work proceeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    Acquiring,
    Materializing,
    ExtractingNatives,
    Processing,
    Validating,
    Committing,
}

impl InstallPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acquiring => "acquiring",
            Self::Materializing => "materializing",
            Self::ExtractingNatives => "extractingNatives",
            Self::Processing => "processing",
            Self::Validating => "validating",
            Self::Committing => "committing",
        }
    }
}

/// A narrow typed progress report owned by Rust; the frontend only displays
/// it. Progress never implies completion — completion is the committed
/// installed-state manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    pub completed_items: u32,
    pub total_items: u32,
    /// The logical name (Maven coordinate, asset hash, role) of the item in
    /// flight — never a remote-controlled filename or arbitrary path.
    pub current_item: Option<String>,
}

/// Deterministic failure injection for transaction tests. Production always
/// passes the default (no injected failures).
#[derive(Debug, Clone, Copy, Default)]
pub struct InstallFaults {
    /// Stop at one durable transition, leaving on-disk evidence for replay.
    pub stop_at: Option<journal::TransactionPoint>,
    /// Rust-only observer used by opt-in native process-kill acceptance.
    pub transition_observer: Option<fn(journal::TransactionPoint)>,
    /// Fail immediately before the staged tree is promoted, after the
    /// manifest is written into staging.
    pub fail_before_commit: bool,
    /// Fail partway through materialization, after the first staged copy.
    pub fail_during_materialization: bool,
    /// Fail after the processing receipt is written but before any
    /// processor runs (crash-before-processing window).
    pub fail_after_processing_receipt: bool,
    /// Fail after the previous game tree is reserved in staging but before
    /// the new tree is promoted (crash-between-renames window).
    pub fail_between_renames: bool,
}

/// The result of a committed installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledGame {
    game_directory: PathBuf,
    manifest: InstalledGameManifest,
    total_bytes: u64,
}

impl InstalledGame {
    pub fn game_directory(&self) -> &Path {
        &self.game_directory
    }

    pub fn manifest(&self) -> &InstalledGameManifest {
        &self.manifest
    }

    pub fn total_byte_count(&self) -> u64 {
        self.total_bytes
    }
}

/// One acquired-and-materialized file pairing the verified cache source
/// with its staged destination and recorded trust.
struct PlannedFile {
    source: PathBuf,
    relative: String,
    trust: ArtifactTrust,
    size_bytes: u64,
    role: InstalledFileRole,
    origin: ArtifactOrigin,
}

/// Executes one complete installation of `plan` into `instance`.
///
/// On success the instance owns a validated, committed game directory whose
/// installed-state manifest describes every managed file and how it is
/// trusted. On failure the instance never gains (or loses) a complete
/// installation; leftover staging is deliberate recoverable state that the
/// next attempt removes.
pub async fn install_game(
    managed: &ManagedPaths,
    instance: &InstanceId,
    plan: &GameInstallPlan,
    context: &InstallContext,
    progress: &mut (dyn FnMut(InstallProgress) + Send),
    faults: InstallFaults,
) -> Result<InstalledGame, InstallError> {
    check_cancel(context)?;
    let started = Instant::now();
    let instance_paths = managed.instance_paths(instance);
    let instance_root = instance_paths.root().to_path_buf();
    let _guard = lock_game_target(managed, instance)?;
    if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
        eprintln!(
            "[aurora-install] target={instance} lock_acquire_us={}",
            started.elapsed().as_micros()
        );
    }
    let game_directory = instance_paths.game().to_path_buf();
    let staging_directory = instance_root.join(STAGING_DIR_NAME);
    let staging_game = staging_directory.join("game");

    let recovery = journal::recover_locked(&instance_root, instance, InstallFaults::default())?;
    if recovery != journal::RecoveryOutcome::None {
        eprintln!("[aurora-install] target={instance} recovery={recovery:?}");
        progress(InstallProgress {
            phase: InstallPhase::Validating,
            completed_items: 0,
            total_items: 0,
            current_item: Some(
                match recovery {
                    journal::RecoveryOutcome::PreviousRestored => {
                        "Previous installation restored; preparing the requested retry"
                    }
                    journal::RecoveryOutcome::Completed => {
                        "Previous installation operation completed; preparing the requested retry"
                    }
                    _ => "Interrupted installation recovered; preparing the requested retry",
                }
                .into(),
            ),
        });
    }
    let replacing = decide_replacement(&game_directory)?;
    let minecraft = plan.minecraft();

    // An existing receipt names an interrupted Aurora transaction (or is
    // honestly unknown evidence); staging disposal below stays governed by
    // the derived-path proof either way.
    match receipt::classify_existing(&staging_directory) {
        receipt::Disposition::NoReceipt => {}
        receipt::Disposition::Interrupted {
            transaction_id,
            instance_id,
            last_stage,
        } => {
            eprintln!(
                "[aurora-launcher] recovering interrupted installation transaction {transaction_id} for instance {instance_id}, last completed stage '{last_stage}'; disposing its staging"
            );
        }
        receipt::Disposition::Unknown { reason } => {
            eprintln!(
                "[aurora-launcher] found unknown staging receipt evidence at {} ({reason}); disposing staging after the derived-path proof",
                staging_directory.display()
            );
        }
    }

    let mut durable_operation = journal::Journal::begin(&instance_root, instance)?;
    journal::checkpoint(&instance_root, faults, journal::TransactionPoint::Prepared)?;
    prepare_staging(&instance_root, &staging_directory, &staging_game)?;

    // The durable transaction receipt: written before any long work, at
    // every stage transition, and before each promotion rename. Its
    // presence after a crash distinguishes an interrupted Aurora
    // transaction from any other staging content.
    let installation_id = durable_operation.transaction_id().to_owned();
    let platform_kind = serde_json::to_value(plan.platform())
        .ok()
        .and_then(|value| {
            value
                .get("kind")
                .and_then(|kind| kind.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let mut transaction_receipt = receipt::TransactionReceipt::begin(
        &staging_directory,
        installation_id.clone(),
        instance.as_str().to_owned(),
        platform_kind,
        minecraft.minecraft_version().to_owned(),
    )
    .map_err(InstallError::Storage)?;

    let cache = ArtifactCache::new(managed.clone());

    // ---- Acquisition: everything flows through the verified stores first.
    let mut report = |phase, completed, total, item| {
        progress(InstallProgress {
            phase,
            completed_items: completed,
            total_items: total,
            current_item: item,
        })
    };

    let index_requirement = minecraft.asset_index();
    report(
        InstallPhase::Acquiring,
        0,
        acquisition_total(plan, &UnknownObjects::Pending),
        Some("asset index".to_owned()),
    );

    let index_artifact = acquire_mojang(&cache, index_requirement.artifact(), context).await?;
    check_cancel(context)?;
    let index_document = parse_verified_asset_index(&index_artifact, index_requirement)?;
    let objects = assets::plan_asset_objects(&index_document).map_err(|error| {
        InstallError::AssetInvalid {
            name: error.name,
            reason: error.reason,
        }
    })?;
    let index_finished = Instant::now();

    let total_items = acquisition_total(plan, &UnknownObjects::Known(objects.len()));
    let mut completed = 1u32;
    let mut files: Vec<PlannedFile> = Vec::new();

    // Client jar.
    report(
        InstallPhase::Acquiring,
        completed,
        total_items,
        Some(format!("{} client", minecraft.minecraft_version())),
    );
    let client = acquire_mojang(&cache, minecraft.client(), context).await?;
    check_cancel(context)?;
    let client_finished = Instant::now();
    completed += 1;
    files.push(PlannedFile {
        source: client.path.clone(),
        relative: format!("versions/{}/client.jar", minecraft.minecraft_version()),
        trust: client.trust(),
        size_bytes: client.bytes,
        role: InstalledFileRole::Client,
        origin: client.origin,
    });

    // Logging configuration, when the version requires one.
    if let Some(logging) = minecraft.logging() {
        check_cancel(context)?;
        report(
            InstallPhase::Acquiring,
            completed,
            total_items,
            Some(format!("logging configuration {}", logging.file_name())),
        );
        let artifact = acquire_mojang(&cache, logging.artifact(), context).await?;
        completed += 1;
        files.push(PlannedFile {
            source: artifact.path.clone(),
            relative: format!(
                "versions/{}/{}",
                minecraft.minecraft_version(),
                logging.file_name()
            ),
            trust: artifact.trust(),
            size_bytes: artifact.bytes,
            role: InstalledFileRole::LoggingConfig,
            origin: artifact.origin,
        });
    }
    let logging_finished = Instant::now();

    // The verified index itself is an installed file.
    files.push(PlannedFile {
        source: index_artifact.path.clone(),
        relative: format!(
            "assets/indexes/{}.json",
            validated_index_file_stem(index_requirement)?
        ),
        trust: index_artifact.trust(),
        size_bytes: index_artifact.bytes,
        role: InstalledFileRole::AssetIndex,
        origin: index_artifact.origin,
    });

    // Preserve classpath order even when independent acquisitions complete
    // out of order. A failed batch is fully settled before returning, so no
    // download task is abandoned with an open untrusted staging file.
    for batch in plan
        .libraries()
        .iter()
        .filter(|library| match library {
            // The generated patched client has no upstream URL; the
            // processor chain materializes and verifies it later in this
            // installation.
            GameLibrary::Forge(entry) => entry.artifact().is_downloadable(),
            _ => true,
        })
        .collect::<Vec<_>>()
        .chunks(context.small_artifact_concurrency)
    {
        check_cancel(context)?;
        let acquired = join_all(
            batch
                .iter()
                .map(|library| async { acquire_library(&cache, library, context).await }),
        )
        .await;
        check_cancel(context)?;
        for (library, result) in batch.iter().zip(acquired) {
            let (source, trust, size, role, origin) = result?;
            completed += 1;
            files.push(PlannedFile {
                source,
                relative: format!("libraries/{}", library.repository_path()),
                trust,
                size_bytes: size,
                role,
                origin,
            });
            report(
                InstallPhase::Acquiring,
                completed,
                total_items,
                Some(library.coordinate_string()),
            );
        }
    }
    let libraries_finished = Instant::now();

    // ---- NeoForge-specific artifacts: the verified installer (kept for
    // the processor pass, never an installed file), the universal
    // artifact, and the installer-tool libraries processors run from.
    let mut installer_path: Option<PathBuf> = None;
    if let Some(neoforge) = plan.neoforge_loader() {
        let installer = acquire_neoforge_artifact(&cache, neoforge.installer(), context).await?;
        completed += 1;
        report(
            InstallPhase::Acquiring,
            completed,
            total_items,
            Some(format!("neoforge {}", neoforge.neoforge_version())),
        );

        let universal = acquire_neoforge_library(&cache, neoforge.universal(), context).await?;
        completed += 1;
        files.push(PlannedFile {
            source: universal.path.clone(),
            relative: format!("libraries/{}", neoforge.universal().path()),
            trust: universal.trust(),
            size_bytes: universal.bytes,
            role: InstalledFileRole::Library,
            origin: universal.origin,
        });
        report(
            InstallPhase::Acquiring,
            completed,
            total_items,
            Some(neoforge.universal().coordinate().as_maven_string()),
        );

        for library in neoforge.installer_libraries() {
            check_cancel(context)?;
            let artifact = acquire_neoforge_library(&cache, library, context).await?;
            completed += 1;
            files.push(PlannedFile {
                source: artifact.path.clone(),
                relative: format!("libraries/{}", library.path()),
                trust: artifact.trust(),
                size_bytes: artifact.bytes,
                role: InstalledFileRole::Library,
                origin: artifact.origin,
            });
            report(
                InstallPhase::Acquiring,
                completed,
                total_items,
                Some(library.coordinate().as_maven_string()),
            );
        }
        installer_path = Some(installer.path);
    }

    // ---- Forge-specific artifacts: the verified installer (kept for
    // the processor pass, never an installed file) and the
    // installer-tool libraries processors run from. The generated
    // patched client is not acquired here — the processor chain
    // produces and digest-verifies it.
    let mut forge_installer_path: Option<PathBuf> = None;
    if let Some(forge) = plan.forge_loader() {
        let source = Sha1ArtifactSource::https_or_loopback(
            forge
                .installer()
                .url()
                .expect("the Forge installer always carries its repository URL")
                .as_str(),
            &forge.installer().sha1().as_hex(),
            forge.installer().size_bytes(),
        )
        .map_err(|error| InstallError::InvalidSource(error.to_string()))?;
        let installer = cache
            .acquire_sha1(&source, context.download_options())
            .await
            .map_err(InstallError::Acquisition)?;
        completed += 1;
        report(
            InstallPhase::Acquiring,
            completed,
            total_items,
            Some(format!("forge {}", forge.forge_version())),
        );

        for library in forge.installer_libraries() {
            check_cancel(context)?;
            let artifact = acquire_forge_library(&cache, library, context).await?;
            completed += 1;
            files.push(PlannedFile {
                source: artifact.path.clone(),
                relative: format!("libraries/{}", library.path()),
                trust: artifact.trust(),
                size_bytes: artifact.bytes,
                role: InstalledFileRole::Library,
                origin: artifact.origin,
            });
            report(
                InstallPhase::Acquiring,
                completed,
                total_items,
                Some(library.coordinate().as_maven_string()),
            );
        }
        forge_installer_path = Some(installer.path);
    }

    // Asset objects, deduplicated by hash.
    for batch in objects.chunks(context.small_artifact_concurrency) {
        check_cancel(context)?;
        let acquired = join_all(batch.iter().map(|object| async {
            let hex = object.sha1().as_hex();
            let source = Sha1ArtifactSource::https_or_loopback(
                context.asset_endpoints().object_url(object.sha1()).as_str(),
                &hex,
                Some(object.size_bytes()),
            )
            .map_err(|error| InstallError::InvalidSource(error.to_string()))?;
            cache
                .acquire_sha1(&source, context.download_options())
                .await
                .map_err(InstallError::Acquisition)
        }))
        .await;
        check_cancel(context)?;
        for (object, result) in batch.iter().zip(acquired) {
            let artifact = result?;
            completed += 1;
            files.push(PlannedFile {
                source: artifact.path.clone(),
                relative: object.game_relative_path(),
                trust: artifact.trust(),
                size_bytes: artifact.bytes,
                role: InstalledFileRole::AssetObject,
                origin: artifact.origin,
            });
            report(
                InstallPhase::Acquiring,
                completed,
                total_items,
                Some(format!("asset {}", object.sha1().as_hex())),
            );
        }
    }
    let assets_finished = Instant::now();
    check_cancel(context)?;
    transaction_receipt
        .record(receipt::Stage::Acquired)
        .map_err(InstallError::Storage)?;

    // ---- Materialization: verified cache objects are copied into staging.
    let total_files = files.len() as u32;
    for (index, file) in files.iter().enumerate() {
        check_cancel(context)?;
        report(
            InstallPhase::Materializing,
            index as u32,
            total_files,
            Some(file.relative.clone()),
        );
        materialize_file(&staging_game, file)?;
        if faults.fail_during_materialization {
            return Err(InstallError::Materialization {
                path: file.relative.clone(),
                source: std::io::Error::other(
                    "deterministic fault injected during materialization",
                ),
            });
        }
    }
    let materialization_finished = Instant::now();
    check_cancel(context)?;

    // ---- Native extraction from the verified native archives. Modern
    // Minecraft documents append a subdirectory to their own
    // java.library.path template; extraction follows the document.
    let natives_relative = format!("natives/{}", minecraft.minecraft_version());
    let natives_subdirectory = minecraft.natives_subdirectory().map(str::to_owned);
    let extraction_relative = match &natives_subdirectory {
        Some(subdirectory) => format!("{natives_relative}/{subdirectory}"),
        None => natives_relative.clone(),
    };
    let natives_root = staging_game.join(&extraction_relative);
    let native_libraries: Vec<&GameLibrary> = plan
        .libraries()
        .iter()
        .filter(|library| {
            matches!(
                library,
                GameLibrary::Minecraft(entry) if entry.kind() == LibraryKind::NativeArtifact)
        })
        .collect();
    let mut native_ownership = Vec::new();
    for (index, library) in native_libraries.iter().enumerate() {
        check_cancel(context)?;
        report(
            InstallPhase::ExtractingNatives,
            index as u32,
            native_libraries.len() as u32,
            Some(library.coordinate_string()),
        );
        let GameLibrary::Minecraft(entry) = library else {
            unreachable!("the filter above selects only Mojang native libraries")
        };
        let archive_path = files
            .iter()
            .find(|file| file.relative == format!("libraries/{}", entry.path()))
            .map(|file| file.source.clone())
            .expect("every planned library was acquired and materialized");
        let extracted = natives::extract_native_archive(
            &archive_path,
            &entry.coordinate().as_maven_string(),
            &natives_root,
        )
        .map_err(InstallError::Native)?;
        native_ownership.extend(extracted.files.into_iter().map(|mut file| {
            file.path = format!("{extraction_relative}/{}", file.path);
            file
        }));
    }
    if native_libraries.is_empty() {
        return Err(InstallError::Validation {
            path: natives_relative,
            reason: "the plan selected no native libraries for this platform".to_owned(),
        });
    }
    let natives_finished = Instant::now();

    // ---- Loader processing: NeoForge's bounded installer processors run
    // here, against staged inputs, writing only into the staged tree. Any
    // produced artifact is recorded as a locally generated observation.
    if let Some(neoforge) = plan.neoforge_loader() {
        if neoforge.requires_install_time_java() {
            let java_executable = context
                .processor_java
                .as_deref()
                .ok_or(InstallError::Processing {
                reason:
                    "the NeoForge plan requires the managed Java runtime, which was not provided"
                        .to_owned(),
            })?;
            let processor_context = crate::neoforge::processors::ProcessorContext::new(
                staging_game.clone(),
                staging_directory.join("processor-inputs"),
                installer_path
                    .clone()
                    .expect("a NeoForge plan acquired its installer above"),
                *neoforge.installer().sha1(),
                java_executable.to_path_buf(),
                staging_game.join(format!(
                    "versions/{}/client.jar",
                    minecraft.minecraft_version()
                )),
                minecraft.minecraft_version().to_owned(),
            )
            .with_timeout(context.processor_timeout)
            .with_cancel(context.processor_cancel.clone());
            transaction_receipt
                .record_processing(
                    neoforge
                        .processors()
                        .iter()
                        .map(|step| step.jar().as_maven_string())
                        .collect(),
                )
                .map_err(InstallError::Storage)?;
            if faults.fail_after_processing_receipt {
                return Err(InstallError::Processing {
                    reason: "deterministic fault injected after the processing receipt".to_owned(),
                });
            }
            let outcome = run_client_processors(neoforge, &processor_context, &mut |item| {
                report(InstallPhase::Processing, 0, 0, Some(item.to_owned()));
            })
            .await
            .map_err(|error| InstallError::Processing {
                reason: error.to_string(),
            })?;
            transaction_receipt
                .record_processed(
                    outcome
                        .generated
                        .iter()
                        .map(|artifact| receipt::GeneratedObservation {
                            relative: artifact.relative.clone(),
                            observed_sha256: artifact.observed_sha256.clone(),
                            size_bytes: artifact.size_bytes,
                        })
                        .collect(),
                )
                .map_err(InstallError::Storage)?;
            for artifact in outcome.generated {
                let staged = staging_game.join(artifact.relative.split('/').collect::<PathBuf>());
                files.push(PlannedFile {
                    source: staged,
                    relative: artifact.relative.clone(),
                    trust: ArtifactTrust::LocallyGenerated {
                        observed_sha256: artifact.observed_sha256,
                    },
                    size_bytes: artifact.size_bytes,
                    role: InstalledFileRole::Library,
                    origin: ArtifactOrigin::CacheHit,
                });
            }
        }
    }
    if let Some(forge) = plan.forge_loader() {
        if forge.requires_install_time_java() {
            let java_executable =
                context
                    .processor_java
                    .as_deref()
                    .ok_or(InstallError::Processing {
                    reason:
                        "the Forge plan requires the managed Java runtime, which was not provided"
                            .to_owned(),
                })?;
            let processor_context = crate::forge::processors::ProcessorContext::new(
                staging_game.clone(),
                staging_directory.join("processor-inputs"),
                forge_installer_path
                    .clone()
                    .expect("a Forge plan acquired its installer above"),
                *forge.installer().sha1(),
                java_executable.to_path_buf(),
                staging_game.join(format!(
                    "versions/{}/client.jar",
                    minecraft.minecraft_version()
                )),
                minecraft.minecraft_version().to_owned(),
            )
            .with_timeout(context.processor_timeout)
            .with_cancel(context.processor_cancel.clone());
            transaction_receipt
                .record_processing(
                    forge
                        .processors()
                        .iter()
                        .map(|step| step.jar().as_maven_string())
                        .collect(),
                )
                .map_err(InstallError::Storage)?;
            if faults.fail_after_processing_receipt {
                return Err(InstallError::Processing {
                    reason: "deterministic fault injected after the processing receipt".to_owned(),
                });
            }
            let outcome = crate::forge::processors::run_client_processors(
                forge,
                &processor_context,
                &mut |item| {
                    report(InstallPhase::Processing, 0, 0, Some(item.to_owned()));
                },
            )
            .await
            .map_err(|error| InstallError::Processing {
                reason: error.to_string(),
            })?;
            transaction_receipt
                .record_processed(
                    outcome
                        .generated
                        .iter()
                        .map(|artifact| receipt::GeneratedObservation {
                            relative: artifact.relative.clone(),
                            observed_sha256: artifact.observed_sha256.clone(),
                            size_bytes: artifact.size_bytes,
                        })
                        .collect(),
                )
                .map_err(InstallError::Storage)?;
            for artifact in outcome.generated {
                let staged = staging_game.join(artifact.relative.split('/').collect::<PathBuf>());
                // An output that matched its published SHA-1 keeps the
                // stronger, honest upstream expectation (the digest was
                // known in trusted metadata before the step ran and the
                // generated bytes verified against it); every other
                // generated file is a locally generated observation.
                let trust = match outcome.published_sha1.get(&artifact.relative) {
                    Some(sha1) => ArtifactTrust::verified_sha1(sha1),
                    None => ArtifactTrust::LocallyGenerated {
                        observed_sha256: artifact.observed_sha256,
                    },
                };
                files.push(PlannedFile {
                    source: staged,
                    relative: artifact.relative.clone(),
                    trust,
                    size_bytes: artifact.size_bytes,
                    role: InstalledFileRole::Library,
                    origin: ArtifactOrigin::CacheHit,
                });
            }
        }
    }
    let processing_finished = Instant::now();
    let _ = processing_finished;

    // ---- Staged validation: every file re-verified before completion.
    transaction_receipt
        .record(receipt::Stage::Verifying)
        .map_err(InstallError::Storage)?;
    report(InstallPhase::Validating, 0, total_files, None);
    let installed_files = validate_staged_files(
        &staging_game,
        &files,
        &extraction_relative,
        &context.processor_cancel,
    )
    .await?;
    let staged_validation_finished = Instant::now();
    transaction_receipt
        .record(receipt::Stage::Verified)
        .map_err(InstallError::Storage)?;

    // ---- Completion record, written last inside staging.
    let installed_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let manifest = InstalledGameManifest::for_platform(
        minecraft.minecraft_version(),
        plan.platform(),
        installation_id,
        installed_at,
        installed_files,
        NativesRecord::with_subdirectory(natives_relative, natives_subdirectory),
    );
    let manifest_path = staging_game.join(INSTALLED_GAME_FILE_NAME);
    // Optional forward-only ownership evidence. Failure protects the backup;
    // it cannot turn a successful installation into a failure.
    if retention::seal_staging(&staging_game, &manifest, native_ownership).is_err() {
        eprintln!("[aurora-retention] outcome=retainedUnknownOwnership reason=sealUnavailable");
    }
    let ownership_finished = Instant::now();
    // The completion manifest remains the final staged file, as before F3R-B.
    std::fs::write(&manifest_path, manifest.to_json()).map_err(|error| InstallError::Commit {
        context: "writing the installed-state manifest".to_owned(),
        source: error,
    })?;
    let state_finished = Instant::now();
    if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
        eprintln!(
            "[aurora-durability] phase=ownership_seal elapsed_us={} files={} bytes=0",
            ownership_finished
                .duration_since(staged_validation_finished)
                .as_micros(),
            manifest.files().len()
        );
        eprintln!(
            "[aurora-durability] phase=installed_state_write elapsed_us={} files=1 bytes={}",
            state_finished
                .duration_since(ownership_finished)
                .as_micros(),
            manifest_path
                .metadata()
                .map(|value| value.len())
                .unwrap_or_default()
        );
    }

    if faults.fail_before_commit {
        return Err(InstallError::Commit {
            context: "deterministic fault injected immediately before commit".to_owned(),
            source: std::io::Error::other("injected pre-commit failure"),
        });
    }

    // ---- Commit: promote the staged tree with one rename.
    report(InstallPhase::Committing, 0, 1, None);
    if context.processor_cancel.is_cancelled() {
        return Err(InstallError::Cancelled);
    }
    _guard
        .recheck(managed.data_root(), &instance_root)
        .map_err(InstallError::Storage)?;
    if replacing {
        transaction_receipt
            .record_replacing()
            .map_err(InstallError::Storage)?;
    } else {
        transaction_receipt
            .record(receipt::Stage::Committing)
            .map_err(InstallError::Storage)?;
    }
    durable_operation.promote(&instance_root, faults)?;

    if std::env::var_os("AURORA_INSTALL_DIAGNOSTICS").is_some() {
        let hits = files
            .iter()
            .filter(|file| file.origin == ArtifactOrigin::CacheHit)
            .count();
        let downloaded_bytes: u64 = files
            .iter()
            .filter(|file| file.origin == ArtifactOrigin::Downloaded)
            .map(|file| file.size_bytes)
            .sum();
        eprintln!(
            "[aurora-install] total_ms={} asset_index_ms={} client_ms={} logging_ms={} libraries_ms={} asset_objects_ms={} materialization_ms={} natives_ms={} staged_validation_ms={} installed_state_ms={} commit_ms={} files={} assets={} cache_hits={} downloads={} downloaded_bytes={} limit={}",
            started.elapsed().as_millis(),
            index_finished.duration_since(started).as_millis(),
            client_finished.duration_since(index_finished).as_millis(),
            logging_finished.duration_since(client_finished).as_millis(),
            libraries_finished
                .duration_since(logging_finished)
                .as_millis(),
            assets_finished
                .duration_since(libraries_finished)
                .as_millis(),
            materialization_finished
                .duration_since(assets_finished)
                .as_millis(),
            natives_finished
                .duration_since(materialization_finished)
                .as_millis(),
            staged_validation_finished
                .duration_since(natives_finished)
                .as_millis(),
            state_finished
                .duration_since(ownership_finished)
                .as_millis(),
            Instant::now().duration_since(state_finished).as_millis(),
            files.len(),
            objects.len(),
            hits,
            files.len() - hits,
            downloaded_bytes,
            context.small_artifact_concurrency,
        );
    }

    let total_bytes = files.iter().map(|file| file.size_bytes).sum();
    Ok(InstalledGame {
        game_directory,
        manifest,
        total_bytes,
    })
}

/// The outcome of [`validate_installed_game`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationOutcome {
    /// No complete installation exists in this instance.
    NotInstalled,
    Installed(InstalledGameValidation),
}

/// The deterministic validation report of one installed game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledGameValidation {
    pub status: ValidationStatus,
    pub problems: Vec<ValidationProblem>,
    pub minecraft_version: String,
    pub fabric_loader_version: Option<String>,
    pub installation_id: String,
    pub checked_files: usize,
    pub verified_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Valid,
    Damaged,
}

/// One concrete validation problem (missing file, size drift, digest drift,
/// missing natives).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationProblem {
    pub path: String,
    pub reason: String,
}

/// Validates one instance's installed game against its own installed-state
/// record: every managed file must exist with its recorded size and must
/// re-verify according to its recorded trust.
///
/// This is validation, not repair: nothing is downloaded and nothing is
/// mutated. A game directory without a manifest is `NotInstalled`; a
/// malformed manifest is a deliberate error.
pub fn validate_installed_game(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<ValidationOutcome, InstallError> {
    let _guard = read_game_target(managed, instance)?;
    validate_installed_game_under_lock(managed, instance)
}

pub(crate) fn validate_installed_game_under_lock(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<ValidationOutcome, InstallError> {
    let _performance = crate::performance::scope(crate::performance::Event::GameIntegrity);
    let game_directory = managed.instance_paths(instance).game().to_path_buf();
    let instance_root = managed.instance_paths(instance).root().to_path_buf();
    if instance_root.exists() {
        coordination::checked_directory(managed.data_root(), &instance_root)
            .map_err(InstallError::Storage)?;
    }
    if std::fs::symlink_metadata(&game_directory).is_ok() {
        coordination::checked_directory(managed.data_root(), &game_directory)
            .map_err(InstallError::Storage)?;
    }
    journal::readiness_gate(&instance_root, instance)?;
    validate_game_directory(&game_directory)
}

/// Network-free restoration under the same kernel lock as game installation.
pub fn recover_installation(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<journal::RecoveryOutcome, InstallError> {
    recover_installation_with_faults(managed, instance, InstallFaults::default())
}

/// Rust-only fault harness; ordinary callers use `recover_installation`.
pub fn recover_installation_with_faults(
    managed: &ManagedPaths,
    instance: &InstanceId,
    faults: InstallFaults,
) -> Result<journal::RecoveryOutcome, InstallError> {
    let paths = managed.instance_paths(instance);
    if !paths.root().exists() {
        return Ok(journal::RecoveryOutcome::None);
    }
    let guard = lock_game_target(managed, instance)?;
    guard
        .recheck(managed.data_root(), paths.root())
        .map_err(InstallError::Storage)?;
    let result = journal::recover_locked(paths.root(), instance, faults)?;
    if result != journal::RecoveryOutcome::None {
        eprintln!("[aurora-install] target={instance} recovery={result:?}");
    }
    Ok(result)
}

pub(crate) fn validate_game_directory(
    game_directory: &Path,
) -> Result<ValidationOutcome, InstallError> {
    if !game_directory.is_dir() {
        return Ok(ValidationOutcome::NotInstalled);
    }
    let metadata = std::fs::symlink_metadata(game_directory).map_err(InstallError::Storage)?;
    if coordination::redirected(&metadata) {
        return Err(InstallError::Storage(coordination::unsafe_path()));
    }

    let Some(manifest) = load_installed_state(&game_directory).map_err(InstallError::State)? else {
        return Ok(ValidationOutcome::NotInstalled);
    };

    let mut problems = Vec::new();
    let mut verified_bytes = 0u64;

    for file in manifest.files() {
        match verify_managed_file(&game_directory, file) {
            Ok(bytes) => verified_bytes += bytes,
            Err(reason) => problems.push(ValidationProblem {
                path: file.path().to_owned(),
                reason,
            }),
        }
    }

    let natives_directory = game_directory.join(manifest.natives().directory());
    if natives_directory.exists() {
        coordination::checked_directory(game_directory, &natives_directory)
            .map_err(InstallError::Storage)?;
    }
    let natives_ok = natives_directory.is_dir()
        && std::fs::read_dir(&natives_directory)
            .map(|entries| entries.filter_map(Result::ok).count() > 0)
            .unwrap_or(false);
    if !natives_ok {
        problems.push(ValidationProblem {
            path: manifest.natives().directory().to_owned(),
            reason: "the extracted natives directory is missing or empty".to_owned(),
        });
    }

    let status = if problems.is_empty() {
        ValidationStatus::Valid
    } else {
        ValidationStatus::Damaged
    };

    Ok(ValidationOutcome::Installed(InstalledGameValidation {
        status,
        problems,
        minecraft_version: manifest.minecraft_version().to_owned(),
        fabric_loader_version: manifest.fabric_loader_version().map(str::to_owned),
        installation_id: manifest.installation_id().to_owned(),
        checked_files: manifest.files().len(),
        verified_bytes,
    }))
}

/// Re-verifies one installed file against its recorded trust and size.
fn verify_managed_file(game_root: &Path, file: &InstalledFile) -> Result<u64, String> {
    let path = game_root.join(file.path());
    let parent = path
        .parent()
        .ok_or_else(|| "installed file has no parent".to_owned())?;
    coordination::checked_directory(game_root, parent).map_err(|error| error.to_string())?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.is_file() || coordination::redirected(&metadata) {
        return Err("installed file is not a contained regular file".to_owned());
    }
    match file.trust() {
        ArtifactTrust::ExpectedDigestVerified { algorithm, digest } => match algorithm {
            DigestAlgorithm::Sha256 => {
                let expected = ArtifactDigest::parse(digest).map_err(|e| e.to_string())?;
                verify_file(&path, &expected, Some(file.size_bytes()))
                    .map_err(|error| error.to_string())
            }
            DigestAlgorithm::Sha1 => {
                let expected = Sha1Digest::parse(digest).map_err(|e| e.to_string())?;
                verify_file_sha1(&path, &expected, Some(file.size_bytes()))
                    .map_err(|error| error.to_string())
            }
        },
        ArtifactTrust::SecureTransportObserved { observed_sha256 }
        | ArtifactTrust::LocallyGenerated { observed_sha256 } => {
            let expected = ArtifactDigest::parse(observed_sha256).map_err(|e| e.to_string())?;
            verify_file(&path, &expected, Some(file.size_bytes()))
                .map_err(|error| error.to_string())
        }
    }
}

enum UnknownObjects {
    Pending,
    Known(usize),
}

fn acquisition_total(plan: &GameInstallPlan, objects: &UnknownObjects) -> u32 {
    let mut total = 2; // asset index + client
    if plan.minecraft().logging().is_some() {
        total += 1;
    }
    let downloadable = plan
        .libraries()
        .iter()
        .filter(|library| match library {
            // The processor-generated patched client is not an
            // acquisition: the processor chain produces it.
            GameLibrary::Forge(entry) => entry.artifact().is_downloadable(),
            _ => true,
        })
        .count();
    total += downloadable as u32;
    if let Some(neoforge) = plan.neoforge_loader() {
        // The installer jar itself plus the universal artifact and the
        // installer-tool libraries.
        total += 2 + neoforge.installer_libraries().len() as u32;
    }
    if let Some(forge) = plan.forge_loader() {
        // The installer jar itself plus the installer-tool libraries.
        total += 1 + forge.installer_libraries().len() as u32;
    }
    if let UnknownObjects::Known(count) = objects {
        total += *count as u32;
    }
    total
}

/// Decides whether installation replaces a proven existing installation or
/// conflicts with unrecognizable existing state.
///
/// - no `game/` directory → fresh installation;
/// - `game/` with a valid installed-state manifest → deliberate replacement;
/// - `game/` without one (partial prior install, foreign files, a stray
///   file) → a hard conflict; Aurora neither wipes nor guesses.
fn decide_replacement(game_directory: &Path) -> Result<bool, InstallError> {
    if std::fs::symlink_metadata(game_directory).is_err() {
        return Ok(false);
    }
    if !game_directory.is_dir() {
        return Err(InstallError::TargetConflict {
            path: game_directory.display().to_string(),
            reason: "the instance's game location exists but is not a directory".to_owned(),
        });
    }
    match load_installed_state(game_directory) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Err(InstallError::TargetConflict {
            path: game_directory.display().to_string(),
            reason: "the instance's game directory exists without a valid installed-state manifest; it may be a partial or foreign tree, and Aurora does not wipe it"
                .to_owned(),
        }),
        Err(error) => Err(InstallError::State(error)),
    }
}

/// Resets the staging tree: stale staging from a failed attempt is removed
/// only after proving the path is the fixed, derived staging location, then a
/// fresh staging game directory is created.
fn prepare_staging(
    instance_root: &Path,
    staging_directory: &Path,
    staging_game: &Path,
) -> Result<(), InstallError> {
    prove_staging_path(instance_root, staging_directory)?;

    journal::cleanup_staging(instance_root)?;
    std::fs::create_dir_all(staging_game).map_err(InstallError::Storage)?;
    Ok(())
}

/// The containment proof required before any recursive removal: staging must
/// be exactly `<instance-root>/.install-staging`.
fn prove_staging_path(instance_root: &Path, staging_directory: &Path) -> Result<(), InstallError> {
    let is_derived_staging = staging_directory.parent() == Some(instance_root)
        && staging_directory.file_name().and_then(|name| name.to_str()) == Some(STAGING_DIR_NAME);
    if !is_derived_staging || !staging_directory.starts_with(instance_root) {
        return Err(InstallError::Storage(std::io::Error::other(format!(
            "refusing to modify {} : it is not the derived instance staging directory",
            staging_directory.display()
        ))));
    }
    Ok(())
}

/// Acquires one official Mojang artifact through the SHA-1 store.
async fn acquire_mojang(
    cache: &ArtifactCache,
    requirement: &ArtifactRequirement,
    context: &InstallContext,
) -> Result<VerifiedSha1Artifact, InstallError> {
    let source = Sha1ArtifactSource::https_or_loopback(
        requirement.url(),
        &requirement.sha1().as_hex(),
        Some(requirement.size_bytes()),
    )
    .map_err(|error| InstallError::InvalidSource(error.to_string()))?;

    cache
        .acquire_sha1(&source, context.download_options())
        .await
        .map_err(InstallError::Acquisition)
}

/// Parses the verified asset-index document.
///
/// The index is a verified cache object by the time this runs; parsing is
/// bounded, and any structural failure is a deliberate hard error.
fn parse_verified_asset_index(
    artifact: &VerifiedSha1Artifact,
    requirement: &AssetIndexRequirement,
) -> Result<AssetIndexObjectsDocument, InstallError> {
    if artifact.bytes as usize > MAX_ASSET_INDEX_BYTES {
        return Err(InstallError::AssetIndexInvalid {
            reason: format!(
                "asset index '{}' is {} bytes, above the parsing bound",
                requirement.id(),
                artifact.bytes
            ),
        });
    }
    let text = std::fs::read_to_string(&artifact.path).map_err(|error| {
        InstallError::AssetIndexInvalid {
            reason: format!("the verified asset index could not be read: {error}"),
        }
    })?;
    AssetIndexObjectsDocument::from_json(&text).map_err(|error| InstallError::AssetIndexInvalid {
        reason: error.to_string(),
    })
}

/// Acquires one composed library through the store its trust class demands.
async fn acquire_library(
    cache: &ArtifactCache,
    library: &GameLibrary,
    context: &InstallContext,
) -> Result<
    (
        PathBuf,
        ArtifactTrust,
        u64,
        InstalledFileRole,
        ArtifactOrigin,
    ),
    InstallError,
> {
    match library {
        GameLibrary::Minecraft(entry) => {
            let artifact = acquire_mojang(cache, entry.artifact(), context).await?;
            let role = if entry.kind() == LibraryKind::NativeArtifact {
                InstalledFileRole::NativeLibrary
            } else {
                InstalledFileRole::Library
            };
            Ok((
                artifact.path.clone(),
                artifact.trust(),
                artifact.bytes,
                role,
                artifact.origin,
            ))
        }
        GameLibrary::NeoForge(entry) => {
            let artifact = acquire_neoforge_library(cache, entry, context).await?;
            Ok((
                artifact.path.clone(),
                artifact.trust(),
                artifact.bytes,
                InstalledFileRole::Library,
                artifact.origin,
            ))
        }
        GameLibrary::Forge(entry) => {
            let artifact = acquire_forge_library(cache, entry, context).await?;
            Ok((
                artifact.path.clone(),
                artifact.trust(),
                artifact.bytes,
                InstalledFileRole::Library,
                artifact.origin,
            ))
        }
        GameLibrary::Fabric(entry) => {
            let artifact = entry.artifact();
            if let Some(sha256) = artifact.sha256() {
                let source = https_or_loopback_artifact_source(
                    artifact.url().as_str(),
                    sha256,
                    artifact.size_bytes(),
                )
                .map_err(|error| InstallError::InvalidSource(error.to_string()))?;
                let verified: VerifiedArtifact = cache
                    .acquire_with(&source, context.download_options())
                    .await?;
                Ok((
                    verified.path.clone(),
                    verified.trust(),
                    verified.bytes,
                    InstalledFileRole::Library,
                    verified.origin,
                ))
            } else {
                let source = ObservedArtifactSource::https_or_loopback(artifact.url().as_str())
                    .map_err(|error| InstallError::InvalidSource(error.to_string()))?;
                let observed: ObservedArtifact = cache
                    .acquire_observed(&source, context.download_options())
                    .await?;
                Ok((
                    observed.path.clone(),
                    observed.trust(),
                    observed.bytes,
                    InstalledFileRole::Library,
                    observed.origin,
                ))
            }
        }
    }
}

/// Acquires one NeoForge-published library through the SHA-1 verified
/// store, exactly like a Mojang library: NeoForge publishes official
/// SHA-1 digests inline in its metadata.
async fn acquire_neoforge_library(
    cache: &ArtifactCache,
    library: &crate::neoforge::plan::NeoForgeLibrary,
    context: &InstallContext,
) -> Result<VerifiedSha1Artifact, InstallError> {
    acquire_neoforge_artifact(cache, library.artifact(), context).await
}

/// Acquires one SHA-1-verified Forge library. Only downloadable artifacts
/// reach this boundary: the generated patched client is filtered out
/// before acquisition and produced by the processor chain.
async fn acquire_forge_library(
    cache: &ArtifactCache,
    library: &crate::forge::plan::ForgeLibrary,
    context: &InstallContext,
) -> Result<VerifiedSha1Artifact, InstallError> {
    let artifact = library.artifact();
    let url = artifact
        .url()
        .expect("downloadable Forge libraries carry their repository URL");
    let source = Sha1ArtifactSource::https_or_loopback(
        url.as_str(),
        &artifact.sha1().as_hex(),
        artifact.size_bytes(),
    )
    .map_err(|error| InstallError::InvalidSource(error.to_string()))?;
    cache
        .acquire_sha1(&source, context.download_options())
        .await
        .map_err(InstallError::Acquisition)
}

/// Acquires one SHA-1-verified NeoForge artifact (library, universal, or
/// installer).
async fn acquire_neoforge_artifact(
    cache: &ArtifactCache,
    artifact: &crate::neoforge::plan::NeoForgeArtifact,
    context: &InstallContext,
) -> Result<VerifiedSha1Artifact, InstallError> {
    let source = Sha1ArtifactSource::https_or_loopback(
        artifact.url().as_str(),
        &artifact.sha1().as_hex(),
        artifact.size_bytes(),
    )
    .map_err(|error| InstallError::InvalidSource(error.to_string()))?;

    cache
        .acquire_sha1(&source, context.download_options())
        .await
        .map_err(InstallError::Acquisition)
}

/// Constructs a production HTTPS SHA-256 artifact source, falling back to
/// the explicit loopback test transport for the offline deterministic tests.
fn https_or_loopback_artifact_source(
    url: &str,
    sha256: &ArtifactDigest,
    size_bytes: Option<u64>,
) -> Result<ArtifactSource, InvalidArtifactSource> {
    ArtifactSource::https(url, &sha256.as_hex(), size_bytes)
        .or_else(|_| ArtifactSource::loopback_http_for_testing(url, &sha256.as_hex(), size_bytes))
}

/// Copies one verified cache object into the staged game tree and checks the
/// copied byte count. Content re-verification happens in the staged
/// validation pass.
fn materialize_file(staging_game: &Path, file: &PlannedFile) -> Result<(), InstallError> {
    let mut target = staging_game.to_path_buf();
    for segment in file.relative.split('/') {
        target.push(segment);
    }
    if !target.starts_with(staging_game) {
        return Err(InstallError::Materialization {
            path: file.relative.clone(),
            source: std::io::Error::other("the derived path escaped the staging root"),
        });
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| InstallError::Materialization {
            path: file.relative.clone(),
            source: error,
        })?;
    }
    std::fs::copy(&file.source, &target).map_err(|error| InstallError::Materialization {
        path: file.relative.clone(),
        source: error,
    })?;
    let copied = std::fs::metadata(&target)
        .map_err(|error| InstallError::Materialization {
            path: file.relative.clone(),
            source: error,
        })?
        .len();
    if copied != file.size_bytes {
        return Err(InstallError::Materialization {
            path: file.relative.clone(),
            source: std::io::Error::other(format!(
                "copied {copied} bytes but the verified artifact holds {}",
                file.size_bytes
            )),
        });
    }
    Ok(())
}

/// Re-verifies every staged file against its recorded trust, producing the
/// manifest entries. This is the pass that proves a successful API call
/// really produced valid installed bytes.
async fn validate_staged_files(
    staging_game: &Path,
    files: &[PlannedFile],
    natives_relative: &str,
    cancel: &processors::CancelSignal,
) -> Result<Vec<InstalledFile>, InstallError> {
    let mut installed = Vec::with_capacity(files.len());
    let root = Arc::new(staging_game.to_path_buf());
    for batch in files.chunks(STAGED_VALIDATION_CONCURRENCY) {
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
        let tasks = batch.iter().map(|file| {
            let root = Arc::clone(&root);
            let record = InstalledFile::new(
                file.role,
                file.relative.clone(),
                file.trust.clone(),
                file.size_bytes,
            );
            tokio::task::spawn_blocking(move || {
                verify_managed_file(&root, &record).map_err(|reason| InstallError::Validation {
                    path: record.path().to_owned(),
                    reason,
                })?;
                Ok::<_, InstallError>(record)
            })
        });
        for result in join_all(tasks).await {
            let record = result.map_err(|error| InstallError::Validation {
                path: "staged files".to_owned(),
                reason: format!("a staged verification worker failed: {error}"),
            })??;
            installed.push(record);
        }
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }
    }

    let natives_root = staging_game.join(natives_relative);
    let has_content = natives_root.is_dir()
        && std::fs::read_dir(&natives_root)
            .map(|entries| entries.filter_map(Result::ok).count() > 0)
            .unwrap_or(false);
    if !has_content {
        return Err(InstallError::Validation {
            path: natives_relative.to_owned(),
            reason: "the staged natives directory is missing or empty".to_owned(),
        });
    }

    Ok(installed)
}

/// The asset-index id becomes one installed file name; it is validated as a
/// safe single segment (official ids are numeric strings).
fn validated_index_file_stem(requirement: &AssetIndexRequirement) -> Result<String, InstallError> {
    let id = requirement.id();
    let valid = !id.is_empty()
        && !id.starts_with('.')
        && id == id.trim()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(InstallError::AssetIndexInvalid {
            reason: format!("asset index id '{id}' is not a safe file name"),
        });
    }
    Ok(id.to_owned())
}

fn check_cancel(context: &InstallContext) -> Result<(), InstallError> {
    if context.processor_cancel.is_cancelled() {
        Err(InstallError::Cancelled)
    } else {
        Ok(())
    }
}

/// Kernel-owned per-target exclusion, held through publication or recovery.
pub(crate) fn lock_game_target(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<coordination::TargetLock, InstallError> {
    coordination::TargetLock::acquire(
        managed.data_root(),
        managed.instance_paths(instance).root(),
        ".game-install.lock",
    )
    .map_err(|error| game_lock_error(error, instance))
}

pub(crate) fn read_game_target(
    managed: &ManagedPaths,
    instance: &InstanceId,
) -> Result<Option<coordination::TargetLock>, InstallError> {
    coordination::TargetLock::existing_shared(
        managed.data_root(),
        managed.instance_paths(instance).root(),
        ".game-install.lock",
    )
    .map_err(|error| game_lock_error(error, instance))
}

fn game_lock_error(error: coordination::LockError, instance: &InstanceId) -> InstallError {
    match error {
        coordination::LockError::Busy => InstallError::AlreadyInProgress {
            instance_id: instance.to_string(),
        },
        coordination::LockError::Io(error) => InstallError::Storage(error),
    }
}

/// A failed installation. Every variant fails closed: the instance never
/// ends up with a complete-looking installation it did not earn.
#[derive(Debug)]
pub enum InstallError {
    /// Unresolved or conflicting durable evidence must be preserved.
    RecoveryRequired {
        reason: String,
    },
    Cancelled,
    /// Another installation is already running for this instance.
    AlreadyInProgress {
        instance_id: String,
    },
    /// Existing state at the target cannot be replaced safely.
    TargetConflict {
        path: String,
        reason: String,
    },
    /// The persisted installed state is malformed or unsupported.
    State(InstalledStateError),
    /// The verified asset-index document is unusable.
    AssetIndexInvalid {
        reason: String,
    },
    /// One asset-object entry is unusable.
    AssetInvalid {
        name: String,
        reason: String,
    },
    /// An artifact acquisition failed (transport, integrity, or cache).
    Acquisition(AcquisitionError),
    /// Plan-supplied artifact metadata could not become a valid source.
    InvalidSource(String),
    /// Copying a verified object into staging failed or short-changed.
    Materialization {
        path: String,
        source: std::io::Error,
    },
    /// Native extraction failed (unsafe, conflicting, or unreadable).
    Native(NativeExtractionError),
    /// A bounded installer-processor step failed.
    Processing {
        reason: String,
    },
    /// Staged content failed validation before commit.
    Validation {
        path: String,
        reason: String,
    },
    /// The staged installation could not be committed.
    Commit {
        context: String,
        source: std::io::Error,
    },
    /// Staging could not be prepared or cleaned.
    Storage(std::io::Error),
}

impl fmt::Display for InstallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecoveryRequired { reason } => write!(
                formatter,
                "installation recovery requires attention: {reason}"
            ),
            Self::Cancelled => formatter
                .write_str("installation cancelled before commit; previous installation preserved"),
            Self::AlreadyInProgress { instance_id } => write!(
                formatter,
                "an installation is already in progress for instance '{instance_id}'"
            ),
            Self::TargetConflict { path, reason } => {
                write!(
                    formatter,
                    "the installation target conflicts with existing state at {path}: {reason}"
                )
            }
            Self::State(error) => write!(formatter, "{error}"),
            Self::AssetIndexInvalid { reason } => {
                write!(formatter, "the verified asset index is unusable: {reason}")
            }
            Self::AssetInvalid { name, reason } => write!(
                formatter,
                "asset object '{name}' cannot be installed: {reason}"
            ),
            Self::Acquisition(error) => write!(formatter, "{error}"),
            Self::InvalidSource(reason) => {
                write!(formatter, "planned artifact metadata is unusable: {reason}")
            }
            Self::Materialization { path, source } => write!(
                formatter,
                "the verified artifact for '{path}' could not be materialized into the installation: {source}"
            ),
            Self::Native(error) => write!(formatter, "{error}"),
            Self::Processing { reason } => write!(
                formatter,
                "a bounded NeoForge installer-processor step failed: {reason}"
            ),
            Self::Validation { path, reason } => write!(
                formatter,
                "the staged installation failed validation at '{path}': {reason}"
            ),
            Self::Commit { context, source } => write!(
                formatter,
                "the staged installation could not be committed while {context}: {source}"
            ),
            Self::Storage(error) => write!(
                formatter,
                "the installation staging area could not be managed: {error}"
            ),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<NativeExtractionError> for InstallError {
    fn from(error: NativeExtractionError) -> Self {
        Self::Native(error)
    }
}

impl From<AcquisitionError> for InstallError {
    fn from(error: AcquisitionError) -> Self {
        Self::Acquisition(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fabric::metadata::LoaderProfileDocument;
    use crate::fabric::plan::{compose_game_plan, plan_loader_profile};
    use crate::minecraft::metadata::VersionDocument;
    use crate::minecraft::plan::plan_version_document;
    use crate::minecraft::rules::PlatformProfile;
    use crate::test_support::{TestResponse, TestServer};
    use std::collections::{BTreeMap, HashSet};
    use std::io::Write as _;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// Everything the synthetic installation serves, from one loopback
    /// server. Bodies are fixed at build time; selected paths can be made
    /// to fail (HTTP 404) to inject acquisition failures.
    struct Fixture {
        server: TestServer,
        bodies: BTreeMap<String, Vec<u8>>,
        broken: Arc<Mutex<HashSet<String>>>,
    }

    impl Fixture {
        fn break_path(&self, path: &str) {
            self.broken.lock().unwrap().insert(path.to_owned());
        }
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        Sha1Digest::compute(bytes).as_hex()
    }

    fn sha256_digest(bytes: &[u8]) -> ArtifactDigest {
        use sha2::Digest as _;
        ArtifactDigest::from_sha256(sha2::Sha256::digest(bytes).into())
    }

    /// A tiny deterministic native archive: one DLL, one dylib, plus
    /// META-INF content that must not be extracted.
    fn native_zip_bytes() -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, bytes) in [
                ("META-INF/MANIFEST.MF", &b"Manifest-Version: 1.0"[..]),
                ("lwjgl.dll", b"synthetic native dll bytes"),
                ("liblwjgl.dylib", b"synthetic native dylib bytes"),
            ] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    /// Builds the complete synthetic game world and the composed plan for it.
    fn synthetic_game() -> (Fixture, GameInstallPlan) {
        let client = b"synthetic client jar bytes".to_vec();
        let logging = b"<Configuration status=\"WARN\"></Configuration>".to_vec();
        let mojang_library = b"synthetic mojang library jar".to_vec();
        let native_archive = native_zip_bytes();
        let fabric_common = b"synthetic digested fabric library jar".to_vec();
        let fabric_loader = b"synthetic digest-less fabric loader jar".to_vec();
        let intermediary = b"synthetic digest-less intermediary jar".to_vec();

        let asset_a = b"tiny png bytes a".to_vec();
        let asset_b = b"tiny png bytes b".to_vec();
        let asset_a_hash = sha1_hex(&asset_a);
        let asset_b_hash = sha1_hex(&asset_b);
        let asset_index_body = format!(
            r#"{{"objects": {{
                "icons/icon_16x16.png": {{"hash": "{asset_a_hash}", "size": {}}},
                "icons/icon_16x16_hd.png": {{"hash": "{asset_a_hash}", "size": {}}},
                "minecraft/sounds/click.ogg": {{"hash": "{asset_b_hash}", "size": {}}}
            }}}}"#,
            asset_a.len(),
            asset_a.len(),
            asset_b.len()
        )
        .into_bytes();

        let mut bodies: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        bodies.insert("/mojang/client.jar".to_owned(), client.clone());
        bodies.insert(
            "/mojang/logging/client-1.21.2.xml".to_owned(),
            logging.clone(),
        );
        bodies.insert(
            "/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar".to_owned(),
            mojang_library.clone(),
        );
        bodies.insert(
            "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar".to_owned(),
            native_archive.clone(),
        );
        bodies.insert(
            "/mojang/asset-index/32.json".to_owned(),
            asset_index_body.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_a_hash[..2], asset_a_hash),
            asset_a.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_b_hash[..2], asset_b_hash),
            asset_b.clone(),
        );
        bodies.insert(
            "/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar".to_owned(),
            fabric_common.clone(),
        );
        bodies.insert(
            "/fabric-maven/net/fabricmc/intermediary/26.2/intermediary-26.2.jar".to_owned(),
            intermediary.clone(),
        );
        bodies.insert(
            "/fabric-maven/net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar".to_owned(),
            fabric_loader.clone(),
        );

        let broken = Arc::new(Mutex::new(HashSet::new()));
        let broken_for_handler = Arc::clone(&broken);
        let bodies_for_handler = Arc::new(bodies.clone());
        let server = TestServer::spawn(Arc::new(move |request| {
            if broken_for_handler.lock().unwrap().contains(&request.path) {
                return TestResponse::status(404);
            }
            match bodies_for_handler.get(&request.path) {
                Some(body) => TestResponse::ok(body),
                None => TestResponse::status(404),
            }
        }));

        let fixture = Fixture {
            server,
            bodies,
            broken,
        };
        let base = fixture.server.base_url().to_owned();
        let plan = synthetic_plan(
            &base,
            &client,
            &logging,
            &mojang_library,
            &native_archive,
            &asset_index_body,
            &fabric_common,
        );
        (fixture, plan)
    }

    #[allow(clippy::too_many_arguments)]
    fn synthetic_plan(
        base: &str,
        client: &[u8],
        logging: &[u8],
        mojang_library: &[u8],
        native_archive: &[u8],
        asset_index_body: &[u8],
        fabric_common: &[u8],
    ) -> GameInstallPlan {
        let version_document_json = format!(
            r#"{{
                "id": "26.2",
                "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {{ "component": "java-runtime-epsilon", "majorVersion": 25 }},
                "assetIndex": {{
                    "id": "32",
                    "sha1": "{}",
                    "size": {},
                    "totalSize": 999999,
                    "url": "{base}/mojang/asset-index/32.json"
                }},
                "downloads": {{
                    "client": {{
                        "sha1": "{}",
                        "size": {},
                        "url": "{base}/mojang/client.jar"
                    }}
                }},
                "logging": {{
                    "client": {{
                        "argument": "-Dlog4j.configurationFile=${{path}}",
                        "file": {{
                            "id": "client-1.21.2.xml",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/logging/client-1.21.2.xml"
                        }},
                        "type": "log4j2-xml"
                    }}
                }},
                "libraries": [
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                        }} }},
                        "name": "com.mojang:brigadier:1.0.18"
                    }},
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"
                        }} }},
                        "name": "org.lwjgl:lwjgl:3.4.1:natives-windows"
                    }}
                ],
                "arguments": {{
                    "game": ["--username", "${{auth_player_name}}"],
                    "jvm": ["-Djava.library.path=${{natives_directory}}"]
                }}
            }}"#,
            sha1_hex(asset_index_body),
            asset_index_body.len(),
            sha1_hex(client),
            client.len(),
            sha1_hex(logging),
            logging.len(),
            sha1_hex(mojang_library),
            mojang_library.len(),
            sha1_hex(native_archive),
            native_archive.len(),
        );
        let document = VersionDocument::from_json(&version_document_json).unwrap();
        let minecraft =
            plan_version_document(&document, PlatformProfile::current().unwrap()).unwrap();

        let fabric_common_sha256 = sha256_digest(fabric_common);
        let profile_json = format!(
            r#"{{
                "loader": {{ "separator": ".", "build": 5, "maven": "net.fabricmc:fabric-loader:0.19.5", "version": "0.19.5", "stable": true }},
                "intermediary": {{ "maven": "net.fabricmc:intermediary:26.2", "version": "26.2", "stable": true }},
                "launcherMeta": {{
                    "version": 2,
                    "min_java_version": 8,
                    "libraries": {{
                        "client": [],
                        "common": [
                            {{
                                "name": "org.ow2.asm:asm:9.10.1",
                                "url": "{base}/fabric-maven/",
                                "sha256": "{}",
                                "size": {}
                            }}
                        ],
                        "server": [],
                        "development": []
                    }},
                    "mainClass": {{
                        "client": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                        "server": "net.fabricmc.loader.impl.launch.knot.KnotServer"
                    }}
                }}
            }}"#,
            fabric_common_sha256.as_hex(),
            fabric_common.len(),
        );
        let profile = LoaderProfileDocument::from_json(&profile_json).unwrap();
        let fabric = plan_loader_profile(
            &profile,
            &crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap(),
            &crate::fabric::metadata::LoaderVersionId::new("0.19.5").unwrap(),
        )
        .unwrap();

        compose_game_plan(minecraft, fabric).unwrap()
    }

    fn test_context(server_base: &str) -> InstallContext {
        InstallContext {
            processor_java: None,
            processor_timeout: processors::SupervisorLimits::default().timeout,
            processor_cancel: processors::CancelSignal::default(),
            download_options: crate::downloads::DownloadOptions {
                connect_timeout: Duration::from_secs(5),
                idle_read_timeout: Duration::from_secs(5),
                max_redirects: crate::downloads::MAX_REDIRECTS,
            },
            asset_endpoints: AssetObjectEndpoints::loopback_for_testing(&format!(
                "{server_base}/assets/"
            )),
            small_artifact_concurrency: SMALL_ARTIFACT_CONCURRENCY,
        }
    }

    struct TestEnv {
        managed: ManagedPaths,
        instance: InstanceId,
    }

    impl TestEnv {
        /// Each test owns a distinct instance id: the installation exclusion
        /// is process-global, so shared ids would serialize (and collide)
        /// across concurrently running tests.
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join("aurora-install-test")
                .join(std::process::id().to_string())
                .join(name);
            let _ = std::fs::remove_dir_all(&root);
            let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
            let instance = InstanceId::new(&format!("inst-{name}")).unwrap();
            Self { managed, instance }
        }

        /// Creates user-sensitive content that installation must never touch.
        fn plant_user_data(&self) {
            let paths = self.managed.instance_paths(&self.instance);
            std::fs::create_dir_all(paths.mods()).unwrap();
            std::fs::write(paths.mods().join("user-mod.jar"), b"precious user mod").unwrap();
            std::fs::create_dir_all(paths.root().join("saves")).unwrap();
            std::fs::write(
                paths.root().join("saves").join("world.dat"),
                b"precious save",
            )
            .unwrap();
            std::fs::create_dir_all(paths.config()).unwrap();
            std::fs::write(paths.config().join("options.txt"), b"gamma:1.0").unwrap();
        }

        fn assert_user_data_intact(&self) {
            let paths = self.managed.instance_paths(&self.instance);
            assert_eq!(
                std::fs::read(paths.mods().join("user-mod.jar")).unwrap(),
                b"precious user mod"
            );
            assert_eq!(
                std::fs::read(paths.root().join("saves").join("world.dat")).unwrap(),
                b"precious save"
            );
            assert_eq!(
                std::fs::read(paths.config().join("options.txt")).unwrap(),
                b"gamma:1.0"
            );
        }

        fn staging(&self) -> PathBuf {
            self.managed
                .instance_paths(&self.instance)
                .root()
                .join(STAGING_DIR_NAME)
        }

        fn game(&self) -> PathBuf {
            self.managed
                .instance_paths(&self.instance)
                .game()
                .to_path_buf()
        }
    }

    async fn install(
        env: &TestEnv,
        fixture: &Fixture,
        plan: &GameInstallPlan,
        faults: InstallFaults,
    ) -> Result<InstalledGame, InstallError> {
        install_game(
            &env.managed,
            &env.instance,
            plan,
            &test_context(fixture.server.base_url()),
            &mut |_| {},
            faults,
        )
        .await
    }

    #[tokio::test]
    async fn a_synthetic_game_installs_completely_and_validates() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("happy");
        env.plant_user_data();

        let installed = install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the synthetic game must install");

        // The committed tree: everything the plan described, in place.
        let game = env.game();
        assert!(game.join("versions/26.2/client.jar").is_file());
        assert!(game.join("versions/26.2/client-1.21.2.xml").is_file());
        assert!(game.join("assets/indexes/32.json").is_file());
        assert!(
            game.join("libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar")
                .is_file()
        );
        assert!(
            game.join("libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar")
                .is_file()
        );
        assert!(
            game.join("libraries/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar")
                .is_file()
        );
        assert!(
            game.join("libraries/net/fabricmc/intermediary/26.2/intermediary-26.2.jar")
                .is_file()
        );
        assert!(
            game.join("libraries/net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar")
                .is_file()
        );

        // Asset objects: deduplicated by hash (two names, one object).
        let index = AssetIndexObjectsDocument::from_json(
            &std::fs::read_to_string(game.join("assets/indexes/32.json")).unwrap(),
        )
        .unwrap();
        let objects = assets::plan_asset_objects(&index).unwrap();
        assert_eq!(objects.len(), 2);
        for object in &objects {
            assert!(game.join(object.game_relative_path()).is_file());
        }

        // Natives: extracted, without META-INF.
        let natives = game.join("natives/26.2");
        assert!(natives.join("lwjgl.dll").is_file());
        assert!(natives.join("liblwjgl.dylib").is_file());
        assert!(!natives.join("META-INF").exists());

        // The completion record is committed inside the game directory.
        let manifest = load_installed_state(&game)
            .unwrap()
            .expect("manifest committed");
        assert_eq!(manifest.minecraft_version(), "26.2");
        assert_eq!(manifest.fabric_loader_version(), Some("0.19.5"));
        assert!(manifest.files().len() >= 9);

        // Trust classes are distinct and honest.
        let trust: Vec<&ArtifactTrust> = manifest.files().iter().map(|f| f.trust()).collect();
        assert!(trust.iter().any(|t| matches!(
            t,
            ArtifactTrust::ExpectedDigestVerified {
                algorithm: DigestAlgorithm::Sha1,
                ..
            }
        )));
        assert!(trust.iter().any(|t| matches!(
            t,
            ArtifactTrust::ExpectedDigestVerified {
                algorithm: DigestAlgorithm::Sha256,
                ..
            }
        )));
        assert!(
            trust
                .iter()
                .any(|t| matches!(t, ArtifactTrust::SecureTransportObserved { .. }))
        );

        // Deterministic installed-state: validation passes cleanly.
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
                assert!(validation.problems.is_empty());
                assert_eq!(validation.checked_files, manifest.files().len());
            }
            ValidationOutcome::NotInstalled => panic!("the installed game must validate"),
        }

        // No staging remains; user data untouched.
        assert!(!env.staging().exists());
        env.assert_user_data_intact();
        assert_eq!(installed.total_byte_count() > 0, true);
    }

    #[tokio::test]
    async fn an_acquisition_failure_leaves_no_complete_looking_installation() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("acquisition-failure");
        env.plant_user_data();
        fixture.break_path("/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar");

        let error = install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect_err("a 404 artifact must fail the installation");

        assert!(matches!(error, InstallError::Acquisition(_)), "{error}");
        assert!(!env.game().exists(), "no game directory may exist");
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn a_materialization_failure_leaves_only_staging() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("materialization-failure");
        env.plant_user_data();

        let error = install(
            &env,
            &fixture,
            &plan,
            InstallFaults {
                fail_during_materialization: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected materialization fault must fail");

        assert!(matches!(error, InstallError::Materialization { .. }));
        assert!(!env.game().exists());
        // Staging exists (deliberate recoverable state) but is clearly not a
        // game directory, and it contains no completion record at its own
        // root or the instance root.
        assert!(env.staging().exists());
        assert!(
            !env.managed
                .instance_paths(&env.instance)
                .root()
                .join(INSTALLED_GAME_FILE_NAME)
                .exists()
        );
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn a_malicious_native_archive_fails_extraction_safely() {
        // A traversal-bearing archive whose SHA-1 is computed from its own
        // bytes: it "verifies" as an artifact, so extraction safety must be
        // the line of defense.
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            writer
                .start_file("../evil.dll", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"escape attempt").unwrap();
            writer.finish().unwrap();
        }
        let malicious = cursor.into_inner();

        let (fixture, _plan) = synthetic_game();
        let env = TestEnv::new("malicious-native");
        // Serve the fixture bodies with the native artifact replaced by the
        // malicious archive (whose SHA-1 the plan is rebuilt around, so it
        // "verifies" — extraction safety is the line of defense).
        let mut bodies = fixture.bodies.clone();
        bodies.insert(
            "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar".to_owned(),
            malicious.clone(),
        );
        let served: Arc<BTreeMap<String, Vec<u8>>> = Arc::new(bodies);
        let server = TestServer::spawn(Arc::new(move |request| {
            served
                .get(&request.path)
                .map(|body| TestResponse::ok(body))
                .unwrap_or(TestResponse::status(404))
        }));

        let plan = synthetic_plan(
            server.base_url(),
            &fixture.bodies["/mojang/client.jar"],
            &fixture.bodies["/mojang/logging/client-1.21.2.xml"],
            &fixture.bodies["/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"],
            &malicious,
            &fixture.bodies["/mojang/asset-index/32.json"],
            &fixture.bodies["/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"],
        );

        let error = install_game(
            &env.managed,
            &env.instance,
            &plan,
            &test_context(server.base_url()),
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .expect_err("a traversal archive must fail extraction");

        assert!(
            matches!(
                error,
                InstallError::Native(NativeExtractionError::ArchiveInvalid { .. })
            ),
            "{error}"
        );
        assert!(!env.game().exists());
        // Nothing escaped staging.
        let instance_root = env
            .managed
            .instance_paths(&env.instance)
            .root()
            .to_path_buf();
        assert!(!instance_root.join("evil.dll").exists());
    }

    #[tokio::test]
    async fn an_injected_pre_commit_failure_never_completes_the_installation() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("pre-commit-failure");
        env.plant_user_data();

        let error = install(
            &env,
            &fixture,
            &plan,
            InstallFaults {
                fail_before_commit: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected commit fault must fail");

        assert!(matches!(error, InstallError::Commit { .. }));
        // No game directory: the staged tree (complete, manifest included)
        // lives only under staging and cannot be mistaken for an install.
        assert!(!env.game().exists());
        assert!(
            env.staging()
                .join("game")
                .join(INSTALLED_GAME_FILE_NAME)
                .is_file()
        );
        env.assert_user_data_intact();

        // Retry is deliberate: the next attempt clears staging and succeeds.
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the retry must succeed");
        assert!(env.game().join(INSTALLED_GAME_FILE_NAME).is_file());
        assert!(!env.staging().exists());
    }

    #[tokio::test]
    async fn stale_staging_from_an_interrupted_attempt_is_cleared_and_rebuilt() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("stale-staging");
        let staging = env.staging();
        std::fs::create_dir_all(staging.join("game").join("junk")).unwrap();
        std::fs::write(
            staging.join("game/junk/leftover.bin"),
            b"interrupted debris",
        )
        .unwrap();

        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("stale staging must not block a fresh install");

        assert!(env.game().join(INSTALLED_GAME_FILE_NAME).is_file());
        assert!(!staging.exists());
    }

    #[tokio::test]
    async fn an_unrecognized_existing_game_directory_is_a_hard_conflict() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("foreign-game");
        let game = env.game();
        std::fs::create_dir_all(game.join("versions")).unwrap();
        std::fs::write(
            game.join("versions").join("foreign.txt"),
            b"someone else's data",
        )
        .unwrap();

        let error = install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect_err("unrecognizable existing state must conflict");

        assert!(
            matches!(error, InstallError::TargetConflict { .. }),
            "{error}"
        );
        // The foreign content is untouched.
        assert_eq!(
            std::fs::read(game.join("versions").join("foreign.txt")).unwrap(),
            b"someone else's data"
        );
    }

    #[tokio::test]
    async fn a_valid_installation_is_deliberately_replaced_and_user_data_survives() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("replace");
        env.plant_user_data();

        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .unwrap();
        let first_manifest = load_installed_state(&env.game()).unwrap().unwrap();

        // A second installation of the same plan replaces the first: the
        // game directory is proven-managed by its manifest.
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("replacement must be supported");

        let second_manifest = load_installed_state(&env.game()).unwrap().unwrap();
        assert_ne!(
            first_manifest.installation_id(),
            second_manifest.installation_id(),
            "the replacement is a new revision"
        );
        assert!(!env.staging().exists(), "the retired tree is gone");
        env.assert_user_data_intact();

        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the replacement must validate"),
        }
    }

    #[tokio::test]
    async fn tampering_is_detected_by_validation_without_mutation() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("tampered");
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .unwrap();

        // Corrupt one managed file after installation.
        let client = env.game().join("versions/26.2/client.jar");
        std::fs::write(&client, b"tampered bytes").unwrap();
        // Remove one asset object entirely.
        let index_text =
            std::fs::read_to_string(env.game().join("assets/indexes/32.json")).unwrap();
        let index = AssetIndexObjectsDocument::from_json(&index_text).unwrap();
        let victim = &assets::plan_asset_objects(&index).unwrap()[0];
        std::fs::remove_file(env.game().join(victim.game_relative_path())).unwrap();

        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Damaged);
                assert_eq!(validation.problems.len(), 2, "{:?}", validation.problems);
                assert!(
                    validation
                        .problems
                        .iter()
                        .any(|p| p.path.ends_with("client.jar"))
                );
            }
            ValidationOutcome::NotInstalled => panic!("tampering must be reported, not hidden"),
        }

        // Validation mutated nothing: the tampered file is still the tampered
        // bytes, and the manifest still describes the original expectation.
        assert_eq!(std::fs::read(&client).unwrap(), b"tampered bytes");
    }

    #[tokio::test]
    async fn validation_reports_an_absent_installation_as_not_installed() {
        let env = TestEnv::new("not-installed");
        assert_eq!(
            validate_installed_game(&env.managed, &env.instance).unwrap(),
            ValidationOutcome::NotInstalled
        );
    }

    #[tokio::test]
    async fn cancellation_at_acquisition_copy_and_validation_preserves_the_previous_game() {
        for phase in [
            InstallPhase::Acquiring,
            InstallPhase::Materializing,
            InstallPhase::Validating,
        ] {
            let (fixture, plan) = synthetic_game();
            let env = TestEnv::new(&format!("f3r-cancel-{}", phase.as_str()));
            env.plant_user_data();
            install(&env, &fixture, &plan, Default::default())
                .await
                .unwrap();
            let before = std::fs::read(env.game().join(INSTALLED_GAME_FILE_NAME)).unwrap();
            let (cancel, signal) = processors::cancellation_channel();
            let context = test_context(fixture.server.base_url()).with_processor_cancel(signal);
            let error = install_game(
                &env.managed,
                &env.instance,
                &plan,
                &context,
                &mut |event| {
                    if event.phase == phase {
                        cancel.cancel();
                    }
                },
                Default::default(),
            )
            .await
            .unwrap_err();
            assert!(
                matches!(error, InstallError::Cancelled),
                "{phase:?}: {error}"
            );
            assert_eq!(
                std::fs::read(env.game().join(INSTALLED_GAME_FILE_NAME)).unwrap(),
                before
            );
            recover_installation(&env.managed, &env.instance).unwrap();
            assert!(
                matches!(validate_installed_game(&env.managed, &env.instance).unwrap(), ValidationOutcome::Installed(validation) if validation.status == ValidationStatus::Valid)
            );
            env.assert_user_data_intact();
        }
    }

    #[tokio::test]
    async fn a_malformed_installed_state_manifest_fails_deliberately() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("malformed-state");
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .unwrap();

        let manifest_path = env.game().join(INSTALLED_GAME_FILE_NAME);
        std::fs::write(&manifest_path, "{ not valid json").unwrap();

        let error = validate_installed_game(&env.managed, &env.instance)
            .expect_err("malformed installed state must fail");
        assert!(matches!(error, InstallError::State(_)), "{error}");

        // Reinstallation also refuses to wipe it: the state is the user's to
        // repair, not the installer's to overwrite.
        let error = install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect_err("conflicting malformed state must not be replaced");
        assert!(matches!(error, InstallError::State(_)), "{error}");
        assert_eq!(
            std::fs::read_to_string(&manifest_path).unwrap(),
            "{ not valid json"
        );
    }

    #[tokio::test]
    async fn simultaneous_installs_of_one_instance_are_excluded() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("concurrent");

        // Slow the client transfer so the first install is still running
        // when the second attempt arrives. A drip-delayed re-serve of the
        // client body achieves this without touching the fixture map.
        let slow_bodies: Arc<BTreeMap<String, Vec<u8>>> = Arc::new(fixture.bodies.clone());
        let slow_server = TestServer::spawn(Arc::new(move |request| {
            slow_bodies
                .get(&request.path)
                .map(|body| TestResponse::ok(body).with_drip_delay(Duration::from_millis(8)))
                .unwrap_or(TestResponse::status(404))
        }));
        let slow_plan = synthetic_plan(
            slow_server.base_url(),
            &fixture.bodies["/mojang/client.jar"],
            &fixture.bodies["/mojang/logging/client-1.21.2.xml"],
            &fixture.bodies["/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"],
            &native_zip_bytes(),
            &fixture.bodies["/mojang/asset-index/32.json"],
            &fixture.bodies["/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"],
        );

        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let first = {
            let managed = env.managed.clone();
            let instance = env.instance.clone();
            let context = test_context(slow_server.base_url());
            let plan = slow_plan.clone();
            let mut started_tx = Some(started_tx);
            tokio::spawn(async move {
                install_game(
                    &managed,
                    &instance,
                    &plan,
                    &context,
                    &mut |_| {
                        if let Some(sender) = started_tx.take() {
                            let _ = sender.send(());
                        }
                    },
                    InstallFaults::default(),
                )
                .await
            })
        };
        started_rx.await.expect("the first installation must start");

        let second = install(&env, &fixture, &plan, InstallFaults::default()).await;

        assert!(
            matches!(second, Err(InstallError::AlreadyInProgress { .. })),
            "the overlapping attempt must be excluded, got: {:?}",
            second.err()
        );
        first
            .await
            .unwrap()
            .expect("the first installation completes");
        assert!(env.game().join(INSTALLED_GAME_FILE_NAME).is_file());
    }

    #[tokio::test]
    async fn digest_less_fabric_artifacts_are_never_reported_as_digest_verified() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("trust-honesty");
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .unwrap();

        let manifest = load_installed_state(&env.game()).unwrap().unwrap();
        let loader_entry = manifest
            .files()
            .iter()
            .find(|file| file.path().contains("fabric-loader"))
            .unwrap();
        assert_eq!(loader_entry.trust().kind_name(), "secureTransportObserved");

        let asm_entry = manifest
            .files()
            .iter()
            .find(|file| file.path().contains("org/ow2/asm"))
            .unwrap();
        assert_eq!(loader_entry.trust().kind_name(), "secureTransportObserved");
        assert_eq!(asm_entry.trust().kind_name(), "expectedDigestVerified");

        // And the persisted document keeps the distinction machine-readable.
        let text = std::fs::read_to_string(env.game().join(INSTALLED_GAME_FILE_NAME)).unwrap();
        assert!(text.contains("\"secureTransportObserved\""));
        assert!(text.contains("\"expectedDigestVerified\""));
        assert!(text.contains("\"algorithm\": \"sha1\""));
        assert!(text.contains("\"algorithm\": \"sha256\""));
    }

    fn parallel_test_world(
        broken: Option<String>,
    ) -> (TestServer, GameInstallPlan, Arc<AtomicUsize>) {
        let (fixture, _) = synthetic_game();
        let bodies = Arc::new(fixture.bodies.clone());
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let server = TestServer::spawn(Arc::new({
            let bodies = Arc::clone(&bodies);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            move |request| {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(count, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(35));
                let response = if broken.as_deref() == Some(request.path.as_str()) {
                    TestResponse::status(503)
                } else {
                    bodies
                        .get(&request.path)
                        .map(|body| TestResponse::ok(body))
                        .unwrap_or_else(|| TestResponse::status(404))
                };
                active.fetch_sub(1, Ordering::SeqCst);
                response
            }
        }));
        let base = server.base_url();
        let plan = synthetic_plan(
            base,
            &fixture.bodies["/mojang/client.jar"],
            &fixture.bodies["/mojang/logging/client-1.21.2.xml"],
            &fixture.bodies["/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"],
            &fixture.bodies["/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"],
            &fixture.bodies["/mojang/asset-index/32.json"],
            &fixture.bodies["/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"],
        );
        (server, plan, maximum)
    }

    #[tokio::test]
    async fn independent_acquisitions_are_concurrent_and_bounded() {
        let (server, plan, maximum) = parallel_test_world(None);
        let root =
            std::env::temp_dir().join(format!("aurora-parallel-test-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let instance = InstanceId::new("parallel-bound").unwrap();
        let context = test_context(server.base_url()).with_concurrency_for_testing(2);
        install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap();
        let observed = maximum.load(Ordering::SeqCst);
        assert!(
            observed > 1,
            "independent requests must overlap: {observed}"
        );
        assert!(observed <= 2, "the configured bound must hold: {observed}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    async fn one_failed_concurrent_acquisition_never_commits_installed_state() {
        let (fixture, _) = synthetic_game();
        let broken_asset = fixture
            .bodies
            .keys()
            .find(|path| path.starts_with("/assets/"))
            .unwrap()
            .clone();
        let (server, plan, maximum) = parallel_test_world(Some(broken_asset));
        let root =
            std::env::temp_dir().join(format!("aurora-parallel-failure-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let instance = InstanceId::new("parallel-failure").unwrap();
        let context = test_context(server.base_url()).with_concurrency_for_testing(2);
        let error = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, InstallError::Acquisition(_)), "{error}");
        assert!(maximum.load(Ordering::SeqCst) <= 2);
        assert!(!managed.instance_paths(&instance).game().exists());
        assert!(
            std::fs::read_dir(ArtifactCache::new(managed.clone()).staging_dir())
                .unwrap()
                .next()
                .is_none()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Opt-in wall-clock diagnostic through the real game executor. The
    /// loopback host supplies many small independent assets with fixed
    /// latency; no public service or user launcher data is involved.
    #[tokio::test]
    #[ignore = "controlled installation performance diagnostic"]
    async fn benchmark_controlled_game_installation() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        use std::time::Instant;

        let (fixture, _) = synthetic_game();
        let mut bodies = fixture.bodies.clone();
        let mut objects = serde_json::Map::new();
        for number in 0..192u32 {
            let body = format!("benchmark asset {number:04} fixed payload").into_bytes();
            let hash = sha1_hex(&body);
            bodies.insert(format!("/assets/{}/{hash}", &hash[..2]), body.clone());
            objects.insert(
                format!("minecraft/sounds/benchmark/{number:04}.ogg"),
                serde_json::json!({"hash": hash, "size": body.len()}),
            );
        }
        let index = serde_json::json!({"objects": objects})
            .to_string()
            .into_bytes();
        bodies.insert("/mojang/asset-index/32.json".to_owned(), index.clone());
        let served = Arc::new(bodies);
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let bytes = Arc::new(AtomicU64::new(0));
        let server = TestServer::spawn(Arc::new({
            let served = Arc::clone(&served);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            let bytes = Arc::clone(&bytes);
            move |request| {
                let in_flight = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(in_flight, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(18));
                let response = served
                    .get(&request.path)
                    .map(|body| {
                        bytes.fetch_add(body.len() as u64, Ordering::SeqCst);
                        TestResponse::ok(body)
                    })
                    .unwrap_or_else(|| TestResponse::status(404));
                active.fetch_sub(1, Ordering::SeqCst);
                response
            }
        }));
        let base = server.base_url();
        let plan = synthetic_plan(
            base,
            &served["/mojang/client.jar"],
            &served["/mojang/logging/client-1.21.2.xml"],
            &served["/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"],
            &served["/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"],
            &index,
            &served["/fabric-maven/org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"],
        );
        let root = std::env::temp_dir().join(format!("aurora-perf-bench-{}", uuid::Uuid::new_v4()));
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let concurrency = std::env::var("AURORA_BENCH_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| (1..=32).contains(value))
            .unwrap_or(SMALL_ARTIFACT_CONCURRENCY);
        let context = test_context(base).with_concurrency_for_testing(concurrency);
        for (scenario, id) in [
            ("cold", "benchmark-first"),
            ("warm-reinstall", "benchmark-first"),
            ("second-instance", "benchmark-second"),
        ] {
            let instance = InstanceId::new(id).unwrap();
            let before_requests = server.request_count();
            let before_bytes = bytes.load(Ordering::SeqCst);
            maximum.store(0, Ordering::SeqCst);
            let start = Instant::now();
            let mut phases = Vec::new();
            let mut previous = "start";
            install_game(
                &managed,
                &instance,
                &plan,
                &context,
                &mut |progress| {
                    let next = if progress.phase == InstallPhase::Acquiring {
                        match progress.current_item.as_deref() {
                            Some("asset index") => "asset-index",
                            Some(item) if item.contains(" client") => "client",
                            Some(item) if item.starts_with("logging configuration") => "logging",
                            Some(item) if item.starts_with("asset ") => "assets",
                            _ => "libraries",
                        }
                    } else {
                        progress.phase.as_str()
                    };
                    if next != previous {
                        phases.push((next, start.elapsed().as_millis()));
                        previous = next;
                    }
                },
                InstallFaults::default(),
            )
            .await
            .unwrap();
            let total_ms = start.elapsed().as_millis();
            let validation_start = Instant::now();
            let validation = validate_installed_game(&managed, &instance).unwrap();
            assert!(matches!(
                validation,
                ValidationOutcome::Installed(InstalledGameValidation {
                    status: ValidationStatus::Valid,
                    ..
                })
            ));
            println!(
                "BENCH limit={concurrency} {scenario} total_ms={total_ms} validation_ms={} requests={} downloaded_bytes={} max_concurrency={} phases={phases:?}",
                validation_start.elapsed().as_millis(),
                server.request_count() - before_requests,
                bytes.load(Ordering::SeqCst) - before_bytes,
                maximum.load(Ordering::SeqCst),
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Controlled live installation against the real official Mojang and
    /// Fabric chains, end to end: resolve both plans, compose, install into
    /// a disposable temp-root instance, and validate the result.
    ///
    /// Ignored by default so the offline suite never depends on public
    /// services or downloads hundreds of megabytes; run explicitly with
    /// `cargo test -- --ignored --nocapture` when verifying. It downloads
    /// the real game artifacts into a throwaway directory outside the
    /// launcher's own managed data root — never the user's `.minecraft`,
    /// never the launcher's real instance storage.
    #[tokio::test]
    #[ignore = "downloads the real Minecraft + Fabric artifact set from official sources"]
    async fn live_official_game_installs_and_validates() {
        use crate::fabric;
        use crate::fabric::metadata::{FabricMetaEndpoints, LoaderVersionId};
        use crate::minecraft::metadata::{MetadataEndpoints, MinecraftVersionId};

        let game_version = "26.2";
        let loader_version = "0.19.5";
        let platform = PlatformProfile::current().expect("the host platform must be plannable");
        let options = crate::downloads::DownloadOptions::default();

        let plan = fabric::resolve_game_plan(
            &MetadataEndpoints::official(),
            &FabricMetaEndpoints::official(),
            &MinecraftVersionId::new(game_version).unwrap(),
            &LoaderVersionId::new(loader_version).unwrap(),
            platform,
            &options,
        )
        .await
        .expect("the live plan must resolve");

        // A throwaway managed root under the system temp directory.
        let root = std::env::temp_dir()
            .join("aurora-live-install-test")
            .join(format!("{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let managed = ManagedPaths::from_app_local_data_dir(root.join("managed")).unwrap();
        let instance = InstanceId::new("live-verify").unwrap();

        let mut events = 0usize;
        let installed = install_game(
            &managed,
            &instance,
            &plan,
            &InstallContext::official(),
            &mut |progress| {
                events += 1;
                eprintln!(
                    "[live] {:?} {}/{} {:?}",
                    progress.phase,
                    progress.completed_items,
                    progress.total_items,
                    progress.current_item
                );
            },
            InstallFaults::default(),
        )
        .await
        .expect("the live installation must complete");

        eprintln!(
            "[live] installed {} + {} ({} files, {} bytes) into {}",
            installed.manifest().minecraft_version(),
            installed
                .manifest()
                .fabric_loader_version()
                .unwrap_or("Vanilla"),
            installed.manifest().files().len(),
            installed.total_byte_count(),
            installed.game_directory().display()
        );

        // Trust classification of digest-less Fabric artifacts is honest.
        let manifest = load_installed_state(installed.game_directory())
            .unwrap()
            .expect("the manifest must be committed");
        let loader_files: Vec<&InstalledFile> = manifest
            .files()
            .iter()
            .filter(|file| {
                file.path().contains("fabric-loader") || file.path().contains("intermediary")
            })
            .collect();
        assert!(!loader_files.is_empty());
        for file in &loader_files {
            assert_eq!(file.trust().kind_name(), "secureTransportObserved");
            eprintln!(
                "[live] digest-less artifact {} → {}",
                file.path(),
                file.trust()
            );
        }

        // Validation passes against the committed manifest.
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
                eprintln!(
                    "[live] validated {} files ({} bytes), installation {}",
                    validation.checked_files,
                    verification_bytes(&validation),
                    validation.installation_id
                );
            }
            ValidationOutcome::NotInstalled => panic!("the live install must validate"),
        }

        // No staging debris, and no user-data directories were created.
        let instance_paths = managed.instance_paths(&instance);
        let instance_root = instance_paths.root();
        assert!(!instance_root.join(STAGING_DIR_NAME).exists());
        assert!(events > 0, "progress must have been reported");

        fn verification_bytes(validation: &InstalledGameValidation) -> u64 {
            validation.verified_bytes
        }
    }
    // ---- NeoForge end-to-end installation (Phase K) ----

    /// The synthetic installer documents. `client_processor` selects the
    /// client-side processor mode (`write`, `fail`, ...); `None` keeps the
    /// profile server-only so no install-time Java is needed.
    fn neoforge_documents(
        base: &str,
        client_processor: Option<&str>,
        installertools: &[u8],
    ) -> (String, String) {
        let client_step = match client_processor {
            Some(mode) => format!(
                r#",
                {{"jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--mode", "{mode}", "--input", "{{MINECRAFT_JAR}}", "--output", "{{PATCHED}}", "--apply-patches", "{{BINPATCH}}"]}}"#
            ),
            None => String::new(),
        };
        let profile = format!(
            r#"{{
              "spec": 1,
              "profile": "NeoForge",
              "version": "neoforge-26.2.0.88",
              "minecraft": "26.2",
              "json": "/version.json",
              "data": {{
                "BINPATCH": {{"client": "/data/client.lzma", "server": "/data/client.lzma"}},
                "PATCHED": {{"client": "[net.neoforged:minecraft-client-patched:26.2.0.88]", "server": "[net.neoforged:minecraft-server-patched:26.2.0.88]"}}
              }},
              "processors": [
                {{"sides": ["server"], "jar": "net.neoforged.installertools:installertools:4.0.17:fatjar", "classpath": ["net.neoforged.installertools:installertools:4.0.17:fatjar"], "args": ["--task", "EXTRACT_FILES"]}}{client_step}
              ],
              "libraries": [
                {{"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {{"artifact": {{"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "{base}/neoforge-maven/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "{}", "size": {}}}}}}},
                {{"name": "net.neoforged.installertools:installertools:4.0.17:fatjar", "downloads": {{"artifact": {{"path": "net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "url": "{base}/neoforge-maven/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar", "sha1": "{}", "size": {}}}}}}}
              ]
            }}"#,
            sha1_hex(b"synthetic fml loader jar"),
            b"synthetic fml loader jar".len(),
            sha1_hex(installertools),
            installertools.len(),
        );
        let version = format!(
            r#"{{
              "id": "neoforge-26.2.0.88",
              "inheritsFrom": "26.2",
              "mainClass": "net.neoforged.fml.startup.Client",
              "arguments": {{
                "jvm": ["-DlibraryDirectory=${{library_directory}}"],
                "game": ["--fml.neoForgeVersion", "26.2.0.88", "--fml.mcVersion", "26.2"]
              }},
              "libraries": [
                {{"name": "net.neoforged.fancymodloader:loader:11.0.16", "downloads": {{"artifact": {{"path": "net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "url": "{base}/neoforge-maven/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar", "sha1": "{}", "size": {}}}}}}},
                {{"name": "cpw.mods:bootstraplauncher:2.0.2", "downloads": {{"artifact": {{"path": "cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "url": "{base}/neoforge-maven/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "sha1": "{}", "size": {}}}}}}}
              ]
            }}"#,
            sha1_hex(b"synthetic fml loader jar"),
            b"synthetic fml loader jar".len(),
            sha1_hex(b"synthetic bootstraplauncher jar"),
            b"synthetic bootstraplauncher jar".len(),
        );
        (profile, version)
    }

    fn neoforge_installer_jar(profile: &str, version: &str) -> Vec<u8> {
        use std::io::Write as _;
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, bytes) in [
                ("install_profile.json", profile.as_bytes().to_vec()),
                ("version.json", version.as_bytes().to_vec()),
                ("data/client.lzma", b"synthetic binpatch bundle".to_vec()),
            ] {
                writer
                    .start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                writer.write_all(&bytes).unwrap();
            }
            writer.finish().unwrap();
        }
        cursor.into_inner()
    }

    /// The complete synthetic NeoForge world: Mojang pieces plus the
    /// NeoForge repository, and the composed plan over them.
    fn synthetic_neoforge_game(client_processor: Option<&str>) -> (Fixture, GameInstallPlan) {
        let client = b"synthetic client jar bytes".to_vec();
        let logging = b"<Configuration status=\"WARN\"></Configuration>".to_vec();
        let mojang_library = b"synthetic mojang library jar".to_vec();
        let native_archive = native_zip_bytes();

        let asset_a = b"tiny png bytes a".to_vec();
        let asset_b = b"tiny png bytes b".to_vec();
        let asset_a_hash = sha1_hex(&asset_a);
        let asset_b_hash = sha1_hex(&asset_b);
        let asset_index_body = format!(
            r#"{{"objects": {{
                "icons/icon_16x16.png": {{"hash": "{asset_a_hash}", "size": {}}},
                "minecraft/sounds/click.ogg": {{"hash": "{asset_b_hash}", "size": {}}}
            }}}}"#,
            asset_a.len(),
            asset_b.len(),
        )
        .into_bytes();

        let mut bodies: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        bodies.insert("/mojang/client.jar".to_owned(), client.clone());
        bodies.insert(
            "/mojang/logging/client-1.21.2.xml".to_owned(),
            logging.clone(),
        );
        bodies.insert(
            "/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar".to_owned(),
            mojang_library.clone(),
        );
        bodies.insert(
            "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar".to_owned(),
            native_archive.clone(),
        );
        bodies.insert(
            "/mojang/asset-index/32.json".to_owned(),
            asset_index_body.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_a_hash[..2], asset_a_hash),
            asset_a.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_b_hash[..2], asset_b_hash),
            asset_b.clone(),
        );
        bodies.insert(
            "/neoforge-maven/net/neoforged/fancymodloader/loader/11.0.16/loader-11.0.16.jar"
                .to_owned(),
            b"synthetic fml loader jar".to_vec(),
        );
        bodies.insert(
            "/neoforge-maven/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar"
                .to_owned(),
            b"synthetic bootstraplauncher jar".to_vec(),
        );
        // When a client processor runs, the installertools artifact is the
        // compiled synthetic tool itself; otherwise any verified bytes do.
        let installertools_body = if client_processor.is_some() {
            crate::neoforge::processors::test_tools::tool_and_java()
                .map(|(tool, _java)| std::fs::read(&tool).expect("the compiled tool jar"))
                .expect("a JDK is required for the processing fixture")
        } else {
            b"synthetic installertools fatjar".to_vec()
        };
        bodies.insert(
            "/neoforge-maven/net/neoforged/installertools/installertools/4.0.17/installertools-4.0.17-fatjar.jar".to_owned(),
            installertools_body.clone(),
        );

        let broken = Arc::new(Mutex::new(HashSet::new()));

        // The installer documents embed the server base, and the installer
        // bytes embed the documents: serve from a shared mutable body map
        // so the installer can be published once the base is known.
        let shared_bodies = Arc::new(Mutex::new(bodies.clone()));
        let bodies_for_handler = Arc::clone(&shared_bodies);
        let broken_for_handler = Arc::clone(&broken);
        let server = TestServer::spawn(Arc::new(move |request| {
            if broken_for_handler.lock().unwrap().contains(&request.path) {
                return TestResponse::status(404);
            }
            let bodies = bodies_for_handler.lock().unwrap();
            match bodies.get(&request.path) {
                Some(body) => TestResponse::ok(body),
                None => TestResponse::status(404),
            }
        }));
        let base = server.base_url().to_owned();

        let endpoints =
            crate::neoforge::metadata::NeoForgeMavenEndpoints::loopback_for_testing(&base);
        let (profile, version) = neoforge_documents(&base, client_processor, &installertools_body);
        let installer = neoforge_installer_jar(&profile, &version);
        let universal = b"synthetic universal jar".to_vec();
        let installer_path =
            "/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-installer.jar";
        let universal_path =
            "/releases/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-universal.jar";
        shared_bodies
            .lock()
            .unwrap()
            .insert(installer_path.to_owned(), installer.clone());
        shared_bodies.lock().unwrap().insert(
            format!("{installer_path}.sha1"),
            sha1_hex(&installer).into_bytes(),
        );
        shared_bodies
            .lock()
            .unwrap()
            .insert(universal_path.to_owned(), universal.clone());
        shared_bodies.lock().unwrap().insert(
            format!("{universal_path}.sha1"),
            sha1_hex(&universal).into_bytes(),
        );

        let version_document_json = format!(
            r#"{{
                "id": "26.2",
                "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {{ "component": "java-runtime-epsilon", "majorVersion": 25 }},
                "assetIndex": {{
                    "id": "32",
                    "sha1": "{}",
                    "size": {},
                    "totalSize": 999999,
                    "url": "{base}/mojang/asset-index/32.json"
                }},
                "downloads": {{
                    "client": {{
                        "sha1": "{}",
                        "size": {},
                        "url": "{base}/mojang/client.jar"
                    }}
                }},
                "logging": {{
                    "client": {{
                        "argument": "-Dlog4j.configurationFile=${{path}}",
                        "file": {{
                            "id": "client-1.21.2.xml",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/logging/client-1.21.2.xml"
                        }},
                        "type": "log4j2-xml"
                    }}
                }},
                "libraries": [
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                        }} }},
                        "name": "com.mojang:brigadier:1.0.18"
                    }},
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"
                        }} }},
                        "name": "org.lwjgl:lwjgl:3.4.1:natives-windows"
                    }}
                ],
                "arguments": {{
                    "game": ["--username", "${{auth_player_name}}"],
                    "jvm": ["-Djava.library.path=${{natives_directory}}"]
                }}
            }}"#,
            sha1_hex(&asset_index_body),
            asset_index_body.len(),
            sha1_hex(&client),
            client.len(),
            sha1_hex(&logging),
            logging.len(),
            sha1_hex(&mojang_library),
            mojang_library.len(),
            sha1_hex(&native_archive),
            native_archive.len(),
        );
        let document = VersionDocument::from_json(&version_document_json).unwrap();
        let minecraft =
            plan_version_document(&document, PlatformProfile::current().unwrap()).unwrap();

        let neoforge = crate::neoforge::plan::NeoForgePlan::from_documents(
            &crate::neoforge::metadata::InstallProfileDocument::from_json(&profile).unwrap(),
            &crate::neoforge::metadata::VersionProfileDocument::from_json(&version).unwrap(),
            &crate::minecraft::metadata::MinecraftVersionId::new("26.2").unwrap(),
            &crate::neoforge::metadata::NeoForgeVersionId::new("26.2.0.88").unwrap(),
            &endpoints,
            crate::integrity::Sha1Digest::parse(&sha1_hex(&installer)).unwrap(),
            crate::integrity::Sha1Digest::parse(&sha1_hex(b"synthetic universal jar")).unwrap(),
        )
        .unwrap();

        let plan = crate::neoforge::plan::compose_neoforge_game_plan(minecraft, neoforge).unwrap();
        let fixture = Fixture {
            server,
            bodies: shared_bodies.lock().unwrap().clone(),
            broken,
        };
        (fixture, plan)
    }

    fn neoforge_managed_root(name: &str) -> (crate::paths::ManagedPaths, std::path::PathBuf) {
        let root = std::env::temp_dir()
            .join("aurora-neoforge-install-tests")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        (managed, root)
    }

    #[tokio::test]
    async fn a_synthetic_neoforge_game_installs_and_validates() {
        let (fixture, plan) = synthetic_neoforge_game(None);
        assert!(plan.neoforge_loader().unwrap().processors().is_empty());
        let context = test_context(fixture.server.base_url());

        let (managed, root) = neoforge_managed_root("basic");
        let instance = InstanceId::new("11111111-1111-4111-8111-111111111111").unwrap();
        let mut progress_events = 0usize;
        let installed = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_progress| progress_events += 1,
            InstallFaults::default(),
        )
        .await
        .unwrap();

        let manifest = installed.manifest();
        assert_eq!(manifest.minecraft_version(), "26.2");
        assert_eq!(
            manifest.platform(),
            crate::instances::platform::PlatformPin::NeoForge {
                version: "26.2.0.88".to_owned()
            }
        );
        // The universal artifact is a managed library-tree discovery.
        assert!(manifest.files().iter().any(|file| file.path()
            == "libraries/net/neoforged/neoforge/26.2.0.88/neoforge-26.2.0.88-universal.jar"));
        // Every NeoForge launch library is a recorded, digest-verified
        // classpath entry.
        for coordinate in [
            "net.neoforged.fancymodloader:loader:11.0.16",
            "cpw.mods:bootstraplauncher:2.0.2",
        ] {
            let library = plan
                .libraries()
                .iter()
                .find(|library| library.coordinate_string() == coordinate)
                .expect("composed plan carries the launch library");
            assert!(library.is_classpath_entry());
            let file = manifest
                .files()
                .iter()
                .find(|file| file.path() == format!("libraries/{}", library.repository_path()))
                .expect("the manifest records the launch library");
            assert!(matches!(
                file.trust(),
                crate::integrity::ArtifactTrust::ExpectedDigestVerified { .. }
            ));
        }
        assert!(progress_events > 0);

        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must validate"),
        }

        // A corrupted artifact is detected without mutation.
        let target = managed
            .instance_paths(&instance)
            .game()
            .join("libraries/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar");
        std::fs::write(&target, b"tampered bytes").unwrap();
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Damaged);
                assert!(
                    validation
                        .problems
                        .iter()
                        .any(|problem| problem.path.ends_with("bootstraplauncher-2.0.2.jar"))
                );
            }
            ValidationOutcome::NotInstalled => panic!("the installation must still exist"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Builds a synthetic-but-faithful Forge world over the loopback
    /// fixture: the real official document shapes (side-mapped data,
    /// brace-wrapped output expectations, the URL-less generated client
    /// library) with tool jars served from the fixture server. The
    /// processor chain uses the real JDK and the shared synthetic Java
    /// tool; each step consumes the previous step's output as its input,
    /// and every output carries its published SHA-1.
    fn synthetic_forge_game() -> (Fixture, GameInstallPlan) {
        synthetic_forge_game_with_chain_mode("write")
    }

    /// `chain_mode` selects the synthetic tool's behavior for the client
    /// chain steps ("write" or "fail").
    fn synthetic_forge_game_with_chain_mode(chain_mode: &str) -> (Fixture, GameInstallPlan) {
        let Some((tool, _java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            panic!("this synthetic world requires the compiled processor tool");
        };
        let tool_bytes = std::fs::read(&tool).unwrap();
        let tool_sha1 = sha1_hex(&tool_bytes);
        let tool_path =
            "net/minecraftforge/installertools/1.4.3/installertools-1.4.3.jar".to_owned();

        // The synthetic tool writes fixed bytes for every output; every
        // published digest is the digest of those exact bytes.
        let chain_output = b"patched client artifact bytes".to_vec();
        let chain_sha1 = sha1_hex(&chain_output);

        let client = b"synthetic client jar bytes".to_vec();
        let logging = b"<Configuration status=\"WARN\"></Configuration>".to_vec();
        let mojang_library = b"synthetic mojang library jar".to_vec();
        let native_archive = native_zip_bytes();
        let universal = b"synthetic forge universal jar".to_vec();
        let bootstrap = b"synthetic bootstraplauncher jar".to_vec();

        let asset_a = b"tiny png bytes a".to_vec();
        let asset_b = b"tiny png bytes b".to_vec();
        let asset_a_hash = sha1_hex(&asset_a);
        let asset_b_hash = sha1_hex(&asset_b);
        let asset_index_body = format!(
            r#"{{"objects": {{
                "icons/icon_16x16.png": {{"hash": "{asset_a_hash}", "size": {}}},
                "minecraft/sounds/click.ogg": {{"hash": "{asset_b_hash}", "size": {}}}
            }}}}"#,
            asset_a.len(),
            asset_b.len(),
        )
        .into_bytes();

        let mut bodies: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        bodies.insert("/mojang/client.jar".to_owned(), client.clone());
        bodies.insert(
            "/mojang/logging/client-1.21.2.xml".to_owned(),
            logging.clone(),
        );
        bodies.insert(
            "/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar".to_owned(),
            mojang_library.clone(),
        );
        bodies.insert(
            "/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar".to_owned(),
            native_archive.clone(),
        );
        bodies.insert(
            "/mojang/asset-index/32.json".to_owned(),
            asset_index_body.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_a_hash[..2], asset_a_hash),
            asset_a.clone(),
        );
        bodies.insert(
            format!("/assets/{}/{}", &asset_b_hash[..2], asset_b_hash),
            asset_b.clone(),
        );
        bodies.insert(format!("/forge-maven/{tool_path}"), tool_bytes.clone());
        bodies.insert(
            "/forge-maven/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar"
                .to_owned(),
            universal.clone(),
        );
        bodies.insert(
            "/forge-maven/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar".to_owned(),
            bootstrap.clone(),
        );

        let broken = Arc::new(Mutex::new(HashSet::new()));
        // The installer documents embed the server base, and the installer
        // bytes embed the documents: serve from a shared mutable body map
        // so the installer can be published once the base is known.
        let shared_bodies = Arc::new(Mutex::new(bodies.clone()));
        let bodies_for_handler = Arc::clone(&shared_bodies);
        let broken_for_handler = Arc::clone(&broken);
        let server = TestServer::spawn(Arc::new(move |request| {
            if broken_for_handler.lock().unwrap().contains(&request.path) {
                return TestResponse::status(404);
            }
            match bodies_for_handler.lock().unwrap().get(&request.path) {
                Some(body) => TestResponse::ok(body),
                None => TestResponse::status(404),
            }
        }));
        let base = server.base_url().to_owned();

        // The version document (Mojang half) for Minecraft 1.21.11.
        let version_document_json = format!(
            r#"{{
                "id": "1.21.11",
                "type": "release",
                "mainClass": "net.minecraft.client.main.Main",
                "javaVersion": {{ "component": "java-runtime-delta", "majorVersion": 21 }},
                "assetIndex": {{
                    "id": "32",
                    "sha1": "{}",
                    "size": {},
                    "totalSize": 999999,
                    "url": "{base}/mojang/asset-index/32.json"
                }},
                "downloads": {{
                    "client": {{
                        "sha1": "{}",
                        "size": {},
                        "url": "{base}/mojang/client.jar"
                    }}
                }},
                "logging": {{
                    "client": {{
                        "argument": "-Dlog4j.configurationFile=${{path}}",
                        "file": {{
                            "id": "client-1.21.2.xml",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/logging/client-1.21.2.xml"
                        }},
                        "type": "log4j2-xml"
                    }}
                }},
                "libraries": [
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                        }} }},
                        "name": "com.mojang:brigadier:1.0.18"
                    }},
                    {{
                        "downloads": {{ "artifact": {{
                            "path": "org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar",
                            "sha1": "{}",
                            "size": {},
                            "url": "{base}/mojang/libraries/org/lwjgl/lwjgl/3.4.1/lwjgl-3.4.1-natives-windows.jar"
                        }} }},
                        "name": "org.lwjgl:lwjgl:3.4.1:natives-windows"
                    }}
                ],
                "arguments": {{
                    "game": ["--username", "${{auth_player_name}}"],
                    "jvm": ["-Djava.library.path=${{natives_directory}}"]
                }}
            }}"#,
            sha1_hex(&asset_index_body),
            asset_index_body.len(),
            sha1_hex(&client),
            client.len(),
            sha1_hex(&logging),
            logging.len(),
            sha1_hex(&mojang_library),
            mojang_library.len(),
            sha1_hex(&native_archive),
            native_archive.len(),
        );
        let document = VersionDocument::from_json(&version_document_json).unwrap();
        let minecraft =
            plan_version_document(&document, PlatformProfile::current().unwrap()).unwrap();

        // The install profile (Forge half), faithful to the real document:
        // side-mapped data, brace-wrapped output keys, URL-less generated
        // client, and a three-step client chain whose inputs chain from
        // the previous step's outputs.
        let tool_coordinate = "net.minecraftforge:installertools:1.4.3";
        let profile = format!(
            r#"{{
              "spec": 1, "profile": "forge", "version": "1.21.11-forge-61.2.0",
              "minecraft": "1.21.11", "json": "/version.json",
              "data": {{
                "MOJMAPS": {{"client": "[net.minecraft:client:1.21.11:mappings@tsrg]", "server": "[net.minecraft:server:1.21.11:mappings@tsrg]"}},
                "MOJMAPS_SHA": {{"client": "'{chain_sha1}'", "server": "'{chain_sha1}'"}},
                "MC_OFF": {{"client": "[net.minecraft:client:1.21.11:official]", "server": "[net.minecraft:server:1.21.11:official]"}},
                "MC_OFF_SHA": {{"client": "'{chain_sha1}'", "server": "'{chain_sha1}'"}},
                "BINPATCH": {{"client": "/data/client.lzma", "server": "/data/server.lzma"}},
                "PATCHED": {{"client": "[net.minecraftforge:forge:1.21.11-61.2.0:client]", "server": "[net.minecraftforge:forge:1.21.11-61.2.0:server]"}},
                "PATCHED_SHA": {{"client": "'{chain_sha1}'", "server": "'{chain_sha1}'"}}
              }},
              "processors": [
                {{"jar": "{tool_coordinate}", "classpath": ["{tool_coordinate}"], "args": ["--mode", "{chain_mode}", "--output", "{{MOJMAPS}}"], "outputs": {{"{{MOJMAPS}}": "{{MOJMAPS_SHA}}"}}}},
                {{"sides": ["client"], "jar": "{tool_coordinate}", "classpath": ["{tool_coordinate}"], "args": ["--mode", "{chain_mode}", "--input", "{{MOJMAPS}}", "--output", "{{MC_OFF}}"], "outputs": {{"{{MC_OFF}}": "{{MC_OFF_SHA}}"}}}},
                {{"sides": ["server"], "jar": "{tool_coordinate}", "classpath": [], "args": ["--mode", "write"], "outputs": {{}}}},
                {{"jar": "{tool_coordinate}", "classpath": ["{tool_coordinate}"], "args": ["--mode", "{chain_mode}", "--input", "{{MC_OFF}}", "--output", "{{PATCHED}}", "--extra", "{{BINPATCH}}"], "outputs": {{"{{PATCHED}}": "{{PATCHED_SHA}}"}}}}
              ],
              "libraries": [
                {{"name": "{tool_coordinate}", "downloads": {{"artifact": {{"path": "{tool_path}", "url": "{base}/forge-maven/{tool_path}", "sha1": "{tool_sha1}", "size": {tool_len}}}}}}}
              ]
            }}"#,
            tool_len = tool_bytes.len(),
        );
        // The version profile (launch metadata) with the generated client.
        let version = format!(
            r#"{{
              "id": "1.21.11-forge-61.2.0", "inheritsFrom": "1.21.11",
              "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
              "logging": {{}},
              "arguments": {{"jvm": ["-Djava.net.preferIPv6Addresses=system"], "game": ["--launchTarget", "forge_client"]}},
              "libraries": [
                {{"name": "net.minecraftforge:forge:1.21.11-61.2.0:universal", "downloads": {{"artifact": {{"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "url": "{base}/forge-maven/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-universal.jar", "sha1": "{}", "size": {}}}}}}},
                {{"name": "net.minecraftforge:forge:1.21.11-61.2.0:client", "downloads": {{"artifact": {{"path": "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-client.jar", "url": "", "sha1": "{chain_sha1}", "size": {}}}}}}},
                {{"name": "cpw.mods:bootstraplauncher:2.0.2", "downloads": {{"artifact": {{"path": "cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "url": "{base}/forge-maven/cpw/mods/bootstraplauncher/2.0.2/bootstraplauncher-2.0.2.jar", "sha1": "{}", "size": {}}}}}}}
              ]
            }}"#,
            sha1_hex(&universal),
            universal.len(),
            chain_output.len(),
            sha1_hex(&bootstrap),
            bootstrap.len(),
        );

        // The synthetic installer jar embeds both documents plus the
        // binpatch entry, and is itself served with a sidecar digest.
        let installer = {
            use std::io::Write as _;
            let mut cursor = std::io::Cursor::new(Vec::new());
            {
                let mut writer = zip::ZipWriter::new(&mut cursor);
                for (name, bytes) in [
                    ("install_profile.json", profile.into_bytes()),
                    ("version.json", version.into_bytes()),
                    ("data/client.lzma", b"synthetic binpatch bundle".to_vec()),
                ] {
                    writer
                        .start_file(name, zip::write::SimpleFileOptions::default())
                        .unwrap();
                    writer.write_all(&bytes).unwrap();
                }
                writer.finish().unwrap();
            }
            cursor.into_inner()
        };
        let installer_url_path = "/forge-maven/net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-installer.jar";
        shared_bodies
            .lock()
            .unwrap()
            .insert(installer_url_path.to_owned(), installer.clone());
        shared_bodies.lock().unwrap().insert(
            format!("{installer_url_path}.sha1"),
            sha1_hex(&installer).into_bytes(),
        );

        let (profile, version) = {
            let document_bytes = &installer;
            let _ = document_bytes;
            let mut cursor = std::io::Cursor::new(&installer);
            let mut archive = zip::ZipArchive::new(&mut cursor).unwrap();
            let mut read_entry = |name: &str| -> String {
                use std::io::Read as _;
                let mut entry = archive.by_name(name).unwrap();
                let mut text = String::new();
                entry.read_to_string(&mut text).unwrap();
                text
            };
            (
                read_entry("install_profile.json"),
                read_entry("version.json"),
            )
        };
        let forge = crate::forge::plan::ForgePlan::from_documents(
            &crate::forge::metadata::InstallProfileDocument::from_json(&profile).unwrap(),
            &crate::forge::metadata::VersionProfileDocument::from_json(&version).unwrap(),
            &crate::minecraft::metadata::MinecraftVersionId::new("1.21.11").unwrap(),
            &crate::forge::metadata::ForgeVersionId::new("1.21.11-61.2.0").unwrap(),
            &crate::forge::metadata::ForgeMavenEndpoints::loopback_for_testing(
                &format!("{base}/forge-maven/"),
                &format!("{base}/forge-promotions.json"),
            )
            .unwrap(),
            crate::integrity::Sha1Digest::parse(&sha1_hex(&installer)).unwrap(),
        )
        .unwrap();
        let plan = crate::forge::plan::compose_forge_game_plan(minecraft, forge).unwrap();
        let bodies = shared_bodies.lock().unwrap().clone();
        let fixture = Fixture {
            server,
            bodies,
            broken,
        };
        (fixture, plan)
    }

    #[tokio::test]
    async fn a_forge_install_runs_the_client_chain_and_verifies_published_outputs() {
        let Some((_tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let (fixture, plan) = synthetic_forge_game();
        let forge = plan.forge_loader().unwrap();
        // Server-only step dropped; the client chain is three steps whose
        // inputs chain from earlier outputs.
        assert_eq!(forge.processors().len(), 3);
        let context = test_context(fixture.server.base_url()).with_processor_java(java);

        let root = std::env::temp_dir()
            .join("aurora-forge-install-tests")
            .join(std::process::id().to_string())
            .join("chain");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new("66666666-6666-4666-8666-666666666666").unwrap();
        let installed = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap();

        // The patched client is recorded as upstream-verified: its
        // published SHA-1 was known before the chain ran and the
        // generated bytes verified against it.
        let patched_relative =
            "net/minecraftforge/forge/1.21.11-61.2.0/forge-1.21.11-61.2.0-client.jar";
        let file = installed
            .manifest()
            .files()
            .iter()
            .find(|file| file.path() == format!("libraries/{patched_relative}"))
            .expect("the generated patched client is recorded");
        assert!(
            matches!(
                file.trust(),
                crate::integrity::ArtifactTrust::ExpectedDigestVerified { .. }
            ),
            "the patched client keeps its published upstream expectation: {:?}",
            file.trust()
        );

        // Forge publishes SHA-1s for every chain output, so the chained
        // intermediates (mappings, official rename) are upstream-digest-
        // verified too — never mere local observations.
        for relative in [
            "net/minecraft/client/1.21.11/client-1.21.11-mappings.tsrg",
            "net/minecraft/client/1.21.11/client-1.21.11-official.jar",
        ] {
            let file = installed
                .manifest()
                .files()
                .iter()
                .find(|file| file.path() == format!("libraries/{relative}"))
                .unwrap_or_else(|| panic!("chained intermediate {relative} is recorded"));
            assert!(
                matches!(
                    file.trust(),
                    crate::integrity::ArtifactTrust::ExpectedDigestVerified { .. }
                ),
                "the chained intermediate keeps its published expectation: {:?}",
                file.trust()
            );
        }

        // The whole installation validates read-only.
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must validate"),
        }
        assert!(
            !managed
                .instance_paths(&instance)
                .root()
                .join(STAGING_DIR_NAME)
                .exists()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_forge_chain_failure_never_commits_and_recovers_by_retry() {
        let Some((_tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let (fixture, plan) = synthetic_forge_game_with_chain_mode("fail");
        let context = test_context(fixture.server.base_url()).with_processor_java(java.clone());

        let root = std::env::temp_dir()
            .join("aurora-forge-install-tests")
            .join(std::process::id().to_string())
            .join("chain-failure");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let managed = crate::paths::ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        let instance = InstanceId::new("77777777-7777-4777-8777-777777777777").unwrap();

        // A chain whose second step fails (nonzero exit) never commits;
        // the third step never runs against partial output.
        let _ = fixture;
        let error = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, InstallError::Processing { .. }), "{error}");
        assert!(
            !managed
                .instance_paths(&instance)
                .game()
                .join(INSTALLED_GAME_FILE_NAME)
                .exists(),
            "no complete-looking installation may exist"
        );

        // A retry against an unbroken fixture commits.
        let (fixture, plan) = synthetic_forge_game();
        let context = test_context(fixture.server.base_url()).with_processor_java(java.clone());
        install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .expect("the honest retry completes");
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the retried installation must validate"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_processing_neoforge_install_records_generated_artifacts() {
        let Some((_tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let (fixture, plan) = synthetic_neoforge_game(Some("write"));
        assert_eq!(plan.neoforge_loader().unwrap().processors().len(), 1);
        let context = test_context(fixture.server.base_url()).with_processor_java(java);

        let (managed, root) = neoforge_managed_root("processed");
        let instance = InstanceId::new("33333333-3333-4333-8333-333333333333").unwrap();
        let installed = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap();

        let patched_relative = "libraries/net/neoforged/minecraft-client-patched/26.2.0.88/minecraft-client-patched-26.2.0.88.jar";
        let file = installed
            .manifest()
            .files()
            .iter()
            .find(|file| file.path() == patched_relative)
            .expect("the generated patched client is recorded");
        assert!(matches!(
            file.trust(),
            crate::integrity::ArtifactTrust::LocallyGenerated { .. }
        ));

        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must validate"),
        }

        // Deleting the generated artifact is detected; repair is a
        // deliberate reinstall, never silent.
        let patched = managed
            .instance_paths(&instance)
            .game()
            .join(patched_relative.split('/').collect::<std::path::PathBuf>());
        std::fs::remove_file(&patched).unwrap();
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Damaged);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must still exist"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_failing_processor_never_commits_installed_state() {
        let Some((_tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let (fixture, plan) = synthetic_neoforge_game(Some("fail"));
        let context = test_context(fixture.server.base_url()).with_processor_java(java);

        let (managed, root) = neoforge_managed_root("failed-processor");
        let instance = InstanceId::new("44444444-4444-4444-8444-444444444444").unwrap();
        let error = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, InstallError::Processing { .. }), "{error}");
        assert!(
            !managed
                .instance_paths(&instance)
                .game()
                .join(INSTALLED_GAME_FILE_NAME)
                .exists(),
            "no complete-looking installation may exist"
        );
        // The verified shared cache survived the failure and is reusable.
        assert!(
            crate::cache::ArtifactCache::new(managed.clone())
                .verified_sha1_path(
                    &crate::integrity::Sha1Digest::parse(&sha1_hex(b"synthetic fml loader jar"))
                        .unwrap()
                )
                .is_file()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn an_interrupted_installation_writes_a_recovery_receipt() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-interrupted");
        env.plant_user_data();

        let error = install(
            &env,
            &fixture,
            &plan,
            InstallFaults {
                fail_during_materialization: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected fault must interrupt the installation");

        assert!(matches!(error, InstallError::Materialization { .. }));
        // The staging tree carries a schema-1 receipt whose last stage is
        // 'acquired': an interrupted Aurora transaction, never mistakable
        // for a completed installation.
        match receipt::classify_existing(&env.staging()) {
            receipt::Disposition::Interrupted { last_stage, .. } => {
                assert_eq!(last_stage, "acquired");
            }
            other => panic!("unexpected receipt disposition: {other:?}"),
        }
        assert!(!env.game().exists());
        assert!(
            matches!(
                validate_installed_game(&env.managed, &env.instance),
                Err(InstallError::RecoveryRequired { .. })
            ),
            "interrupted authority must block readiness"
        );

        // The next attempt recovers deterministically: it detects the
        // interrupted transaction, disposes staging after the path proof,
        // rebuilds, and leaves no receipt behind.
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the retry completes");
        assert!(!env.staging().exists());
        assert!(
            !env.staging().join(receipt::RECEIPT_FILE_NAME).exists(),
            "a committed installation leaves no receipt"
        );
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn a_crash_after_verification_is_recorded_and_retryable() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-verified-crash");
        env.plant_user_data();

        let error = install(
            &env,
            &fixture,
            &plan,
            InstallFaults {
                fail_before_commit: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected pre-commit fault must fail");

        assert!(matches!(error, InstallError::Commit { .. }));
        match receipt::classify_existing(&env.staging()) {
            receipt::Disposition::Interrupted { last_stage, .. } => {
                assert_eq!(last_stage, "verified");
            }
            other => panic!("unexpected receipt disposition: {other:?}"),
        }
        assert!(!env.game().exists(), "no incomplete tree was promoted");

        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the retry completes");
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the retried installation must validate"),
        }
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn a_crash_after_the_processing_receipt_leaves_interrupted_evidence() {
        let Some((_tool, java)) = crate::neoforge::processors::test_tools::tool_and_java() else {
            eprintln!("SKIP: no JDK on this host for processor execution");
            return;
        };
        let (fixture, plan) = synthetic_neoforge_game(Some("write"));
        let context = test_context(fixture.server.base_url()).with_processor_java(java);

        let (managed, root) = neoforge_managed_root("receipt-processing-crash");
        let instance = InstanceId::new("55555555-5555-4555-8555-555555555555").unwrap();
        let staging = managed
            .instance_paths(&instance)
            .root()
            .join(STAGING_DIR_NAME);
        let error = install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults {
                fail_after_processing_receipt: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected processing fault must fail");

        assert!(matches!(error, InstallError::Processing { .. }), "{error}");
        match receipt::classify_existing(&staging) {
            receipt::Disposition::Interrupted {
                last_stage,
                transaction_id,
                ..
            } => {
                assert_eq!(last_stage, "processing");
                assert!(!transaction_id.is_empty());
            }
            other => panic!("unexpected receipt disposition: {other:?}"),
        }
        assert!(
            !managed
                .instance_paths(&instance)
                .game()
                .join(INSTALLED_GAME_FILE_NAME)
                .exists()
        );

        // Retry runs the processors for real and commits.
        install_game(
            &managed,
            &instance,
            &plan,
            &context,
            &mut |_| {},
            InstallFaults::default(),
        )
        .await
        .expect("the retry completes");
        match validate_installed_game(&managed, &instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the retried installation must validate"),
        }
        assert!(!staging.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_crash_between_renames_leaves_recovery_evidence_and_rebuilds() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-between-renames");
        env.plant_user_data();

        // A first, successful installation becomes the replacement target.
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the first installation completes");

        let error = install(
            &env,
            &fixture,
            &plan,
            InstallFaults {
                fail_between_renames: true,
                ..InstallFaults::default()
            },
        )
        .await
        .expect_err("the injected between-renames fault must fail");

        assert!(matches!(error, InstallError::RecoveryRequired { .. }));
        // The crash window: no game directory exists, the retired tree and
        // the receipt wait in staging, and the receipt names 'replacing'.
        assert!(!env.game().exists());
        match receipt::classify_existing(&env.staging()) {
            receipt::Disposition::Interrupted { last_stage, .. } => {
                assert_eq!(last_stage, "replacing");
            }
            other => panic!("unexpected receipt disposition: {other:?}"),
        }
        let journal = journal::Journal::load(
            env.managed.instance_paths(&env.instance).root(),
            &env.instance,
        )
        .unwrap()
        .unwrap();
        assert!(
            env.managed
                .instance_paths(&env.instance)
                .root()
                .join(journal::HISTORY_DIR)
                .join(journal.transaction_id())
                .join("previous-game")
                .is_dir()
        );
        assert!(!env.staging().join(REPLACED_GAME_DIR_NAME).exists());

        // The next attempt recovers by rebuilding from verified caches; no
        // mixture of old and new trees can be observed, and user data was
        // never touched.
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the retry completes");
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the rebuilt installation must validate"),
        }
        assert!(!env.staging().exists());
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn a_corrupt_receipt_is_reported_and_staging_is_rebuilt_safely() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-corrupt");
        env.plant_user_data();
        // Pre-plant a corrupt receipt as the crash evidence of a previous
        // attempt; the next attempt must classify it as unknown evidence
        // and still recover through the derived-path staging proof.
        receipt::write_test_receipt(&env.staging(), "{ definitely not json");
        assert!(matches!(
            receipt::classify_existing(&env.staging()),
            receipt::Disposition::Unknown { .. }
        ));

        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the unknown receipt cannot wedge the install");
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must commit"),
        }
        assert!(!env.staging().exists());
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn an_unknown_schema_receipt_is_not_interpreted() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-future-schema");
        env.plant_user_data();
        receipt::write_test_receipt(
            &env.staging(),
            r#"{"schemaVersion": 2, "transactionId": "t", "instanceId": "i", "stages": []}"#,
        );
        assert!(matches!(
            receipt::classify_existing(&env.staging()),
            receipt::Disposition::Unknown { .. }
        ));

        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("an unknown receipt is reported, never obeyed");
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the installation must commit"),
        }
        assert!(!env.staging().exists());
        env.assert_user_data_intact();
    }

    #[tokio::test]
    async fn repeated_recovery_invocations_settle_identically() {
        let (fixture, plan) = synthetic_game();
        let env = TestEnv::new("receipt-repeated");
        env.plant_user_data();

        // Two interrupted attempts in a row, then recovery: classification
        // is idempotent and disposal never wedges.
        for _ in 0..2 {
            let error = install(
                &env,
                &fixture,
                &plan,
                InstallFaults {
                    fail_during_materialization: true,
                    ..InstallFaults::default()
                },
            )
            .await
            .expect_err("the injected fault interrupts");
            assert!(matches!(error, InstallError::Materialization { .. }));
            assert!(matches!(
                receipt::classify_existing(&env.staging()),
                receipt::Disposition::Interrupted { .. }
            ));
        }
        install(&env, &fixture, &plan, InstallFaults::default())
            .await
            .expect("the final attempt recovers");
        match validate_installed_game(&env.managed, &env.instance).unwrap() {
            ValidationOutcome::Installed(validation) => {
                assert_eq!(validation.status, ValidationStatus::Valid);
            }
            ValidationOutcome::NotInstalled => panic!("the recovered installation must validate"),
        }
        env.assert_user_data_intact();
    }
}
