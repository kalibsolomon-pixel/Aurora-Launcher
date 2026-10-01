//! Managed-content update discovery and policy. Availability is computed
//! read-only on explicit user request: no timers, no watchers, no startup or
//! background provider polling. The mutating half of updates stays inside the
//! existing provider lifecycle transaction in `instance_content`.

use std::collections::HashSet;

use serde::Serialize;

use crate::instance_content::{
    ContentError, ContentState, ContentType, ProviderRecord, UpdateChannel, validate_provider_file,
};
use crate::instances::InstanceId;
use crate::modrinth::{Client, Context, VersionChoice};
use crate::paths::ManagedPaths;

/// Typed availability for one managed record. States stay distinct so the UI
/// can explain each outcome instead of collapsing them into "no updates".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateStatus {
    /// The installed publication is the newest compatible version under the
    /// current release-channel policy.
    UpToDate,
    /// A policy-allowed compatible version published after the installed one
    /// exists and may be applied.
    UpdateAvailable,
    /// A candidate exists, but the record is pinned. Normal update actions
    /// (single, selected, all) must not advance it; unpinning is the separate
    /// explicit action.
    PinnedUpdateAvailable,
    /// Newer compatible versions exist, but the release-channel policy
    /// excludes all of them. Never silently downgraded or auto-widened.
    NoNewerUnderPolicy,
    /// The provider no longer resolves the installed exact version identity,
    /// or the published file no longer matches the recorded provider identity.
    CurrentVersionUnknown,
    /// The provider lookup failed for this record (network, rate limit, or
    /// malformed response).
    ProviderUnavailable,
    /// Local evidence blocks updating this record until the user resolves it.
    Blocked,
}

/// Why a record is locally blocked from updating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateBlock {
    /// The exact installed modpack owns this file; changing it would alter
    /// the authored snapshot without pack reconciliation.
    PackOwned,
    /// The managed file is currently disabled; re-enable it before updating.
    Disabled,
    /// Local bytes no longer match the managed record's digest. Aurora will
    /// not overwrite local modifications; resolve the discrepancy first.
    LocallyModified,
    /// The managed file is missing from its content directory.
    Missing,
    /// The file state is unsafe or conflicting (for example both enabled and
    /// disabled counterparts exist).
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailability {
    pub content_type: ContentType,
    pub project_id: String,
    pub current_version: String,
    pub status: UpdateStatus,
    pub block: Option<UpdateBlock>,
    pub candidate: Option<VersionChoice>,
    pub pinned: bool,
    pub channel: UpdateChannel,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesReport {
    pub instance_id: String,
    pub entries: Vec<UpdateAvailability>,
    /// Provider-managed dependency records are advanced with (or removed by)
    /// their parents and are never independently updateable.
    pub dependency_managed: usize,
}

/// Classify local evidence for one managed record without touching the
/// provider. `Ok(())` means the bytes are present and still verified.
fn local_block(
    managed: &ManagedPaths,
    instance: &InstanceId,
    record: &ProviderRecord,
) -> Option<UpdateBlock> {
    match validate_provider_file(managed, instance, record) {
        Ok(path) => {
            let disabled = record.content_type == ContentType::Mod
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("disabled"));
            disabled.then_some(UpdateBlock::Disabled)
        }
        Err(ContentError::HashMismatch) => Some(UpdateBlock::LocallyModified),
        Err(ContentError::Collision) | Err(ContentError::UnsafePath) => Some(UpdateBlock::Conflict),
        Err(ContentError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Some(UpdateBlock::Missing)
        }
        Err(_) => Some(UpdateBlock::Missing),
    }
}

