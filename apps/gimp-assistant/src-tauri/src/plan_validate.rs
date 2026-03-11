use serde::{Deserialize, Serialize};

use crate::plan_schema::{ActionStep, Op, Target};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedStep {
    pub op: Op,
    pub target: Target,
    pub stop_on_error: bool,
    pub params: ValidatedParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidatedParams {
    DrawLine { x1: i32, y1: i32, x2: i32, y2: i32 },
    CropSquare {},
    ResizeWidth { width: i32 },
    BrightnessContrast { brightness: f64, contrast: f64 },
    Blur { radius: f64 },
    Undo { steps: i32 },
    Redo { steps: i32 },
}

fn clamp_i32(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo { lo } else if x > hi { hi } else { x }
}

pub fn validate_step(step: &ActionStep) -> Result<ValidatedStep, String> {
    let validated = match step.op {
        Op::DrawLine => {
            #[derive(Deserialize)]
            struct P { x1: i32, y1: i32, x2: i32, y2: i32 }
            let p: P = serde_json::from_value(step.params.clone())
                .map_err(|e| format!("draw_line params invalid: {e}"))?;
            ValidatedParams::DrawLine { x1: p.x1, y1: p.y1, x2: p.x2, y2: p.y2 }
        }

        Op::CropSquare => {
            // No params required; allow empty object or null
            ValidatedParams::CropSquare {}
        }

        Op::ResizeWidth => {
            #[derive(Deserialize)]
            struct P { width: i32 }
            let p: P = serde_json::from_value(step.params.clone())
                .map_err(|e| format!("resize_width params invalid: {e}"))?;
            let width = clamp_i32(p.width, 16, 8192);
            ValidatedParams::ResizeWidth { width }
        }

        Op::BrightnessContrast => {
            #[derive(Deserialize)]
            struct P { brightness: f64, contrast: f64 }
            let p: P = serde_json::from_value(step.params.clone())
                .map_err(|e| format!("brightness_contrast params invalid: {e}"))?;
            let brightness = clamp_f64(p.brightness, -100.0, 100.0);
            let contrast = clamp_f64(p.contrast, -100.0, 100.0);
            ValidatedParams::BrightnessContrast { brightness, contrast }
        }

        Op::Blur => {
            #[derive(Deserialize)]
            struct P { radius: f64 }
            let p: P = serde_json::from_value(step.params.clone())
                .map_err(|e| format!("blur params invalid: {e}"))?;
            let radius = clamp_f64(p.radius, 0.0, 200.0);
            ValidatedParams::Blur { radius }
        }

        Op::Undo => {
            #[derive(Deserialize)]
            struct P { steps: Option<i32> }
            let p: P = serde_json::from_value(step.params.clone())
                .unwrap_or(P { steps: Some(1) });
            let steps = clamp_i32(p.steps.unwrap_or(1), 1, 50);
            ValidatedParams::Undo { steps }
        }

        Op::Redo => {
            #[derive(Deserialize)]
            struct P { steps: Option<i32> }
            let p: P = serde_json::from_value(step.params.clone())
                .unwrap_or(P { steps: Some(1) });
            let steps = clamp_i32(p.steps.unwrap_or(1), 1, 50);
            ValidatedParams::Redo { steps }
        }
    };

    Ok(ValidatedStep {
        op: step.op.clone(),
        target: step.target.clone(),
        stop_on_error: step.stop_on_error,
        params: validated,
    })
}
