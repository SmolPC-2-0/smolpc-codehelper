#[cfg(target_os = "windows")]
use smolpc_engine_core::inference::{
    OpenVinoGenAiGenerator, OpenVinoGenerationControls, OpenVinoPipelineConfig,
};
use smolpc_engine_core::inference::{OpenVinoRuntimeBundle, OpenVinoRuntimeLoader};
use smolpc_engine_core::GenerationConfig;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const OPENVINO_NPU_DRIVER_RECOMMENDED_FLOOR: &str = "32.0.100.3104";
const OPENVINO_NPU_MAX_PROMPT_LEN_ENV: &str = "SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN";
const OPENVINO_NPU_MIN_RESPONSE_LEN_ENV: &str = "SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN";
const DEFAULT_OPENVINO_NPU_MAX_PROMPT_LEN: usize = 512;
const DEFAULT_OPENVINO_NPU_MIN_RESPONSE_LEN: usize = 1024;
const DEFAULT_QWEN_EOS_TOKEN_ID: i64 = 151645;
const DEFAULT_QWEN_STOP_TOKEN_IDS: [i64; 2] = [151643, 151645];
const QWEN25_OPENVINO_MODEL_ID: &str = "qwen2.5-1.5b-instruct";
const QWEN3_OPENVINO_MODEL_ID: &str = "qwen3-4b";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenVinoNpuTuning {
    pub max_prompt_len: usize,
    pub min_response_len: usize,
}

impl Default for OpenVinoNpuTuning {
    fn default() -> Self {
        Self {
            max_prompt_len: DEFAULT_OPENVINO_NPU_MAX_PROMPT_LEN,
            min_response_len: DEFAULT_OPENVINO_NPU_MIN_RESPONSE_LEN,
        }
    }
}

