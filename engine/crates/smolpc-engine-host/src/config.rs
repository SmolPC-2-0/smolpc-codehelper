use smolpc_engine_core::inference::backend::InferenceBackend;
use smolpc_engine_core::models::ModelRegistry;
use std::cmp::Ordering as CmpOrdering;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::types::{
    ParsedArgs, ReadinessState, StartupError, StartupPolicy, ENGINE_DEFAULT_MODEL_ENV,
    LEGACY_DEFAULT_MODEL_ENV, STARTUP_DEFAULT_MODEL_INVALID, STARTUP_DML_REQUIRED_UNAVAILABLE,
    STARTUP_MEMORY_PRESSURE, STARTUP_MODEL_ASSET_MISSING, STARTUP_MODEL_LOAD_FAILED,
};
const MEMORY_PRESSURE_HINT_SENTINEL: &str = "Memory pressure detected.";

pub(crate) fn epoch_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as u64,
        Err(_) => 0,
    }
}

pub(crate) fn default_data_dir() -> PathBuf {
    if let Some(path) = dirs::data_local_dir() {
        return path.join("SmolPC").join("engine");
    }
    PathBuf::from(".smolpc-engine")
}

pub(crate) fn normalize_non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|candidate| {
        let trimmed = candidate.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub(crate) fn env_default_model_id() -> Option<String> {
    normalize_non_empty(std::env::var(ENGINE_DEFAULT_MODEL_ENV).ok())
        .or_else(|| normalize_non_empty(std::env::var(LEGACY_DEFAULT_MODEL_ENV).ok()))
}

pub(crate) fn select_best_model_for_ram(
    models: &[smolpc_engine_core::ModelDefinition],
    total_ram_gb: f64,
) -> Option<(String, f32)> {
    let mut eligible: Vec<_> = models
        .iter()
        .filter(|m| (m.min_ram_gb as f64) <= total_ram_gb)
        .collect();
    eligible.sort_by(|a, b| {
        b.min_ram_gb
            .partial_cmp(&a.min_ram_gb)
            .unwrap_or(CmpOrdering::Equal)
    });
    eligible.first().map(|m| (m.id.clone(), m.min_ram_gb))
}

pub(crate) fn built_in_default_model_id() -> Option<String> {
    let models = ModelRegistry::available_models();
    let total_ram_gb = hardware_query::HardwareInfo::query()
        .ok()
        .map(|info| info.memory().total_gb());
    if let Some(ram) = total_ram_gb {
        if let Some((id, min)) = select_best_model_for_ram(&models, ram) {
            log::info!(
                "Auto-selected default model '{id}' (total RAM: {ram:.1}GB, model requires: {min:.1}GB)",
            );
            return Some(id);
        }
        log::warn!(
            "No model fits available RAM ({ram:.1}GB) — falling back to smallest registered model",
        );
    }
    models.into_iter().next().map(|m| m.id)
}

pub(crate) fn resolve_default_model_id_with_sources(
    request_model_id: Option<String>,
    config_model_id: Option<String>,
    built_in_model_id: Option<String>,
) -> Result<String, StartupError> {
    if let Some(request_model) = normalize_non_empty(request_model_id) {
        return Ok(request_model);
    }
    if let Some(config_model) = normalize_non_empty(config_model_id) {
        return Ok(config_model);
    }
    if let Some(built_in_model) = normalize_non_empty(built_in_model_id) {
        return Ok(built_in_model);
    }
    Err(StartupError {
        phase: ReadinessState::ResolvingAssets,
        code: STARTUP_DEFAULT_MODEL_INVALID,
        message: "No default model is configured or registered".to_string(),
        retryable: false,
    })
}

pub(crate) fn resolve_default_model_id(
    startup_policy: &StartupPolicy,
) -> Result<String, StartupError> {
    resolve_default_model_id_with_sources(
        startup_policy.default_model_id.clone(),
        env_default_model_id(),
        built_in_default_model_id(),
    )
}

pub(crate) fn classify_startup_model_error(error: &str) -> StartupError {
    let lowered = error.to_ascii_lowercase();
    if is_memory_pressure_error(error) {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: STARTUP_MEMORY_PRESSURE,
            message: with_memory_pressure_hint(error, None),
            retryable: true,
        };
    }
    if lowered.contains("unknown model id") {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: STARTUP_DEFAULT_MODEL_INVALID,
            message: error.to_string(),
            retryable: false,
        };
    }
    if lowered.contains("not found")
        || lowered.contains("missing")
        || lowered.contains("artifact is incomplete")
    {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: STARTUP_MODEL_ASSET_MISSING,
            message: error.to_string(),
            retryable: false,
        };
    }
    if lowered.contains("requires directml backend") || lowered.contains("directml") {
        return StartupError {
            phase: ReadinessState::LoadingModel,
            code: STARTUP_DML_REQUIRED_UNAVAILABLE,
            message: error.to_string(),
            retryable: false,
        };
    }
    StartupError {
        phase: ReadinessState::LoadingModel,
        code: STARTUP_MODEL_LOAD_FAILED,
        message: error.to_string(),
        retryable: true,
    }
}

