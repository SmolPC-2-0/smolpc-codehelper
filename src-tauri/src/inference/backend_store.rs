use super::backend::{
    BackendDecision, BackendDecisionKey, DecisionReason, FailureCounters, InferenceBackend,
};
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

const BACKEND_STORE_VERSION: u32 = 1;
const BACKEND_STORE_FILENAME: &str = "backend_decisions.v1.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendDecisionRecord {
    pub key: BackendDecisionKey,
    pub decision: BackendDecision,
    pub failure_counters: FailureCounters,
    pub updated_at: String,
}

impl BackendDecisionRecord {
    pub fn new(
        key: BackendDecisionKey,
        backend: InferenceBackend,
        reason: DecisionReason,
        benchmark: Option<super::backend::BackendBenchmarkComparison>,
        failure_counters: FailureCounters,
    ) -> Self {
        Self {
            key,
            decision: BackendDecision::new(backend, reason, benchmark),
            failure_counters,
            updated_at: Utc::now().to_rfc3339(),
        }
    }
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, key: &BackendDecisionKey) -> Option<&BackendDecisionRecord> {
        self.file.records.get(&key.fingerprint())
    }

    pub fn upsert(&mut self, record: BackendDecisionRecord) {
        self.file.records.insert(record.key.fingerprint(), record);
    }

    pub fn remove_stale_for_model(&mut self, key: &BackendDecisionKey) -> usize {
        let keep = key.fingerprint();
        let mut removed = 0usize;

        self.file.records.retain(|fingerprint, record| {
            let stale = record.key.model_id == key.model_id && *fingerprint != keep;
            if stale {
                removed += 1;
                return false;
            }
            true
        });

        removed
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
            let _ = fs::remove_file(&tmp_path);
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

pub fn backend_store_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;

    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
    let inference_dir = app_data_dir.join("inference");
    if !inference_dir.exists() {
        fs::create_dir_all(&inference_dir)
            .map_err(|e| format!("Failed to create inference data directory: {e}"))?;
    }
    Ok(inference_dir.join(BACKEND_STORE_FILENAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn decision_record(
        key: BackendDecisionKey,
        backend: InferenceBackend,
    ) -> BackendDecisionRecord {
        BackendDecisionRecord::new(
            key,
            backend,
            DecisionReason::DefaultCpu,
            None,
            FailureCounters::default(),
        )
    }

    #[test]
    fn round_trip_persistence() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");

        let mut store = BackendStore::load(&path).expect("load");
        let key = BackendDecisionKey {
            model_id: "qwen2.5-coder-1.5b".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: "31.0.101.5522".to_string(),
            app_version: "2.2.0".to_string(),
            ort_version: "1.23".to_string(),
            directml_device_id: None,
        };
        store.upsert(decision_record(key.clone(), InferenceBackend::Cpu));
        store.persist().expect("persist");

        let reloaded = BackendStore::load(&path).expect("reload");
        let record = reloaded.get(&key).expect("record should exist");
        assert_eq!(record.decision.backend, InferenceBackend::Cpu);
    }

    #[test]
    fn remove_stale_for_model_keeps_only_current_fingerprint() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");
        let mut store = BackendStore::load(&path).expect("load");

        let key_a = BackendDecisionKey {
            model_id: "qwen2.5-coder-1.5b".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: "31.0.101.5522".to_string(),
            app_version: "2.2.0".to_string(),
            ort_version: "1.23".to_string(),
            directml_device_id: None,
        };
        let mut key_b = key_a.clone();
        key_b.driver_version = "31.0.101.5590".to_string();

        store.upsert(decision_record(key_a.clone(), InferenceBackend::Cpu));
        store.upsert(decision_record(key_b.clone(), InferenceBackend::DirectML));

        let removed = store.remove_stale_for_model(&key_b);
        assert_eq!(removed, 1);
        assert!(store.get(&key_b).is_some());
        assert!(store.get(&key_a).is_none());
    }

    #[test]
    fn invalid_json_store_is_reset_to_empty() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("backend_store.json");
        fs::write(&path, "{not-json").expect("write invalid json");

        let store = BackendStore::load(&path).expect("load");
        let key = BackendDecisionKey {
            model_id: "qwen2.5-coder-1.5b".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: "31.0.101.5522".to_string(),
            app_version: "2.2.0".to_string(),
            ort_version: "1.23".to_string(),
            directml_device_id: None,
        };
        assert!(store.get(&key).is_none());
    }
}
