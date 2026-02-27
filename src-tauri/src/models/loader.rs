/// Model file loading utilities
use std::path::PathBuf;

const MODELS_DIR_OVERRIDE_ENV: &str = "SMOLPC_MODELS_DIR";
const LEGACY_MODEL_FILENAME: &str = "model.onnx";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArtifactBackend {
    Cpu,
    DirectML,
}

impl ModelArtifactBackend {
    pub fn as_dir(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::DirectML => "dml",
        }
    }
}

/// Model loader for finding and validating model files
pub struct ModelLoader;

impl ModelLoader {
    fn default_models_dir() -> PathBuf {
        // Resolve from crate root (`src-tauri`) so paths are stable regardless of process CWD.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")
    }

    fn resolve_models_dir(override_dir: Option<PathBuf>) -> PathBuf {
        match override_dir {
            Some(path) if !path.as_os_str().is_empty() => path,
            _ => Self::default_models_dir(),
        }
    }

    /// Get the models directory path
    ///
    /// Resolution order:
    /// 1. `SMOLPC_MODELS_DIR` environment variable (if set and non-empty)
    /// 2. Deterministic default: `<src-tauri>/models`
    pub fn models_dir() -> PathBuf {
        let override_dir = std::env::var_os(MODELS_DIR_OVERRIDE_ENV).map(PathBuf::from);
        Self::resolve_models_dir(override_dir)
    }

    /// Get path to a specific model
    ///
    /// # Arguments
    /// * `model_name` - Model directory name (e.g., "qwen2.5-coder-1.5b")
    ///
    /// # Returns
    /// Path to model directory
    pub fn model_path(model_name: &str) -> PathBuf {
        Self::models_dir().join(model_name)
    }

    /// Get path to model ONNX file
    pub fn model_file(model_name: &str) -> PathBuf {
        Self::model_path(model_name).join(LEGACY_MODEL_FILENAME)
    }

    /// Get path to backend-specific model ONNX file.
    pub fn backend_model_file(model_name: &str, backend: ModelArtifactBackend) -> PathBuf {
        Self::model_path(model_name)
            .join(backend.as_dir())
            .join(LEGACY_MODEL_FILENAME)
    }

    /// Resolve the CPU model path with backward compatibility.
    ///
    /// Preferred layout:
    /// `<models>/<model_id>/cpu/model.onnx`
    ///
    /// Legacy fallback:
    /// `<models>/<model_id>/model.onnx`
    pub fn resolve_cpu_model_file(model_name: &str) -> PathBuf {
        let cpu_path = Self::backend_model_file(model_name, ModelArtifactBackend::Cpu);
        if cpu_path.exists() {
            cpu_path
        } else {
            Self::model_file(model_name)
        }
    }

    /// Resolve model path for a backend.
    ///
    /// CPU supports legacy fallback. DirectML requires the dedicated backend artifact.
    pub fn resolve_model_file_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> Option<PathBuf> {
        match backend {
            ModelArtifactBackend::Cpu => Some(Self::resolve_cpu_model_file(model_name)),
            ModelArtifactBackend::DirectML => {
                let path = Self::backend_model_file(model_name, backend);
                path.exists().then_some(path)
            }
        }
    }

    /// Get path to tokenizer file
    pub fn tokenizer_file(model_name: &str) -> PathBuf {
        Self::model_path(model_name).join("tokenizer.json")
    }

    /// Check if model files exist
    ///
    /// # Returns
    /// (model_exists, tokenizer_exists)
    pub fn check_model_files(model_name: &str) -> (bool, bool) {
        let model_exists = Self::resolve_cpu_model_file(model_name).exists();
        let tokenizer_exists = Self::tokenizer_file(model_name).exists();
        (model_exists, tokenizer_exists)
    }

    /// Check if model files exist for a specific backend.
    ///
    /// CPU includes fallback to legacy layout.
    pub fn check_model_files_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> (bool, bool) {
        let model_exists = Self::resolve_model_file_for_backend(model_name, backend)
            .as_ref()
            .is_some_and(|path| path.exists());
        let tokenizer_exists = Self::tokenizer_file(model_name).exists();
        (model_exists, tokenizer_exists)
    }

    /// Validate model directory structure
    pub fn validate_model(model_name: &str) -> Result<(), String> {
        Self::validate_model_for_backend(model_name, ModelArtifactBackend::Cpu).map(|_| ())
    }

    /// Validate model directory structure for a specific backend and return resolved model path.
    pub fn validate_model_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> Result<PathBuf, String> {
        let (model_exists, tokenizer_exists) =
            Self::check_model_files_for_backend(model_name, backend);
        let resolved_model_path = Self::resolve_model_file_for_backend(model_name, backend)
            .unwrap_or_else(|| Self::backend_model_file(model_name, backend));

        if !model_exists {
            return Err(format!(
                "Model file for backend '{}' not found: {}",
                backend.as_dir(),
                resolved_model_path.display()
            ));
        }

        if !tokenizer_exists {
            return Err(format!(
                "Tokenizer file not found: {}",
                Self::tokenizer_file(model_name).display()
            ));
        }

        Ok(resolved_model_path)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelArtifactBackend, ModelLoader};
    use std::path::PathBuf;

    #[test]
    fn default_models_dir_is_absolute_and_points_to_models_folder() {
        let path = ModelLoader::default_models_dir();
        assert!(path.is_absolute());
        assert!(path.ends_with("models"));
    }

    #[test]
    fn resolve_models_dir_uses_non_empty_override() {
        let override_path = PathBuf::from("C:/custom/models");
        let resolved = ModelLoader::resolve_models_dir(Some(override_path.clone()));
        assert_eq!(resolved, override_path);
    }

    #[test]
    fn resolve_models_dir_falls_back_for_empty_override() {
        let resolved = ModelLoader::resolve_models_dir(Some(PathBuf::new()));
        assert_eq!(resolved, ModelLoader::default_models_dir());
    }

    #[test]
    fn backend_dir_names_are_stable() {
        assert_eq!(ModelArtifactBackend::Cpu.as_dir(), "cpu");
        assert_eq!(ModelArtifactBackend::DirectML.as_dir(), "dml");
    }
}
