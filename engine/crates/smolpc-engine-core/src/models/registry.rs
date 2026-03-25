/// Model registry for available models.
use serde::{Deserialize, Serialize};

/// Model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// Unique model ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Model size (for example, "1.5B").
    pub size: String,
    /// Approximate disk size in GB.
    pub disk_size_gb: f32,
    /// Minimum RAM required in GB.
    pub min_ram_gb: f32,
    /// Approximate RAM used while loaded for inference.
    pub estimated_runtime_ram_gb: f32,
    /// Model directory name.
    pub directory: String,
    /// Description.
    pub description: String,
}

pub struct ModelRegistry;

impl ModelRegistry {
    pub fn available_models() -> Vec<ModelDefinition> {
        vec![
            ModelDefinition {
                id: "qwen2.5-1.5b-instruct".to_string(),
                name: "Qwen2.5 1.5B Instruct".to_string(),
                size: "1.5B".to_string(),
                disk_size_gb: 0.9,
                min_ram_gb: 8.0,
                estimated_runtime_ram_gb: 1.5,
                directory: "qwen2.5-1.5b-instruct".to_string(),
                description: "Default shared model with OpenVINO CPU/NPU and DirectML artifacts"
                    .to_string(),
            },
            ModelDefinition {
                id: "qwen3-4b".to_string(),
                name: "Qwen3 4B".to_string(),
                size: "4B".to_string(),
                disk_size_gb: 2.5,
                min_ram_gb: 15.0,
                estimated_runtime_ram_gb: 4.0,
                directory: "qwen3-4b".to_string(),
                description:
                    "Higher-capability shared model with OpenVINO CPU/NPU and DirectML artifacts"
                        .to_string(),
            },
        ]
    }

    pub fn get_model(model_id: &str) -> Option<ModelDefinition> {
        Self::available_models()
            .into_iter()
            .find(|model| model.id == model_id)
    }
}

#[cfg(test)]
mod tests {
    use super::ModelRegistry;

    #[test]
    fn available_models_ordering() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|model| model.id)
            .collect();

        assert_eq!(ids, vec!["qwen2.5-1.5b-instruct", "qwen3-4b"]);
    }

    #[test]
    fn available_models_include_supported_ids() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|model| model.id)
            .collect();

        assert!(ids.contains(&"qwen2.5-1.5b-instruct".to_string()));
        assert!(ids.contains(&"qwen3-4b".to_string()));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn get_model_returns_supported_models_only() {
        assert!(ModelRegistry::get_model("qwen2.5-1.5b-instruct").is_some());
        assert!(ModelRegistry::get_model("qwen3-4b").is_some());
        assert!(ModelRegistry::get_model("phi-4-mini-instruct").is_none());
    }
}
