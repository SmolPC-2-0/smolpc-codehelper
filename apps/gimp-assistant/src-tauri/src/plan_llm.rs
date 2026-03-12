use crate::plan_schema::{ActionPlan, Op, Target};
use serde_json::json;

fn planner_prompt(user_text: &str) -> String {
    // IMPORTANT: LLM must output ONLY JSON matching ActionPlan schema.
    // No Python. No tools. No prose.
    format!(
r#"
You are a planner for a GIMP assistant.

Your job: convert the user's request into an ActionPlan JSON object.

Allowed ops (must be one of these):
- draw_line
- crop_square
- resize_width
- brightness_contrast
- blur
- undo
- redo

Rules:
- Output MUST be valid JSON only.
- Output MUST start with '{{' and contain no extra text, no markdown, no backticks.
- Use this schema exactly:
{{
  "summary": "optional short summary",
  "steps": [
    {{
      "op": "blur|brightness_contrast|resize_width|crop_square|draw_line|undo|redo",
      "params": {{ ... }},
      "target": "active_layer",
      "stop_on_error": true|false
    }}
  ]
}}

Parameter requirements:
- blur params: {{ "radius": number }}
- brightness_contrast params: {{ "brightness": number, "contrast": number }}
- resize_width params: {{ "width": integer }}
- crop_square params: {{ }}
- draw_line params: {{ "x1": int, "y1": int, "x2": int, "y2": int }}
  If no coordinates are given, use defaults: {{ "x1": 0, "y1": 0, "x2": 200, "y2": 200 }}
  "add a line", "paint a line", "insert a line", "make a line", "black line" all map to draw_line.
- undo/redo params: {{ "steps": int }} (default 1 if omitted)

If the user is vague (e.g. "make it nicer"), return a plan with ZERO steps and a summary asking ONE clarification question.
If the user requests any kind of line (regardless of color or phrasing), always use draw_line — never treat a line request as vague.

User request:
{user}
"#,
        user = user_text
    )
}

pub async fn make_plan_from_text(user_text: &str) -> Result<ActionPlan, String> {
    let prompt = planner_prompt(user_text);

    // Uses your existing Ollama client
    let raw = crate::llm_client::chat(&prompt).await?;

    // Strip anything before first '{' in case model misbehaves
    let json_str = if let Some(idx) = raw.find('{') { &raw[idx..] } else { raw.as_str() };

    let plan: ActionPlan = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse ActionPlan JSON: {e}\nLLM output was:\n{raw}"))?;

    Ok(plan)
}
