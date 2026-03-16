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
                id: "qwen2.5-coder-1.5b".to_string(),
                name: "Qwen2.5-Coder 1.5B".to_string(),
                size: "1.5B".to_string(),
                disk_size_gb: 0.9,
                min_ram_gb: 8.0,
                directory: "qwen2.5-coder-1.5b".to_string(),
                description:
                    "Budget coding model for low-memory systems with shared OpenVINO and DirectML artifacts"
                        .to_string(),
            },
            ModelDefinition {
                id: "phi-4-mini-instruct".to_string(),
                name: "Phi-4 Mini Instruct".to_string(),
                size: "3.8B".to_string(),
                disk_size_gb: 2.4,
                min_ram_gb: 16.0,
                directory: "phi-4-mini-instruct".to_string(),
                description:
                    "Higher-capability tier with official OpenVINO and ONNX Runtime GenAI artifacts"
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
    fn available_models_prioritize_qwen_then_phi() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|model| model.id)
            .collect();

        assert_eq!(ids, vec!["qwen2.5-coder-1.5b", "phi-4-mini-instruct"]);
    }

    #[test]
    fn available_models_include_supported_ids() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|model| model.id)
            .collect();

        assert!(ids.contains(&"qwen2.5-coder-1.5b".to_string()));
        assert!(ids.contains(&"phi-4-mini-instruct".to_string()));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn get_model_returns_supported_models_only() {
        assert!(ModelRegistry::get_model("qwen2.5-coder-1.5b").is_some());
        assert!(ModelRegistry::get_model("phi-4-mini-instruct").is_some());
        assert!(ModelRegistry::get_model("qwen3-4b-instruct-2507").is_none());
    }
}