/// One explicit, user-triggered availability check over the managed Modrinth
/// records of an instance. Each record costs one project-versions request;
/// blocked records are classified locally without network traffic. A rate
/// limit stops further requests instead of hammering the provider.
pub async fn check_updates(
    managed: &ManagedPaths,
    instance: &InstanceId,
    context: &Context,
    client: &Client,
    kind: Option<ContentType>,
) -> Result<UpdatesReport, ContentError> {
    let state = ContentState::load_and_migrate(managed, instance)?;
    let pack = crate::pack_state::InstalledPack::load(managed, instance)
        .map_err(|error| ContentError::StateMalformed(error.to_string()))?;
    let mut entries = Vec::new();
    let mut dependency_managed = 0usize;
    let mut rate_limited = false;
    let mut reported_pack_projects = HashSet::new();
    for record in &state.entries {
        if record.provider != "modrinth" || kind.is_some_and(|kind| record.content_type != kind) {
            continue;
        }
        if !record.explicitly_retained {
            dependency_managed += 1;
            continue;
        }
        let current_version = record
            .display_version
            .clone()
            .unwrap_or_else(|| record.version_id.clone());
        if pack
            .as_ref()
            .is_some_and(|pack| pack.owns_provider(&record.identity()))
        {
            // Distinct pack files can share a project. Update discovery is
            // project-scoped, so show one explanatory row for that project.
            if reported_pack_projects.insert((record.content_type, record.project_id.clone())) {
                entries.push(UpdateAvailability {
                    content_type: record.content_type,
                    project_id: record.project_id.clone(),
                    current_version,
                    status: UpdateStatus::Blocked,
                    block: Some(UpdateBlock::PackOwned),
                    candidate: None,
                    pinned: record.pinned,
                    channel: record.update_channel,
                    detail: Some(format!(
                        "Required by {} {}. Pack component updates require a future reconciliation workflow.",
                        pack.as_ref().unwrap().identity.name,
                        pack.as_ref().unwrap().identity.pack_version
                    )),
                });
            }
            continue;
        }
        if let Some(block) = local_block(managed, instance, record) {
            entries.push(UpdateAvailability {
                content_type: record.content_type,
                project_id: record.project_id.clone(),
                current_version,
                status: UpdateStatus::Blocked,
                block: Some(block),
                candidate: None,
                pinned: record.pinned,
                channel: record.update_channel,
                detail: Some(match block {
                    UpdateBlock::PackOwned => unreachable!("pack ownership is classified first"),
                    UpdateBlock::Disabled => {
                        "This mod is disabled. Re-enable it before updating; updates never change the enabled state."
                    }
                    UpdateBlock::LocallyModified => {
                        "The local file changed since Aurora recorded it. Aurora will not overwrite local modifications; use Recognize local files or restore the file first."
                    }
                    UpdateBlock::Missing => {
                        "The managed file is missing. Restore it before updating."
                    }
                    UpdateBlock::Conflict => {
                        "The file state is conflicting or unsafe. Inspect the content folder before updating."
                    }
                }
                .to_owned()),
            });
            continue;
        }
        if rate_limited {
            entries.push(UpdateAvailability {
                content_type: record.content_type,
                project_id: record.project_id.clone(),
                current_version,
                status: UpdateStatus::ProviderUnavailable,
                block: None,
                candidate: None,
                pinned: record.pinned,
                channel: record.update_channel,
                detail: Some(
                    "Update check paused: the provider is rate limiting requests. Try again later."
                        .into(),
                ),
            });
            continue;
        }
        let availability = match client.update_discovery(context, record).await {
            Ok(discovery) => {
                let status = match discovery.candidate {
                    Some(_) => {
                        if record.pinned {
                            UpdateStatus::PinnedUpdateAvailable
                        } else {
                            UpdateStatus::UpdateAvailable
                        }
                    }
                    None if discovery.newer_compatible_exists => UpdateStatus::NoNewerUnderPolicy,
                    None => UpdateStatus::UpToDate,
                };
                let detail = (status == UpdateStatus::NoNewerUnderPolicy).then(|| {
                    format!(
                        "Newer versions exist but none is allowed by the {} release-channel policy for this content.",
                        channel_label(record.update_channel)
                    )
                });
                UpdateAvailability {
                    content_type: record.content_type,
                    project_id: record.project_id.clone(),
                    current_version,
                    status,
                    block: None,
                    candidate: discovery.candidate,
                    pinned: record.pinned,
                    channel: record.update_channel,
                    detail,
                }
            }
            Err(crate::modrinth::Error::NotFound) => UpdateAvailability {
                content_type: record.content_type,
                project_id: record.project_id.clone(),
                current_version,
                status: UpdateStatus::CurrentVersionUnknown,
                block: None,
                candidate: None,
                pinned: record.pinned,
                channel: record.update_channel,
                detail: Some("The provider no longer lists this exact project or version.".into()),
            },
            Err(crate::modrinth::Error::InvalidResponse) => UpdateAvailability {
                content_type: record.content_type,
                project_id: record.project_id.clone(),
                current_version,
                status: UpdateStatus::CurrentVersionUnknown,
                block: None,
                candidate: None,
                pinned: record.pinned,
                channel: record.update_channel,
                detail: Some(
                    "The published version no longer matches the recorded provider identity."
                        .into(),
                ),
            },
            Err(crate::modrinth::Error::RateLimited(_)) => {
                rate_limited = true;
                UpdateAvailability {
                    content_type: record.content_type,
                    project_id: record.project_id.clone(),
                    current_version,
                    status: UpdateStatus::ProviderUnavailable,
                    block: None,
                    candidate: None,
                    pinned: record.pinned,
                    channel: record.update_channel,
                    detail: Some("The provider is rate limiting requests. Try again later.".into()),
                }
            }
            Err(_) => UpdateAvailability {
                content_type: record.content_type,
                project_id: record.project_id.clone(),
                current_version,
                status: UpdateStatus::ProviderUnavailable,
                block: None,
                candidate: None,
                pinned: record.pinned,
                channel: record.update_channel,
                detail: Some("The provider could not be reached for this project.".into()),
            },
        };
        entries.push(availability);
    }
    Ok(UpdatesReport {
        instance_id: instance.to_string(),
        entries,
        dependency_managed,
    })
}

