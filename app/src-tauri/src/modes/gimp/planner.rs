use async_trait::async_trait;
use serde_json::{json, Value};
use smolpc_engine_client::{EngineChatMessage, EngineClient};
use smolpc_engine_core::GenerationConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedTool {
    GetGimpInfo,
    GetImageMetadata,
    CallApi,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSelection {
    pub tool: SelectedTool,
    pub reason: String,
}

#[async_trait]
pub trait TextGenerator: Send + Sync {
    async fn generate(&self, prompt: &str) -> Result<String, String>;
}

pub struct EngineTextGenerator {
    client: EngineClient,
}

impl EngineTextGenerator {
    pub fn new(client: EngineClient) -> Self {
        Self { client }
    }

    fn config() -> GenerationConfig {
        GenerationConfig {
            max_length: 768,
            temperature: 0.0,
            top_k: Some(40),
            top_p: Some(0.85),
            repetition_penalty: 1.05,
            repetition_penalty_last_n: 128,
        }
    }
}

#[async_trait]
impl TextGenerator for EngineTextGenerator {
    async fn generate(&self, prompt: &str) -> Result<String, String> {
        let messages = vec![EngineChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }];

        let mut collected = String::new();
        self.client
            .generate_stream_messages(&messages, Some(Self::config()), |token| {
                collected.push_str(&token);
            })
            .await
            .map_err(|error| format!("GIMP planner generation failed: {error}"))?;
        Ok(collected)
    }
}

pub async fn select_tool(
    generator: &dyn TextGenerator,
    user_text: &str,
) -> Result<ToolSelection, String> {
    let prompt = format!(
        r#"
You are a tool selector for a GIMP assistant.

Decide which single tool is best for the user's request.

Tools:
- "get_gimp_info": GIMP version, platform, system, install, environment.
- "get_image_metadata": what image is open, details of the current image, size, dimensions, file name, layers.
- "call_api": editing the image (resize, crop, rotate, flip, draw, filters, colors, etc).
- "none": no tool needed, just answer in natural language.

Return ONLY JSON in this format:
{{"tool": "get_gimp_info" | "get_image_metadata" | "call_api" | "none", "reason": "short reason"}}

The response MUST:
- Start with '{{'
- Contain only JSON
- Have no explanation, no prose, no backticks, no prefix

User request: {user_text}
"#
    );

    let raw = generator.generate(&prompt).await?;
    let value = match extract_json_object(&raw) {
        Ok(value) => value,
        Err(_) => {
            return Ok(ToolSelection {
                tool: fallback_selected_tool(user_text),
                reason: "Planner output was not valid JSON. Used deterministic fallback."
                    .to_string(),
            });
        }
    };
    let tool = match value.get("tool").and_then(Value::as_str).unwrap_or("none") {
        "get_gimp_info" => SelectedTool::GetGimpInfo,
        "get_image_metadata" => SelectedTool::GetImageMetadata,
        "call_api" => SelectedTool::CallApi,
        _ => SelectedTool::None,
    };
    let reason = value
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("No reason provided")
        .to_string();

    Ok(ToolSelection { tool, reason })
}

pub async fn answer_without_tool(
    generator: &dyn TextGenerator,
    user_text: &str,
) -> Result<String, String> {
    let prompt = format!(
        "You are a helpful assistant that knows about GIMP.\nAnswer the user's question in natural language. Do not mention tools.\n\nUser: {user_text}\nAssistant:"
    );
    generator.generate(&prompt).await
}

