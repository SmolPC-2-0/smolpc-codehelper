use super::backend::{BackendDecision, BackendDecisionKey, FailureCounters};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

const BACKEND_STORE_VERSION: u32 = 2;
const BACKEND_STORE_FILENAME: &str = "backend_decisions.v2.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendDecisionRecord {
    pub key: BackendDecisionKey,
    pub persisted_decision: Option<BackendDecision>,
    pub failure_counters: FailureCounters,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BackendStoreFile {
    version: u32,
    records: HashMap<String, BackendDecisionRecord>,
}

#[derive(Debug, Clone)]
pub struct BackendStore {
    path: PathBuf,
    file: BackendStoreFile,
}

impl BackendStore {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let file = if path.exists() {
            let raw = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read backend decision store: {e}"))?;
            match serde_json::from_str::<BackendStoreFile>(&raw) {
                Ok(parsed) if parsed.version == BACKEND_STORE_VERSION => parsed,
                Ok(parsed) => {
                    log::warn!(
                        "Ignoring backend decision store with unsupported version {} (expected {})",
                        parsed.version,
                        BACKEND_STORE_VERSION
                    );
                    BackendStoreFile {
                        version: BACKEND_STORE_VERSION,
                        records: HashMap::new(),
                    }
                }
                Err(e) => {
                    log::warn!("Backend decision store is invalid JSON; resetting store: {e}");
                    BackendStoreFile {
                        version: BACKEND_STORE_VERSION,
                        records: HashMap::new(),
                    }
                }
            }
        } else {
            BackendStoreFile {
                version: BACKEND_STORE_VERSION,
                records: HashMap::new(),
            }
        };

        Ok(Self { path, file })
    }

    pub fn get(&self, key: &BackendDecisionKey) -> Option<&BackendDecisionRecord> {
        self.file.records.get(&key.fingerprint())
    }

    pub fn upsert(&mut self, record: BackendDecisionRecord) {
        self.file.records.insert(record.key.fingerprint(), record);
    }

    pub fn persist(&self) -> Result<(), String> {
        let parent = self.path.parent().ok_or_else(|| {
            format!(
                "Invalid backend decision store path: {}",
                self.path.display()
            )
        })?;
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create backend decision store dir: {e}"))?;
        }

        let json = serde_json::to_string_pretty(&self.file)
            .map_err(|e| format!("Failed to serialize backend decision store: {e}"))?;
        let tmp_path = self.path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            Utc::now().timestamp_millis()
        ));

        fs::write(&tmp_path, json)
            .map_err(|e| format!("Failed to write temporary backend decision store file: {e}"))?;

        replace_file_atomic(&tmp_path, &self.path).map_err(|e| {
            if let Err(cleanup_err) = fs::remove_file(&tmp_path) {
                log::warn!(
                    "Failed to clean up temporary backend decision store file {}: {}",
                    tmp_path.display(),
                    cleanup_err
                );
            }
            format!("Failed to atomically replace backend decision store file: {e}")
        })?;

        Ok(())
    }
}

