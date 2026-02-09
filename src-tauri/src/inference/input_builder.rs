//! Pre-allocated input builder for efficient ONNX session input creation.
//!
//! This module reduces hot-path overhead by:
//! 1. Pre-computing all input names and their model-order indices at initialization
//! 2. Storing inputs in pre-allocated slots keyed by index (no per-step key cloning)
//! 3. Reusing an ordered value vector for `session.run(&[...])`
//!
//! # Performance Impact
//! - Before: per-step key cloning + HashMap ownership handoff
//! - After: index-based writes + reusable ordered input buffer
//!
//! # Usage
//! ```ignore
//! let mut builder = InputBuilder::new();
//!
//! // For each decode step:
//! builder.clear();
//! builder.set_input_ids(input_ids_value);
//! builder.set_attention_mask(attention_mask_value);
//! for layer in 0..NUM_LAYERS {
//!     builder.set_past_key(layer, key_value)?;
//!     builder.set_past_value(layer, value_value)?;
//! }
//! let inputs = builder.ordered_inputs()?;
//! session.run(inputs)?;
//! ```

use super::kv_cache::NUM_LAYERS;
use ort::session::SessionInputValue;
use std::collections::{HashMap, HashSet};

/// Pre-allocated input builder for ONNX inference
///
/// Holds pre-computed key/index metadata and reusable input buffers to avoid
/// per-token key handling overhead during the decode loop.
pub struct InputBuilder {
    // Input names in model-declared order
    model_input_names: Vec<String>,

    // Pre-computed indices into model input order
    input_ids_index: usize,
    attention_mask_index: usize,
    past_key_indices: Vec<usize>,
    past_value_indices: Vec<usize>,

    // Input storage by model input order
    input_slots: Vec<Option<SessionInputValue<'static>>>,

    // Reusable ordered values for session.run(&[...])
    ordered_inputs: Vec<SessionInputValue<'static>>,
}

impl InputBuilder {
    /// Create a new InputBuilder with pre-allocated key strings
    ///
    /// # Memory Layout
    /// Pre-computes all 58 input key names:
    /// - "input_ids"
    /// - "attention_mask"
    /// - "past_key_values.0.key" through "past_key_values.27.key"
    /// - "past_key_values.0.value" through "past_key_values.27.value"
    pub fn new() -> Self {
        let past_key_names: Vec<String> = (0..NUM_LAYERS)
            .map(|i| format!("past_key_values.{}.key", i))
            .collect();
        let past_value_names: Vec<String> = (0..NUM_LAYERS)
            .map(|i| format!("past_key_values.{}.value", i))
            .collect();

        Self::with_names(
            "input_ids",
            "attention_mask",
            past_key_names,
            past_value_names,
        )
        .expect("InputBuilder::new should always construct valid default names")
    }

    /// Create an InputBuilder with explicit input/cache tensor names.
    pub fn with_names(
        input_ids_key: impl Into<String>,
        attention_mask_key: impl Into<String>,
        past_key_names: Vec<String>,
        past_value_names: Vec<String>,
    ) -> Result<Self, String> {
        let input_ids_key = input_ids_key.into();
        let attention_mask_key = attention_mask_key.into();
        let mut model_input_names = Vec::with_capacity(2 + past_key_names.len() * 2);
        model_input_names.push(input_ids_key.clone());
        model_input_names.push(attention_mask_key.clone());
        model_input_names.extend(past_key_names.iter().cloned());
        model_input_names.extend(past_value_names.iter().cloned());

        Self::with_names_and_input_order(
            input_ids_key,
            attention_mask_key,
            past_key_names,
            past_value_names,
            model_input_names,
        )
    }

