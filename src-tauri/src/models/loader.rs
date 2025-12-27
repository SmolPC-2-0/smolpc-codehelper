/// Model file loading utilities
use std::path::PathBuf;

/// Model loader for finding and validating model files
pub struct ModelLoader;

impl ModelLoader {
    /// Get the models directory path
    ///
    /// # Phase 0
    /// Models are expected at: `src-tauri/models/`
    ///
    /// # Phase 5
    /// Will use app data directory for user downloads
    pub fn models_dir() -> PathBuf {
        PathBuf::from("models")
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