#[cfg(windows)]
fn to_wide_os(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn replace_file_atomic(from: &Path, to: &Path) -> Result<(), String> {
    let from_wide = to_wide_os(from);
    let to_wide = to_wide_os(to);
    let flags = MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH;

    let result = unsafe { MoveFileExW(from_wide.as_ptr(), to_wide.as_ptr(), flags) };
    if result == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file_atomic(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|e| e.to_string())
}

pub fn backend_store_path(data_dir: &Path) -> Result<PathBuf, String> {
    let inference_dir = data_dir.join("inference");
    if !inference_dir.exists() {
        fs::create_dir_all(&inference_dir)
            .map_err(|e| format!("Failed to create inference data directory: {e}"))?;
    }
    Ok(inference_dir.join(BACKEND_STORE_FILENAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::backend::{DecisionReason, InferenceBackend};
    use tempfile::tempdir;

    fn decision_key(driver_version: &str) -> BackendDecisionKey {
        BackendDecisionKey {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            model_artifact_fingerprint: Some("artifact-v1".to_string()),
            app_version: "2.2.0".to_string(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: Some("2.0.0-rc.11".to_string()),
            ort_bundle_fingerprint: Some("ort-bundle-v1".to_string()),
            openvino_runtime_version: Some("2026.0.0".to_string()),
            openvino_genai_version: Some("2026.0.0".to_string()),
            openvino_tokenizers_version: Some("2026.0.0".to_string()),
            openvino_bundle_fingerprint: Some("openvino-bundle-v1".to_string()),
            gpu_adapter_identity: Some("intel:arc:a370m".to_string()),
            gpu_driver_version: Some(driver_version.to_string()),
            gpu_device_id: Some(0),
            npu_adapter_identity: Some("intel:npu".to_string()),
            npu_driver_version: Some("32.0.100.3104".to_string()),
            openvino_npu_max_prompt_len: Some(256),
            openvino_npu_min_response_len: Some(8),
            openvino_message_mode: Some("structured_messages".to_string()),
            selection_profile: Some("default".to_string()),
        }
    }

    fn decision_record(
        key: BackendDecisionKey,
        backend: Option<InferenceBackend>,
    ) -> BackendDecisionRecord {
        BackendDecisionRecord {
            key,
            persisted_decision: backend
                .map(|selected| BackendDecision::new(selected, DecisionReason::DefaultCpu, None)),
            failure_counters: FailureCounters::default(),
            updated_at: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn round_trip_persistence() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");

        let mut store = BackendStore::load(&path).expect("load");
        let key = decision_key("31.0.101.5522");
        store.upsert(decision_record(key.clone(), Some(InferenceBackend::Cpu)));
        store.persist().expect("persist");

        let reloaded = BackendStore::load(&path).expect("reload");
        let record = reloaded.get(&key).expect("record should exist");
        assert_eq!(
            record
                .persisted_decision
                .as_ref()
                .expect("persisted decision")
                .backend,
            InferenceBackend::Cpu
        );
    }

    #[test]
    fn multiple_records_for_same_model_are_retained_when_fingerprints_differ() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");
        let mut store = BackendStore::load(&path).expect("load");

        let key_a = decision_key("31.0.101.5522");
        let key_b = decision_key("31.0.101.5590");

        store.upsert(decision_record(key_a.clone(), Some(InferenceBackend::Cpu)));
        store.upsert(decision_record(
            key_b.clone(),
            Some(InferenceBackend::DirectML),
        ));
        store.persist().expect("persist");

        let reloaded = BackendStore::load(&path).expect("reload");
        assert!(reloaded.get(&key_a).is_some());
        assert!(reloaded.get(&key_b).is_some());
        assert_ne!(key_a.fingerprint(), key_b.fingerprint());
    }

    #[test]
    fn records_can_persist_failure_counters_without_persisted_winner() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");
        let mut store = BackendStore::load(&path).expect("load");
        let key = decision_key("31.0.101.5522");
        let mut record = decision_record(key.clone(), None);
        record.failure_counters.record_directml_failure(
            crate::inference::backend::DirectMLFailureStage::Init,
            "init",
        );

        store.upsert(record);
        store.persist().expect("persist");

        let reloaded = BackendStore::load(&path).expect("reload");
        let record = reloaded.get(&key).expect("record should exist");
        assert!(record.persisted_decision.is_none());
        assert_eq!(record.failure_counters.directml_init_failures, 1);
    }

    #[test]
    fn invalid_json_store_is_reset_to_empty() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");
        fs::write(&path, "{not-json").expect("write invalid json");

        let store = BackendStore::load(&path).expect("load");
        let key = decision_key("31.0.101.5522");
        assert!(store.get(&key).is_none());
    }
}