    /// Create an InputBuilder with explicit input/cache tensor names and model input order.
    pub fn with_names_and_input_order(
        input_ids_key: impl Into<String>,
        attention_mask_key: impl Into<String>,
        past_key_names: Vec<String>,
        past_value_names: Vec<String>,
        model_input_names: Vec<String>,
    ) -> Result<Self, String> {
        let input_ids_key = input_ids_key.into();
        let attention_mask_key = attention_mask_key.into();

        if input_ids_key.trim().is_empty() {
            return Err("InputBuilder requires a non-empty input_ids tensor name".to_string());
        }
        if attention_mask_key.trim().is_empty() {
            return Err("InputBuilder requires a non-empty attention_mask tensor name".to_string());
        }
        if past_key_names.is_empty() || past_value_names.is_empty() {
            return Err("InputBuilder requires at least one KV cache tensor name".to_string());
        }
        if past_key_names.len() != past_value_names.len() {
            return Err(format!(
                "Mismatched KV name counts: {} key names vs {} value names",
                past_key_names.len(),
                past_value_names.len()
            ));
        }
        if model_input_names.is_empty() {
            return Err("InputBuilder requires at least one model input name".to_string());
        }

        let mut required_names = Vec::with_capacity(2 + past_key_names.len() * 2);
        required_names.push(input_ids_key.clone());
        required_names.push(attention_mask_key.clone());
        required_names.extend(past_key_names.iter().cloned());
        required_names.extend(past_value_names.iter().cloned());

        let mut seen_required = HashSet::with_capacity(required_names.len());
        for name in &required_names {
            if !seen_required.insert(name.as_str()) {
                return Err(format!("Duplicate runtime input tensor name: '{name}'"));
            }
        }

        if model_input_names.len() != required_names.len() {
            return Err(format!(
                "Model input count mismatch: runtime expects {}, model declares {}",
                required_names.len(),
                model_input_names.len()
            ));
        }

        let mut model_index_by_name = HashMap::with_capacity(model_input_names.len());
        for (index, name) in model_input_names.iter().enumerate() {
            if name.trim().is_empty() {
                return Err(format!("Model input name at index {index} is empty"));
            }
            if model_index_by_name.insert(name.clone(), index).is_some() {
                return Err(format!("Duplicate model input tensor name: '{name}'"));
            }
        }

        for name in &required_names {
            if !model_index_by_name.contains_key(name) {
                return Err(format!("Model is missing required input tensor '{name}'"));
            }
        }

        for name in &model_input_names {
            if !seen_required.contains(name.as_str()) {
                return Err(format!(
                    "Model input tensor '{name}' is not declared in runtime spec"
                ));
            }
        }

        let input_ids_index = *model_index_by_name
            .get(input_ids_key.as_str())
            .ok_or_else(|| format!("Model is missing required input tensor '{}'", input_ids_key))?;
        let attention_mask_index = *model_index_by_name
            .get(attention_mask_key.as_str())
            .ok_or_else(|| {
                format!(
                    "Model is missing required input tensor '{}'",
                    attention_mask_key
                )
            })?;

        let past_key_indices = past_key_names
            .iter()
            .map(|name| {
                model_index_by_name
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| format!("Model is missing required input tensor '{name}'"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let past_value_indices = past_value_names
            .iter()
            .map(|name| {
                model_index_by_name
                    .get(name.as_str())
                    .copied()
                    .ok_or_else(|| format!("Model is missing required input tensor '{name}'"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let capacity = model_input_names.len();

        Ok(Self {
            model_input_names,
            input_ids_index,
            attention_mask_index,
            past_key_indices,
            past_value_indices,
            input_slots: (0..capacity).map(|_| None).collect(),
            ordered_inputs: Vec::with_capacity(capacity),
        })
    }

    fn resolve_layer_index(
        indices: &[usize],
        layer: usize,
        tensor_kind: &str,
    ) -> Result<usize, String> {
        indices.get(layer).copied().ok_or_else(|| {
            format!(
                "Invalid {tensor_kind} layer index {layer}; configured layers: {}",
                indices.len()
            )
        })
    }

    /// Clear input slots for reuse.
    ///
    /// Must be called at the start of each prefill/decode step.
    #[inline]
    pub fn clear(&mut self) {
        for slot in &mut self.input_slots {
            *slot = None;
        }
        self.ordered_inputs.clear();
    }

    /// Set the input_ids tensor
    #[inline]
    pub fn set_input_ids(&mut self, value: SessionInputValue<'static>) {
        self.input_slots[self.input_ids_index] = Some(value);
    }

    /// Set the attention_mask tensor
    #[inline]
    pub fn set_attention_mask(&mut self, value: SessionInputValue<'static>) {
        self.input_slots[self.attention_mask_index] = Some(value);
    }

    /// Set the past key cache for a specific layer
    #[inline]
    pub fn set_past_key(
        &mut self,
        layer: usize,
        value: SessionInputValue<'static>,
    ) -> Result<(), String> {
        let index = Self::resolve_layer_index(&self.past_key_indices, layer, "past_key")?;
        self.input_slots[index] = Some(value);
        Ok(())
    }

    /// Set the past value cache for a specific layer
    #[inline]
    pub fn set_past_value(
        &mut self,
        layer: usize,
        value: SessionInputValue<'static>,
    ) -> Result<(), String> {
        let index = Self::resolve_layer_index(&self.past_value_indices, layer, "past_value")?;
        self.input_slots[index] = Some(value);
        Ok(())
    }

    /// Build ordered input values in model input order for `session.run(&[...])`.
    ///
    /// Returns an error when any required input has not been set for this step.
    pub fn ordered_inputs(&mut self) -> Result<&[SessionInputValue<'static>], String> {
        self.ordered_inputs.clear();

        for index in 0..self.input_slots.len() {
            let value = self.input_slots[index].take().ok_or_else(|| {
                format!(
                    "Missing value for required model input '{}'",
                    self.model_input_names[index]
                )
            })?;
            self.ordered_inputs.push(value);
        }

        Ok(self.ordered_inputs.as_slice())
    }

    /// Get a reference to the inputs (for inspection/debugging)
    #[allow(dead_code)]
    pub fn inputs(&self) -> &[Option<SessionInputValue<'static>>] {
        self.input_slots.as_slice()
    }

    /// Get the number of currently populated inputs
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.input_slots
            .iter()
            .filter(|slot| slot.is_some())
            .count()
    }

    /// Check if no inputs are set
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for InputBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_builder_creation() {
        let builder = InputBuilder::new();

        // Verify model-order names are precomputed correctly
        assert_eq!(builder.model_input_names.len(), 2 + NUM_LAYERS * 2);
        assert_eq!(builder.model_input_names[0], "input_ids");
        assert_eq!(builder.model_input_names[1], "attention_mask");
        assert_eq!(builder.model_input_names[2], "past_key_values.0.key");
        assert_eq!(
            builder.model_input_names[2 + NUM_LAYERS - 1],
            "past_key_values.27.key"
        );
        assert_eq!(
            builder.model_input_names[2 + NUM_LAYERS],
            "past_key_values.0.value"
        );
        assert_eq!(
            builder.model_input_names[2 + NUM_LAYERS * 2 - 1],
            "past_key_values.27.value"
        );
    }

    #[test]
    fn test_input_builder_clear() {
        let mut builder = InputBuilder::new();

        // Simulate adding some inputs (we can't easily create SessionInputValue in tests,
        // so we just test the clear behavior)
        builder.clear();
        assert!(builder.is_empty());
    }

    #[test]
    fn test_input_builder_capacity() {
        let builder = InputBuilder::new();

        // Verify internal buffers have correct pre-allocated capacity
        assert_eq!(builder.input_slots.len(), 2 + NUM_LAYERS * 2);
        assert!(builder.ordered_inputs.capacity() >= 2 + NUM_LAYERS * 2);
    }

    #[test]
    fn test_with_names_rejects_empty_base_names() {
        let key_names = vec!["past_key_values.0.key".to_string()];
        let value_names = vec!["past_key_values.0.value".to_string()];

        let err =
            InputBuilder::with_names("", "attention_mask", key_names.clone(), value_names.clone())
                .err()
                .expect("empty input_ids key should be rejected");
        assert!(err.contains("input_ids"));

        let err = InputBuilder::with_names("input_ids", " ", key_names, value_names)
            .err()
            .expect("empty attention_mask key should be rejected");
        assert!(err.contains("attention_mask"));
    }

    #[test]
    fn test_with_names_rejects_mismatched_kv_name_counts() {
        let err = InputBuilder::with_names(
            "input_ids",
            "attention_mask",
            vec!["past_key_values.0.key".to_string()],
            vec![
                "past_key_values.0.value".to_string(),
                "past_key_values.1.value".to_string(),
            ],
        )
        .err()
        .expect("mismatched KV counts should be rejected");

        assert!(err.contains("Mismatched KV name counts"));
    }

    #[test]
    fn test_with_names_and_input_order_accepts_reordered_model_inputs() {
        let key_names = vec!["past_key_values.0.key".to_string()];
        let value_names = vec!["past_key_values.0.value".to_string()];
        let model_input_order = vec![
            "past_key_values.0.value".to_string(),
            "input_ids".to_string(),
            "past_key_values.0.key".to_string(),
            "attention_mask".to_string(),
        ];

        let builder = InputBuilder::with_names_and_input_order(
            "input_ids",
            "attention_mask",
            key_names,
            value_names,
            model_input_order,
        )
        .expect("reordered model input names should be accepted");

        assert_eq!(builder.input_ids_index, 1);
        assert_eq!(builder.attention_mask_index, 3);
        assert_eq!(builder.past_key_indices, vec![2]);
        assert_eq!(builder.past_value_indices, vec![0]);
        assert_eq!(builder.input_slots.len(), 4);
    }

    #[test]
    fn test_with_names_and_input_order_rejects_missing_required_input() {
        let err = InputBuilder::with_names_and_input_order(
            "input_ids",
            "attention_mask",
            vec!["past_key_values.0.key".to_string()],
            vec!["past_key_values.0.value".to_string()],
            vec![
                "input_ids".to_string(),
                "attention_mask".to_string(),
                "past_key_values.0.key".to_string(),
                "unexpected_input".to_string(),
            ],
        )
        .err()
        .expect("missing required input should be rejected");

        assert!(err.contains("missing required input tensor"));
    }

    #[test]
    fn test_resolve_layer_index_rejects_out_of_bounds() {
        let err = InputBuilder::resolve_layer_index(&[0usize, 1usize], 2, "past_key")
            .err()
            .expect("out-of-bounds layer should return an error");
        assert!(err.contains("Invalid past_key layer index"));
    }
}
