/// Shared types for inference engine
use serde::{Deserialize, Serialize};

/// Result of text generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    /// Generated text
    pub text: String,

    /// Performance metrics
    pub metrics: GenerationMetrics,
}

/// Performance metrics for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationMetrics {
    /// Total tokens generated (excluding prompt)
    pub total_tokens: usize,

    /// Time to generate first token (milliseconds)
    pub time_to_first_token_ms: Option<u64>,

    /// Average tokens per second
    pub tokens_per_second: f64,

    /// Total generation time (milliseconds)
    pub total_time_ms: u64,
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model file name
    pub name: String,

    /// Model input names
    pub inputs: Vec<String>,

    /// Model output names
    pub outputs: Vec<String>,
}

/// Generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Maximum number of tokens to generate
    pub max_length: usize,

    /// Temperature for sampling (Phase 1)
    pub temperature: f32,

    /// Top-k sampling parameter (Phase 1)
    pub top_k: Option<usize>,

    /// Top-p (nucleus) sampling parameter (Phase 1)
    pub top_p: Option<f32>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_length: 2048,
            temperature: 1.0,
            top_k: None,
            top_p: None,
        }
    }
}
