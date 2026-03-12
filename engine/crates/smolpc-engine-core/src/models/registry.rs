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
    /// Returns hard-coded list of models.
    ///
    /// # Phase 5
    /// Will include multiple variants per hardware type
    pub fn available_models() -> Vec<ModelDefinition> {
        // Ordering is intentional: first entry is treated as default model preference by clients.
        vec![
            ModelDefinition {
                id: "qwen3-4b-instruct-2507".to_string(),
                name: "Qwen3 4B Instruct (2507)".to_string(),
                size: "4B".to_string(),
                disk_size_gb: 6.0,
                min_ram_gb: 8.0,
                directory: "qwen3-4b-instruct-2507".to_string(),
                description: "Higher-quality local instruct model for shared engine demos and app integration".to_string(),
            },
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
                id: "qwen3-4b-int4-ov".to_string(),
                name: "Qwen3 4B INT4 (OpenVINO)".to_string(),
                size: "4B".to_string(),
                disk_size_gb: 3.0,
                min_ram_gb: 8.0,
                directory: "qwen3-4b-int4-ov".to_string(),
                description: "Official OpenVINO INT4 Qwen3 artifact for Intel NPU bring-up and smoke testing".to_string(),
            },
            ModelDefinition {
                id: "qwen3-4b-int4-ov-npu".to_string(),
                name: "Qwen3 4B INT4 (OpenVINO NPU)".to_string(),
                size: "4B".to_string(),
                disk_size_gb: 3.0,
                min_ram_gb: 8.0,
                directory: "qwen3-4b-int4-ov-npu".to_string(),
                description: "NPU-optimized OpenVINO INT4 Qwen3 artifact (FluidInference) — requires NPU driver >= 32.0.100.4023".to_string(),
            },
        ]
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
            "qwen3-4b-instruct-2507" => Some(ModelRuntimeSpec {
                model_id: "qwen3-4b-instruct-2507",
                backend_target,
                architecture: ModelArchitecture {
                    num_layers: 36,
                    num_kv_heads: 8,
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
                // Qwen3 config defines EOS as 151645 and BOS as 151643.
                // Keep CPU stop criteria focused on EOS to avoid early termination on BOS.
                stop_token_ids: &[151645],
            }),
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
    fn available_models_prioritize_qwen3_then_qwen2_5() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|m| m.id)
            .collect();

        assert_eq!(
            ids.first().map(String::as_str),
            Some("qwen3-4b-instruct-2507")
        );
        assert_eq!(ids.get(1).map(String::as_str), Some("qwen2.5-coder-1.5b"));
        assert_eq!(ids.get(2).map(String::as_str), Some("qwen3-4b-int4-ov"));
        assert_eq!(ids.get(3).map(String::as_str), Some("qwen3-4b-int4-ov-npu"));
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn available_models_include_supported_ids() {
        let ids: Vec<String> = ModelRegistry::available_models()
            .into_iter()
            .map(|m| m.id)
            .collect();

        assert!(ids.contains(&"qwen3-4b-instruct-2507".to_string()));
        assert!(ids.contains(&"qwen2.5-coder-1.5b".to_string()));
        assert!(ids.contains(&"qwen3-4b-int4-ov".to_string()));
        assert!(ids.contains(&"qwen3-4b-int4-ov-npu".to_string()));
    }

    #[test]
    fn runtime_spec_only_defined_for_supported_models() {
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen3-4b-instruct-2507",
            RuntimeBackendTarget::Cpu
        )
        .is_some());
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen2.5-coder-1.5b",
            RuntimeBackendTarget::Cpu
        )
        .is_some());
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen3-4b-int4-ov",
            RuntimeBackendTarget::Cpu
        )
        .is_none());
        assert!(ModelRegistry::runtime_spec_for_backend(
            "qwen3-4b-int4-ov-npu",
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
            "qwen3-4b-instruct-2507",
            RuntimeBackendTarget::Cpu,
        )
        .expect("cpu runtime spec");
        let dml = ModelRegistry::runtime_spec_for_backend(
            "qwen3-4b-instruct-2507",
            RuntimeBackendTarget::DirectML,
        )
        .expect("dml runtime spec");

        assert_eq!(cpu.backend_target, RuntimeBackendTarget::Cpu);
        assert_eq!(dml.backend_target, RuntimeBackendTarget::DirectML);
    }
}
