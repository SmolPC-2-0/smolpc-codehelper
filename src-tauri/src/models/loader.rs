/// Model file loading utilities
use std::path::PathBuf;
use std::sync::OnceLock;

const MODELS_DIR_OVERRIDE_ENV: &str = "SMOLPC_MODELS_DIR";
static RUNTIME_MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Model loader for finding and validating model files
pub struct ModelLoader;

impl ModelLoader {
    fn default_models_dir() -> PathBuf {
        // Resolve from crate root (`src-tauri`) so paths are stable regardless of process CWD.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models")
    }

    /// Configure models directory discovered at runtime (e.g., Tauri bundle resources).
    /// This can only be set once and is idempotent for the same path.
    pub fn set_runtime_models_dir(path: PathBuf) -> Result<(), String> {
        if path.as_os_str().is_empty() {
            return Err("Runtime models directory cannot be empty".to_string());
        }

        match RUNTIME_MODELS_DIR.set(path.clone()) {
            Ok(()) => Ok(()),
            Err(existing) => {
                if existing == path {
                    Ok(())
                } else {
                    Err(format!(
                        "Runtime models directory already configured: {}",
                        existing.display()
                    ))
                }
            }
        }
    }

    fn configured_runtime_models_dir() -> Option<PathBuf> {
        RUNTIME_MODELS_DIR.get().cloned()
    }

    fn discover_models_dir() -> Option<PathBuf> {
        if let Some(configured) = Self::configured_runtime_models_dir() {
            return Some(configured);
        }

        // Packaged Windows builds usually place resources under `<exe_dir>/resources`.
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let resources_models = exe_dir.join("resources").join("models");
                if resources_models.exists() {
                    return Some(resources_models);
                }

                let sibling_models = exe_dir.join("models");
                if sibling_models.exists() {
                    return Some(sibling_models);
                }
            }
        }

        let local_models = PathBuf::from("models");
        if local_models.exists() {
            return Some(local_models);
        }

        None
    }

    fn resolve_models_dir_with_discovered(
        override_dir: Option<PathBuf>,
        discovered_dir: Option<PathBuf>,
    ) -> PathBuf {
        match override_dir {
            Some(path) if !path.as_os_str().is_empty() => path,
            _ => discovered_dir.unwrap_or_else(Self::default_models_dir),
        }
    }

    fn resolve_models_dir(override_dir: Option<PathBuf>) -> PathBuf {
        Self::resolve_models_dir_with_discovered(override_dir, Self::discover_models_dir())
    }

    /// Get the models directory path
    ///
    /// Resolution order:
    /// 1. `SMOLPC_MODELS_DIR` environment variable (if set and non-empty)
    /// 2. Runtime configured directory (`set_runtime_models_dir`)
    /// 3. Packaged resource candidates (`<exe>/resources/models`, `<exe>/models`)
    /// 4. Local working directory `models/` (dev fallback)
    /// 5. Deterministic default: `<src-tauri>/models`
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
        Self::model_path(model_name).join("model.onnx")
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
        let model_exists = Self::model_file(model_name).exists();
        let tokenizer_exists = Self::tokenizer_file(model_name).exists();
        (model_exists, tokenizer_exists)
    }

    /// Validate model directory structure
    pub fn validate_model(model_name: &str) -> Result<(), String> {
        let (model_exists, tokenizer_exists) = Self::check_model_files(model_name);

        if !model_exists {
            return Err(format!(
                "Model file not found: {}",
                Self::model_file(model_name).display()
            ));
        }

        if !tokenizer_exists {
            return Err(format!(
                "Tokenizer file not found: {}",
                Self::tokenizer_file(model_name).display()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ModelLoader;
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
        let resolved = ModelLoader::resolve_models_dir_with_discovered(
            Some(override_path.clone()),
            Some(PathBuf::from("C:/detected/models")),
        );
        assert_eq!(resolved, override_path);
    }

    #[test]
    fn resolve_models_dir_uses_discovered_when_override_empty() {
        let discovered = PathBuf::from("C:/detected/models");
        let resolved = ModelLoader::resolve_models_dir_with_discovered(
            Some(PathBuf::new()),
            Some(discovered.clone()),
        );
        assert_eq!(resolved, discovered);
    }

    #[test]
    fn resolve_models_dir_falls_back_to_default_when_no_override_or_discovered() {
        let resolved = ModelLoader::resolve_models_dir_with_discovered(Some(PathBuf::new()), None);
        assert_eq!(resolved, ModelLoader::default_models_dir());
    }
}