pub async fn plan_call_api(
    generator: &dyn TextGenerator,
    user_text: &str,
) -> Result<Value, String> {
    let prompt = format!(
        r#"
You write Python console commands to control GIMP 3 via the PyGObject console.

User request: {user_text}

Respond ONLY with valid JSON in this format:
{{
  "thought": "short explanation of what you will do",
  "explain": "2-3 sentences for a beginner describing how to do this manually in GIMP using menus and toolbar. Start with 'To do this yourself in GIMP:'. Do NOT mention Python.",
  "steps": [
    {{
      "tool": "call_api",
      "arguments": {{
        "api_path": "exec",
        "args": [
          "pyGObject-console",
          [
            "from gi.repository import Gimp, Gegl",
            "image = Gimp.get_images()[0]",
            "layer = image.flatten()",
            "w = image.get_width()",
            "h = image.get_height()",
            "drawable = layer",
            "... your commands ...",
            "Gimp.displays_flush()"
          ]
        ],
        "kwargs": {{}}
      }}
    }}
  ]
}}

Rules:
- Output JSON only.
- Do not use comments.
- Do not use multiline control flow.
- Only use approved GIMP 3 API calls.
- Keep the plan to one tightly bounded edit flow.
- Always start with the exact setup block shown above.
"#
    );

    let raw = generator.generate(&prompt).await?;
    let mut plan = match extract_json_object(&raw) {
        Ok(plan) => plan,
        Err(primary_error) => {
            let retry_prompt = format!(
                r#"{prompt}

Your previous response could not be parsed as JSON.
Return ONLY a valid JSON object in the required format, with no prose or markdown.
"#
            );
            let retry_raw = generator.generate(&retry_prompt).await?;
            extract_json_object(&retry_raw).map_err(|secondary_error| {
                format!("{primary_error}\nPlanner retry also failed: {secondary_error}")
            })?
        }
    };
    validate_call_api_plan(&plan)?;
    if let Value::Object(ref mut object) = plan {
        object.insert("planner".to_string(), json!("engine"));
    }
    Ok(plan)
}

fn extract_json_object(raw: &str) -> Result<Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Planner output was empty".to_string());
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(object) = value_into_object(value) {
            return Ok(object);
        }
    }

    if let Some(object) = extract_first_json_object_substring(trimmed) {
        return Ok(object);
    }

    let preview = trimmed.lines().next().unwrap_or_default();
    Err(format!(
        "Planner output did not contain a valid JSON object. First line: {}",
        truncate_for_error(preview, 160)
    ))
}

fn value_into_object(value: Value) -> Option<Value> {
    match value {
        Value::Object(_) => Some(value),
        Value::Array(mut values) if values.len() == 1 && values[0].is_object() => {
            Some(values.remove(0))
        }
        _ => None,
    }
}

