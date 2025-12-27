/// Model registry for available models
///
/// Defines which models are supported and their metadata.
/// Phase 0: Hard-coded list
/// Phase 5: Dynamic registry with download support

use serde::{Deserialize, Serialize};

/// Model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// Unique model ID
    pub id: String,

    /// Display name
    pub name: String,

    /// Model size (e.g., "1.5B", "7B")
    pub size: String,

    /// Disk size in GB
    pub disk_size_gb: f32,

    /// Minimum RAM required (GB)
    pub min_ram_gb: f32,

    /// Model directory name
    pub directory: String,

    /// Description
    pub description: String,
}

/// Model registry
pub struct ModelRegistry;

impl ModelRegistry {
    /// Get all available models
    ///
    /// # Phase 0
    /// Returns hard-coded list of Qwen2.5-Coder models
    ///
    /// # Phase 5
    /// Will include multiple variants per hardware type
    pub fn available_models() -> Vec<ModelDefinition> {
        vec![
            ModelDefinition {
                id: "qwen2.5-coder-1.5b".to_string(),
                name: "Qwen2.5-Coder 1.5B".to_string(),
                size: "1.5B".to_string(),
                disk_size_gb: 0.9,
                min_ram_gb: 2.0,
                directory: "qwen2.5-coder-1.5b".to_string(),
                description: "Lightweight coding model for basic tasks and low-RAM devices".to_string(),
            },
            ModelDefinition {
                id: "qwen2.5-coder-7b".to_string(),
                name: "Qwen2.5-Coder 7B".to_string(),
                size: "7B".to_string(),
                disk_size_gb: 4.5,
                min_ram_gb: 6.0,
                directory: "qwen2.5-coder-7b".to_string(),
                description: "High-capability coding model for complex tasks".to_string(),
            },
        ]
    }

    /// Get model by ID
    pub fn get_model(model_id: &str) -> Option<ModelDefinition> {
        Self::available_models()
            .into_iter()
            .find(|m| m.id == model_id)
    }

    /// Recommend model based on available RAM
    #[allow(dead_code)]
    pub fn recommend_model(available_ram_gb: f32) -> Option<ModelDefinition> {
        Self::available_models()
            .into_iter()
            .filter(|m| m.min_ram_gb <= available_ram_gb)
            .max_by(|a, b| {
                a.min_ram_gb
                    .partial_cmp(&b.min_ram_gb)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}
