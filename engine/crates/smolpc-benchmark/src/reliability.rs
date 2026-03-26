use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::output::PromptResult;

/// Outcome of a single benchmark run.
#[derive(Debug, Clone)]
pub enum RunOutcome {
    Success,
    Error { message: String },
}

/// Why a generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Model emitted EOS before reaching max_tokens
    NaturalEos,
    /// Output was truncated at max_tokens
    MaxTokens,
    /// Output was exactly max_tokens (ambiguous)
    MaxTokensExact,
    /// Generation failed with an error
    Error,
}

/// Aggregated reliability metrics for a backend × model combo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComboReliability {
    pub total_runs: usize,
    pub successful_runs: usize,
    pub failed_runs: usize,
    /// Fraction of successful runs that were truncated (0.0 - 1.0)
    pub truncation_rate: f64,
    /// Fraction of successful runs that stopped at natural EOS (0.0 - 1.0)
    pub natural_eos_rate: f64,
    /// Error messages grouped by occurrence count
    pub error_breakdown: HashMap<String, usize>,
}

/// Aggregate reliability stats from all run outcomes and prompt results.
pub fn aggregate(outcomes: &[RunOutcome], prompt_results: &[PromptResult]) -> ComboReliability {
    let total_runs = outcomes.len();
    let successful_runs = outcomes
        .iter()
        .filter(|o| matches!(o, RunOutcome::Success))
        .count();
    let failed_runs = total_runs - successful_runs;

    // Count error messages
    let mut error_breakdown: HashMap<String, usize> = HashMap::new();
    for outcome in outcomes {
        if let RunOutcome::Error { message } = outcome {
            // Truncate long error messages for grouping
            let key = if message.len() > 100 {
                format!("{}...", &message[..100])
            } else {
                message.clone()
            };
            *error_breakdown.entry(key).or_default() += 1;
        }
    }

    // Compute truncation and EOS rates from successful runs in prompt results
    let mut truncated_count: usize = 0;
    let mut natural_eos_count: usize = 0;
    let mut total_successful_in_prompts: usize = 0;

    for pr in prompt_results {
        for run in &pr.runs {
            if run.error.is_none() {
                total_successful_in_prompts += 1;
                if run.stop_reason == StopReason::MaxTokens {
                    truncated_count += 1;
                } else if run.stop_reason == StopReason::NaturalEos {
                    natural_eos_count += 1;
                }
            }
        }
    }

    let truncation_rate = if total_successful_in_prompts > 0 {
        truncated_count as f64 / total_successful_in_prompts as f64
    } else {
        0.0
    };

    let natural_eos_rate = if total_successful_in_prompts > 0 {
        natural_eos_count as f64 / total_successful_in_prompts as f64
    } else {
        0.0
    };

    ComboReliability {
        total_runs,
        successful_runs,
        failed_runs,
        truncation_rate,
        natural_eos_rate,
        error_breakdown,
    }
}
