use smolpc_engine_client::{EngineChatMessage, RuntimeModePreference};
use smolpc_engine_core::inference::backend::{BackendStatus, CheckModelResponse};
use smolpc_engine_core::models::registry::{ModelDefinition, ModelRegistry};
use smolpc_engine_core::GenerationConfig;
use smolpc_engine_core::GenerationMetrics;
use sysinfo::System;
use tauri::ipc::Channel;

const MEMORY_WARNING_THRESHOLD_GB: f64 = 1.0;
const MEMORY_CRITICAL_THRESHOLD_GB: f64 = 0.6;
const HEAVY_MODE_ADVISORY_THRESHOLD_GB: f64 = 2.0;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct MemoryPressureRequest {
    pub active_mode: Option<String>,
    pub app_minimized: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPressureLevel {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MemoryPressureStatus {
    pub total_gb: f64,
    pub available_gb: f64,
    pub level: MemoryPressureLevel,
    pub threshold_warning_gb: f64,
    pub threshold_critical_gb: f64,
    pub current_model_id: Option<String>,
    pub current_model_estimated_ram_gb: Option<f32>,
    pub recommended_model_id: Option<String>,
    pub model_switch_recommended: bool,
    pub heavy_mode_active: bool,
    pub auto_unloaded: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AvailableModelDto {
    #[serde(flatten)]
    pub model: ModelDefinition,
    /// Legacy alias retained for compatibility with older frontend payload contracts.
    pub path: String,
}

impl From<ModelDefinition> for AvailableModelDto {
    fn from(model: ModelDefinition) -> Self {
        Self {
            path: model.directory.clone(),
            model,
        }
    }
}

fn parse_runtime_mode(mode: &str) -> Result<RuntimeModePreference, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(RuntimeModePreference::Auto),
        "cpu" => Ok(RuntimeModePreference::Cpu),
        "dml" | "directml" => Ok(RuntimeModePreference::Dml),
        "npu" | "openvino" | "openvino_npu" => Ok(RuntimeModePreference::Npu),
        _ => Err(format!(
            "Unsupported runtime mode '{mode}'. Use one of: auto, cpu, dml, npu"
        )),
    }
}

pub(super) fn runtime_mode_label(mode: RuntimeModePreference) -> &'static str {
    match mode {
        RuntimeModePreference::Auto => "auto",
        RuntimeModePreference::Cpu => "cpu",
        RuntimeModePreference::Dml => "dml",
        RuntimeModePreference::Npu => "npu",
    }
}

fn sysinfo_memory_values_are_bytes(total_raw: u64) -> bool {
    // sysinfo may expose bytes (newer releases) or KiB (older releases).
    // Cargo.toml pins sysinfo to 0.32.1; this check guards us if that contract
    // changes during dependency updates.
    // This heuristic is intentionally scoped to supported SmolPC targets where
    // model minimum RAM starts at 8 GB. In that range, KiB totals remain far
    // below 1e9 while byte totals are above 1e9.
    // Sub-1 GB machines are unsupported and may be misclassified.
    total_raw > 1_000_000_000
}

fn raw_memory_to_gb(raw: u64, values_are_bytes: bool) -> f64 {
    if values_are_bytes {
        raw as f64 / (1024.0 * 1024.0 * 1024.0)
    } else {
        raw as f64 / (1024.0 * 1024.0)
    }
}

fn sample_system_memory_gb() -> (f64, f64) {
    let mut system = System::new();
    system.refresh_memory();
    let total_raw = system.total_memory();
    let available_raw = system.available_memory();
    let values_are_bytes = sysinfo_memory_values_are_bytes(total_raw);
    (
        raw_memory_to_gb(total_raw, values_are_bytes),
        raw_memory_to_gb(available_raw, values_are_bytes),
    )
}

fn classify_memory_level(available_gb: f64) -> MemoryPressureLevel {
    if available_gb < MEMORY_CRITICAL_THRESHOLD_GB {
        MemoryPressureLevel::Critical
    } else if available_gb < MEMORY_WARNING_THRESHOLD_GB {
        MemoryPressureLevel::Warning
    } else {
        MemoryPressureLevel::Normal
    }
}

