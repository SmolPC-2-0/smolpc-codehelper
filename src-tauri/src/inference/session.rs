/// ONNX Runtime session wrapper
///
/// Wraps `ort::Session` to provide a cleaner API for model loading and inference.
/// Handles execution provider configuration and session options.

use super::types::ModelInfo;
use super::InferenceBackend;
#[cfg(target_os = "windows")]
use ort::ep;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use std::path::Path;

/// Wrapper around ONNX Runtime session
pub struct InferenceSession {
    pub session: Session,
    model_name: String,
}

impl InferenceSession {
    /// Load an ONNX model from file path
    ///
    /// # Arguments
    /// * `model_path` - Path to .onnx model file
    ///
    /// # Phase 0 Configuration
    /// - Execution Provider: CPU only
    /// - Optimization Level: Level3 (maximum)
    /// - Intra-op threads: 4 (CPU parallelism)
    ///
    /// # Future Phases
    /// Phase 2: Add GPU EP selection (CUDA, DirectML)
    /// Phase 3: Add Intel OpenVINO EP (Core Ultra NPU)
    /// Phase 4: Add Qualcomm QNN EP (Snapdragon X Elite NPU)
    pub fn new<P: AsRef<Path>>(model_path: P) -> Result<Self, String> {
        Self::new_with_backend(model_path, InferenceBackend::Cpu)
    }

    /// Load an ONNX model with an explicit backend selection.
    pub fn new_with_backend<P: AsRef<Path>>(
        model_path: P,
        backend: InferenceBackend,
    ) -> Result<Self, String> {
        let model_path = model_path.as_ref();

        log::info!(
            "Loading ONNX model: {} (backend: {})",
            model_path.display(),
            backend.as_str()
        );

        let mut builder = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {e}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("Failed to set optimization level: {e}"))?;

        builder = match backend {
            InferenceBackend::Cpu => builder
                .with_intra_threads(4)
                .map_err(|e| format!("Failed to set CPU thread count: {e}"))?,
            InferenceBackend::DirectML => {
                #[cfg(target_os = "windows")]
                {
                    builder
                        .with_execution_providers([ep::DirectML::default().build().error_on_failure()])
                        .map_err(|e| format!("Failed to register DirectML EP: {e}"))?
                        .with_parallel_execution(false)
                        .map_err(|e| format!("Failed to set DirectML execution mode: {e}"))?
                        .with_memory_pattern(false)
                        .map_err(|e| format!("Failed to disable memory pattern for DirectML: {e}"))?
                }
                #[cfg(not(target_os = "windows"))]
                {
                    return Err("DirectML backend is only supported on Windows".to_string());
                }
            }
        };

        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| format!("Failed to load model from file: {e}"))?;

        let model_name = model_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        log::info!(
            "Model loaded successfully: {} (backend: {})",
            model_name,
            backend.as_str()
        );

        Ok(Self {
            session,
            model_name,
        })
    }

    /// Get model metadata (input/output names)
    pub fn info(&self) -> ModelInfo {
        ModelInfo {
            name: self.model_name.clone(),
            inputs: self
                .session
                .inputs()
                .iter()
                .map(|input| input.name().to_string())
                .collect(),
            outputs: self
                .session
                .outputs()
                .iter()
                .map(|output| output.name().to_string())
                .collect(),
        }
    }

    /// Get model name
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.model_name
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::init_onnx_runtime;

    #[test]
    #[ignore] // Requires model file - run manually
    fn test_load_model() {
        // This test requires a model file at the specified path
        // Run with: cargo test test_load_model -- --ignored --nocapture

        // Initialize ONNX Runtime with correct DLL path first
        init_onnx_runtime(None).expect("Failed to initialize ONNX Runtime");

        let model_path = "models/qwen2.5-coder-1.5b/model.onnx";

        match InferenceSession::new(model_path) {
            Ok(session) => {
                let info = session.info();
                println!("Model loaded: {:?}", info);

                // Verify expected inputs/outputs exist
                assert!(info.inputs.iter().any(|i| i.contains("input_ids")));
                assert!(info.outputs.iter().any(|o| o.contains("logits")));
            }
            Err(e) => {
                eprintln!("Failed to load model: {}", e);
                eprintln!("Make sure model exists at: {}", model_path);
            }
        }
    }
}
