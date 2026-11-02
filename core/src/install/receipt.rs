//! Installation transaction receipts: durable, staging-local evidence that
//! distinguishes an interrupted launcher installation from anything else.
//!
//! The receipt lives *inside* the disposable `.install-staging/` directory
//! (never inside the game tree) and is written atomically at every stage
//! transition. Because the installed-state manifest is still the sole
//! completion marker and staging is removed when an installation commits,
//! the receipt's roles are narrow and honest:
//!
//! - **Detection.** A receipt present at the start of the next install
//!   attempt proves the staging tree belonged to an interrupted Aurora
//!   transaction (and names its last completed stage), so its disposal is
//!   an explained recovery rather than blind cleanup of an unknown tree.
//! - **Explainability.** The stage history, processor tool identities, and
//!   generated-output observations answer "what was happening when this
//!   died" for users and developers without touching user data.
//!
//! The receipt grants no authority by its filename: a malformed or
//! future-schema receipt is reported as unknown evidence, staging disposal
//! still relies on the derived-path proof, and no committed state is ever
//! derived from it. This is deliberately *not* the broad installed-game
//! schema migration; it is the small separate document F1 requires.

use std::io;
use std::path::{Path, PathBuf};

/// The receipt file name inside the staging directory.
pub const RECEIPT_FILE_NAME: &str = "transaction-receipt.json";

/// The only supported receipt schema.
const SCHEMA_VERSION: u32 = 1;

/// The closed stage vocabulary of an installation transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Prepared,
    Acquired,
    Processing,
    Processed,
    Verifying,
    Verified,
    Replacing,
    Committing,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Acquired => "acquired",
            Self::Processing => "processing",
            Self::Processed => "processed",
            Self::Verifying => "verifying",
            Self::Verified => "verified",
            Self::Replacing => "replacing",
            Self::Committing => "committing",
        }
    }
}

/// One generated-output observation recorded after a processor pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedObservation {
    pub relative: String,
    pub observed_sha256: String,
    pub size_bytes: u64,
}

/// What an existing receipt said about the staging tree being recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// No receipt: nothing claimed the staging tree (for example, the
    /// process died between directory creation and the first write).
    NoReceipt,
    /// A schema-1 receipt describing an interrupted transaction and its
    /// last completed stage.
    Interrupted {
        transaction_id: String,
        instance_id: String,
        last_stage: String,
    },
    /// A receipt exists but is malformed or from an unsupported schema:
    /// reported honestly, never interpreted.
    Unknown { reason: String },
}

/// Reads and classifies an existing receipt without mutating anything.
pub fn classify_existing(staging_directory: &Path) -> Disposition {
    let path = staging_directory.join(RECEIPT_FILE_NAME);
    use std::io::Read;
    let bounded = || -> io::Result<Vec<u8>> {
        super::coordination::require_regular_or_missing(&path)?;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(io::Error::other("receipt exceeds its parsing bound"));
        }
        Ok(bytes)
    };
    let bytes = match bounded() {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Disposition::NoReceipt,
        Err(error) => {
            return Disposition::Unknown {
                reason: format!("the receipt could not be read: {error}"),
            };
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => {
            return Disposition::Unknown {
                reason: format!("the receipt is not valid JSON: {error}"),
            };
        }
    };
    let schema = value
        .get("schemaVersion")
        .and_then(|schema| schema.as_u64());
    if schema != Some(SCHEMA_VERSION as u64) {
        return Disposition::Unknown {
            reason: format!("unsupported receipt schema {schema:?}"),
        };
    }
    let transaction_id = value
        .get("transactionId")
        .and_then(|id| id.as_str())
        .unwrap_or_default()
        .to_owned();
    let instance_id = value
        .get("instanceId")
        .and_then(|id| id.as_str())
        .unwrap_or_default()
        .to_owned();
    let last_stage = value
        .get("stages")
        .and_then(|stages| stages.as_array())
        .and_then(|stages| stages.last())
        .and_then(|stage| stage.get("stage"))
        .and_then(|stage| stage.as_str())
        .unwrap_or("unknown")
        .to_owned();
    if transaction_id.is_empty() || instance_id.is_empty() {
        return Disposition::Unknown {
            reason: "the receipt lacks transaction or instance identity".to_owned(),
        };
    }
    Disposition::Interrupted {
        transaction_id,
        instance_id,
        last_stage,
    }
}