impl OpenVinoNpuTuning {
    #[cfg(target_os = "windows")]
    fn pipeline_config(
        self,
        cache_dir: &Path,
        generation_controls: OpenVinoGenerationControls,
        disable_thinking: bool,
    ) -> OpenVinoPipelineConfig {
        OpenVinoPipelineConfig::npu(cache_dir, self.max_prompt_len, self.min_response_len)
            .with_generation_controls(generation_controls)
            .with_disable_thinking(disable_thinking)
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpenVinoModelTuning {
    pub request_defaults: Option<GenerationConfig>,
    pub disable_thinking: bool,
    pub presence_penalty: Option<f32>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
struct OpenVinoStagedGenerationConfig {
    eos_token_id: Option<i64>,
    stop_token_ids: Option<Vec<i64>>,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(default)]
struct OpenVinoLaneManifest {
    entrypoint: Option<String>,
    required_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OpenVinoReadyArtifact {
    pub manifest_path: PathBuf,
    pub required_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum OpenVinoArtifactCheck {
    Missing {
        manifest_path: PathBuf,
    },
    Invalid {
        manifest_path: PathBuf,
        message: String,
    },
    Incomplete {
        manifest_path: PathBuf,
        required_files: Vec<PathBuf>,
        missing_files: Vec<PathBuf>,
    },
    Ready(OpenVinoReadyArtifact),
}

impl OpenVinoArtifactCheck {
    #[cfg(test)]
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }

    pub fn ready_artifact(&self) -> Option<&OpenVinoReadyArtifact> {
        match self {
            Self::Ready(artifact) => Some(artifact),
            _ => None,
        }
    }

    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "artifact_missing",
            Self::Invalid { .. } => "artifact_invalid",
            Self::Incomplete { .. } => "artifact_incomplete",
            Self::Ready(_) => "ready",
        }
    }

    pub fn message(&self) -> Option<String> {
        match self {
            Self::Missing { manifest_path } => Some(format!(
                "OpenVINO manifest not found: {}",
                manifest_path.display()
            )),
            Self::Invalid { message, .. } => Some(message.clone()),
            Self::Incomplete { missing_files, .. } => Some(format!(
                "OpenVINO manifest references missing files: {}",
                missing_files
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            Self::Ready(_) => None,
        }
    }

    pub fn fingerprint_paths(&self) -> Vec<PathBuf> {
        match self {
            Self::Missing { manifest_path } | Self::Invalid { manifest_path, .. } => {
                vec![manifest_path.clone()]
            }
            Self::Incomplete {
                manifest_path,
                required_files,
                ..
            } => {
                let mut paths = vec![manifest_path.clone()];
                paths.extend(required_files.iter().cloned());
                paths
            }
            Self::Ready(artifact) => {
                let mut paths = vec![artifact.manifest_path.clone()];
                paths.extend(artifact.required_files.iter().cloned());
                paths
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OpenVinoStartupProbeResult {
    pub hardware_detected: bool,
    pub startup_ready: bool,
    pub device_visible: bool,
    pub adapter_identity: Option<String>,
    pub device_name: Option<String>,
    pub driver_version: Option<String>,
    pub failure_class: Option<String>,
    pub failure_message: Option<String>,
}

pub struct OpenVinoPreflightReady {
    #[cfg(target_os = "windows")]
    pub generator: OpenVinoGenAiGenerator,
}

pub enum OpenVinoPreflightResult {
    Ready(OpenVinoPreflightReady),
    Timeout,
    Failed { class: String, message: String },
}

pub fn inspect_openvino_artifact(manifest_path: &Path) -> OpenVinoArtifactCheck {
    if !manifest_path.exists() {
        return OpenVinoArtifactCheck::Missing {
            manifest_path: manifest_path.to_path_buf(),
        };
    }

    let raw = match std::fs::read_to_string(manifest_path) {
        Ok(raw) => raw,
        Err(error) => {
            return OpenVinoArtifactCheck::Invalid {
                manifest_path: manifest_path.to_path_buf(),
                message: format!(
                    "Failed to read OpenVINO manifest {}: {error}",
                    manifest_path.display()
                ),
            }
        }
    };

    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);

    let manifest = match serde_json::from_str::<OpenVinoLaneManifest>(raw) {
        Ok(manifest) => manifest,
        Err(error) => {
            return OpenVinoArtifactCheck::Invalid {
                manifest_path: manifest_path.to_path_buf(),
                message: format!(
                    "Failed to parse OpenVINO manifest {}: {error}",
                    manifest_path.display()
                ),
            }
        }
    };

    let manifest_dir = match manifest_path.parent() {
        Some(parent) => parent.to_path_buf(),
        None => {
            return OpenVinoArtifactCheck::Invalid {
                manifest_path: manifest_path.to_path_buf(),
                message: format!(
                    "Invalid OpenVINO manifest path: {}",
                    manifest_path.display()
                ),
            }
        }
    };

    let mut logical_files = BTreeSet::new();
    if let Some(entrypoint) = manifest.entrypoint.as_deref() {
        logical_files.insert(entrypoint.trim().to_string());
    }
    for file in manifest.required_files {
        logical_files.insert(file.trim().to_string());
    }

    if logical_files.is_empty() {
        return OpenVinoArtifactCheck::Invalid {
            manifest_path: manifest_path.to_path_buf(),
            message: format!(
                "OpenVINO manifest {} must declare at least one required file",
                manifest_path.display()
            ),
        };
    }

    let mut required_files = Vec::new();
    for logical_path in logical_files {
        if logical_path.is_empty() {
            return OpenVinoArtifactCheck::Invalid {
                manifest_path: manifest_path.to_path_buf(),
                message: format!(
                    "OpenVINO manifest {} contains an empty file path",
                    manifest_path.display()
                ),
            };
        }
        let relative = PathBuf::from(&logical_path);
        if !is_safe_relative_path(&relative) {
            return OpenVinoArtifactCheck::Invalid {
                manifest_path: manifest_path.to_path_buf(),
                message: format!(
                    "OpenVINO manifest {} contains an unsafe relative path '{}'",
                    manifest_path.display(),
                    logical_path
                ),
            };
        }
        required_files.push(manifest_dir.join(relative));
    }

    let missing_files = required_files
        .iter()
        .filter(|path| !path.exists())
        .cloned()
        .collect::<Vec<_>>();
    if !missing_files.is_empty() {
        return OpenVinoArtifactCheck::Incomplete {
            manifest_path: manifest_path.to_path_buf(),
            required_files,
            missing_files,
        };
    }

    OpenVinoArtifactCheck::Ready(OpenVinoReadyArtifact {
        manifest_path: manifest_path.to_path_buf(),
        required_files,
    })
}

pub fn probe_openvino_startup(
    bundle: &OpenVinoRuntimeBundle,
    hardware_detected: bool,
) -> OpenVinoStartupProbeResult {
    if !hardware_detected {
        return OpenVinoStartupProbeResult {
            hardware_detected,
            failure_class: Some("no_npu_hardware".to_string()),
            failure_message: Some("No NPU hardware detected on this machine".to_string()),
            ..Default::default()
        };
    }

    if let Some(code) = bundle.npu_failure_code() {
        return OpenVinoStartupProbeResult {
            hardware_detected,
            failure_class: Some(code.to_string()),
            failure_message: Some(format!(
                "OpenVINO runtime bundle is incomplete at {}",
                bundle.display_root().display()
            )),
            ..Default::default()
        };
    }

    match OpenVinoRuntimeLoader::probe_npu_device(bundle) {
        Ok(device_probe) => {
            let npu_device_name = device_probe.npu_device_name.clone();
            if npu_device_name.is_none() {
                return OpenVinoStartupProbeResult {
                    hardware_detected,
                    failure_class: Some("openvino_npu_driver_missing".to_string()),
                    failure_message: Some(
                        "NPU hardware was detected, but OpenVINO did not expose an NPU device"
                            .to_string(),
                    ),
                    ..Default::default()
                };
            }

            let driver_version = device_probe.driver_version.clone();
            let full_device_name = device_probe
                .full_device_name
                .clone()
                .or_else(|| npu_device_name.clone());
            let adapter_identity = full_device_name.as_ref().map(|device_name| {
                let device_name = device_name.trim().to_ascii_lowercase().replace(' ', "_");
                let runtime_name = npu_device_name
                    .as_deref()
                    .unwrap_or("NPU")
                    .trim()
                    .to_ascii_lowercase();
                format!("openvino:{runtime_name}:{device_name}")
            });
            let npu_tuning = match resolve_openvino_npu_tuning() {
                Ok(tuning) => tuning,
                Err(error) => {
                    return OpenVinoStartupProbeResult {
                        hardware_detected,
                        device_visible: true,
                        adapter_identity,
                        device_name: full_device_name,
                        driver_version,
                        failure_class: Some("openvino_npu_config_invalid".to_string()),
                        failure_message: Some(error),
                        ..Default::default()
                    };
                }
            };
            if let Err(error) = OpenVinoGenAiGenerator::runtime_available(bundle) {
                let (failure_class, failure_message) =
                    classify_openvino_runtime_activation_error(&error);
                return OpenVinoStartupProbeResult {
                    hardware_detected,
                    device_visible: true,
                    adapter_identity,
                    device_name: full_device_name,
                    driver_version,
                    failure_class: Some(failure_class),
                    failure_message: Some(failure_message),
                    ..Default::default()
                };
            }

            let advisory = classify_driver_diagnostic(driver_version.as_deref());
            log::info!(
                "OpenVINO NPU tuning resolved: max_prompt_len={}, min_response_len={}",
                npu_tuning.max_prompt_len,
                npu_tuning.min_response_len,
            );

            OpenVinoStartupProbeResult {
                hardware_detected,
                startup_ready: true,
                device_visible: true,
                adapter_identity,
                device_name: full_device_name,
                driver_version,
                failure_class: advisory.as_ref().map(|(class, _)| class.clone()),
                failure_message: advisory.map(|(_, message)| message),
            }
        }
        Err(error) => OpenVinoStartupProbeResult {
            hardware_detected,
            failure_class: Some("openvino_npu_plugin_unavailable".to_string()),
            failure_message: Some(error),
            ..Default::default()
        },
    }
}

pub fn is_blocking_openvino_probe_failure(class: &str) -> bool {
    !matches!(
        class,
        "openvino_npu_driver_unknown" | "openvino_npu_driver_recommended_update"
    )
}

pub fn run_openvino_preflight(
    bundle: &OpenVinoRuntimeBundle,
    model_id: &str,
    artifact: &OpenVinoReadyArtifact,
    probe: &OpenVinoStartupProbeResult,
    cache_dir: &Path,
) -> OpenVinoPreflightResult {
    if !probe.startup_ready || !probe.device_visible {
        return OpenVinoPreflightResult::Failed {
            class: "openvino_npu_startup_probe_failed".to_string(),
            message: probe.failure_message.clone().unwrap_or_else(|| {
                "OpenVINO startup probe did not expose a usable NPU device".to_string()
            }),
        };
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (bundle, model_id, artifact, cache_dir);
        return OpenVinoPreflightResult::Failed {
            class: "openvino_npu_platform_unsupported".to_string(),
            message: "OpenVINO NPU runtime is only supported on Windows".to_string(),
        };
    }

    #[cfg(target_os = "windows")]
    {
        let tuning = match resolve_openvino_npu_tuning() {
            Ok(tuning) => tuning,
            Err(message) => {
                return OpenVinoPreflightResult::Failed {
                    class: "openvino_npu_config_invalid".to_string(),
                    message,
                };
            }
        };
        let Some(model_dir) = artifact.manifest_path.parent() else {
            return OpenVinoPreflightResult::Failed {
                class: "openvino_npu_compile_failed".to_string(),
                message: format!(
                    "Invalid OpenVINO manifest path: {}",
                    artifact.manifest_path.display()
                ),
            };
        };

        let model_tuning = openvino_model_tuning_for_model(model_id);
        let generation_controls = openvino_generation_controls_for_model(model_id, model_dir);
        log::info!(
            "OpenVINO generation controls for {}: eos_token_id={:?}, min_new_tokens={:?}, stop_token_ids={:?}, stop_strings={:?}, ignore_eos={:?}, presence_penalty={:?}",
            model_id,
            generation_controls.eos_token_id,
            generation_controls.min_new_tokens,
            generation_controls.stop_token_ids,
            generation_controls.stop_strings,
            generation_controls.ignore_eos,
            generation_controls.presence_penalty
        );
        let pipeline_config = tuning.pipeline_config(
            cache_dir,
            generation_controls,
            model_tuning.disable_thinking,
        );
        let generator = match OpenVinoGenAiGenerator::new(bundle, model_dir, &pipeline_config) {
            Ok(generator) => generator,
            Err(message) => {
                return OpenVinoPreflightResult::Failed {
                    class: "openvino_npu_compile_failed".to_string(),
                    message,
                }
            }
        };

        match generator.run_preflight("Warmup preflight") {
            Ok(_) => OpenVinoPreflightResult::Ready(OpenVinoPreflightReady { generator }),
            Err(message) => OpenVinoPreflightResult::Failed {
                class: "openvino_npu_runtime_failed".to_string(),
                message,
            },
        }
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }

    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn classify_driver_diagnostic(driver_version: Option<&str>) -> Option<(String, String)> {
    match driver_version {
        Some(version) if !version.trim().is_empty() => {
            if version_is_older_than(version, OPENVINO_NPU_DRIVER_RECOMMENDED_FLOOR) {
                Some((
                    "openvino_npu_driver_recommended_update".to_string(),
                    format!(
                        "Intel NPU driver {version} is below the troubleshooting floor {OPENVINO_NPU_DRIVER_RECOMMENDED_FLOOR}"
                    ),
                ))
            } else {
                None
            }
        }
        _ => Some((
            "openvino_npu_driver_unknown".to_string(),
            "OpenVINO exposed an NPU device, but the NPU driver version was unreadable".to_string(),
        )),
    }
}

fn classify_openvino_runtime_activation_error(error: &str) -> (String, String) {
    if error.contains("Missing symbol ov_genai_")
        || (error.contains("ov_genai_") && error.contains("GetProcAddress failed"))
    {
        return (
            "openvino_genai_c_api_missing".to_string(),
            format!(
                "The staged openvino_genai_c.dll is missing the required ov_genai_* C API exports for the native adapter: {error}"
            ),
        );
    }

    (
        "openvino_genai_runtime_unavailable".to_string(),
        format!("OpenVINO GenAI runtime activation failed: {error}"),
    )
}

pub fn resolve_openvino_npu_tuning() -> Result<OpenVinoNpuTuning, String> {
    Ok(OpenVinoNpuTuning {
        max_prompt_len: parse_positive_env_usize(
            OPENVINO_NPU_MAX_PROMPT_LEN_ENV,
            DEFAULT_OPENVINO_NPU_MAX_PROMPT_LEN,
        )?,
        min_response_len: parse_positive_env_usize(
            OPENVINO_NPU_MIN_RESPONSE_LEN_ENV,
            DEFAULT_OPENVINO_NPU_MIN_RESPONSE_LEN,
        )?,
    })
}

fn parse_positive_env_usize(name: &str, default: usize) -> Result<usize, String> {
    let Some(raw) = std::env::var(name).ok() else {
        return Ok(default);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }

    let value = trimmed
        .parse::<usize>()
        .map_err(|error| format!("{name} must be a positive integer, got '{trimmed}': {error}"))?;
    if value == 0 {
        return Err(format!("{name} must be greater than zero, got '{trimmed}'"));
    }
    Ok(value)
}

#[cfg(target_os = "windows")]
pub(crate) fn openvino_model_tuning_for_model(model_id: &str) -> OpenVinoModelTuning {
    if model_id == QWEN3_OPENVINO_MODEL_ID {
        return OpenVinoModelTuning {
            request_defaults: Some(GenerationConfig {
                temperature: 0.7,
                top_p: Some(0.8),
                top_k: Some(20),
                ..GenerationConfig::default()
            }),
            disable_thinking: true,
            presence_penalty: Some(1.5),
        };
    }

    if model_id == QWEN25_OPENVINO_MODEL_ID {
        return OpenVinoModelTuning {
            request_defaults: Some(GenerationConfig {
                temperature: 0.7,
                top_p: Some(0.8),
                top_k: Some(20),
                ..GenerationConfig::default()
            }),
            disable_thinking: false,
            presence_penalty: Some(1.5),
        };
    }

    OpenVinoModelTuning::default()
}

#[cfg(target_os = "windows")]
pub(crate) fn openvino_generation_controls_for_model(
    model_id: &str,
    model_dir: &Path,
) -> OpenVinoGenerationControls {
    let staged = read_openvino_staged_generation_config(model_dir);
    let model_tuning = openvino_model_tuning_for_model(model_id);
    let stop_token_ids = staged
        .as_ref()
        .and_then(|config| config.stop_token_ids.clone())
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| DEFAULT_QWEN_STOP_TOKEN_IDS.to_vec());

    // The supported Qwen baseline uses ChatML with identical
    // special token IDs: <|im_end|>=151645, <|endoftext|>=151643.
    // Explicit controls are required because the OpenVINO C config is built manually and does
    // not inherit generation_config.json defaults.
    OpenVinoGenerationControls {
        eos_token_id: staged
            .as_ref()
            .and_then(|config| config.eos_token_id)
            .or(Some(DEFAULT_QWEN_EOS_TOKEN_ID)),
        min_new_tokens: None,
        stop_token_ids: Some(stop_token_ids),
        // stop_strings operates on accumulated decoded text (incl. special tokens) — catches
        // <|im_end|> even when the NPU StaticLLMPipeline doesn't honour stop_token_ids reliably.
        stop_strings: Some(vec!["<|im_end|>".to_string(), "<|endoftext|>".to_string()]),
        ignore_eos: Some(false),
        presence_penalty: model_tuning.presence_penalty,
    }
}

#[cfg(target_os = "windows")]
fn read_openvino_staged_generation_config(
    model_dir: &Path,
) -> Option<OpenVinoStagedGenerationConfig> {
    let path = model_dir.join("generation_config.json");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!(
                "Failed to read OpenVINO generation config at {}: {}",
                path.display(),
                error
            );
            return None;
        }
    };
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    match serde_json::from_str::<OpenVinoStagedGenerationConfig>(raw) {
        Ok(config) => Some(config),
        Err(error) => {
            log::warn!(
                "Failed to parse OpenVINO generation config at {}: {}",
                path.display(),
                error
            );
            None
        }
    }
}

fn version_is_older_than(actual: &str, floor: &str) -> bool {
    let actual = parse_version(actual);
    let floor = parse_version(floor);
    match (actual, floor) {
        (Some(actual), Some(floor)) => actual < floor,
        _ => false,
    }
}

fn parse_version(raw: &str) -> Option<Vec<u32>> {
    raw.split('.')
        .map(|segment| segment.trim().parse::<u32>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        classify_openvino_runtime_activation_error, inspect_openvino_artifact,
        openvino_generation_controls_for_model, openvino_model_tuning_for_model,
        resolve_openvino_npu_tuning, version_is_older_than,
    };
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    #[test]
    fn openvino_manifest_requires_declared_files() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(&manifest_path, "{}").expect("write manifest");

        let artifact = inspect_openvino_artifact(&manifest_path);
        assert_eq!(artifact.reason_code(), "artifact_invalid");
    }

    #[test]
    fn openvino_manifest_is_ready_when_required_files_exist() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(temp.path().join("model.xml"), []).expect("write model");
        fs::write(&manifest_path, r#"{"required_files":["model.xml"]}"#).expect("write manifest");

        let artifact = inspect_openvino_artifact(&manifest_path);
        assert!(artifact.is_ready());
    }

    #[test]
    fn openvino_manifest_tolerates_utf8_bom() {
        let temp = tempdir().expect("tempdir");
        let manifest_path = temp.path().join("manifest.json");
        fs::write(temp.path().join("model.xml"), []).expect("write model");
        fs::write(
            &manifest_path,
            b"\xEF\xBB\xBF{\"required_files\":[\"model.xml\"]}",
        )
        .expect("write manifest");

        let artifact = inspect_openvino_artifact(&manifest_path);
        assert!(artifact.is_ready());
    }

    #[test]
    fn driver_version_floor_detection_is_ordered() {
        assert!(version_is_older_than("32.0.100.3103", "32.0.100.3104"));
        assert!(!version_is_older_than("32.0.100.3104", "32.0.100.3104"));
        assert!(!version_is_older_than("32.0.100.3105", "32.0.100.3104"));
    }

    #[test]
    fn openvino_runtime_activation_symbol_errors_are_classified() {
        let (class, message) = classify_openvino_runtime_activation_error(
            "Missing symbol ov_genai_llm_pipeline_create: GetProcAddress failed",
        );

        assert_eq!(class, "openvino_genai_c_api_missing");
        assert!(message.contains("ov_genai_*"));
    }

    #[test]
    fn openvino_npu_tuning_uses_defaults_when_env_is_unset() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN");
        std::env::remove_var("SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN");

        let tuning = resolve_openvino_npu_tuning().expect("default tuning");
        assert_eq!(tuning.max_prompt_len, 512);
        assert_eq!(tuning.min_response_len, 1024);
    }

    #[test]
    fn openvino_npu_tuning_rejects_invalid_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN", "bad");
        std::env::remove_var("SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN");

        let err = resolve_openvino_npu_tuning().expect_err("invalid env should fail");
        assert!(err.contains("SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN"));

        std::env::remove_var("SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN");
    }

    #[test]
    fn openvino_npu_tuning_rejects_zero() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN", "0");
        std::env::remove_var("SMOLPC_OPENVINO_NPU_MAX_PROMPT_LEN");

        let err = resolve_openvino_npu_tuning().expect_err("zero should fail");
        assert!(err.contains("greater than zero"));

        std::env::remove_var("SMOLPC_OPENVINO_NPU_MIN_RESPONSE_LEN");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn qwen3_openvino_tuning_matches_non_thinking_defaults() {
        let tuning = openvino_model_tuning_for_model("qwen3-4b");
        let defaults = tuning
            .request_defaults
            .as_ref()
            .expect("qwen3 should provide request defaults");

        assert!(tuning.disable_thinking);
        assert_eq!(tuning.presence_penalty, Some(1.5));
        assert_eq!(defaults.temperature, 0.7);
        assert_eq!(defaults.top_p, Some(0.8));
        assert_eq!(defaults.top_k, Some(20));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn qwen25_openvino_tuning_matches_cpu_defaults() {
        let tuning = openvino_model_tuning_for_model("qwen2.5-1.5b-instruct");
        let defaults = tuning
            .request_defaults
            .as_ref()
            .expect("qwen2.5 should provide request defaults");

        assert!(!tuning.disable_thinking);
        assert_eq!(tuning.presence_penalty, Some(1.5));
        assert_eq!(defaults.temperature, 0.7);
        assert_eq!(defaults.top_p, Some(0.8));
        assert_eq!(defaults.top_k, Some(20));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn openvino_generation_controls_prefer_staged_stop_token_ids() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("generation_config.json"),
            r#"{"eos_token_id":7,"stop_token_ids":[7,9,11]}"#,
        )
        .expect("write generation config");

        let controls = openvino_generation_controls_for_model("qwen3-4b", temp.path());
        assert_eq!(controls.eos_token_id, Some(7));
        assert_eq!(controls.stop_token_ids, Some(vec![7, 9, 11]));
        assert_eq!(controls.presence_penalty, Some(1.5));
    }

    /// Documents the NPU greedy constraint: host tuning requests sampling
    /// (temperature > 0), but core `create_generation_config()` overrides to
    /// `do_sample=false` when the device target is NPU.
    #[cfg(target_os = "windows")]
    #[test]
    fn npu_greedy_constraint_documented() {
        let tuning = openvino_model_tuning_for_model("qwen3-4b");
        let defaults = tuning
            .request_defaults
            .expect("qwen3 should provide request defaults");
        assert!(
            defaults.temperature > 0.0,
            "Host tuning uses sampling; NPU override is in core create_generation_config()"
        );
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