fn normalize_mode_id(mode: Option<&str>) -> Option<String> {
    mode.and_then(|value| {
        let trimmed = value.trim().to_ascii_lowercase();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn is_heavy_host_mode(mode: Option<&str>) -> bool {
    // Keep this list aligned with host-tool mode registration in
    // apps/codehelper/src-tauri/src/modes/config.rs.
    matches!(normalize_mode_id(mode).as_deref(), Some("gimp" | "blender"))
}

fn smallest_model_id() -> Option<String> {
    // Recommendations are constrained to the static registry IDs.
    ModelRegistry::available_models()
        .into_iter()
        .min_by(|a, b| {
            a.estimated_runtime_ram_gb
                .partial_cmp(&b.estimated_runtime_ram_gb)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|model| model.id)
}

fn current_model_estimated_ram_gb(current_model_id: Option<&str>) -> Option<f32> {
    current_model_id
        .and_then(ModelRegistry::get_model)
        .map(|model| model.estimated_runtime_ram_gb)
}

fn should_recommend_model_switch(
    level: MemoryPressureLevel,
    available_gb: f64,
    current_model_id: Option<&str>,
    recommended_model_id: Option<&str>,
    heavy_mode_active: bool,
) -> bool {
    let Some(recommended_model_id) = recommended_model_id else {
        return false;
    };
    let Some(current_model_id) = current_model_id else {
        return false;
    };
    if current_model_id == recommended_model_id {
        return false;
    }

    !matches!(level, MemoryPressureLevel::Normal)
        || (heavy_mode_active && available_gb < HEAVY_MODE_ADVISORY_THRESHOLD_GB)
}

fn build_memory_pressure_message(
    available_gb: f64,
    level: MemoryPressureLevel,
    recommended_model_id: Option<&str>,
    model_switch_recommended: bool,
    heavy_mode_active: bool,
    auto_unloaded: bool,
) -> Option<String> {
    let recommendation = recommended_model_id
        .map(|model_id| format!("Switch to '{model_id}' for lower memory usage."))
        .unwrap_or_else(|| "Close other heavy apps and retry.".to_string());

    if auto_unloaded {
        return Some(format!(
            "Available RAM is {available_gb:.1} GB and the app was minimized, so the model was unloaded to avoid instability. {recommendation}"
        ));
    }

    match level {
        MemoryPressureLevel::Critical => Some(format!(
            "Available RAM is critically low ({available_gb:.1} GB). {recommendation}"
        )),
        MemoryPressureLevel::Warning => {
            if model_switch_recommended {
                Some(format!(
                    "Available RAM is low ({available_gb:.1} GB). {recommendation}"
                ))
            } else {
                Some(format!(
                    "Available RAM is low ({available_gb:.1} GB). Close heavy apps to avoid generation failures."
                ))
            }
        }
        MemoryPressureLevel::Normal => {
            if heavy_mode_active && model_switch_recommended {
                Some(format!(
                    "Blender/GIMP mode is active with {available_gb:.1} GB free RAM. {recommendation}"
                ))
            } else {
                None
            }
        }
    }
}

fn is_generation_in_progress_unload_error(error: &str) -> bool {
    error.contains("Cannot unload model while generation is in progress")
}

#[tauri::command]
pub async fn load_model(
    model_id: String,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<String, String> {
    log::info!("Loading model via supervisor: {model_id}");
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client.load_model(&model_id).await.map_err(|e| {
        log::error!("Model load failed for {model_id}: {e}");
        format!("Failed to load model: {e}")
    })?;
    supervisor.set_desired_model(Some(model_id.clone())).await;
    supervisor.refresh_status().await;
    if let Ok(status) = client.status().await {
        log::info!(
            "Model loaded: model={} backend={:?} runtime_engine={:?} selection_reason={:?}",
            model_id,
            status.backend_status.active_backend,
            status.backend_status.runtime_engine,
            status.backend_status.selection_reason
        );
    }
    Ok(format!("Model loaded: {model_id}"))
}

#[tauri::command]
pub async fn unload_model(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<String, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .unload_model(false)
        .await
        .map_err(|e| format!("Failed to unload model: {e}"))?;
    supervisor.set_desired_model(None).await;
    supervisor.refresh_status().await;
    Ok("Model unloaded successfully".to_string())
}

#[tauri::command]
pub async fn list_models(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<Vec<AvailableModelDto>, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .list_models()
        .await
        .map(|models| models.into_iter().map(AvailableModelDto::from).collect())
        .map_err(|e| format!("Failed to list models: {e}"))
}

#[tauri::command]
pub async fn get_current_model(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<Option<String>, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to get engine status: {e}"))?;
    Ok(status.current_model)
}

#[tauri::command]
pub async fn get_inference_backend_status(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<BackendStatus, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to get backend status: {e}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
pub async fn set_inference_runtime_mode(
    mode: String,
    model_id: Option<String>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<BackendStatus, String> {
    let requested_mode = parse_runtime_mode(&mode)?;
    log::info!(
        "Applying inference runtime mode via supervisor: requested={}",
        runtime_mode_label(requested_mode)
    );

    supervisor.set_runtime_mode(requested_mode).await?;

    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;

    if let Some(model_id) = model_id.as_ref().map(|value| value.trim()) {
        if !model_id.is_empty() {
            log::info!(
                "Reloading model '{}' after runtime mode switch to {}",
                model_id,
                runtime_mode_label(requested_mode)
            );
            client
                .load_model(model_id)
                .await
                .map_err(|e| format!("Failed to load model after mode switch: {e}"))?;
            supervisor
                .set_desired_model(Some(model_id.to_string()))
                .await;
            supervisor.refresh_status().await;
        }
    }

    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query engine status after mode switch: {e}"))?;
    Ok(status.backend_status)
}

#[tauri::command]
pub async fn check_model_readiness(
    model_id: String,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<CheckModelResponse, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .check_model_readiness(&model_id)
        .await
        .map_err(|e| format!("Failed to check model readiness: {e}"))
}

/// Compatibility shim for older callers.
///
/// Prefer `check_model_readiness` for new code so lane detail is not lost.
#[tauri::command]
pub async fn check_model_exists(
    model_id: String,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<bool, String> {
    Ok(check_model_readiness(model_id, supervisor)
        .await?
        .any_ready())
}

#[tauri::command]
pub async fn inference_generate(
    prompt: String,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<GenerationMetrics, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .generate_stream(&prompt, config, |token| {
            if let Err(e) = on_token.send(token) {
                log::warn!("Failed to send token via channel: {e}");
            }
        })
        .await
        .map_err(|e| format!("Streaming generation failed: {e}"))
}

#[tauri::command]
pub async fn inference_generate_messages(
    messages: Vec<ChatMessageInput>,
    config: Option<GenerationConfig>,
    on_token: Channel<String>,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<GenerationMetrics, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    let messages = messages
        .into_iter()
        .map(|message| EngineChatMessage {
            role: message.role,
            content: message.content,
        })
        .collect::<Vec<_>>();
    client
        .generate_stream_messages(&messages, config, |token| {
            if let Err(e) = on_token.send(token) {
                log::warn!("Failed to send token via channel: {e}");
            }
        })
        .await
        .map_err(|e| format!("Streaming generation failed: {e}"))
}

#[tauri::command]
pub async fn inference_cancel(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<(), String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    client
        .cancel()
        .await
        .map_err(|e| format!("Failed to cancel generation: {e}"))
}

#[tauri::command]
pub async fn is_generating(
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<bool, String> {
    let client = supervisor
        .get_client(std::time::Duration::from_secs(60))
        .await?;
    let status = client
        .status()
        .await
        .map_err(|e| format!("Failed to query generation state: {e}"))?;
    Ok(status.generating)
}

#[tauri::command]
pub async fn evaluate_memory_pressure(
    request: MemoryPressureRequest,
    supervisor: tauri::State<'_, crate::engine::EngineSupervisorHandle>,
) -> Result<MemoryPressureStatus, String> {
    let (total_gb, available_gb) = sample_system_memory_gb();
    let level = classify_memory_level(available_gb);
    let heavy_mode_active = is_heavy_host_mode(request.active_mode.as_deref());

    let client = supervisor.get_client_if_ready();
    let mut current_model_id = None;
    if let Some(client) = client.as_ref() {
        if let Ok(status) = client.status().await {
            current_model_id = status.current_model;
        }
    }

    let recommended_model_id = smallest_model_id();
    let model_switch_recommended = should_recommend_model_switch(
        level,
        available_gb,
        current_model_id.as_deref(),
        recommended_model_id.as_deref(),
        heavy_mode_active,
    );

    let mut auto_unloaded = false;
    if request.app_minimized && level == MemoryPressureLevel::Critical && current_model_id.is_some()
    {
        if let Some(client) = client.as_ref() {
            match client.unload_model(false).await {
                Ok(()) => {
                    supervisor.set_desired_model(None).await;
                    current_model_id = None;
                    auto_unloaded = true;
                }
                Err(error) => {
                    let error_text = error.to_string();
                    if is_generation_in_progress_unload_error(&error_text) {
                        log::debug!(
                            "Skipped auto-unload during critical memory pressure because generation was in progress"
                        );
                    } else {
                        log::warn!(
                            "Failed to auto-unload model during critical memory pressure: {error_text}"
                        );
                    }
                }
            }
        }
    }

    let current_model_estimated_ram_gb =
        current_model_estimated_ram_gb(current_model_id.as_deref());
    let message = build_memory_pressure_message(
        available_gb,
        level,
        recommended_model_id.as_deref(),
        model_switch_recommended,
        heavy_mode_active,
        auto_unloaded,
    );

    Ok(MemoryPressureStatus {
        total_gb,
        available_gb,
        level,
        threshold_warning_gb: MEMORY_WARNING_THRESHOLD_GB,
        threshold_critical_gb: MEMORY_CRITICAL_THRESHOLD_GB,
        current_model_id,
        current_model_estimated_ram_gb,
        recommended_model_id,
        model_switch_recommended,
        heavy_mode_active,
        auto_unloaded,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysinfo_memory_unit_heuristic_targets_supported_ram_range() {
        assert!(!sysinfo_memory_values_are_bytes(8 * 1024 * 1024));
        assert!(sysinfo_memory_values_are_bytes(8 * 1024 * 1024 * 1024));
        assert!(!sysinfo_memory_values_are_bytes(999_999_488));
    }

    #[test]
    fn sysinfo_032_contract_matches_ci_host_expectation() {
        let mut system = System::new();
        system.refresh_memory();
        let total_raw = system.total_memory();

        assert!(
            !sysinfo_memory_values_are_bytes(total_raw),
            "Pinned sysinfo=0.32.1 is expected to report KiB totals on CI/dev hosts (<1 TiB RAM). Got raw total {total_raw}; revisit memory unit conversion on dependency upgrade."
        );
    }

    #[test]
    fn unload_in_progress_error_detection_matches_engine_message() {
        assert!(is_generation_in_progress_unload_error(
            "/engine/unload failed with HTTP 409: Cannot unload model while generation is in progress"
        ));
        assert!(!is_generation_in_progress_unload_error(
            "Failed to auto-unload model during critical memory pressure: network timeout"
        ));
    }

    #[test]
    fn classify_memory_level_uses_warning_and_critical_thresholds() {
        assert_eq!(
            classify_memory_level(MEMORY_WARNING_THRESHOLD_GB + 0.01),
            MemoryPressureLevel::Normal
        );
        assert_eq!(
            classify_memory_level(MEMORY_WARNING_THRESHOLD_GB - 0.01),
            MemoryPressureLevel::Warning
        );
        assert_eq!(
            classify_memory_level(MEMORY_CRITICAL_THRESHOLD_GB - 0.01),
            MemoryPressureLevel::Critical
        );
    }

    #[test]
    fn recommend_switch_returns_false_when_current_model_is_already_recommended() {
        assert!(!should_recommend_model_switch(
            MemoryPressureLevel::Warning,
            0.8,
            Some("qwen2.5-1.5b-instruct"),
            Some("qwen2.5-1.5b-instruct"),
            false,
        ));
    }

    #[test]
    fn recommend_switch_triggers_for_heavy_mode_even_when_level_is_normal() {
        assert!(should_recommend_model_switch(
            MemoryPressureLevel::Normal,
            HEAVY_MODE_ADVISORY_THRESHOLD_GB - 0.1,
            Some("qwen3-4b"),
            Some("qwen2.5-1.5b-instruct"),
            true,
        ));
        assert!(!should_recommend_model_switch(
            MemoryPressureLevel::Normal,
            HEAVY_MODE_ADVISORY_THRESHOLD_GB + 0.1,
            Some("qwen3-4b"),
            Some("qwen2.5-1.5b-instruct"),
            true,
        ));
    }

    #[test]
    fn heavy_mode_detection_flags_blender_and_gimp() {
        assert!(is_heavy_host_mode(Some("blender")));
        assert!(is_heavy_host_mode(Some("  GIMP ")));
        assert!(!is_heavy_host_mode(Some("code")));
    }

    #[test]
    fn memory_message_reports_auto_unload_when_triggered() {
        let message = build_memory_pressure_message(
            0.4,
            MemoryPressureLevel::Critical,
            Some("qwen2.5-1.5b-instruct"),
            true,
            false,
            true,
        )
        .expect("auto-unload message");
        assert!(message.contains("was minimized"));
        assert!(message.contains("qwen2.5-1.5b-instruct"));
    }
}
