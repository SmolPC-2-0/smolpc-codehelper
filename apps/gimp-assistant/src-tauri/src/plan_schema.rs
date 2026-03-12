use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A safe, structured plan produced by the LLM (planner only).
/// Execution code will validate + route each step to deterministic macros.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    /// Human-readable summary (for UI/debug). Not trusted for execution.
    pub summary: Option<String>,

    /// Ordered steps to execute.
    pub steps: Vec<ActionStep>,
}

/// One atomic action that maps to exactly one macro/tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub op: Op,

    /// Parameters for the operation (typed per op, validated later).
    pub params: Value,

    /// Target context (kept simple for Phase 2.1).
    #[serde(default)]
    pub target: Target,

    /// Optional: if true, executor should call undo on failure and stop.
    #[serde(default)]
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    ActiveLayer,
}

impl Default for Target {
    fn default() -> Self {
        Target::ActiveLayer
    }
}

/// Allowed operations (the LLM may only choose from this list).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    DrawLine,
    CropSquare,
    ResizeWidth,
    BrightnessContrast,
    Blur,
    Undo,
    Redo,
}
