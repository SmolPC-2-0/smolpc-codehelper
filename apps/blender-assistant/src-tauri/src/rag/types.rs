use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagChunk {
    pub text: String,
    pub signature: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagContext {
    pub text: String,
    pub signature: String,
    pub url: String,
    pub similarity: f32,
}
