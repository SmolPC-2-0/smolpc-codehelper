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
    let value = extract_json_object(&raw)?;
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
    let mut plan = extract_json_object(&raw)?;
    validate_call_api_plan(&plan)?;
    if let Value::Object(ref mut object) = plan {
        object.insert("planner".to_string(), json!("engine"));
    }
    Ok(plan)
}

fn extract_json_object(raw: &str) -> Result<Value, String> {
    let start = raw
        .find('{')
        .ok_or_else(|| format!("Planner output did not contain JSON: {raw}"))?;
    let suffix = &raw[start..];
    let end = suffix
        .rfind('}')
        .map(|index| index + 1)
        .ok_or_else(|| format!("Planner output did not contain a closing brace: {raw}"))?;
    let json = &suffix[..end];
    serde_json::from_str(json).map_err(|error| {
        format!("Failed to parse planner JSON: {error}\nPlanner output was:\n{raw}")
    })
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
}