/// The journal for one running installation transaction.
#[derive(Debug, Clone)]
pub struct TransactionReceipt {
    path: PathBuf,
    transaction_id: String,
    instance_id: String,
    platform: String,
    minecraft_version: String,
    stages: Vec<StageRecord>,
    processors: Option<ProcessorRecord>,
    replacement_reserved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StageRecord {
    stage: String,
    unix_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessorRecord {
    tools: Vec<String>,
    generated: Vec<GeneratedObservation>,
    completed: bool,
}

impl TransactionReceipt {
    /// Starts a transaction and persists the `prepared` stage.
    pub fn begin(
        staging_directory: &Path,
        transaction_id: String,
        instance_id: String,
        platform: String,
        minecraft_version: String,
    ) -> io::Result<Self> {
        let mut receipt = Self {
            path: staging_directory.join(RECEIPT_FILE_NAME),
            transaction_id,
            instance_id,
            platform,
            minecraft_version,
            stages: Vec::new(),
            processors: None,
            replacement_reserved: false,
        };
        receipt.record(Stage::Prepared)?;
        Ok(receipt)
    }

    /// Records a stage transition durably before the work it names.
    pub fn record(&mut self, stage: Stage) -> io::Result<()> {
        let unix_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis() as u64)
            .unwrap_or_default();
        self.stages.push(StageRecord {
            stage: stage.as_str().to_owned(),
            unix_millis,
        });
        self.persist()
    }

    /// Records the processor identities before the processor pass runs.
    pub fn record_processing(&mut self, tools: Vec<String>) -> io::Result<()> {
        self.processors = Some(ProcessorRecord {
            tools,
            generated: Vec::new(),
            completed: false,
        });
        self.record(Stage::Processing)
    }

    /// Records the generated-output observations after a completed pass.
    pub fn record_processed(&mut self, generated: Vec<GeneratedObservation>) -> io::Result<()> {
        if let Some(record) = self.processors.as_mut() {
            record.generated = generated;
            record.completed = true;
        }
        self.record(Stage::Processed)
    }

    /// Records that the proven-managed previous game tree was moved into
    /// staging and awaits either promotion or disposal.
    pub fn record_replacement_reserved(&mut self) -> io::Result<()> {
        self.replacement_reserved = true;
        self.record(Stage::Replacing)
    }

    /// F3R backup placement is authoritative in the separate operation
    /// journal. Do not emit the legacy staging-relative retiredGame claim.
    pub fn record_replacing(&mut self) -> io::Result<()> {
        self.record(Stage::Replacing)
    }

    fn to_document(&self) -> serde_json::Value {
        let mut stages = Vec::with_capacity(self.stages.len());
        for record in &self.stages {
            stages.push(serde_json::json!({
                "stage": record.stage,
                "unixMillis": record.unix_millis,
            }));
        }
        let mut document = serde_json::json!({
            "schemaVersion": SCHEMA_VERSION,
            "transactionId": self.transaction_id,
            "instanceId": self.instance_id,
            "platform": self.platform,
            "minecraftVersion": self.minecraft_version,
            "stages": stages,
        });
        if let Some(record) = &self.processors {
            document["processors"] = serde_json::json!({
                "tools": record.tools,
                "completed": record.completed,
                "generated": record.generated.iter().map(|observation| {
                    serde_json::json!({
                        "relative": observation.relative,
                        "observedSha256": observation.observed_sha256,
                        "sizeBytes": observation.size_bytes,
                    })
                }).collect::<Vec<_>>(),
            });
        }
        if self.replacement_reserved {
            document["replacement"] = serde_json::json!({ "retiredGame": "replaced-game" });
        }
        document
    }

    /// Persists through the repository's atomic sibling-plus-rename
    /// discipline: a partial write can never be observed as a receipt.
    fn persist(&self) -> io::Result<()> {
        let serialized = serde_json::to_string_pretty(&self.to_document())
            .map_err(|error| io::Error::other(error.to_string()))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serialized.as_bytes())?;
        std::fs::rename(&temporary, &self.path)
    }
}

