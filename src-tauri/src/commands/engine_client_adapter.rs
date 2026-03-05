use super::inference::{apply_runtime_mode_preference, resolve_client, InferenceState};
use chrono::Utc;
use smolpc_engine_client::{EngineStatus, RuntimeModePreference};

const CONTRACT_STATES: [&str; 7] = [
    "idle",
    "starting",
    "probing",
    "resolving_assets",
    "loading_model",
    "ready",
    "failed",
];

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartupModeDto {
    #[default]
    Auto,
    DirectmlRequired,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct StartupPolicyDto {
    pub default_model_id: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct EnsureStartedRequestDto {
    pub mode: StartupModeDto,
    pub startup_policy: Option<StartupPolicyDto>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EngineReadinessDto {
    pub attempt_id: String,
    pub state: String,
    pub state_since: String,
    pub active_backend: Option<String>,
    pub active_model_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
}

impl EngineReadinessDto {
    fn failed_from(
        baseline: &EngineReadinessDto,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            attempt_id: baseline.attempt_id.clone(),
            state: "failed".to_string(),
            state_since: now_rfc3339(),
            active_backend: baseline.active_backend.clone(),
            active_model_id: baseline.active_model_id.clone(),
            error_code: Some(error_code.into()),
            error_message: Some(error_message.into()),
            retryable,
        }
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn startup_mode_to_runtime_mode(mode: StartupModeDto) -> RuntimeModePreference {
    match mode {
        StartupModeDto::Auto => RuntimeModePreference::Auto,
        StartupModeDto::DirectmlRequired => RuntimeModePreference::Dml,
    }
}

fn normalize_contract_state(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if CONTRACT_STATES.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

fn resolve_legacy_state(status: &EngineStatus) -> String {
    if let Some(phase) = status.startup_phase.as_deref() {
        if let Some(normalized) = normalize_contract_state(phase) {
            return normalized;
        }
    }

    if status.ready.unwrap_or(false) || status.current_model.is_some() {
        "ready".to_string()
    } else {
        "starting".to_string()
    }
}

fn resolve_active_backend(status: &EngineStatus) -> Option<String> {
    if let Some(active_backend) = status.active_backend.as_ref() {
        return Some(active_backend.to_ascii_lowercase());
    }

    status
        .backend_status
        .active_backend
        .map(|backend| backend.as_str().to_string())
}

fn resolve_default_model_id(request: &EnsureStartedRequestDto) -> Option<String> {
    request
        .startup_policy
        .as_ref()
        .and_then(|policy| policy.default_model_id.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn map_engine_status_to_readiness(status: &EngineStatus) -> EngineReadinessDto {
    let state = status
        .state
        .as_deref()
        .and_then(normalize_contract_state)
        .unwrap_or_else(|| resolve_legacy_state(status));
    let attempt_id = status
        .attempt_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "legacy-attempt".to_string());
    let state_since = status
        .state_since
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(now_rfc3339);
    let error_message = status
        .error_message
        .clone()
        .or_else(|| status.last_error.clone());
    let error_code = status.error_code.clone().or_else(|| {
        error_message
            .as_ref()
            .map(|_| "ENGINE_STATUS_LEGACY_ERROR".to_string())
    });
    let retryable = status.retryable.unwrap_or(state != "ready");

    EngineReadinessDto {
        attempt_id,
        state,
        state_since,
        active_backend: resolve_active_backend(status),
        active_model_id: status
            .active_model_id
            .clone()
            .or_else(|| status.current_model.clone()),
        error_code,
        error_message,
        retryable,
    }
}

#[tauri::command]
pub async fn engine_status(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<EngineReadinessDto, String> {
    let client = resolve_client(&app_handle, &state, false).await?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query engine status: {error}"))?;
    Ok(map_engine_status_to_readiness(&status))
}

#[tauri::command]
pub async fn engine_ensure_started(
    request: EnsureStartedRequestDto,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, InferenceState>,
) -> Result<EngineReadinessDto, String> {
    let runtime_mode = startup_mode_to_runtime_mode(request.mode.clone());
    let mode_changed = apply_runtime_mode_preference(&state, runtime_mode).await;
    let client = resolve_client(&app_handle, &state, mode_changed).await?;

    let mut status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query engine status: {error}"))?;

    let requested_model_id = resolve_default_model_id(&request);
    if let (Some(active_model_id), Some(default_model_id)) = (
        status.current_model.as_deref(),
        requested_model_id.as_deref(),
    ) {
        if active_model_id != default_model_id {
            let baseline = map_engine_status_to_readiness(&status);
            return Ok(EngineReadinessDto::failed_from(
                &baseline,
                "STARTUP_POLICY_CONFLICT",
                format!(
                    "Engine already started with model '{}' but startup policy requested '{}'",
                    active_model_id, default_model_id
                ),
                false,
            ));
        }
    }

    if status.current_model.is_none() {
        let Some(default_model_id) = requested_model_id else {
            let baseline = map_engine_status_to_readiness(&status);
            return Ok(EngineReadinessDto::failed_from(
                &baseline,
                "DEFAULT_MODEL_REQUIRED",
                "Startup policy must include default_model_id until engine-native ensure_started is available",
                false,
            ));
        };

        if let Err(error) = client.load_model(&default_model_id).await {
            let baseline = map_engine_status_to_readiness(&status);
            return Ok(EngineReadinessDto::failed_from(
                &baseline,
                "STARTUP_LOAD_MODEL_FAILED",
                format!("Failed to load startup model '{default_model_id}': {error}"),
                true,
            ));
        }

        status = client
            .status()
            .await
            .map_err(|error| format!("Failed to query engine status: {error}"))?;
    }

    let readiness = map_engine_status_to_readiness(&status);
    if request.mode == StartupModeDto::DirectmlRequired
        && readiness.active_backend.as_deref() != Some("directml")
    {
        return Ok(EngineReadinessDto::failed_from(
            &readiness,
            "DIRECTML_REQUIRED_UNAVAILABLE",
            "directml_required startup policy requested but engine reported non-DirectML backend",
            false,
        ));
    }

    Ok(readiness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smolpc_engine_core::inference::backend::{BackendStatus, InferenceBackend};

    fn sample_status() -> EngineStatus {
        EngineStatus {
            ok: true,
            current_model: None,
            generating: false,
            backend_status: BackendStatus::default(),
            attempt_id: None,
            state: None,
            state_since: None,
            active_backend: None,
            active_model_id: None,
            error_code: None,
            error_message: None,
            retryable: None,
            ready: None,
            startup_phase: None,
            last_error: None,
            engine_api_version: None,
        }
    }

    #[test]
    fn startup_mode_maps_to_runtime_mode() {
        assert_eq!(
            startup_mode_to_runtime_mode(StartupModeDto::Auto),
            RuntimeModePreference::Auto
        );
        assert_eq!(
            startup_mode_to_runtime_mode(StartupModeDto::DirectmlRequired),
            RuntimeModePreference::Dml
        );
    }

    #[test]
    fn normalize_contract_state_rejects_unknown_values() {
        assert_eq!(normalize_contract_state("ready"), Some("ready".to_string()));
        assert_eq!(normalize_contract_state("unknown"), None);
    }

    #[test]
    fn map_engine_status_prefers_canonical_contract_fields() {
        let mut status = sample_status();
        status.attempt_id = Some("attempt-7".to_string());
        status.state = Some("loading_model".to_string());
        status.state_since = Some("2026-03-05T18:00:00Z".to_string());
        status.active_backend = Some("directml".to_string());
        status.active_model_id = Some("qwen3-4b-instruct-2507".to_string());
        status.error_code = Some("E_TEST".to_string());
        status.error_message = Some("boom".to_string());
        status.retryable = Some(false);
        status.ready = Some(true);
        status.startup_phase = Some("ready".to_string());
        status.current_model = Some("legacy-model".to_string());
        status.backend_status.active_backend = Some(InferenceBackend::Cpu);

        let readiness = map_engine_status_to_readiness(&status);
        assert_eq!(readiness.attempt_id, "attempt-7");
        assert_eq!(readiness.state, "loading_model");
        assert_eq!(readiness.state_since, "2026-03-05T18:00:00Z");
        assert_eq!(readiness.active_backend.as_deref(), Some("directml"));
        assert_eq!(
            readiness.active_model_id.as_deref(),
            Some("qwen3-4b-instruct-2507")
        );
        assert_eq!(readiness.error_code.as_deref(), Some("E_TEST"));
        assert_eq!(readiness.error_message.as_deref(), Some("boom"));
        assert!(!readiness.retryable);
    }

    #[test]
    fn map_engine_status_falls_back_to_legacy_aliases() {
        let mut status = sample_status();
        status.current_model = Some("qwen2.5-coder-1.5b".to_string());
        status.backend_status.active_backend = Some(InferenceBackend::Cpu);
        status.ready = Some(true);
        status.last_error = Some("legacy error".to_string());

        let readiness = map_engine_status_to_readiness(&status);
        assert_eq!(readiness.state, "ready");
        assert_eq!(readiness.active_backend.as_deref(), Some("cpu"));
        assert_eq!(
            readiness.active_model_id.as_deref(),
            Some("qwen2.5-coder-1.5b")
        );
        assert_eq!(
            readiness.error_code.as_deref(),
            Some("ENGINE_STATUS_LEGACY_ERROR")
        );
        assert_eq!(readiness.error_message.as_deref(), Some("legacy error"));
    }

    #[test]
    fn resolve_default_model_id_trims_and_validates() {
        let request = EnsureStartedRequestDto {
            mode: StartupModeDto::Auto,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("  qwen3  ".to_string()),
            }),
        };
        assert_eq!(resolve_default_model_id(&request).as_deref(), Some("qwen3"));

        let missing = EnsureStartedRequestDto {
            mode: StartupModeDto::Auto,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("  ".to_string()),
            }),
        };
        assert!(resolve_default_model_id(&missing).is_none());
    }

    #[test]
    fn ensure_started_request_dto_serializes_with_contract_field_names() {
        let request = EnsureStartedRequestDto {
            mode: StartupModeDto::DirectmlRequired,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("qwen3-4b-instruct-2507".to_string()),
            }),
        };

        let value =
            serde_json::to_value(&request).expect("ensure_started request should serialize");
        assert_eq!(value["mode"], "directml_required");
        assert_eq!(
            value["startup_policy"]["default_model_id"],
            "qwen3-4b-instruct-2507"
        );
    }

    #[test]
    fn readiness_dto_deserializes_from_contract_shape() {
        let value = serde_json::json!({
            "attempt_id": "attempt-42",
            "state": "ready",
            "state_since": "2026-03-05T18:00:00Z",
            "active_backend": "directml",
            "active_model_id": "qwen3-4b-instruct-2507",
            "error_code": null,
            "error_message": null,
            "retryable": false
        });

        let readiness: EngineReadinessDto =
            serde_json::from_value(value).expect("readiness dto should deserialize");
        assert_eq!(readiness.attempt_id, "attempt-42");
        assert_eq!(readiness.state, "ready");
        assert!(!readiness.retryable);
    }
}