fn extract_first_json_object_substring(raw: &str) -> Option<Value> {
    let mut starts = Vec::new();
    for (index, character) in raw.char_indices() {
        if character == '{' {
            starts.push(index);
        }
    }

    for start in starts {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;

        for (offset, character) in raw[start..].char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                    continue;
                }
                if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }

            match character {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        let end = start + offset + character.len_utf8();
                        let candidate = &raw[start..end];
                        if let Ok(value) = serde_json::from_str::<Value>(candidate) {
                            if let Some(object) = value_into_object(value) {
                                return Some(object);
                            }
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn truncate_for_error(text: &str, max_chars: usize) -> String {
    let mut truncated = String::new();
    for (count, character) in text.chars().enumerate() {
        if count >= max_chars {
            truncated.push_str("...");
            return truncated;
        }
        truncated.push(character);
    }
    truncated
}

fn fallback_selected_tool(user_text: &str) -> SelectedTool {
    let lower = user_text.to_lowercase();

    if (lower.contains("gimp") && (lower.contains("version") || lower.contains("platform")))
        || lower.contains("what version of gimp")
        || lower.contains("gimp info")
    {
        return SelectedTool::GetGimpInfo;
    }

    if lower.contains("describe") && lower.contains("image")
        || lower.contains("what image is open")
        || lower.contains("image metadata")
        || lower.contains("current image")
        || lower.contains("layers")
    {
        return SelectedTool::GetImageMetadata;
    }

    let edit_keywords = [
        "draw",
        "add",
        "paint",
        "resize",
        "crop",
        "rotate",
        "flip",
        "blur",
        "sharpen",
        "brightness",
        "contrast",
        "color",
        "filter",
        "erase",
        "fill",
    ];
    if edit_keywords.iter().any(|keyword| lower.contains(keyword)) {
        return SelectedTool::CallApi;
    }

    SelectedTool::None
}

fn validate_call_api_plan(plan: &Value) -> Result<(), String> {
    let steps = plan
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| "GIMP plan is missing a steps array".to_string())?;

    if steps.is_empty() {
        return Err("GIMP plan must contain at least one step".to_string());
    }

    for (index, step) in steps.iter().enumerate() {
        if step.get("tool").and_then(Value::as_str) != Some("call_api") {
            return Err(format!("GIMP plan step {index} must use the call_api tool"));
        }

        let arguments = step
            .get("arguments")
            .ok_or_else(|| format!("GIMP plan step {index} is missing arguments"))?;

        if arguments.get("api_path").and_then(Value::as_str) != Some("exec") {
            return Err(format!("GIMP plan step {index} must use api_path 'exec'"));
        }

        let args = arguments
            .get("args")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("GIMP plan step {index} is missing args"))?;

        if args.first().and_then(Value::as_str) != Some("pyGObject-console") {
            return Err(format!(
                "GIMP plan step {index} must call pyGObject-console"
            ));
        }

        let lines = args
            .get(1)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("GIMP plan step {index} is missing Python lines"))?;

        if lines.is_empty() || !lines.iter().all(Value::is_string) {
            return Err(format!(
                "GIMP plan step {index} must contain a non-empty string array of Python lines"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{plan_call_api, select_tool, SelectedTool, TextGenerator};
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::Mutex;

    struct MockGenerator {
        responses: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl TextGenerator for MockGenerator {
        async fn generate(&self, _prompt: &str) -> Result<String, String> {
            self.responses
                .lock()
                .expect("responses lock")
                .pop()
                .ok_or_else(|| "no mock response".to_string())
        }
    }

    #[tokio::test]
    async fn tool_selection_parses_selector_json() {
        let generator = MockGenerator {
            responses: Mutex::new(vec![
                r#"{"tool":"get_image_metadata","reason":"Needs current image info"}"#.to_string(),
            ]),
        };

        let selection = select_tool(&generator, "What image is open?")
            .await
            .expect("selection");
        assert_eq!(selection.tool, SelectedTool::GetImageMetadata);
    }

    #[tokio::test]
    async fn call_api_plan_rejects_invalid_step_tools() {
        let generator = MockGenerator {
            responses: Mutex::new(vec![
                r#"{"thought":"bad","steps":[{"tool":"get_image_metadata","arguments":{}}]}"#
                    .to_string(),
            ]),
        };

        let error = plan_call_api(&generator, "Rotate the image")
            .await
            .expect_err("invalid plan");
        assert!(error.contains("must use the call_api tool"));
    }

    #[tokio::test]
    async fn tool_selection_parses_json_inside_code_fence() {
        let generator = MockGenerator {
            responses: Mutex::new(vec![
                "```json\n{\"tool\":\"get_image_metadata\",\"reason\":\"Need active image details\"}\n```"
                    .to_string(),
            ]),
        };

        let selection = select_tool(&generator, "Which image is open?")
            .await
            .expect("selection");
        assert_eq!(selection.tool, SelectedTool::GetImageMetadata);
    }

    #[tokio::test]
    async fn tool_selection_falls_back_when_output_is_not_json() {
        let generator = MockGenerator {
            responses: Mutex::new(vec![
                "Use get_image_metadata because this asks about the current image.".to_string(),
            ]),
        };

        let selection = select_tool(&generator, "What image is open right now?")
            .await
            .expect("selection");
        assert_eq!(selection.tool, SelectedTool::GetImageMetadata);
        assert!(selection.reason.contains("deterministic fallback"));
    }

    #[tokio::test]
    async fn call_api_plan_retries_once_when_first_output_is_not_json() {
        let generator = MockGenerator {
            responses: Mutex::new(vec![
                r#"{"thought":"Rotate the image 90 degrees clockwise.","explain":"To do this yourself in GIMP: open Image → Transform → Rotate 90° clockwise.","steps":[{"tool":"call_api","arguments":{"api_path":"exec","args":["pyGObject-console",["from gi.repository import Gimp, Gegl","image = Gimp.get_images()[0]","layer = image.flatten()","w = image.get_width()","h = image.get_height()","drawable = layer","image.rotate(Gimp.RotationType.DEGREES90)","Gimp.displays_flush()"]],"kwargs":{}}}]}"#.to_string(),
                "I will rotate the image by 90 degrees.".to_string(),
            ]),
        };

        let plan = plan_call_api(&generator, "Rotate the image 90 degrees clockwise")
            .await
            .expect("plan");
        assert_eq!(plan.get("planner").and_then(Value::as_str), Some("engine"));
        assert_eq!(
            plan.get("steps")
                .and_then(Value::as_array)
                .map(|steps| steps.len()),
            Some(1)
        );
    }
}
