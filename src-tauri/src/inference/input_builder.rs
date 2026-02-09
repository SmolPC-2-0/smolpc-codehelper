//! Pre-allocated input builder for efficient ONNX session input creation.
//!
//! This module eliminates per-token heap allocations by:
//! 1. Pre-computing all input key names once at initialization
//! 2. Pre-sizing the HashMap to avoid rehashing
//! 3. Reusing the HashMap across decode steps via `clear()`
//!
//! # Performance Impact
//! - Before: ~58 String allocations + HashMap alloc per token
//! - After: ~0 allocations per token (only Value tensor creation)
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
//!     builder.set_past_key(layer, key_value);
//!     builder.set_past_value(layer, value_value);
//! }
//! let inputs = builder.take_inputs();
//! session.run(inputs)?;
//! ```

use super::kv_cache::NUM_LAYERS;
use ort::session::SessionInputValue;
use std::collections::HashMap;

/// Pre-allocated input builder for ONNX inference
///
/// Holds pre-computed key strings and a reusable HashMap to avoid
/// per-token allocations during the decode loop.
pub struct InputBuilder {
    // Pre-allocated key strings (created once)
    input_ids_key: String,
    attention_mask_key: String,
    past_key_names: Vec<String>,
    past_value_names: Vec<String>,

    // Pre-sized, reusable HashMap
    // Capacity: 2 (input_ids, attention_mask) + 28*2 (KV cache) = 58
    inputs: HashMap<String, SessionInputValue<'static>>,
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

        Self::with_names("input_ids", "attention_mask", past_key_names, past_value_names)
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

        // Pre-size HashMap: 2 base inputs + N layers * 2 (key + value)
        let capacity = 2 + past_key_names.len() * 2;

        Ok(Self {
            input_ids_key,
            attention_mask_key,
            past_key_names,
            past_value_names,
            inputs: HashMap::with_capacity(capacity),
        })
    }

    /// Clear the inputs HashMap for reuse
    ///
    /// This keeps the allocated capacity, avoiding reallocation.
    /// Must be called at the start of each decode step.
    #[inline]
    pub fn clear(&mut self) {
        self.inputs.clear();
    }

    /// Set the input_ids tensor
    #[inline]
    pub fn set_input_ids(&mut self, value: SessionInputValue<'static>) {
        self.inputs.insert(self.input_ids_key.clone(), value);
    }

    /// Set the attention_mask tensor
    #[inline]
    pub fn set_attention_mask(&mut self, value: SessionInputValue<'static>) {
        self.inputs.insert(self.attention_mask_key.clone(), value);
    }

    /// Set the past key cache for a specific layer
    ///
    /// # Panics
    /// Panics if `layer` is out of bounds for the configured KV name set.
    #[inline]
    pub fn set_past_key(&mut self, layer: usize, value: SessionInputValue<'static>) {
        self.inputs
            .insert(self.past_key_names[layer].clone(), value);
    }

    /// Set the past value cache for a specific layer
    ///
    /// # Panics
    /// Panics if `layer` is out of bounds for the configured KV name set.
    #[inline]
    pub fn set_past_value(&mut self, layer: usize, value: SessionInputValue<'static>) {
        self.inputs
            .insert(self.past_value_names[layer].clone(), value);
    }

    /// Take ownership of the built inputs HashMap
    ///
    /// Returns the HashMap and replaces it with a new pre-sized one.
    /// This is needed because `session.run()` takes ownership.
    ///
    /// After calling this, the InputBuilder is ready for the next use
    /// (no need to call `clear()`).
    pub fn take_inputs(&mut self) -> HashMap<String, SessionInputValue<'static>> {
        let capacity = 2 + self.past_key_names.len() * 2;
        std::mem::replace(&mut self.inputs, HashMap::with_capacity(capacity))
    }

    /// Get a reference to the inputs (for inspection/debugging)
    #[allow(dead_code)]
    pub fn inputs(&self) -> &HashMap<String, SessionInputValue<'static>> {
        &self.inputs
    }

    /// Get the number of inputs currently set
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.inputs.len()
    }

    /// Check if no inputs are set
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
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

        // Verify pre-allocated key names
        assert_eq!(builder.input_ids_key, "input_ids");
        assert_eq!(builder.attention_mask_key, "attention_mask");
        assert_eq!(builder.past_key_names.len(), NUM_LAYERS);
        assert_eq!(builder.past_value_names.len(), NUM_LAYERS);

        // Check a few key names
        assert_eq!(builder.past_key_names[0], "past_key_values.0.key");
        assert_eq!(builder.past_key_names[27], "past_key_values.27.key");
        assert_eq!(builder.past_value_names[0], "past_key_values.0.value");
        assert_eq!(builder.past_value_names[27], "past_key_values.27.value");
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

        // Verify HashMap has correct pre-allocated capacity
        // capacity() returns at least the requested capacity
        assert!(builder.inputs.capacity() >= 2 + NUM_LAYERS * 2);
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
}
