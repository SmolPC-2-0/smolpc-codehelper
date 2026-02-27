use super::generator::Generator;
#[cfg(target_os = "windows")]
use super::genai::GenAiDirectMlGenerator;
use super::types::{GenerationConfig, GenerationMetrics};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Runtime adapter abstraction for inference backends.
///
/// CPU inference currently uses the ORT generator path. DirectML uses
/// ONNX Runtime GenAI via native C FFI to align with exported DML artifacts.
pub enum InferenceRuntimeAdapter {
    Ort { generator: Generator },
    #[cfg(target_os = "windows")]
    GenAiDirectMl { generator: GenAiDirectMlGenerator },
}

impl InferenceRuntimeAdapter {
    pub fn ort(generator: Generator) -> Self {
        Self::Ort { generator }
    }

    #[cfg(target_os = "windows")]
    pub fn genai_directml(generator: GenAiDirectMlGenerator) -> Self {
        Self::GenAiDirectMl { generator }
    }

    pub async fn generate_stream<F>(
        &self,
        prompt: &str,
        config: Option<GenerationConfig>,
        cancelled: Arc<AtomicBool>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        match self {
            Self::Ort { generator } => {
                generator
                    .generate_stream(prompt, config, cancelled, on_token)
                    .await
            }
            #[cfg(target_os = "windows")]
            Self::GenAiDirectMl { generator } => {
                generator
                    .generate_stream(prompt, config, cancelled, on_token)
                    .await
            }
        }
    }
}