/// Writes a receipt-shaped note for tests: a deliberately malformed or
/// future-schema receipt to exercise unknown-evidence classification.
#[cfg(test)]
pub(crate) fn write_test_receipt(staging_directory: &Path, contents: &str) {
    let _ = std::fs::create_dir_all(staging_directory);
    std::fs::write(
        staging_directory.join(RECEIPT_FILE_NAME),
        contents.as_bytes(),
    )
    .expect("the test receipt writes");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staging(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("aurora-install-receipt-tests")
            .join(std::process::id().to_string())
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn receipts_persist_stages_and_classify_as_interrupted() {
        let root = staging("stages");
        let mut receipt = TransactionReceipt::begin(
            &root,
            "install-1-2".to_owned(),
            "abc123".to_owned(),
            "neoForge".to_owned(),
            "26.2".to_owned(),
        )
        .unwrap();
        receipt.record_processing(vec!["g:a:1".to_owned()]).unwrap();
        receipt
            .record_processed(vec![GeneratedObservation {
                relative: "libraries/x.jar".to_owned(),
                observed_sha256: "0".repeat(64),
                size_bytes: 12,
            }])
            .unwrap();
        receipt.record(Stage::Verifying).unwrap();

        match classify_existing(&root) {
            Disposition::Interrupted {
                transaction_id,
                instance_id,
                last_stage,
            } => {
                assert_eq!(transaction_id, "install-1-2");
                assert_eq!(instance_id, "abc123");
                assert_eq!(last_stage, "verifying");
            }
            other => panic!("unexpected disposition: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_receipts_classify_as_absent() {
        let root = staging("missing");
        assert_eq!(classify_existing(&root), Disposition::NoReceipt);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_receipts_classify_as_unknown() {
        let root = staging("malformed");
        write_test_receipt(&root, "{ not json");
        assert!(matches!(
            classify_existing(&root),
            Disposition::Unknown { .. }
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn future_schema_receipts_classify_as_unknown() {
        let root = staging("future");
        write_test_receipt(
            &root,
            r#"{"schemaVersion": 2, "transactionId": "t", "instanceId": "i", "stages": []}"#,
        );
        assert!(matches!(
            classify_existing(&root),
            Disposition::Unknown { .. }
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn receipts_never_expose_partial_writes() {
        let root = staging("atomic");
        let mut receipt = TransactionReceipt::begin(
            &root,
            "t".to_owned(),
            "i".to_owned(),
            "fabric".to_owned(),
            "1.21.11".to_owned(),
        )
        .unwrap();
        // Every persisted intermediate state is valid JSON with a known
        // schema: there is no window where a half-written receipt exists.
        for stage in [
            Stage::Acquired,
            Stage::Verifying,
            Stage::Verified,
            Stage::Committing,
        ] {
            receipt.record(stage).unwrap();
            assert!(matches!(
                classify_existing(&root),
                Disposition::Interrupted { .. }
            ));
        }
        assert!(!root.join("transaction-receipt.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn repeated_classification_is_idempotent() {
        let root = staging("idempotent");
        TransactionReceipt::begin(
            &root,
            "t".to_owned(),
            "i".to_owned(),
            "fabric".to_owned(),
            "1.21.11".to_owned(),
        )
        .unwrap();
        let first = classify_existing(&root);
        let second = classify_existing(&root);
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn stage_vocabulary_is_closed_and_documented() {
        for (name, stage) in [
            ("prepared", Stage::Prepared),
            ("acquired", Stage::Acquired),
            ("processing", Stage::Processing),
            ("processed", Stage::Processed),
            ("verifying", Stage::Verifying),
            ("verified", Stage::Verified),
            ("replacing", Stage::Replacing),
            ("committing", Stage::Committing),
        ] {
            assert_eq!(stage.as_str(), name);
        }
    }
}