pub(crate) fn is_memory_pressure_error(error: &str) -> bool {
    let lowered = error.to_ascii_lowercase();
    [
        "out of memory",
        "not enough memory",
        "insufficient memory",
        "memory allocation",
        "failed to allocate",
        "cannot allocate memory",
        "std::bad_alloc",
        "bad alloc",
        "e_outofmemory",
        "resource exhausted",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

pub(crate) fn with_memory_pressure_hint(error: &str, current_model_id: Option<&str>) -> String {
    if !is_memory_pressure_error(error) {
        return error.to_string();
    }
    if error.contains(MEMORY_PRESSURE_HINT_SENTINEL) {
        return error.to_string();
    }

    let suggested_model = ModelRegistry::available_models()
        .into_iter()
        .min_by(|a, b| {
            a.estimated_runtime_ram_gb
                .partial_cmp(&b.estimated_runtime_ram_gb)
                .unwrap_or(CmpOrdering::Equal)
        })
        .filter(|model| current_model_id.is_none_or(|current| model.id != current))
        .map(|model| model.id);

    let suggestion = if let Some(model_id) = suggested_model {
        format!("Try switching to '{model_id}' or close other heavy apps and retry.")
    } else {
        "Close other heavy apps and retry.".to_string()
    };

    format!("{error} [{MEMORY_PRESSURE_HINT_SENTINEL} {suggestion}]")
}

pub(crate) fn parse_args() -> ParsedArgs {
    let mut port = 19432u16;
    let mut data_dir = default_data_dir();
    let mut resource_dir = None;
    let mut app_version = "dev".to_string();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => {
                if let Some(v) = args.next() {
                    if let Ok(p) = v.parse::<u16>() {
                        port = p;
                    }
                }
            }
            "--data-dir" => {
                if let Some(v) = args.next() {
                    data_dir = PathBuf::from(v);
                }
            }
            "--resource-dir" => {
                if let Some(v) = args.next() {
                    resource_dir = Some(PathBuf::from(v));
                }
            }
            "--app-version" => {
                if let Some(v) = args.next() {
                    app_version = v;
                }
            }
            _ => {}
        }
    }

    let queue_size = std::env::var("SMOLPC_ENGINE_QUEUE_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3)
        .max(1);
    let queue_timeout = Duration::from_secs(
        std::env::var("SMOLPC_ENGINE_QUEUE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60)
            .max(1),
    );
    let model_idle_unload =
        parse_idle_timeout_secs("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS", None, 30);
    let process_idle_exit =
        parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", None, 60);

    ParsedArgs {
        port,
        data_dir,
        resource_dir,
        app_version,
        queue_size,
        queue_timeout,
        model_idle_unload,
        process_idle_exit,
    }
}

pub(crate) fn parse_idle_timeout_secs(
    key: &str,
    default_secs: Option<u64>,
    min_secs: u64,
) -> Option<Duration> {
    match std::env::var(key) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(0) => None,
            Ok(secs) => Some(Duration::from_secs(secs.max(min_secs))),
            Err(_) => default_secs.map(|secs| Duration::from_secs(secs.max(min_secs))),
        },
        Err(_) => default_secs.map(|secs| Duration::from_secs(secs.max(min_secs))),
    }
}

pub(crate) fn parse_force_override() -> Option<InferenceBackend> {
    let value = std::env::var("SMOLPC_FORCE_EP").ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" => Some(InferenceBackend::Cpu),
        "dml" | "directml" => Some(InferenceBackend::DirectML),
        "openvino" | "openvino_npu" => Some(InferenceBackend::OpenVinoNpu),
        _ => None,
    }
}

pub(crate) fn parse_dml_device_id_env() -> Option<i32> {
    std::env::var("SMOLPC_DML_DEVICE_ID")
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
}
