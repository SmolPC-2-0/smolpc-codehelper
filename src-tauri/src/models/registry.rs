use super::runtime_spec::{ModelArchitecture, ModelIoSpec, ModelRuntimeSpec};
/// Model registry for available models
///
/// Defines which models are supported and their metadata.
/// Phase 0: Hard-coded list
/// Phase 5: Dynamic registry with download support
use serde::{Deserialize, Serialize};

pub const PRIMARY_MODEL_ID: &str = "qwen2.5-coder-1.5b";

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
        vec![ModelDefinition {
            id: PRIMARY_MODEL_ID.to_string(),
            name: "Qwen2.5-Coder 1.5B".to_string(),
            size: "1.5B".to_string(),
            disk_size_gb: 0.9,
            min_ram_gb: 2.0,
            directory: PRIMARY_MODEL_ID.to_string(),
            description: "Lightweight coding model for basic tasks and low-RAM devices".to_string(),
        }]
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

    /// Runtime inference spec for a model.
    ///
    /// Only models with implemented runtime specs are considered supported for ONNX inference.
    pub fn runtime_spec(model_id: &str) -> Option<ModelRuntimeSpec> {
        match model_id {
            PRIMARY_MODEL_ID => Some(ModelRuntimeSpec {
                model_id: PRIMARY_MODEL_ID,
                architecture: ModelArchitecture {
                    num_layers: 28,
                    num_kv_heads: 2,
                    head_dim: 128,
                },
                io: ModelIoSpec {
                    input_ids: "input_ids",
                    attention_mask: "attention_mask",
                    logits: "logits",
                    past_key_template: "past_key_values.{layer}.key",
                    past_value_template: "past_key_values.{layer}.value",
                    present_key_template: "present.{layer}.key",
                    present_value_template: "present.{layer}.value",
                },
                stop_token_ids: &[151643, 151645],
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelRegistry, PRIMARY_MODEL_ID};

    #[test]
    fn available_models_excludes_unsupported_7b() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|m| m.id)
            .collect();

        assert!(ids.contains(&PRIMARY_MODEL_ID.to_string()));
        assert!(!ids.contains(&"qwen2.5-coder-7b".to_string()));
    }

    #[test]
    fn runtime_spec_only_defined_for_1_5b() {
        assert!(ModelRegistry::runtime_spec(PRIMARY_MODEL_ID).is_some());
        assert!(ModelRegistry::runtime_spec("qwen2.5-coder-7b").is_none());
        assert!(ModelRegistry::runtime_spec("unknown").is_none());
    }
}
