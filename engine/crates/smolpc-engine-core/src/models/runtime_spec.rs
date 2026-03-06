/// Runtime inference contract for a specific model architecture/export.
///
/// This keeps architecture constants and tensor naming in one place so the
/// inference pipeline can validate model compatibility explicitly.

#[derive(Debug, Clone, Copy)]
pub struct ModelArchitecture {
    pub num_layers: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackendTarget {
    Cpu,
    DirectML,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvInputSchema {
    /// CPU path: dynamic cache length and explicit attention mask.
    AttentionMask { attention_mask: &'static str },
    /// DirectML path: fixed KV cache window with explicit sequence length counters.
    SeqlensK {
        seqlens_k: &'static str,
        total_sequence_length: &'static str,
        max_sequence_length: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ModelIoSpec {
    pub input_ids: &'static str,
    pub position_ids: Option<&'static str>,
    pub logits: &'static str,
    pub kv_input_schema: KvInputSchema,
    pub past_key_template: &'static str,
    pub past_value_template: &'static str,
    pub present_key_template: &'static str,
    pub present_value_template: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelRuntimeSpec {
    pub model_id: &'static str,
    pub backend_target: RuntimeBackendTarget,
    pub architecture: ModelArchitecture,
    pub io: ModelIoSpec,
    pub stop_token_ids: &'static [u32],
}

impl ModelRuntimeSpec {
    fn format_layer_name(template: &str, layer: usize) -> String {
        template.replace("{layer}", &layer.to_string())
    }

    pub fn past_key_name(&self, layer: usize) -> String {
        Self::format_layer_name(self.io.past_key_template, layer)
    }

    pub fn past_value_name(&self, layer: usize) -> String {
        Self::format_layer_name(self.io.past_value_template, layer)
    }

    pub fn present_key_name(&self, layer: usize) -> String {
        Self::format_layer_name(self.io.present_key_template, layer)
    }

    pub fn present_value_name(&self, layer: usize) -> String {
        Self::format_layer_name(self.io.present_value_template, layer)
    }

    pub fn past_key_names(&self) -> Vec<String> {
        (0..self.architecture.num_layers)
            .map(|layer| self.past_key_name(layer))
            .collect()
    }

    pub fn past_value_names(&self) -> Vec<String> {
        (0..self.architecture.num_layers)
            .map(|layer| self.past_value_name(layer))
            .collect()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.model_id.trim().is_empty() {
            return Err("Runtime spec must define a non-empty model_id".to_string());
        }
        if self.architecture.num_layers == 0 {
            return Err("Runtime spec must define at least one layer".to_string());
        }
        if self.architecture.num_kv_heads == 0 {
            return Err("Runtime spec must define at least one KV head".to_string());
        }
        if self.architecture.head_dim == 0 {
            return Err("Runtime spec must define a positive head dimension".to_string());
        }
        if self.stop_token_ids.is_empty() {
            return Err("Runtime spec must define at least one stop token".to_string());
        }
        if self.io.input_ids.trim().is_empty() {
            return Err("Runtime spec must define a non-empty input_ids tensor name".to_string());
        }
        if let Some(position_ids) = self.io.position_ids {
            if position_ids.trim().is_empty() {
                return Err(
                    "Runtime spec must define a non-empty position_ids tensor name".to_string(),
                );
            }
        }
        if self.io.logits.trim().is_empty() {
            return Err("Runtime spec must define a non-empty logits tensor name".to_string());
        }
        match self.io.kv_input_schema {
            KvInputSchema::AttentionMask { attention_mask } => {
                if attention_mask.trim().is_empty() {
                    return Err(
                        "Runtime spec must define a non-empty attention_mask tensor name"
                            .to_string(),
                    );
                }
            }
            KvInputSchema::SeqlensK {
                seqlens_k,
                total_sequence_length,
                max_sequence_length,
            } => {
                if seqlens_k.trim().is_empty() {
                    return Err(
                        "Runtime spec must define a non-empty seqlens_k tensor name".to_string()
                    );
                }
                if total_sequence_length.trim().is_empty() {
                    return Err(
                        "Runtime spec must define a non-empty total_sequence_length tensor name"
                            .to_string(),
                    );
                }
                if max_sequence_length == 0 {
                    return Err(
                        "Runtime spec for SeqlensK schema must define a positive max_sequence_length"
                            .to_string(),
                    );
                }
            }
        }

        for (label, template) in [
            ("past_key_template", self.io.past_key_template),
            ("past_value_template", self.io.past_value_template),
            ("present_key_template", self.io.present_key_template),
            ("present_value_template", self.io.present_value_template),
        ] {
            if !template.contains("{layer}") {
                return Err(format!(
                    "Runtime spec field '{label}' must contain '{{layer}}' placeholder"
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spec() -> ModelRuntimeSpec {
        ModelRuntimeSpec {
            model_id: "test-model",
            backend_target: RuntimeBackendTarget::Cpu,
            architecture: ModelArchitecture {
                num_layers: 2,
                num_kv_heads: 2,
                head_dim: 128,
            },
            io: ModelIoSpec {
                input_ids: "input_ids",
                position_ids: None,
                logits: "logits",
                kv_input_schema: KvInputSchema::AttentionMask {
                    attention_mask: "attention_mask",
                },
                past_key_template: "past_key_values.{layer}.key",
                past_value_template: "past_key_values.{layer}.value",
                present_key_template: "present.{layer}.key",
                present_value_template: "present.{layer}.value",
            },
            stop_token_ids: &[42],
        }
    }

    #[test]
    fn test_format_layer_names() {
        let spec = make_spec();
        assert_eq!(spec.past_key_name(1), "past_key_values.1.key");
        assert_eq!(spec.present_value_name(0), "present.0.value");
    }

    #[test]
    fn test_validate_rejects_missing_placeholder() {
        let mut spec = make_spec();
        spec.io.past_key_template = "past_key_values.key";
        let err = spec.validate().expect_err("spec should fail validation");
        assert!(err.contains("placeholder"));
    }

    #[test]
    fn test_validate_rejects_empty_model_id() {
        let mut spec = make_spec();
        spec.model_id = "";
        let err = spec
            .validate()
            .expect_err("empty model_id should fail validation");
        assert!(err.contains("model_id"));
    }

    #[test]
    fn test_validate_rejects_empty_logits_name() {
        let mut spec = make_spec();
        spec.io.logits = "  ";
        let err = spec
            .validate()
            .expect_err("empty logits tensor name should fail validation");
        assert!(err.contains("logits"));
    }

    #[test]
    fn test_validate_rejects_zero_dml_max_sequence_length() {
        let mut spec = make_spec();
        spec.backend_target = RuntimeBackendTarget::DirectML;
        spec.io.kv_input_schema = KvInputSchema::SeqlensK {
            seqlens_k: "seqlens_k",
            total_sequence_length: "total_sequence_length",
            max_sequence_length: 0,
        };
        let err = spec
            .validate()
            .expect_err("invalid dml max sequence length");
        assert!(err.contains("max_sequence_length"));
    }
}
