#[cfg(target_os = "windows")]
use super::genai::GenAiDirectMlGenerator;
#[cfg(target_os = "windows")]
use super::genai::OpenVinoGenAiGenerator;
use super::generator::Generator;
use super::types::{GenerationConfig, GenerationMetrics, InferenceChatMessage};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Runtime adapter abstraction for inference backends.
///
/// CPU inference currently uses the ORT generator path. DirectML uses
/// ONNX Runtime GenAI via native C FFI to align with exported DML artifacts.
pub enum InferenceRuntimeAdapter {
    Ort {
        generator: Generator,
    },
    #[cfg(target_os = "windows")]
    GenAiDirectMl {
        generator: GenAiDirectMlGenerator,
    },
    #[cfg(target_os = "windows")]
    OpenVinoGenAiNpu {
        generator: OpenVinoGenAiGenerator,
    },
}

impl InferenceRuntimeAdapter {
    pub fn ort(generator: Generator) -> Self {
        Self::Ort { generator }
    }

    #[cfg(target_os = "windows")]
    pub fn genai_directml(generator: GenAiDirectMlGenerator) -> Self {
        Self::GenAiDirectMl { generator }
    }

    #[cfg(target_os = "windows")]
    pub fn openvino_genai_npu(generator: OpenVinoGenAiGenerator) -> Self {
        Self::OpenVinoGenAiNpu { generator }
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
            #[cfg(target_os = "windows")]
            Self::OpenVinoGenAiNpu { generator } => {
                generator
                    .generate_stream(prompt, config, cancelled, on_token)
                    .await
            }
        }
    }

    pub async fn generate_stream_messages<F>(
        &self,
        messages: &[InferenceChatMessage],
        config: Option<GenerationConfig>,
        cancelled: Arc<AtomicBool>,
        on_token: F,
    ) -> Result<GenerationMetrics, String>
    where
        F: FnMut(String),
    {
        match self {
            #[cfg(target_os = "windows")]
            Self::OpenVinoGenAiNpu { generator } => {
                generator
                    .generate_stream_messages(messages, config, cancelled, on_token)
                    .await
            }
            _ => Err(
                "Structured chat messages are only supported by the OpenVINO GenAI runtime lane"
                    .to_string(),
            ),
        }
    }
}
