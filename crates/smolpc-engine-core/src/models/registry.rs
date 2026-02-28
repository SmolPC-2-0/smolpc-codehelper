use super::runtime_spec::{
    KvInputSchema, ModelArchitecture, ModelIoSpec, ModelRuntimeSpec, RuntimeBackendTarget,
};
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
        vec![ModelDefinition {
            id: "qwen2.5-coder-1.5b".to_string(),
            name: "Qwen2.5-Coder 1.5B".to_string(),
            size: "1.5B".to_string(),
            disk_size_gb: 0.9,
            min_ram_gb: 2.0,
            directory: "qwen2.5-coder-1.5b".to_string(),
            description: "Lightweight coding model for basic tasks and low-RAM devices".to_string(),
        }]
    }

    /// Get model by ID
    pub fn get_model(model_id: &str) -> Option<ModelDefinition> {
        Self::available_models()
            .into_iter()
            .find(|m| m.id == model_id)
    }

    /// Runtime inference spec for a model/backend pair.
    pub fn runtime_spec_for_backend(
        model_id: &str,
        backend_target: RuntimeBackendTarget,
    ) -> Option<ModelRuntimeSpec> {
        let kv_input_schema = match backend_target {
            RuntimeBackendTarget::Cpu => KvInputSchema::AttentionMask {
                attention_mask: "attention_mask",
            },
            RuntimeBackendTarget::DirectML => KvInputSchema::SeqlensK {
                seqlens_k: "seqlens_k",
                total_sequence_length: "total_sequence_length",
                max_sequence_length: 2048,
            },
        };

        match model_id {
            "qwen2.5-coder-1.5b" => Some(ModelRuntimeSpec {
                model_id: "qwen2.5-coder-1.5b",
                backend_target,
                architecture: ModelArchitecture {
                    num_layers: 28,
                    num_kv_heads: 2,
                    head_dim: 128,
                },
                io: ModelIoSpec {
                    input_ids: "input_ids",
                    position_ids: match backend_target {
                        RuntimeBackendTarget::Cpu => None,
                        RuntimeBackendTarget::DirectML => Some("position_ids"),
                    },
                    logits: "logits",
                    kv_input_schema,
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
    use super::ModelRegistry;
    use crate::models::runtime_spec::RuntimeBackendTarget;

    #[test]
    fn available_models_excludes_unsupported_7b() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|m| m.id)
            .collect();

        assert!(ids.contains(&"qwen2.5-coder-1.5b".to_string()));
        assert!(!ids.contains(&"qwen2.5-coder-7b".to_string()));
    }

    #[test]
    fn runtime_spec_only_defined_for_1_5b() {
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen2.5-coder-1.5b",
            RuntimeBackendTarget::Cpu
        )
        .is_some());
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen2.5-coder-7b",
            RuntimeBackendTarget::Cpu
        )
        .is_none());
        assert!(
            ModelRegistry::runtime_spec_for_backend("unknown", RuntimeBackendTarget::Cpu).is_none()
        );
    }

    #[test]
    fn runtime_spec_for_backend_sets_target() {
        let cpu = ModelRegistry::runtime_spec_for_backend(
            "qwen2.5-coder-1.5b",
            RuntimeBackendTarget::Cpu,
        )
        .expect("cpu runtime spec");
        let dml = ModelRegistry::runtime_spec_for_backend(
            "qwen2.5-coder-1.5b",
            RuntimeBackendTarget::DirectML,
        )
        .expect("dml runtime spec");

        assert_eq!(cpu.backend_target, RuntimeBackendTarget::Cpu);
        assert_eq!(dml.backend_target, RuntimeBackendTarget::DirectML);
    }
}
