use crate::engine::{EngineSupervisorHandle, StartupConfig};
use chrono::Utc;
use smolpc_engine_client::{
    read_runtime_env_overrides, EngineStatus, RuntimeModePreference, StartupMode, StartupPolicy,
};
use std::time::Duration;

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
#[serde(rename_all = "snake_case")]
pub enum RuntimeModePreferenceDto {
    #[default]
    Auto,
    Cpu,
    Dml,
    Npu,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct EnsureStartedRequestDto {
    pub mode: StartupModeDto,
    pub startup_policy: Option<StartupPolicyDto>,
    /// Runtime mode preference from user settings. When set, overrides `mode`
    /// for backend selection so the engine spawns on the preferred backend
    /// in a single spawn (no double-spawn).
    pub runtime_mode_preference: Option<RuntimeModePreferenceDto>,
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

    fn fallback_failed(error_message: impl Into<String>) -> Self {
        Self {
            attempt_id: "unknown".to_string(),
            state: "failed".to_string(),
            state_since: now_rfc3339(),
            active_backend: None,
            active_model_id: None,
            error_code: Some("ENGINE_ENSURE_STARTED_FAILED".to_string()),
            error_message: Some(error_message.into()),
            retryable: true,
        }
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn startup_mode_to_runtime_mode_with_auto_preference(
    mode: StartupModeDto,
    auto_preference: RuntimeModePreference,
) -> RuntimeModePreference {
    match mode {
        StartupModeDto::Auto => auto_preference,
        StartupModeDto::DirectmlRequired => RuntimeModePreference::Dml,
    }
}

fn runtime_mode_preference_dto_to_runtime_mode(
    dto: &RuntimeModePreferenceDto,
) -> RuntimeModePreference {
    match dto {
        RuntimeModePreferenceDto::Auto => RuntimeModePreference::Auto,
        RuntimeModePreferenceDto::Cpu => RuntimeModePreference::Cpu,
        RuntimeModePreferenceDto::Dml => RuntimeModePreference::Dml,
        RuntimeModePreferenceDto::Npu => RuntimeModePreference::Npu,
    }
}

fn startup_mode_to_runtime_mode(mode: StartupModeDto) -> RuntimeModePreference {
    startup_mode_to_runtime_mode_with_auto_preference(
        mode,
        read_runtime_env_overrides().runtime_mode,
    )
}

fn startup_mode_to_engine_mode(mode: StartupModeDto) -> StartupMode {
    match mode {
        StartupModeDto::Auto => StartupMode::Auto,
        StartupModeDto::DirectmlRequired => StartupMode::DirectmlRequired,
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
    if status.ready || status.current_model.is_some() {
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

fn normalize_startup_policy(request: &EnsureStartedRequestDto) -> StartupPolicy {
    let default_model_id = request
        .startup_policy
        .as_ref()
        .and_then(|policy| policy.default_model_id.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    StartupPolicy { default_model_id }
}

fn map_engine_status_to_readiness(status: &EngineStatus) -> EngineReadinessDto {
    let state = status
        .state
        .as_deref()
        .and_then(normalize_contract_state)
        .or_else(|| {
            status
                .startup_phase
                .as_deref()
                .and_then(normalize_contract_state)
        })
        .unwrap_or_else(|| resolve_legacy_state(status));

    let state_since = status
        .state_since
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(now_rfc3339);

    let error_message = status.error_message.clone().or_else(|| {
        status
            .last_error
            .as_ref()
            .map(|error| error.message.clone())
    });
    let error_code = status
        .error_code
        .clone()
        .or_else(|| status.last_error.as_ref().map(|error| error.code.clone()));
    let retryable = status
        .retryable
        .or_else(|| status.last_error.as_ref().map(|error| error.retryable))
        .unwrap_or(state != "ready");

    EngineReadinessDto {
        attempt_id: status.attempt_id.clone(),
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
    supervisor: tauri::State<'_, EngineSupervisorHandle>,
) -> Result<EngineReadinessDto, String> {
    let client = supervisor.get_client(Duration::from_secs(60)).await?;
    let status = client
        .status()
        .await
        .map_err(|error| format!("Failed to query engine status: {error}"))?;
    Ok(map_engine_status_to_readiness(&status))
}

#[tauri::command]
pub async fn engine_ensure_started(
    request: EnsureStartedRequestDto,
    supervisor: tauri::State<'_, EngineSupervisorHandle>,
) -> Result<EngineReadinessDto, String> {
    // runtime_mode_preference (from user settings) takes priority over the
    // legacy `mode` field.  This lets the frontend pass the user's preferred
    // backend in the initial Start command so the engine spawns once on the
    // correct backend — no double-spawn.
    let runtime_mode = match &request.runtime_mode_preference {
        Some(pref) => runtime_mode_preference_dto_to_runtime_mode(pref),
        None => startup_mode_to_runtime_mode(request.mode.clone()),
    };
    let startup_mode = startup_mode_to_engine_mode(request.mode.clone());
    let startup_policy = normalize_startup_policy(&request);

    let config = StartupConfig {
        runtime_mode,
        dml_device_id: read_runtime_env_overrides().dml_device_id,
        default_model_id: startup_policy.default_model_id.clone(),
    };

    supervisor.ensure_started(config).await?;

    let client = supervisor.get_client(Duration::from_secs(60)).await?;

    match client.ensure_started(startup_mode, startup_policy).await {
        Ok(status) => {
            // Track the loaded model so the supervisor can restore it after a crash.
            if let Some(model_id) = status.current_model.as_ref().or(status.active_model_id.as_ref()) {
                supervisor.set_desired_model(Some(model_id.clone())).await;
            }
            supervisor.refresh_status().await;
            Ok(map_engine_status_to_readiness(&status))
        }
        Err(error) => {
            let error_message = error.to_string();
            match client.status().await {
                Ok(status) => {
                    let readiness = map_engine_status_to_readiness(&status);
                    if readiness.state == "failed" {
                        if readiness.error_message.is_some() {
                            return Ok(readiness);
                        }
                        return Ok(EngineReadinessDto {
                            error_code: readiness
                                .error_code
                                .or_else(|| Some("ENGINE_ENSURE_STARTED_FAILED".to_string())),
                            error_message: Some(error_message),
                            ..readiness
                        });
                    }

                    Ok(EngineReadinessDto::failed_from(
                        &readiness,
                        "ENGINE_ENSURE_STARTED_FAILED",
                        error_message,
                        true,
                    ))
                }
                Err(status_error) => Ok(EngineReadinessDto::fallback_failed(format!(
                    "{error}; then status query failed: {status_error}"
                ))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smolpc_engine_client::test_utils::with_runtime_env;
    use smolpc_engine_client::LastStartupError;
    use smolpc_engine_core::inference::backend::{BackendStatus, InferenceBackend};

    fn sample_status() -> EngineStatus {
        EngineStatus {
            ok: true,
            ready: false,
            attempt_id: "startup-1".to_string(),
            state: Some("starting".to_string()),
            startup_phase: Some("starting".to_string()),
            state_since: Some("2026-03-05T18:00:00Z".to_string()),
            active_backend: None,
            active_model_id: None,
            error_code: None,
            error_message: None,
            retryable: None,
            last_error: None,
            engine_api_version: "1.0.0".to_string(),
            effective_mode: Some("auto".to_string()),
            effective_startup_policy: Some(StartupPolicy::default()),
            current_model: None,
            generating: false,
            backend_status: BackendStatus::default(),
        }
    }

    #[test]
    fn startup_mode_maps_to_runtime_mode() {
        assert_eq!(
            startup_mode_to_runtime_mode_with_auto_preference(
                StartupModeDto::Auto,
                RuntimeModePreference::Auto
            ),
            RuntimeModePreference::Auto
        );
        assert_eq!(
            startup_mode_to_runtime_mode_with_auto_preference(
                StartupModeDto::Auto,
                RuntimeModePreference::Cpu
            ),
            RuntimeModePreference::Cpu
        );
        assert_eq!(
            startup_mode_to_runtime_mode_with_auto_preference(
                StartupModeDto::Auto,
                RuntimeModePreference::Dml
            ),
            RuntimeModePreference::Dml
        );
        assert_eq!(
            startup_mode_to_runtime_mode_with_auto_preference(
                StartupModeDto::DirectmlRequired,
                RuntimeModePreference::Auto
            ),
            RuntimeModePreference::Dml
        );
        assert_eq!(
            startup_mode_to_runtime_mode_with_auto_preference(
                StartupModeDto::DirectmlRequired,
                RuntimeModePreference::Cpu
            ),
            RuntimeModePreference::Dml
        );
    }

    #[test]
    fn startup_mode_to_runtime_mode_reads_runtime_env_overrides() {
        with_runtime_env(Some(" directml "), None, || {
            assert_eq!(
                startup_mode_to_runtime_mode(StartupModeDto::Auto),
                RuntimeModePreference::Dml
            );
        });

        with_runtime_env(Some("cpu"), None, || {
            assert_eq!(
                startup_mode_to_runtime_mode(StartupModeDto::Auto),
                RuntimeModePreference::Cpu
            );
            assert_eq!(
                startup_mode_to_runtime_mode(StartupModeDto::DirectmlRequired),
                RuntimeModePreference::Dml
            );
        });

        with_runtime_env(Some("unknown"), None, || {
            assert_eq!(
                startup_mode_to_runtime_mode(StartupModeDto::Auto),
                RuntimeModePreference::Auto
            );
        });
    }

    #[test]
    fn normalize_contract_state_rejects_unknown_values() {
        assert_eq!(normalize_contract_state("ready"), Some("ready".to_string()));
        assert_eq!(normalize_contract_state("unknown"), None);
    }

    #[test]
    fn map_engine_status_prefers_canonical_contract_fields() {
        let mut status = sample_status();
        status.attempt_id = "attempt-7".to_string();
        status.state = Some("loading_model".to_string());
        status.state_since = Some("2026-03-05T18:00:00Z".to_string());
        status.active_backend = Some("directml".to_string());
        status.active_model_id = Some("qwen2.5-1.5b-instruct".to_string());
        status.error_code = Some("E_TEST".to_string());
        status.error_message = Some("boom".to_string());
        status.retryable = Some(false);
        status.ready = true;
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
            Some("qwen2.5-1.5b-instruct")
        );
        assert_eq!(readiness.error_code.as_deref(), Some("E_TEST"));
        assert_eq!(readiness.error_message.as_deref(), Some("boom"));
        assert!(!readiness.retryable);
    }

    #[test]
    fn map_engine_status_falls_back_to_last_error_payload() {
        let mut status = sample_status();
        status.current_model = Some("qwen2.5-1.5b-instruct".to_string());
        status.backend_status.active_backend = Some(InferenceBackend::Cpu);
        status.ready = true;
        status.error_code = None;
        status.error_message = None;
        status.last_error = Some(LastStartupError {
            attempt_id: "attempt-4".to_string(),
            phase: "loading_model".to_string(),
            code: "STARTUP_LOAD_MODEL_FAILED".to_string(),
            message: "model missing".to_string(),
            retryable: true,
            at: "2026-03-05T18:00:00Z".to_string(),
        });

        let readiness = map_engine_status_to_readiness(&status);
        assert_eq!(readiness.state, "starting");
        assert_eq!(readiness.active_backend.as_deref(), Some("cpu"));
        assert_eq!(
            readiness.active_model_id.as_deref(),
            Some("qwen2.5-1.5b-instruct")
        );
        assert_eq!(
            readiness.error_code.as_deref(),
            Some("STARTUP_LOAD_MODEL_FAILED")
        );
        assert_eq!(readiness.error_message.as_deref(), Some("model missing"));
        assert!(readiness.retryable);
    }

    #[test]
    fn normalize_startup_policy_trims_and_validates() {
        let request = EnsureStartedRequestDto {
            mode: StartupModeDto::Auto,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("  qwen3  ".to_string()),
            }),
            runtime_mode_preference: None,
        };
        let normalized = normalize_startup_policy(&request);
        assert_eq!(normalized.default_model_id.as_deref(), Some("qwen3"));

        let missing = EnsureStartedRequestDto {
            mode: StartupModeDto::Auto,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("  ".to_string()),
            }),
            runtime_mode_preference: None,
        };
        let normalized_missing = normalize_startup_policy(&missing);
        assert!(normalized_missing.default_model_id.is_none());
    }

    #[test]
    fn ensure_started_request_dto_serializes_with_contract_field_names() {
        let request = EnsureStartedRequestDto {
            mode: StartupModeDto::DirectmlRequired,
            startup_policy: Some(StartupPolicyDto {
                default_model_id: Some("qwen2.5-1.5b-instruct".to_string()),
            }),
            runtime_mode_preference: None,
        };

        let value =
            serde_json::to_value(&request).expect("ensure_started request should serialize");
        assert_eq!(value["mode"], "directml_required");
        assert_eq!(
            value["startup_policy"]["default_model_id"],
            "qwen2.5-1.5b-instruct"
        );
    }

    #[test]
    fn readiness_dto_deserializes_from_contract_shape() {
        let value = serde_json::json!({
            "attempt_id": "attempt-42",
            "state": "ready",
            "state_since": "2026-03-05T18:00:00Z",
            "active_backend": "directml",
            "active_model_id": "qwen2.5-1.5b-instruct",
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
