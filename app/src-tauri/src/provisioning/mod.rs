pub mod downloader;
pub mod extractor;
pub mod manifest;
pub mod singleton;
pub mod source;
pub mod types;

pub use types::*;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::ipc::Channel;

use crate::commands::hardware::HardwareCache;

// ---------------------------------------------------------------------------
// Managed state: cancel flag shared between provision_models and cancel_provisioning
// ---------------------------------------------------------------------------

pub struct ProvisioningCancel(pub Arc<AtomicBool>);

impl Default for ProvisioningCancel {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Returns the boot-time state so the frontend can decide whether to show the
/// setup wizard or go straight into the main app.
#[tauri::command]
pub fn get_boot_state() -> AppBootState {
    AppBootState {
        models_provisioned: source::models_exist(),
        portable: is_portable(),
    }
}

/// Scans for local model archives (breadcrumb, USB drives) and optionally
/// checks internet connectivity to HuggingFace.
#[tauri::command]
pub async fn detect_model_sources() -> Result<Vec<ModelSource>, ProvisioningError> {
    let client = reqwest::Client::new();
    let internet = downloader::check_internet(
        &client,
        "https://huggingface.co/api/models/OpenVINO/Qwen2.5-1.5B-Instruct-int4-ov",
    )
    .await;
    Ok(source::detect_sources(internet))
}

/// Hardware-aware model recommendation.
///
/// Priority: discrete GPU (DirectML) > NPU (OpenVINO) > CPU (OpenVINO).
/// Within each tier, 16 GB+ RAM gets qwen3-4b, otherwise qwen2.5-1.5b-instruct.
#[tauri::command]
pub async fn get_recommended_model(
    hw_cache: tauri::State<'_, HardwareCache>,
) -> Result<ModelRecommendation, ProvisioningError> {
    let hw = hw_cache.get_or_detect().await.map_err(|e| ProvisioningError {
        code: ProvisioningErrorCode::SourceUnavailable,
        message: format!("Hardware detection failed: {e}"),
    })?;

    let total_gb = hw.memory.total_gb;
    let has_16gb = total_gb >= 16.0;

    // Check for discrete GPU.
    // The engine's DXGI probe populates device_type via wgpu DeviceType::to_string().
    // The app-side sysinfo detector returns an empty GPU list, so this branch is
    // only hit when the engine's hardware info has been forwarded or on future
    // app-side GPU detection improvements.
    let has_discrete_gpu = hw.gpus.iter().any(|gpu| {
        gpu.device_type.eq_ignore_ascii_case("DiscreteGpu")
    });

    let has_npu = hw.npu.as_ref().is_some_and(|npu| npu.detected);

    let rec = if has_discrete_gpu && has_16gb {
        ModelRecommendation {
            model_id: "qwen3-4b".to_string(),
            backend: "dml".to_string(),
            display_name: "Qwen 3 4B (DirectML)".to_string(),
            download_size_bytes: 2_900_000_000,
            reason: "Discrete GPU detected with 16 GB+ RAM".to_string(),
        }
    } else if has_discrete_gpu {
        ModelRecommendation {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            backend: "dml".to_string(),
            display_name: "Qwen 2.5 1.5B (DirectML)".to_string(),
            download_size_bytes: 1_300_000_000,
            reason: "Discrete GPU detected".to_string(),
        }
    } else if has_npu && has_16gb {
        ModelRecommendation {
            model_id: "qwen3-4b".to_string(),
            backend: "openvino".to_string(),
            display_name: "Qwen 3 4B (OpenVINO NPU)".to_string(),
            download_size_bytes: 2_200_000_000,
            reason: "NPU detected with 16 GB+ RAM".to_string(),
        }
    } else if has_npu {
        ModelRecommendation {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            backend: "openvino".to_string(),
            display_name: "Qwen 2.5 1.5B (OpenVINO)".to_string(),
            download_size_bytes: 900_000_000,
            reason: "NPU detected".to_string(),
        }
    } else {
        // CPU-only fallback
        ModelRecommendation {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            backend: "openvino".to_string(),
            display_name: "Qwen 2.5 1.5B (OpenVINO CPU)".to_string(),
            download_size_bytes: 900_000_000,
            reason: "CPU-only — no discrete GPU or NPU detected".to_string(),
        }
    };

    log::info!(
        "Model recommendation: {} ({}) — {}",
        rec.model_id,
        rec.backend,
        rec.reason
    );

    Ok(rec)
}

/// Sets the cancel flag so that an in-progress `provision_models` run will
/// stop at the next checkpoint (archive boundary or extraction entry).
#[tauri::command]
pub fn cancel_provisioning(cancel: tauri::State<'_, ProvisioningCancel>) {
    log::info!("Provisioning cancellation requested");
    cancel.0.store(true, Ordering::Relaxed);
}

/// Main provisioning orchestrator.
///
/// Acquires a system-wide singleton guard, then iterates the manifest from
/// `source`, verifying and extracting each archive into `%LOCALAPPDATA%\SmolPC\models\`.
/// Progress events are streamed to the frontend via the Tauri Channel.
///
/// `model_ids`: if non-empty, only archives whose `id` is in this list are
/// processed. An empty list means "all models in the manifest".
#[tauri::command]
pub async fn provision_models(
    source: ModelSource,
    model_ids: Vec<String>,
    on_event: Channel<ProvisioningEvent>,
    cancel_state: tauri::State<'_, ProvisioningCancel>,
) -> Result<ProvisioningResult, ProvisioningError> {
    // 1. Acquire singleton guard — prevents concurrent provisioning across instances.
    let _guard = singleton::SingletonGuard::acquire().map_err(|e| ProvisioningError {
        code: ProvisioningErrorCode::AlreadyRunning,
        message: e,
    })?;

    // 2. Reset cancel flag at the start of a new run.
    cancel_state.0.store(false, Ordering::Relaxed);
    let cancel = Arc::clone(&cancel_state.0);

    // 3. Resolve target directory.
    let target_dir = dirs::data_local_dir()
        .ok_or_else(|| ProvisioningError {
            code: ProvisioningErrorCode::ExtractionFailed,
            message: "Cannot determine local data directory".to_string(),
        })?
        .join("SmolPC")
        .join("models");

    match source {
        ModelSource::Local { path } => {
            provision_from_local(&path, &target_dir, &model_ids, cancel, &on_event).await
        }
        ModelSource::Internet { .. } => Err(ProvisioningError {
            code: ProvisioningErrorCode::SourceUnavailable,
            message: "Internet download not yet implemented — use local USB bundle".to_string(),
        }),
    }
}

/// Provisions models from a local archive directory.
async fn provision_from_local(
    source_dir: &std::path::Path,
    target_dir: &std::path::Path,
    model_ids: &[String],
    cancel: Arc<AtomicBool>,
    on_event: &Channel<ProvisioningEvent>,
) -> Result<ProvisioningResult, ProvisioningError> {
    // Parse manifest.
    let manifest_path = source_dir.join("models").join("model-archives.json");
    let manifest = manifest::parse_manifest(&manifest_path).map_err(|e| ProvisioningError {
        code: ProvisioningErrorCode::SourceUnavailable,
        message: format!("Manifest error: {e}"),
    })?;

    // Filter entries.
    let entries: Vec<_> = if model_ids.is_empty() {
        manifest.models
    } else {
        manifest
            .models
            .into_iter()
            .filter(|m| model_ids.contains(&m.id))
            .collect()
    };

    if entries.is_empty() {
        return Err(ProvisioningError {
            code: ProvisioningErrorCode::SourceUnavailable,
            message: "No matching model archives found in manifest".to_string(),
        });
    }

    // Disk space check: sum archive sizes and compare to available space.
    let total_archive_bytes: u64 = entries
        .iter()
        .filter_map(|e| {
            let p = source_dir.join(&e.archive_name);
            p.metadata().ok().map(|m| m.len())
        })
        .sum();

    // Ensure target directory exists before querying available space.
    std::fs::create_dir_all(target_dir).map_err(|e| ProvisioningError {
        code: ProvisioningErrorCode::ExtractionFailed,
        message: format!("Cannot create models directory: {e}"),
    })?;

    let available_space = fs2::available_space(target_dir).map_err(|e| ProvisioningError {
        code: ProvisioningErrorCode::ExtractionFailed,
        message: format!("Cannot check available disk space: {e}"),
    })?;

    // Archives extract to roughly their size; require 1.5x headroom.
    let required_space = total_archive_bytes + total_archive_bytes / 2;
    if available_space < required_space {
        return Err(ProvisioningError {
            code: ProvisioningErrorCode::DiskFull,
            message: format!(
                "Not enough disk space: {:.1} GB available, {:.1} GB required",
                available_space as f64 / 1_073_741_824.0,
                required_space as f64 / 1_073_741_824.0,
            ),
        });
    }

    let mut models_installed = Vec::new();

    for entry in &entries {
        // Check cancellation at archive boundary.
        if cancel.load(Ordering::Relaxed) {
            let _ = on_event.send(ProvisioningEvent::Error {
                code: ProvisioningErrorCode::Cancelled,
                message: "Provisioning cancelled".to_string(),
                retryable: true,
            });
            return Err(ProvisioningError {
                code: ProvisioningErrorCode::Cancelled,
                message: "Provisioning cancelled by user".to_string(),
            });
        }

        // archive_name is the filename; archives sit alongside the manifest in source_dir
        let archive_path = source_dir.join(&entry.archive_name);
        let archive_size = archive_path.metadata().map(|m| m.len()).unwrap_or(0);

        let _ = on_event.send(ProvisioningEvent::ArchiveStarted {
            name: entry.archive_name.clone(),
            total_bytes: archive_size,
        });

        // Verify SHA-256.
        let _ = on_event.send(ProvisioningEvent::Verifying {
            name: entry.archive_name.clone(),
        });

        let cancel_verify = Arc::clone(&cancel);
        let archive_path_owned = archive_path.clone();
        let expected_sha = entry.sha256.clone();

        let sha_ok = tokio::task::spawn_blocking(move || {
            if cancel_verify.load(Ordering::Relaxed) {
                return Err("Cancelled during verification".to_string());
            }
            manifest::verify_sha256(&archive_path_owned, &expected_sha)
        })
        .await
        .map_err(|e| ProvisioningError {
            code: ProvisioningErrorCode::ChecksumMismatch,
            message: format!("Verification task failed: {e}"),
        })?
        .map_err(|e| ProvisioningError {
            code: ProvisioningErrorCode::ChecksumMismatch,
            message: format!("SHA-256 verification failed: {e}"),
        })?;

        if !sha_ok {
            let _ = on_event.send(ProvisioningEvent::Error {
                code: ProvisioningErrorCode::ChecksumMismatch,
                message: format!("Checksum mismatch for {}", entry.archive_name),
                retryable: false,
            });
            return Err(ProvisioningError {
                code: ProvisioningErrorCode::ChecksumMismatch,
                message: format!(
                    "SHA-256 mismatch for {} — archive may be corrupted",
                    entry.archive_name
                ),
            });
        }

        // Extract archive.
        // ZIP archives contain the backend dir internally (e.g., openvino/manifest.json),
        // so extract to models/{id}/ — the ZIP creates the backend subdirectory.
        let extract_target = target_dir.join(&entry.id);
        let cancel_extract = Arc::clone(&cancel);
        let archive_path_extract = archive_path.clone();
        let channel_ref = on_event.clone();

        let extract_result = tokio::task::spawn_blocking(move || {
            extractor::extract_zip(
                &archive_path_extract,
                &extract_target,
                cancel_extract,
                Box::new(move |done, total| {
                    let _ = channel_ref.send(ProvisioningEvent::Progress {
                        bytes_done: done,
                        total_bytes: total,
                    });
                }),
            )
        })
        .await
        .map_err(|e| ProvisioningError {
            code: ProvisioningErrorCode::ExtractionFailed,
            message: format!("Extraction task failed: {e}"),
        })?;

        match extract_result {
            Ok(_) => {
                let _ = on_event.send(ProvisioningEvent::ArchiveComplete {
                    name: entry.archive_name.clone(),
                });
                models_installed.push(entry.id.clone());
            }
            Err(e) if e.contains("cancelled") => {
                let _ = on_event.send(ProvisioningEvent::Error {
                    code: ProvisioningErrorCode::Cancelled,
                    message: "Extraction cancelled".to_string(),
                    retryable: true,
                });
                return Err(ProvisioningError {
                    code: ProvisioningErrorCode::Cancelled,
                    message: "Provisioning cancelled during extraction".to_string(),
                });
            }
            Err(e) => {
                let _ = on_event.send(ProvisioningEvent::Error {
                    code: ProvisioningErrorCode::ExtractionFailed,
                    message: e.clone(),
                    retryable: true,
                });
                return Err(ProvisioningError {
                    code: ProvisioningErrorCode::ExtractionFailed,
                    message: format!("Extraction failed for {}: {e}", entry.archive_name),
                });
            }
        }
    }

    let total_bytes = total_archive_bytes;
    let _ = on_event.send(ProvisioningEvent::Complete {
        models_installed: models_installed.clone(),
    });

    log::info!(
        "Provisioning complete: {} model(s) installed",
        models_installed.len()
    );

    Ok(ProvisioningResult {
        models_installed,
        total_bytes,
    })
}
