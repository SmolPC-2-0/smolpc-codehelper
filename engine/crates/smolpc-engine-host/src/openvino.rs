use smolpc_engine_core::inference::{
    OpenVinoRuntimeBundle, OpenVinoRuntimeLoader,
};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

const OPENVINO_NPU_DRIVER_RECOMMENDED_FLOOR: &str = "32.0.100.3104";

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
            Self::Incomplete {
                missing_files, ..
            } => Some(format!(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenVinoPreflightResult {
    Ready,
    Timeout,
    #[allow(dead_code)]
    Failed { class: String, message: String },
    RuntimeUnavailable,
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

    let manifest = match serde_json::from_str::<OpenVinoLaneManifest>(&raw) {
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

    if let Some(code) = bundle.failure_code() {
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
            let advisory = classify_driver_diagnostic(driver_version.as_deref());
            let adapter_identity = full_device_name.as_ref().map(|device_name| {
                let device_name = device_name.trim().to_ascii_lowercase().replace(' ', "_");
                let runtime_name = npu_device_name
                    .as_deref()
                    .unwrap_or("NPU")
                    .trim()
                    .to_ascii_lowercase();
                format!("openvino:{runtime_name}:{device_name}")
            });

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

pub fn openvino_runtime_activation_available() -> bool {
    false
}

pub fn run_openvino_preflight(
    _artifact: &OpenVinoReadyArtifact,
    _probe: &OpenVinoStartupProbeResult,
) -> OpenVinoPreflightResult {
    if !openvino_runtime_activation_available() {
        return OpenVinoPreflightResult::RuntimeUnavailable;
    }

    OpenVinoPreflightResult::Ready
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
                        "Intel NPU driver {version} is below the troubleshooting floor {}",
                        OPENVINO_NPU_DRIVER_RECOMMENDED_FLOOR
                    ),
                ))
            } else {
                None
            }
        }
        _ => Some((
            "openvino_npu_driver_unknown".to_string(),
            "OpenVINO exposed an NPU device, but the NPU driver version was unreadable"
                .to_string(),
        )),
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
    use super::{inspect_openvino_artifact, version_is_older_than};
    use std::fs;
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
        fs::write(&manifest_path, r#"{"required_files":["model.xml"]}"#)
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
}
