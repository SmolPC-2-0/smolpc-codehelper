use serde_json::Value;

use crate::plan_schema::ActionPlan;
use crate::plan_validate::{validate_step, ValidatedParams};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub index: usize,
    pub ok: bool,
    pub message: String,
    pub raw: Option<Value>,
}



/// Execute a plan sequentially using deterministic macros.
/// Returns per-step results for UI/debug.
pub fn execute_plan(plan: ActionPlan) -> Result<Vec<StepResult>, String> {
    let mut results: Vec<StepResult> = Vec::new();

    for (i, step) in plan.steps.iter().enumerate() {
        // Validate & clamp params
        let v = validate_step(step)?;

        // Route to existing macro commands / MCP calls
        let exec_result: Result<Value, String> = match v.params {
            ValidatedParams::DrawLine { x1, y1, x2, y2 } => {
                // Directly call your existing command logic via macros + MCP
                let payload = crate::macros::draw_line(x1, y1, x2, y2);
                run_payload(payload)
            }

            ValidatedParams::CropSquare {} => {
                let payload = crate::macros::crop_to_square();
                run_payload(payload)
            }

            ValidatedParams::ResizeWidth { width } => {
                let payload = crate::macros::resize_width(width);
                run_payload(payload)
            }

            ValidatedParams::BrightnessContrast { brightness, contrast } => {
                let payload = crate::macros::brightness_contrast(brightness, contrast);
                run_payload(payload)
            }

            ValidatedParams::Blur { radius } => {
                let payload = crate::macros::blur(radius);
                run_payload(payload)
            }

            ValidatedParams::Undo { steps } => {
                let mut result: Result<Value, String> = Ok(Value::Null);
                for _ in 0..steps {
                    let payload = crate::macros::undo();
                    result = run_payload(payload);
                    if result.is_err() { break; }
                }
                result
            }

            ValidatedParams::Redo { steps } => {
                Err(format!("Redo not implemented yet (requested steps={steps})"))
            }
        };

        match exec_result {
            Ok(raw) => results.push(StepResult {
                index: i,
                ok: true,
                message: "ok".to_string(),
                raw: Some(raw),
            }),
            Err(e) => {
                results.push(StepResult {
                    index: i,
                    ok: false,
                    message: e.clone(),
                    raw: None,
                });
                if v.stop_on_error {
                    break;
                }
            }
        }
    }

    Ok(results)
}

/// Helper: execute a macro payload shaped like:
/// { "name": "call_api", "arguments": {...} }
fn run_payload(payload: Value) -> Result<Value, String> {
    let tool_name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Macro payload missing 'name'".to_string())?;

    let arguments = payload
        .get("arguments")
        .cloned()
        .ok_or_else(|| "Macro payload missing 'arguments'".to_string())?;

    crate::mcp::call_tool(tool_name, arguments)
}
