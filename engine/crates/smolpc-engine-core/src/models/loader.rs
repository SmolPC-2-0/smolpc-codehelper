/// Model file loading utilities.
use std::path::PathBuf;

const MODELS_DIR_OVERRIDE_ENV: &str = "SMOLPC_MODELS_DIR";
const SHARED_MODELS_VENDOR_DIR: &str = "SmolPC";
const SHARED_MODELS_DIR: &str = "models";
const LEGACY_MODEL_FILENAME: &str = "model.onnx";
const TOKENIZER_FILENAME: &str = "tokenizer.json";
const GENAI_CONFIG_FILENAME: &str = "genai_config.json";
const OPENVINO_MANIFEST_FILENAME: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelArtifactBackend {
    DirectML,
}

impl ModelArtifactBackend {
    pub fn as_dir(self) -> &'static str {
        match self {
            Self::DirectML => "dml",
        }
    }
}

pub struct ModelLoader;

impl ModelLoader {
    fn shared_models_dir() -> Option<PathBuf> {
        dirs::data_local_dir()
            .map(|base| base.join(SHARED_MODELS_VENDOR_DIR).join(SHARED_MODELS_DIR))
    }

    fn default_models_dir() -> PathBuf {
        if let Some(shared) = Self::shared_models_dir() {
            if shared.exists() {
                return shared;
            }
        }

        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")
    }

    fn resolve_models_dir(override_dir: Option<PathBuf>) -> PathBuf {
        match override_dir {
            Some(path) if !path.as_os_str().is_empty() => path,
            _ => Self::default_models_dir(),
        }
    }

    pub fn models_dir() -> PathBuf {
        let override_dir = std::env::var_os(MODELS_DIR_OVERRIDE_ENV).map(PathBuf::from);
        Self::resolve_models_dir(override_dir)
    }

    pub fn model_path(model_name: &str) -> PathBuf {
        Self::models_dir().join(model_name)
    }

    pub fn backend_model_file(model_name: &str, backend: ModelArtifactBackend) -> PathBuf {
        Self::model_path(model_name)
            .join(backend.as_dir())
            .join(LEGACY_MODEL_FILENAME)
    }

    fn directml_dir(model_name: &str) -> PathBuf {
        Self::model_path(model_name).join(ModelArtifactBackend::DirectML.as_dir())
    }

    pub fn openvino_dir(model_name: &str) -> PathBuf {
        Self::model_path(model_name).join("openvino")
    }

    fn directml_genai_config_file(model_name: &str) -> PathBuf {
        Self::directml_dir(model_name).join(GENAI_CONFIG_FILENAME)
    }

    fn directml_tokenizer_file(model_name: &str) -> PathBuf {
        Self::directml_dir(model_name).join(TOKENIZER_FILENAME)
    }

    fn directml_missing_required_files(model_name: &str) -> Vec<PathBuf> {
        let required = [
            Self::backend_model_file(model_name, ModelArtifactBackend::DirectML),
            Self::directml_genai_config_file(model_name),
            Self::directml_tokenizer_file(model_name),
        ];

        required.into_iter().filter(|path| !path.exists()).collect()
    }

    pub fn openvino_manifest_file(model_name: &str) -> PathBuf {
        Self::openvino_dir(model_name).join(OPENVINO_MANIFEST_FILENAME)
    }

    pub fn resolve_model_file_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> Option<PathBuf> {
        match backend {
            ModelArtifactBackend::DirectML => {
                if Self::directml_missing_required_files(model_name).is_empty() {
                    Some(Self::backend_model_file(model_name, backend))
                } else {
                    None
                }
            }
        }
    }

    pub fn check_model_files_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> (bool, bool) {
        match backend {
            ModelArtifactBackend::DirectML => (
                Self::directml_missing_required_files(model_name).is_empty(),
                Self::directml_tokenizer_file(model_name).exists(),
            ),
        }
    }

    pub fn validate_model_for_backend(
        model_name: &str,
        backend: ModelArtifactBackend,
    ) -> Result<PathBuf, String> {
        match backend {
            ModelArtifactBackend::DirectML => {
                let missing = Self::directml_missing_required_files(model_name);
                if !missing.is_empty() {
                    let missing_paths = missing
                        .into_iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!(
                        "DirectML GenAI artifact is incomplete for model '{model_name}'. Missing: {missing_paths}"
                    ));
                }

                Ok(Self::backend_model_file(model_name, backend))
            }
        }
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
        assert_eq!(ModelArtifactBackend::DirectML.as_dir(), "dml");
    }

    #[test]
    fn openvino_manifest_file_uses_lane_directory() {
        let path = ModelLoader::openvino_manifest_file("qwen2.5-1.5b-instruct");
        assert!(path.ends_with("qwen2.5-1.5b-instruct/openvino/manifest.json"));
    }

    #[test]
    fn default_models_dir_prefers_models_suffix() {
        let path = ModelLoader::default_models_dir();
        assert!(path.ends_with("models"));
    }
}
