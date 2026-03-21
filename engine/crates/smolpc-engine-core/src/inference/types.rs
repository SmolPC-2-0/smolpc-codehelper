/// Shared types for inference engine
use serde::{Deserialize, Serialize};

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

    /// Whether output was truncated by a safety limit
    #[serde(default)]
    pub truncated: bool,

    /// Reason for truncation, if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_reason: Option<String>,
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

    /// Repetition penalty (1.0 = disabled, >1.0 = penalize repeats)
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f32,

    /// Number of recent tokens to consider for repetition penalty (0 = all generated tokens)
    #[serde(default = "default_repetition_penalty_last_n")]
    pub repetition_penalty_last_n: usize,
}

/// Structured chat message for backend-native template/rendering paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InferenceChatMessage {
    /// Chat role expected by backend chat templates ("system", "user", "assistant").
    pub role: String,

    /// Plain-text message content.
    pub content: String,
}

fn default_repetition_penalty() -> f32 {
    1.1
}

fn default_repetition_penalty_last_n() -> usize {
    64
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_length: 2048,
            temperature: 1.0,
            top_k: None,
            top_p: None,
            repetition_penalty: default_repetition_penalty(),
            repetition_penalty_last_n: default_repetition_penalty_last_n(),
        }
    }
}