/// User-facing channel label shared by check details and the frontend.
pub fn channel_label(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => "Stable",
        UpdateChannel::Beta => "Beta",
        UpdateChannel::Alpha => "Alpha",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modrinth::Error as ProviderError;
    use crate::test_support::{TestRequest, TestResponse, TestServer};
    use serde_json::{Value, json};
    use sha2::Digest as _;
    use std::collections::HashMap;
    use std::io::Write as _;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Deterministic valid-hex digest for test fixtures.
    fn hex_seed(seed: &str) -> String {
        const HEX: &[u8] = b"0123456789abcdef";
        let bytes = seed.as_bytes();
        (0..128)
            .map(|index| HEX[(usize::from(bytes[index % bytes.len()]) + index) % HEX.len()] as char)
            .collect()
    }

    struct Fixture {
        root: PathBuf,
        managed: ManagedPaths,
        instance: InstanceId,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join("aurora-update-checks")
                .join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir_all(root.join("instances")).unwrap();
            let instance =
                InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
            std::fs::create_dir_all(root.join("instances").join(instance.as_str()).join("mods"))
                .unwrap();
            let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
            Self {
                root,
                managed,
                instance,
            }
        }

        fn mods(&self) -> PathBuf {
            self.managed
                .instance_paths(&self.instance)
                .mods()
                .to_path_buf()
        }

        /// Install a real jar and register a matching provider record.
        fn managed_mod(
            &self,
            project: &str,
            version_id: &str,
            file_seed: &str,
            pinned: bool,
            channel: UpdateChannel,
        ) -> ProviderRecord {
            let bytes = fabric_jar(project);
            let name = format!("{version_id}.jar");
            std::fs::write(self.mods().join(&name), &bytes).unwrap();
            let record = ProviderRecord {
                content_type: ContentType::Mod,
                provider: "modrinth".into(),
                project_id: project.into(),
                version_id: version_id.into(),
                file_id: hex_seed(file_seed),
                file_name: name,
                sha256: format!("{:x}", sha2::Sha256::digest(&bytes)),
                display_version: Some(format!("{version_id}-display")),
                compatibility: crate::instance_content::ContentCompatibility {
                    minecraft_versions: vec!["1.21.11".into()],
                    loader: Some("fabric".into()),
                    environment: Some("client_and_server".into()),
                },
                dependencies: vec![],
                explicitly_retained: true,
                requires: vec![],
                origin: crate::instance_content::ProviderOrigin::Direct,
                installed_at_unix_seconds: None,
                pinned,
                update_channel: channel,
            };
            record.validate().unwrap();
            record
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fabric_jar(id: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(
                serde_json::json!({"schemaVersion":1,"id":id,"version":"1.0.0"})
                    .to_string()
                    .as_bytes(),
            )
            .unwrap();
        writer.finish().unwrap().into_inner()
    }

    fn provider_version(id: &str, project: &str, kind: &str, published: &str, seed: &str) -> Value {
        json!({
            "id": id, "project_id": project, "name": "Test release",
            "version_number": format!("{id}-display"), "version_type": kind,
            "date_published": published,
            "game_versions": ["1.21.11"], "loaders": ["fabric"],
            "environment": "client_and_server",
            "files": [{
                "hashes": {"sha512": hex_seed(seed)},
                "url": "https://cdn.modrinth.com/data/test/file.jar",
                "filename": format!("{id}.jar"), "primary": true,
                "size": 4, "file_type": null
            }],
            "dependencies": []
        })
    }

    fn routes() -> HashMap<String, Value> {
        let mut routes: HashMap<String, Value> = HashMap::new();
        // Up to date: only the installed version is compatible.
        routes.insert(
            "/v2/project/UPTD0001/version".into(),
            json!([provider_version(
                "uptd0001",
                "UPTD0001",
                "release",
                "2026-01-01T00:00:00Z",
                "uptd0001"
            )]),
        );
        // Update available: a newer release exists.
        routes.insert(
            "/v2/project/UPDT0002/version".into(),
            json!([
                provider_version(
                    "updt0002",
                    "UPDT0002",
                    "release",
                    "2026-01-01T00:00:00Z",
                    "updt0002"
                ),
                provider_version(
                    "updt0009",
                    "UPDT0002",
                    "release",
                    "2026-02-01T00:00:00Z",
                    "updt0009"
                )
            ]),
        );
        // Channel-excluded: the only newer version is a beta.
        routes.insert(
            "/v2/project/CHAN0003/version".into(),
            json!([
                provider_version(
                    "chan0003",
                    "CHAN0003",
                    "release",
                    "2026-01-01T00:00:00Z",
                    "chan0003"
                ),
                provider_version(
                    "chan0009",
                    "CHAN0003",
                    "beta",
                    "2026-02-01T00:00:00Z",
                    "chan0009"
                )
            ]),
        );
        // Older-only: the installed version is the newest publication.
        routes.insert(
            "/v2/project/OLDR0004/version".into(),
            json!([
                provider_version(
                    "oldr0001",
                    "OLDR0004",
                    "release",
                    "2025-12-01T00:00:00Z",
                    "oldr0001"
                ),
                provider_version(
                    "oldr0004",
                    "OLDR0004",
                    "release",
                    "2026-01-01T00:00:00Z",
                    "oldr0004"
                )
            ]),
        );
        routes
    }

    fn context() -> Context {
        Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        }
    }

    fn status_for<'a>(report: &'a UpdatesReport, project: &str) -> &'a UpdateAvailability {
        report
            .entries
            .iter()
            .find(|entry| entry.project_id == project)
            .unwrap()
    }

    #[tokio::test]
    async fn pack_owned_files_share_one_blocked_project_row_without_network() {
        let fixture = Fixture::new();
        let mut first =
            fixture.managed_mod("Pack0001", "pack-a", "pack-a", false, UpdateChannel::Stable);
        let mut second =
            fixture.managed_mod("Pack0001", "pack-b", "pack-b", false, UpdateChannel::Stable);
        first.origin = crate::instance_content::ProviderOrigin::Pack;
        second.origin = crate::instance_content::ProviderOrigin::Pack;
        let mut state = ContentState::empty();
        state.entries.extend([first.clone(), second.clone()]);
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let identity = crate::pack_state::PackIdentity {
            provider: "modrinth".into(),
            project_id: "PackRoot".into(),
            version_id: "exact".into(),
            name: "Fixture pack".into(),
            pack_version: "1.0".into(),
            artifact_sha512: hex_seed("archive"),
            artifact_sha256: "a".repeat(64),
            minecraft_version: "1.21.11".into(),
            fabric_loader_version: "0.19.3".into(),
            installed_at_unix_seconds: 1,
        };
        let components = [&first, &second]
            .into_iter()
            .map(|record| crate::pack_state::OwnedComponent {
                path: format!("mods/{}", record.file_name),
                sha256: record.sha256.clone(),
                sha512: record.file_id.clone(),
                provider: Some(record.identity()),
                provider_version_id: Some(record.version_id.clone()),
            })
            .collect();
        crate::pack_state::InstalledPack::new(
            fixture.instance.clone(),
            identity,
            components,
            vec![],
            vec![],
        )
        .unwrap()
        .save(&fixture.managed)
        .unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&hits);
        let server = TestServer::spawn(Arc::new(move |_: &TestRequest| {
            observed.fetch_add(1, Ordering::SeqCst);
            TestResponse::status(404)
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let report = check_updates(
            &fixture.managed,
            &fixture.instance,
            &context(),
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        assert_eq!(report.entries.len(), 1);
        assert_eq!(report.entries[0].status, UpdateStatus::Blocked);
        assert_eq!(report.entries[0].block, Some(UpdateBlock::PackOwned));
        assert!(
            report.entries[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("Fixture pack")
        );
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_updates_reports_the_full_availability_matrix() {
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        state.entries.push(fixture.managed_mod(
            "UPTD0001",
            "uptd0001",
            "uptd0001",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "UPDT0002",
            "updt0002",
            "updt0002",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "Pinn0005",
            "pinn0005",
            "pinn0005",
            true,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "CHAN0003",
            "chan0003",
            "chan0003",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "OLDR0004",
            "oldr0004",
            "oldr0004",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "Gone0006",
            "gone0006",
            "gone0006",
            false,
            UpdateChannel::Stable,
        ));
        // A dependency record: counted, never independently checked.
        let mut dependency = fixture.managed_mod(
            "Depd0007",
            "depd0007",
            "depd0007",
            false,
            UpdateChannel::Stable,
        );
        dependency.explicitly_retained = false;
        state.entries.push(dependency);
        // Disabled file: renamed to .jar.disabled.
        state.entries.push(fixture.managed_mod(
            "Disb0008",
            "disb0008",
            "disb0008",
            false,
            UpdateChannel::Stable,
        ));
        std::fs::rename(
            fixture.mods().join("disb0008.jar"),
            fixture.mods().join("disb0008.jar.disabled"),
        )
        .unwrap();
        // Locally modified bytes.
        state.entries.push(fixture.managed_mod(
            "Modf0009",
            "modf0009",
            "modf0009",
            false,
            UpdateChannel::Stable,
        ));
        std::fs::write(fixture.mods().join("modf0009.jar"), b"user edited bytes").unwrap();
        // Missing file.
        state.entries.push(fixture.managed_mod(
            "Miss0010",
            "miss0010",
            "miss0010",
            false,
            UpdateChannel::Stable,
        ));
        std::fs::remove_file(fixture.mods().join("miss0010.jar")).unwrap();
        state.save(&fixture.managed, &fixture.instance).unwrap();

        // Pinned project serves the same shape as the update-available one.
        let mut routes = routes();
        routes.insert(
            "/v2/project/Pinn0005/version".into(),
            json!([
                provider_version(
                    "pinn0005",
                    "Pinn0005",
                    "release",
                    "2026-01-01T00:00:00Z",
                    "pinn0005"
                ),
                provider_version(
                    "pinn0009",
                    "Pinn0005",
                    "release",
                    "2026-02-01T00:00:00Z",
                    "pinn0009"
                )
            ]),
        );
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            routes
                .get(path)
                .map(|value| TestResponse::ok(&serde_json::to_vec(value).unwrap()))
                .unwrap_or_else(|| TestResponse::status(404))
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));

        let report = check_updates(
            &fixture.managed,
            &fixture.instance,
            &context(),
            &client,
            None,
        )
        .await
        .unwrap();

        assert_eq!(
            status_for(&report, "UPTD0001").status,
            UpdateStatus::UpToDate
        );
        let available = status_for(&report, "UPDT0002");
        assert_eq!(available.status, UpdateStatus::UpdateAvailable);
        assert_eq!(available.candidate.as_ref().unwrap().id, "updt0009");
        assert_eq!(available.current_version, "updt0002-display");
        let pinned = status_for(&report, "Pinn0005");
        assert_eq!(pinned.status, UpdateStatus::PinnedUpdateAvailable);
        assert!(pinned.candidate.is_some());
        let excluded = status_for(&report, "CHAN0003");
        assert_eq!(excluded.status, UpdateStatus::NoNewerUnderPolicy);
        assert!(excluded.candidate.is_none());
        assert!(excluded.detail.as_deref().unwrap().contains("Stable"));
        assert_eq!(
            status_for(&report, "OLDR0004").status,
            UpdateStatus::UpToDate
        );
        // Provider-deleted project: the versions request 404s.
        assert_eq!(
            status_for(&report, "Gone0006").status,
            UpdateStatus::CurrentVersionUnknown
        );
        // Local evidence, without network traffic for those records.
        let disabled = status_for(&report, "Disb0008");
        assert_eq!(disabled.status, UpdateStatus::Blocked);
        assert_eq!(disabled.block, Some(UpdateBlock::Disabled));
        let modified = status_for(&report, "Modf0009");
        assert_eq!(modified.status, UpdateStatus::Blocked);
        assert_eq!(modified.block, Some(UpdateBlock::LocallyModified));
        let missing = status_for(&report, "Miss0010");
        assert_eq!(missing.status, UpdateStatus::Blocked);
        assert_eq!(missing.block, Some(UpdateBlock::Missing));
        assert_eq!(report.dependency_managed, 1);
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.project_id != "Depd0007")
        );
    }

    #[tokio::test]
    async fn check_updates_stops_requesting_after_a_rate_limit() {
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        state.entries.push(fixture.managed_mod(
            "Rate0001",
            "rate0001",
            "rate0001",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "Rate0002",
            "rate0002",
            "rate0002",
            false,
            UpdateChannel::Stable,
        ));
        state.save(&fixture.managed, &fixture.instance).unwrap();

        let requests = Arc::new(AtomicUsize::new(0));
        let counted = requests.clone();
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            if path.contains("/project/") && path.ends_with("/version") {
                let seen = counted.fetch_add(1, Ordering::SeqCst);
                if seen == 0 {
                    return TestResponse::status(429).with_header("X-Ratelimit-Reset", "30");
                }
                return TestResponse::status(500);
            }
            TestResponse::status(404)
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let report = check_updates(
            &fixture.managed,
            &fixture.instance,
            &context(),
            &client,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            status_for(&report, "Rate0001").status,
            UpdateStatus::ProviderUnavailable
        );
        assert_eq!(
            status_for(&report, "Rate0002").status,
            UpdateStatus::ProviderUnavailable
        );
        assert_eq!(requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn check_updates_filters_by_content_type_and_maps_network_errors() {
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        state.entries.push(fixture.managed_mod(
            "Netw0001",
            "netw0001",
            "netw0001",
            false,
            UpdateChannel::Stable,
        ));
        state.entries.push(fixture.managed_mod(
            "Pack0002",
            "pack0002",
            "pack0002",
            false,
            UpdateChannel::Stable,
        ));
        // Give the pack a zip name so record validation accepts it.
        state.entries[1].content_type = ContentType::ResourcePack;
        let pack_bytes = state.entries[1].file_name.clone();
        std::fs::rename(
            fixture.mods().join(&pack_bytes),
            fixture.mods().join(pack_bytes.replace(".jar", ".zip")),
        )
        .unwrap();
        state.entries[1].file_name = pack_bytes.replace(".jar", ".zip");
        state.save(&fixture.managed, &fixture.instance).unwrap();

        let client = Client::for_testing("http://127.0.0.1:0/v2/");
        let mods_only = check_updates(
            &fixture.managed,
            &fixture.instance,
            &context(),
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        assert_eq!(mods_only.entries.len(), 1);
        assert_eq!(
            status_for(&mods_only, "Netw0001").status,
            UpdateStatus::ProviderUnavailable
        );
    }

    #[tokio::test]
    async fn identity_mismatches_map_to_current_version_unknown() {
        // The record's file digest does not match the published version, so
        // the adapter rejects the identity; the availability check surfaces
        // that as an unresolvable current identity, never as an update.
        let mut routes: HashMap<String, Value> = HashMap::new();
        routes.insert(
            "/v2/project/Iden0001/version".into(),
            json!([provider_version(
                "iden0001",
                "Iden0001",
                "release",
                "2026-01-01T00:00:00Z",
                "iden0001"
            )]),
        );
        let server = TestServer::spawn(Arc::new(move |request: &TestRequest| {
            let path = request.path.split('?').next().unwrap_or_default();
            routes
                .get(path)
                .map(|value| TestResponse::ok(&serde_json::to_vec(value).unwrap()))
                .unwrap_or_else(|| TestResponse::status(404))
        }));
        let client = Client::for_testing(&format!("{}/v2/", server.base_url()));
        let fixture = Fixture::new();
        let mut state = ContentState::empty();
        state.entries.push(fixture.managed_mod(
            "Iden0001",
            "iden0001",
            "different-seed",
            false,
            UpdateChannel::Stable,
        ));
        state.save(&fixture.managed, &fixture.instance).unwrap();
        let report = check_updates(
            &fixture.managed,
            &fixture.instance,
            &context(),
            &client,
            None,
        )
        .await
        .unwrap();
        let entry = status_for(&report, "Iden0001");
        assert_eq!(entry.status, UpdateStatus::CurrentVersionUnknown);
        assert!(entry.candidate.is_none());
        // The adapter-level error remains typed for direct callers.
        let record = &state.entries[0];
        assert!(matches!(
            client.update_discovery(&context(), record).await,
            Err(ProviderError::InvalidResponse)
        ));
    }
}

#[cfg(test)]
mod live_acceptance {
    use super::*;
    use crate::application::{
        ProviderLifecycleTarget, apply_resolved_updates, resolve_updates_preview,
    };
    use crate::instance_content::{
        ContentState, ProviderIdentity, ProviderOrigin, install_provider_plans, set_update_policy,
    };
    use crate::instances::InstanceId;
    use crate::modrinth::{Client, Context, VersionChoice};
    use crate::paths::ManagedPaths;
    use std::path::PathBuf;

    /// Live Phase H acceptance against real Modrinth with a disposable
    /// managed root. Never points at production application data. Enabled
    /// only through AURORA_UPDATES_ROOT pointing at a disposable temporary
    /// directory whose name starts with aurora-updates-acceptance-.
    #[tokio::test]
    #[ignore = "requires live Modrinth and an explicit disposable root"]
    async fn live_modrinth_managed_updates_acceptance() {
        let root = PathBuf::from(std::env::var_os("AURORA_UPDATES_ROOT").expect("disposable root"));
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-updates-acceptance-")
        );
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance = InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
        std::fs::create_dir_all(root.join("instances").join(instance.as_str())).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        crate::instance_content::ensure_directory(&managed, &instance, ContentType::Mod).unwrap();
        {
            use crate::instances::{
                InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
                settings::InstanceConfiguration,
            };
            let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
            config.set_aurora_enabled(false);
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(
                InstanceRecord::from_installed(
                    instance.clone(),
                    "Phase H updates acceptance",
                    InstanceState::Ready,
                    InstalledConfiguration {
                        minecraft_version: "1.21.11".into(),
                        platform: crate::instances::platform::PlatformPin::Fabric {
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
        }
        let context = Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        };
        let client = Client::official();

        // Popular projects with long release histories for this game version.
        // The first with at least two policy-compatible versions is used.
        let candidates = [
            "AANobbMI", // Sodium
            "mOgUt4GM", // Mod Menu
            "C7lc0oPV", // Lithium
            "YL57xq9U", // Iris
        ];
        let (project, older, newest) = pick_two_releases(&client, &context, &candidates)
            .await
            .expect("no candidate project publishes two compatible releases for 1.21.11");
        println!(
            "acceptance project: {project} older={} newest={}",
            older.id, newest.id
        );

        // ---- TEST A: direct root install/update -------------------------
        let installed =
            install_older_version(&managed, &instance, &context, &client, &project, &older)
                .await
                .unwrap();
        let identity = installed.identity();
        let before = evidence(&managed, &instance, &identity).await;
        println!("test A before: {before}");

        let report = check_updates(
            &managed,
            &instance,
            &context,
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        let entry = report
            .entries
            .iter()
            .find(|entry| entry.project_id == project)
            .unwrap();
        assert_eq!(entry.status, UpdateStatus::UpdateAvailable);
        assert_eq!(entry.candidate.as_ref().unwrap().id, newest.id);

        let preview = resolve_updates_preview(
            &managed,
            &instance.to_string().as_str(),
            &[target(ContentType::Mod, &project)],
        )
        .await
        .map(|(_, _, _, _, preview)| preview)
        .unwrap();
        assert_eq!(preview.targets[0].candidate.id, newest.id);
        apply_resolved_updates(
            &managed,
            &instance.to_string().as_str(),
            vec![target(ContentType::Mod, &project)],
            &preview.preview_fingerprint,
        )
        .await
        .unwrap();

        let after = evidence(&managed, &instance, &identity).await;
        println!("test A after: {after}");
        assert_eq!(after.version_id, newest.id);
        assert_eq!(after.origin, "direct");
        assert!(after.enabled);
        // The published target digest equals the installed bytes.
        assert_eq!(after.sha512, after.published_sha512);
        if before.file_name != after.file_name {
            let old_path = managed
                .instance_paths(&instance)
                .mods()
                .join(&before.file_name);
            assert!(
                !old_path.exists(),
                "old artifact must be removed after activation"
            );
        }
        println!("TEST A: PASS");

        // ---- TEST C: pin semantics ---------------------------------------
        set_update_policy(&managed, &instance, &identity, Some(true), None).unwrap();
        // A candidate may exist above the pinned version only if the provider
        // published something newer than `newest`; pinning is verified by
        // refusing the normal update action either way.
        let pinned = check_updates(
            &managed,
            &instance,
            &context,
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        let pinned_entry = pinned
            .entries
            .iter()
            .find(|entry| entry.project_id == project)
            .unwrap();
        assert!(pinned_entry.pinned);
        // With no candidate the no-candidate states are honest; with one,
        // pinning must downgrade availability to the pinned variant.
        assert!(
            pinned_entry.status == UpdateStatus::PinnedUpdateAvailable
                || pinned_entry.status == UpdateStatus::UpToDate
                || pinned_entry.status == UpdateStatus::NoNewerUnderPolicy
        );
        let refused = resolve_updates_preview(
            &managed,
            &instance.to_string().as_str(),
            &[target(ContentType::Mod, &project)],
        )
        .await;
        match refused {
            Err(error) => assert_eq!(error.error_code(), "provider_update_pinned"),
            Ok(_) => panic!("a pinned root must refuse the normal update action"),
        }
        set_update_policy(&managed, &instance, &identity, Some(false), None).unwrap();
        println!("TEST C: PASS (pin respected, unpin restored eligibility)");

        // ---- TEST D: release-channel policy (opportunistic live data) ----
        let details = client
            .details(&context, ContentType::Mod, &project)
            .await
            .unwrap();
        let pre_release = details
            .versions
            .iter()
            .find(|version| matches!(version.version_type.as_str(), "beta" | "alpha"))
            .filter(|version| version.date_published > newest.date_published);
        match pre_release {
            Some(beta) => {
                set_update_policy(
                    &managed,
                    &instance,
                    &identity,
                    None,
                    Some(UpdateChannel::Beta),
                )
                .unwrap();
                let widened = check_updates(
                    &managed,
                    &instance,
                    &context,
                    &client,
                    Some(ContentType::Mod),
                )
                .await
                .unwrap();
                let entry = widened
                    .entries
                    .iter()
                    .find(|entry| entry.project_id == project)
                    .unwrap();
                assert_eq!(entry.status, UpdateStatus::UpdateAvailable);
                assert!(beta.version_type != "alpha" || entry.candidate.is_some());
                if beta.version_type == "beta" {
                    assert_eq!(entry.candidate.as_ref().unwrap().id, beta.id);
                }
                set_update_policy(
                    &managed,
                    &instance,
                    &identity,
                    None,
                    Some(UpdateChannel::Stable),
                )
                .unwrap();
                let narrowed = check_updates(
                    &managed,
                    &instance,
                    &context,
                    &client,
                    Some(ContentType::Mod),
                )
                .await
                .unwrap();
                let entry = narrowed
                    .entries
                    .iter()
                    .find(|entry| entry.project_id == project)
                    .unwrap();
                assert_ne!(entry.status, UpdateStatus::UpdateAvailable);
                println!("TEST D: PASS (live beta {} filtered by policy)", beta.id);
            }
            None => {
                println!(
                    "TEST D: live project publishes no newer beta/alpha; channel matrix covered deterministically"
                );
            }
        }

        // ---- TEST E: injected failure keeps the working version ----------
        // The failure is injected by planning an update back to the older
        // version with a corrupted published digest, executed through the
        // raw graph transaction: verification must fail and leave the
        // working version untouched.
        let before_failure = evidence(&managed, &instance, &identity).await;
        let state = ContentState::load(&managed, &instance).unwrap();
        let resolved = client
            .resolve_updates(
                &context,
                &[(ContentType::Mod, project.clone(), older.id.clone())],
                &state,
            )
            .await
            .unwrap();
        let mut plans = resolved.plans;
        assert!(!plans.is_empty());
        for plan in &mut plans {
            if let crate::instance_content::ProviderArtifactSource::Sha512(source) =
                &mut plan.source
            {
                let url = source.url().to_string();
                let size = source.size_bytes();
                *source =
                    crate::downloads::Sha512ArtifactSource::https(&url, &"0".repeat(128), size)
                        .unwrap();
            }
        }
        let failed = crate::instance_content::update_provider_graph_multi(
            &managed,
            &instance,
            &state,
            &[identity.clone()],
            plans,
        )
        .await;
        assert!(failed.is_err(), "tampered digest must fail acquisition");
        let after_failure = evidence(&managed, &instance, &identity).await;
        assert_eq!(after_failure.version_id, before_failure.version_id);
        assert_eq!(after_failure.sha256, before_failure.sha256);
        assert_eq!(
            ContentState::load(&managed, &instance).unwrap(),
            state,
            "managed record must survive the failed transaction unchanged"
        );
        println!(
            "TEST E: PASS (old version survived: {})",
            after_failure.version_id
        );

        // ---- TEST B: recovered root updates through the same pipeline ----
        let (second_project, second_older, second_newest) =
            pick_second_project(&client, &context, &candidates, &project)
                .await
                .expect("second project with two releases required for recovery test");
        let cache = crate::cache::ArtifactCache::new(managed.clone());
        let older_doc = client.version_document(&second_older.id).await.unwrap();
        let file = older_doc.files.iter().find(|file| file.primary).unwrap();
        let source = crate::downloads::Sha512ArtifactSource::https(
            &file.url,
            &file.hashes.sha512,
            Some(file.size),
        )
        .unwrap();
        let artifact = cache.acquire_sha512(&source).await.unwrap();
        let mods = managed.instance_paths(&instance).mods().to_path_buf();
        let recovered_name = format!("recovered-{}.jar", second_older.id);
        std::fs::copy(&artifact.path, mods.join(&recovered_name)).unwrap();

        let local =
            crate::content_recognition::scan_local(&managed, &instance, ContentType::Mod).unwrap();
        let scan =
            crate::content_recognition::recognize(&instance, ContentType::Mod, local, &client)
                .await
                .unwrap();
        let candidate = scan
            .candidates
            .iter()
            .find(|candidate| candidate.file_name == recovered_name)
            .expect("recovered candidate");
        assert!(
            candidate.recognition.is_some(),
            "candidate must be recognized"
        );
        let approval = crate::content_recognition::RecoveredContentApproval {
            instance_id: instance.to_string(),
            content_type: ContentType::Mod,
            inventory_revision: scan.inventory_revision.clone(),
            files: vec![crate::content_recognition::RecoveredFileApproval {
                file_name: recovered_name.clone(),
                sha512: candidate.sha512.clone().unwrap(),
            }],
        };
        let records = crate::content_recognition::register_recovered_content(
            &managed,
            &instance,
            ContentType::Mod,
            &approval,
            &client,
        )
        .await
        .unwrap();
        assert_eq!(records[0].origin, ProviderOrigin::Recovered);
        let recovered_identity = records[0].identity();
        let recovered_before = evidence(&managed, &instance, &recovered_identity).await;
        println!("test B before: {recovered_before}");
        assert_eq!(recovered_before.origin, "recovered");

        let report = check_updates(
            &managed,
            &instance,
            &context,
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        let entry = report
            .entries
            .iter()
            .find(|entry| entry.project_id == second_project)
            .unwrap();
        assert_eq!(entry.status, UpdateStatus::UpdateAvailable);
        assert_eq!(entry.candidate.as_ref().unwrap().id, second_newest.id);

        let preview = resolve_updates_preview(
            &managed,
            &instance.to_string().as_str(),
            &[target(ContentType::Mod, &second_project)],
        )
        .await
        .map(|(_, _, _, _, preview)| preview)
        .unwrap();
        apply_resolved_updates(
            &managed,
            &instance.to_string().as_str(),
            vec![target(ContentType::Mod, &second_project)],
            &preview.preview_fingerprint,
        )
        .await
        .unwrap();
        let recovered_after = evidence(&managed, &instance, &recovered_identity).await;
        println!("test B after: {recovered_after}");
        assert_eq!(recovered_after.version_id, second_newest.id);
        assert_eq!(
            recovered_after.origin, "recovered",
            "updating recovered content must not rewrite provenance"
        );
        assert_eq!(recovered_after.sha512, recovered_after.published_sha512);
        println!("TEST B: PASS (same pipeline as TEST A, origin preserved)");
    }

    fn target(kind: ContentType, project: &str) -> ProviderLifecycleTarget {
        ProviderLifecycleTarget {
            content_type: kind,
            project_id: project.to_owned(),
        }
    }

    struct Evidence {
        project: String,
        version_id: String,
        file_name: String,
        size: u64,
        sha256: String,
        sha512: String,
        published_sha512: String,
        enabled: bool,
        origin: &'static str,
    }

    impl std::fmt::Display for Evidence {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "project={} version={} file={} size={} sha256={} sha512={} published512={} enabled={} origin={}",
                self.project,
                self.version_id,
                self.file_name,
                self.size,
                self.sha256,
                self.sha512,
                self.published_sha512,
                self.enabled,
                self.origin
            )
        }
    }

    async fn evidence(
        managed: &ManagedPaths,
        instance: &InstanceId,
        identity: &ProviderIdentity,
    ) -> Evidence {
        use sha2::Digest as _;
        let state = ContentState::load(managed, instance).unwrap();
        let record = state.find(identity).unwrap();
        let mods = managed.instance_paths(instance).mods().to_path_buf();
        let active = mods.join(&record.file_name);
        let disabled = mods.join(format!("{}.disabled", record.file_name));
        let (path, enabled) = if active.exists() {
            (active, true)
        } else {
            (disabled, false)
        };
        let bytes = std::fs::read(&path).unwrap();
        let version = Client::official()
            .version_document(&record.version_id)
            .await
            .unwrap();
        let published = version
            .files
            .iter()
            .find(|file| file.hashes.sha512.eq_ignore_ascii_case(&record.file_id))
            .map(|file| file.hashes.sha512.to_ascii_lowercase())
            .unwrap_or_default();
        Evidence {
            project: record.project_id.clone(),
            version_id: record.version_id.clone(),
            file_name: record.file_name.clone(),
            size: bytes.len() as u64,
            sha256: format!("{:x}", sha2::Sha256::digest(&bytes)),
            sha512: format!("{:x}", sha2::Sha512::digest(&bytes)),
            published_sha512: published,
            enabled,
            origin: match record.origin {
                ProviderOrigin::Direct => "direct",
                ProviderOrigin::Dependency => "dependency",
                ProviderOrigin::Recovered => "recovered",
                ProviderOrigin::Pack => "pack",
            },
        }
    }

    async fn pick_two_releases(
        client: &Client,
        context: &Context,
        candidates: &[&str],
    ) -> Option<(String, VersionChoice, VersionChoice)> {
        for project in candidates {
            let Ok(details) = client.details(context, ContentType::Mod, project).await else {
                continue;
            };
            let mut releases: Vec<_> = details
                .versions
                .iter()
                .filter(|version| version.version_type == "release")
                .collect();
            releases.sort_by(|a, b| b.date_published.cmp(&a.date_published));
            if releases.len() >= 2 {
                return Some((
                    (*project).to_owned(),
                    releases[1].clone(),
                    releases[0].clone(),
                ));
            }
        }
        None
    }

    async fn pick_second_project(
        client: &Client,
        context: &Context,
        candidates: &[&str],
        exclude: &str,
    ) -> Option<(String, VersionChoice, VersionChoice)> {
        let rest: Vec<&str> = candidates
            .iter()
            .copied()
            .filter(|project| *project != exclude)
            .collect();
        pick_two_releases(client, context, &rest).await
    }

    async fn install_older_version(
        managed: &ManagedPaths,
        instance: &InstanceId,
        context: &Context,
        client: &Client,
        project: &str,
        older: &VersionChoice,
    ) -> Result<crate::instance_content::ProviderRecord, ContentError> {
        let empty = ContentState::empty();
        let resolved = client
            .resolve(context, ContentType::Mod, project, &older.id, &empty)
            .await
            .map_err(|error| ContentError::Acquisition(error.to_string()))?;
        let records = install_provider_plans(managed, instance, resolved.plans)
            .await?
            .pop()
            .expect("root record");
        Ok(records)
    }

    /// Explicit Check-for-Updates performance over a representative set of
    /// real managed mods, plus one coherent bulk preview. Read-only apart
    /// from the initial installs; enabled only through
    /// AURORA_UPDATES_PERF_ROOT pointing at a disposable temporary directory.
    #[tokio::test]
    #[ignore = "requires live Modrinth and an explicit disposable root"]
    async fn live_updates_check_performance() {
        let root =
            PathBuf::from(std::env::var_os("AURORA_UPDATES_PERF_ROOT").expect("disposable root"));
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-updates-perf-")
        );
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance = InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
        std::fs::create_dir_all(root.join("instances").join(instance.as_str())).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        crate::instance_content::ensure_directory(&managed, &instance, ContentType::Mod).unwrap();
        {
            use crate::instances::{
                InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
                settings::InstanceConfiguration,
            };
            let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
            config.set_aurora_enabled(false);
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(
                InstanceRecord::from_installed(
                    instance.clone(),
                    "Phase H updates performance",
                    InstanceState::Ready,
                    InstalledConfiguration {
                        minecraft_version: "1.21.11".into(),
                        platform: crate::instances::platform::PlatformPin::Fabric {
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
        }
        let context = Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        };
        let client = Client::official();

        let candidates = [
            "AANobbMI", // Sodium
            "mOgUt4GM", // Mod Menu
            "C7lc0oPV", // Lithium
            "YL57xq9U", // Iris
            "NNAgCjsB", // Entity Culling
            "uXXizFIs", // FerriteCore
            "P7dR8mSH", // Fabric API
            "Bh37bMjO", // Memory Leak Fix
            "gvQqBUqZ", // Indium
        ];
        let mut installed_projects = Vec::new();
        for project in candidates {
            let Some((_, older, _)) = pick_two_releases(&client, &context, &[project]).await else {
                continue;
            };
            if install_older_version(&managed, &instance, &context, &client, project, &older)
                .await
                .is_ok()
            {
                installed_projects.push(project.to_owned());
            }
        }
        let managed_count = installed_projects.len();
        println!("perf set: {managed_count} managed mods ({installed_projects:?})");
        assert!(managed_count >= 4, "need a representative set of real mods");

        let start = std::time::Instant::now();
        let report = check_updates(
            &managed,
            &instance,
            &context,
            &client,
            Some(ContentType::Mod),
        )
        .await
        .unwrap();
        let discovery = start.elapsed();
        let ready: Vec<_> = report
            .entries
            .iter()
            .filter(|entry| entry.status == UpdateStatus::UpdateAvailable)
            .collect();
        println!(
            "perf discovery: {} mods checked, {} provider version requests, duration {:?}, {} ready / {} pinned / {} blocked / {} current",
            report.entries.len() + report.dependency_managed,
            report.entries.len(),
            discovery,
            ready.len(),
            report
                .entries
                .iter()
                .filter(|e| e.status == UpdateStatus::PinnedUpdateAvailable)
                .count(),
            report
                .entries
                .iter()
                .filter(|e| e.status == UpdateStatus::Blocked)
                .count(),
            report
                .entries
                .iter()
                .filter(|e| e.status == UpdateStatus::UpToDate)
                .count(),
        );

        let targets: Vec<_> = ready
            .iter()
            .map(|entry| target(ContentType::Mod, &entry.project_id))
            .collect();
        if !targets.is_empty() {
            let start = std::time::Instant::now();
            match resolve_updates_preview(&managed, &instance.to_string().as_str(), &targets).await
            {
                Ok((_, _, _, _, preview)) => {
                    let planning = start.elapsed();
                    println!(
                        "perf bulk planning: {} roots in one transaction, duration {planning:?}, fingerprint {}",
                        preview.targets.len(),
                        preview.preview_fingerprint
                    );
                }
                Err(error) => {
                    // A genuinely conflicting target graph is refused whole;
                    // report it as a measured outcome, not a failure.
                    println!(
                        "perf bulk planning: refused coherently in {:?} ({})",
                        start.elapsed(),
                        error.error_code()
                    );
                }
            }
            // No mutation: this is the preview cost only.
        }
    }

    /// Real Modrinth shader-pack installation acceptance through the actual
    /// quick-install command path (provider state validation, bounded
    /// environment candidate loop, verified acquisition, activation),
    /// followed by remove/reinstall lifecycle. Enabled only through
    /// AURORA_SHADER_ACCEPT_ROOT pointing at a disposable temporary
    /// directory whose name starts with aurora-shader-acceptance-.
    #[tokio::test]
    #[ignore = "requires live Modrinth and an explicit disposable root"]
    async fn live_shader_installation_acceptance() {
        let root =
            PathBuf::from(std::env::var_os("AURORA_SHADER_ACCEPT_ROOT").expect("disposable root"));
        assert!(root.starts_with(std::env::temp_dir()));
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("aurora-shader-acceptance-")
        );
        std::fs::create_dir_all(root.join("instances")).unwrap();
        let instance = InstanceId::new(format!("safe-{}", uuid::Uuid::new_v4().simple())).unwrap();
        std::fs::create_dir_all(root.join("instances").join(instance.as_str())).unwrap();
        let managed = ManagedPaths::from_app_local_data_dir(root.clone()).unwrap();
        {
            use crate::instances::{
                InstanceRecord, InstanceRegistry, InstanceState, platform::InstalledConfiguration,
                settings::InstanceConfiguration,
            };
            let mut config = InstanceConfiguration::for_minecraft_version("1.21.11");
            config.set_aurora_enabled(false);
            let mut registry = InstanceRegistry::empty();
            registry.instances_mut().push(
                InstanceRecord::from_installed(
                    instance.clone(),
                    "Shader acceptance",
                    InstanceState::Ready,
                    InstalledConfiguration {
                        minecraft_version: "1.21.11".into(),
                        platform: crate::instances::platform::PlatformPin::Fabric {
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
        }
        let context = Context {
            minecraft_version: "1.21.11".into(),
            loader: "fabric".into(),
            fabric_api_protected: false,
        };
        let client = Client::official();
        let packs = [
            ("HVnmMxH1", "Complementary Reimagined"),
            ("Q1vvjJYV", "BSL Shaders"),
        ];
        let mut installed = Vec::new();
        for (project, title) in packs {
            // The exact quick_install_modrinth body without AppHandle.
            let state =
                crate::instance_content::ContentState::load_and_migrate(&managed, &instance)
                    .unwrap();
            let details = client
                .details(&context, ContentType::ShaderPack, project)
                .await
                .unwrap();
            let mut choices = details.versions;
            choices.sort_by_key(|version| version.version_type != "release");
            let choice = &choices[0];
            let mut resolved = client
                .resolve(
                    &context,
                    ContentType::ShaderPack,
                    project,
                    &choice.id,
                    &state,
                )
                .await
                .unwrap();
            crate::instance_content::reconcile_provider_resolution(
                &managed,
                &instance,
                &mut resolved,
            )
            .await
            .unwrap();
            let conflicts = crate::instance_content::preview_provider_conflicts(
                &managed,
                &instance,
                &resolved.plans,
            )
            .await
            .unwrap();
            assert!(conflicts.is_empty(), "{title}: unexpected conflicts");
            let records = crate::instance_content::install_provider_plans(
                &managed,
                &instance,
                resolved.plans,
            )
            .await
            .unwrap_or_else(|error| panic!("{title}: install failed: {error}"));
            let record = &records[0];
            let path = managed
                .instance_paths(&instance)
                .shaderpacks()
                .join(&record.file_name);
            let bytes = std::fs::read(&path).unwrap();
            use sha2::Digest as _;
            println!(
                "{title}: project={project} version={} file={} size={} sha256={:x} published512_match={}",
                record.version_id,
                record.file_name,
                bytes.len(),
                sha2::Sha256::digest(&bytes),
                record
                    .file_id
                    .eq_ignore_ascii_case(&format!("{:x}", sha2::Sha512::digest(&bytes)))
            );
            assert!(path.is_file());
            assert!(path.starts_with(managed.instance_paths(&instance).shaderpacks()));
            installed.push((record.identity(), record.file_name.clone(), title));
        }
        let state = crate::instance_content::ContentState::load(&managed, &instance).unwrap();
        assert_eq!(state.entries.len(), 2);
        println!("managed state: 2 shader records persisted");

        // Remove/reinstall lifecycle for the first pack.
        let (identity, file_name, title) = installed[0].clone();
        crate::instance_content::remove_provider_graph(&managed, &instance, &state, &identity)
            .unwrap();
        let shaderpacks = managed
            .instance_paths(&instance)
            .shaderpacks()
            .to_path_buf();
        assert!(!shaderpacks.join(&file_name).exists(), "removed file gone");
        let state = crate::instance_content::ContentState::load(&managed, &instance).unwrap();
        assert_eq!(state.entries.len(), 1, "second pack retained");
        println!("{title}: removed; the other pack is untouched");

        // Reinstall through the same path.
        let details = client
            .details(&context, ContentType::ShaderPack, &identity.project_id)
            .await
            .unwrap();
        let mut choices = details.versions;
        choices.sort_by_key(|version| version.version_type != "release");
        let resolved = client
            .resolve(
                &context,
                ContentType::ShaderPack,
                &identity.project_id,
                &choices[0].id,
                &state,
            )
            .await
            .unwrap();
        crate::instance_content::install_provider_plans(&managed, &instance, resolved.plans)
            .await
            .unwrap();
        assert!(shaderpacks.join(&file_name).is_file(), "reinstalled");
        println!("{title}: reinstalled through the same verified pipeline");
    }
}
