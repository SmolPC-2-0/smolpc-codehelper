use super::profiles::{allowed_tool_names, libreoffice_profile};
use super::response::{build_libreoffice_response, build_local_fallback_summary};
use crate::assistant::state::AssistantState;
use crate::modes::provider::ToolProvider;
use crate::modes::text_generation::TextStreamer;
use async_trait::async_trait;
use serde_json::{json, Value};
use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantSendRequestDto, AssistantStreamEventDto,
    ToolDefinitionDto, ToolExecutionResultDto,
};
use smolpc_engine_client::{EngineChatMessage, EngineClient};
use smolpc_engine_core::GenerationConfig;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

const ASSISTANT_CANCELLED: &str = "ASSISTANT_CANCELLED";
// Keep summary generation short because the document mutation already happened
// in the tool step; the follow-up is only user-facing explanation, not the
// authoritative source of truth for the document state.
#[cfg(not(test))]
const SUMMARY_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const SUMMARY_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq)]
struct ToolCall {
    name: String,
    arguments: Value,
}

#[async_trait]
pub trait TextPlanner: Send + Sync {
    async fn generate(&self, messages: &[EngineChatMessage]) -> Result<String, String>;
}

pub struct EngineTextPlanner {
    client: EngineClient,
}

impl EngineTextPlanner {
    pub fn new(client: EngineClient) -> Self {
        Self { client }
    }

    fn config() -> GenerationConfig {
        GenerationConfig {
            max_length: 384,
            temperature: 0.0,
            top_k: Some(20),
            top_p: Some(0.9),
            repetition_penalty: 1.05,
            repetition_penalty_last_n: 128,
        }
    }
}

#[async_trait]
impl TextPlanner for EngineTextPlanner {
    async fn generate(&self, messages: &[EngineChatMessage]) -> Result<String, String> {
        let mut collected = String::new();
        self.client
            .generate_stream_messages(messages, Some(Self::config()), |token| {
                collected.push_str(&token);
            })
            .await
            .map_err(|error| format!("LibreOffice planner generation failed: {error}"))?;
        Ok(collected)
    }
}

fn ensure_not_cancelled(state: &AssistantState) -> Result<(), String> {
    if state.is_cancelled() {
        Err(ASSISTANT_CANCELLED.to_string())
    } else {
        Ok(())
    }
}

fn build_planner_messages(
    mode: AppMode,
    request: &AssistantSendRequestDto,
    tools: &[ToolDefinitionDto],
) -> Vec<EngineChatMessage> {
    let (label, allowed_tools) = libreoffice_profile(mode)
        .map(|profile| (profile.label, profile.allowed_tools))
        .unwrap_or(("LibreOffice", &[]));
    let tool_catalog = tools
        .iter()
        .map(|tool| {
            format!(
                "- {}: {}\n  inputSchema: {}",
                tool.name, tool.description, tool.input_schema
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        "You are the unified LibreOffice {} assistant.\n\
Choose at most one tool call for the user's latest request.\n\
Return JSON only when a tool call is required, using exactly this shape:\n\
{{\"tool_call\":{{\"name\":\"<tool_name>\",\"arguments\":{{...}}}}}}\n\
Do not use markdown fences.\n\
If no tool call is needed, return the final user-facing answer as plain text.\n\
Only use tools from this allowlist:\n{}\n\
Tool catalog:\n{}",
        label,
        allowed_tools.join(", "),
        tool_catalog
    );

    let mut messages = vec![EngineChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];

    if request.messages.is_empty() {
        messages.push(EngineChatMessage {
            role: "user".to_string(),
            content: request.user_text.clone(),
        });
        return messages;
    }

    messages.extend(request.messages.iter().map(|message| EngineChatMessage {
        role: message.role.clone(),
        content: message.content.clone(),
    }));
    messages
}

fn build_summary_messages(
    mode: AppMode,
    user_text: &str,
    tool_result: &ToolExecutionResultDto,
) -> Vec<EngineChatMessage> {
    let label = libreoffice_profile(mode)
        .map(|profile| profile.label)
        .unwrap_or("LibreOffice");

    vec![
        EngineChatMessage {
            role: "system".to_string(),
            content: format!(
                "You are the unified LibreOffice {} assistant. A document tool has already run successfully. Summarize the result for the user in 2 or 3 short sentences. Plain text only. Do not suggest replaying the action. Do not mention undo.",
                label
            ),
        },
        EngineChatMessage {
            role: "user".to_string(),
            content: format!(
                "Original request:\n{}\n\nExecuted tool: {}\n\nTool result JSON:\n{}",
                user_text, tool_result.name, tool_result.payload
            ),
        },
    ]
}

fn parse_tool_arguments(value: Option<&Value>) -> Value {
    match value {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        Some(Value::String(raw)) if !raw.trim().is_empty() => serde_json::from_str(raw)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({})),
        _ => json!({}),
    }
}

fn parse_single_tool_call(value: &Value) -> Option<ToolCall> {
    let object = value.as_object()?;
    let function_field = object.get("function").and_then(Value::as_object);
    let raw_name = object.get("name").and_then(Value::as_str).or_else(|| {
        function_field
            .and_then(|field| field.get("name"))
            .and_then(Value::as_str)
    })?;
    let raw_arguments = object
        .get("arguments")
        .or_else(|| object.get("args"))
        .or_else(|| function_field.and_then(|field| field.get("arguments")))
        .or_else(|| function_field.and_then(|field| field.get("args")));

    let name = raw_name.trim();
    if name.is_empty() {
        return None;
    }

    Some(ToolCall {
        name: name.to_string(),
        arguments: parse_tool_arguments(raw_arguments),
    })
}

fn normalize_tool_calls(value: &Value) -> Vec<ToolCall> {
    if let Some(array) = value.as_array() {
        return array.iter().filter_map(parse_single_tool_call).collect();
    }

    let Some(object) = value.as_object() else {
        return Vec::new();
    };

    if let Some(tool_calls) = object.get("tool_calls") {
        return normalize_tool_calls(tool_calls);
    }
    if let Some(tool_call) = object.get("tool_call") {
        return normalize_tool_calls(tool_call);
    }

    parse_single_tool_call(value).into_iter().collect()
}

fn extract_json_candidates(raw_text: &str) -> Vec<String> {
    let trimmed = raw_text.trim();
    let mut candidates = if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    };

    let mut remaining = raw_text;
    while let Some(start) = remaining.find("```") {
        remaining = &remaining[start + 3..];
        if remaining.starts_with("json") {
            remaining = &remaining[4..];
        }
        if let Some(end) = remaining.find("```") {
            let candidate = remaining[..end].trim();
            if !candidate.is_empty() {
                candidates.push(candidate.to_string());
            }
            remaining = &remaining[end + 3..];
        } else {
            break;
        }
    }

    candidates
}

fn extract_tool_call(raw_text: &str) -> Option<ToolCall> {
    for candidate in extract_json_candidates(raw_text) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&candidate) {
            if let Some(tool_call) = normalize_tool_calls(&parsed).into_iter().next() {
                return Some(tool_call);
            }
        }
    }

    None
}

async fn stream_summary_with_fallback<F>(
    mode: AppMode,
    generator: &dyn TextStreamer,
    state: &AssistantState,
    user_text: &str,
    tool_result: ToolExecutionResultDto,
    mut emit: F,
) -> AssistantResponseDto
where
    F: FnMut(AssistantStreamEventDto) + Send,
{
    let messages = build_summary_messages(mode, user_text, &tool_result);
    emit(AssistantStreamEventDto::Status {
        phase: "generating_summary".to_string(),
        detail: format!(
            "Summarizing the {} tool result for the user.",
            tool_result.name
        ),
    });

    let mut accumulated_text = String::new();
    let mut used_local_fallback = false;
    let reply = match timeout(
        SUMMARY_TIMEOUT,
        generator.generate_stream(&messages, state, &mut |token| {
            accumulated_text.push_str(&token);
            emit(AssistantStreamEventDto::Token { token });
        }),
    )
    .await
    {
        Ok(Ok(reply)) if !reply.trim().is_empty() => reply,
        Ok(Ok(_)) => {
            used_local_fallback = true;
            build_local_fallback_summary(&tool_result)
        }
        Ok(Err(_)) | Err(_) => {
            used_local_fallback = true;
            build_local_fallback_summary(&tool_result)
        }
    };

    let tool_name = tool_result.name.clone();
    let response = build_libreoffice_response(
        mode,
        if accumulated_text.trim().is_empty() {
            reply
        } else {
            accumulated_text
        },
        Some(tool_name.as_str()),
        used_local_fallback,
        vec![tool_result],
    );
    emit(AssistantStreamEventDto::Complete {
        response: response.clone(),
    });
    response
}

pub async fn execute_libreoffice_request<F>(
    provider: Arc<dyn ToolProvider>,
    planner: &dyn TextPlanner,
    streamer: &dyn TextStreamer,
    request: &AssistantSendRequestDto,
    state: &AssistantState,
    mut emit: F,
) -> Result<AssistantResponseDto, String>
where
    F: FnMut(AssistantStreamEventDto) + Send,
{
    let mode = request.mode;
    let profile = libreoffice_profile(mode)
        .ok_or_else(|| format!("LibreOffice provider does not handle mode {mode:?}"))?;
    if !profile.live_in_phase_6b {
        return Err("UNIFIED_ASSISTANT_NOT_IMPLEMENTED".to_string());
    }

    emit(AssistantStreamEventDto::Status {
        phase: "starting_libreoffice_request".to_string(),
        detail: format!("Starting the {} request.", profile.label),
    });

    provider.connect_if_needed(mode).await?;
    ensure_not_cancelled(state)?;

    let tools = provider.list_tools(mode).await?;
    if tools.is_empty() {
        return Err(format!(
            "{} is connected, but no {} tools are available yet.",
            profile.label, profile.label
        ));
    }

    emit(AssistantStreamEventDto::Status {
        phase: "selecting_action".to_string(),
        detail: format!("Selecting the safest {} action.", profile.label),
    });

    let planner_messages = build_planner_messages(mode, request, &tools);
    let planner_output = planner.generate(&planner_messages).await?;
    let tool_call = extract_tool_call(&planner_output);

    if tool_call.is_none() {
        let response = build_libreoffice_response(
            mode,
            if planner_output.trim().is_empty() {
                format!(
                    "I couldn't choose a safe {} action for that request. Try rephrasing it as a single supported step.",
                    profile.label
                )
            } else {
                planner_output.trim().to_string()
            },
            None,
            false,
            Vec::new(),
        );
        emit(AssistantStreamEventDto::Complete {
            response: response.clone(),
        });
        return Ok(response);
    }

    let tool_call = tool_call.ok_or_else(|| {
        format!(
            "I couldn't choose a safe {} action for that request.",
            profile.label
        )
    })?;
    let allowlist = allowed_tool_names(mode);
    if !allowlist.iter().any(|name| *name == tool_call.name) {
        return Err(format!(
            "{} is not an allowed {} tool in Phase 6B.",
            tool_call.name, profile.label
        ));
    }

    ensure_not_cancelled(state)?;
    emit(AssistantStreamEventDto::ToolCall {
        name: tool_call.name.clone(),
        arguments: tool_call.arguments.clone(),
    });
    let tool_result = provider
        .execute_tool(mode, &tool_call.name, tool_call.arguments.clone())
        .await?;
    emit(AssistantStreamEventDto::ToolResult {
        name: tool_call.name,
        result: tool_result.clone(),
    });

    if !tool_result.ok {
        return Err(tool_result.summary);
    }

    let response =
        stream_summary_with_fallback(mode, streamer, state, &request.user_text, tool_result, emit)
            .await;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{execute_libreoffice_request, extract_tool_call, TextPlanner};
    use crate::assistant::state::AssistantState;
    use crate::modes::provider::{provider_state, ToolProvider};
    use crate::modes::text_generation::TextStreamer;
    use async_trait::async_trait;
    use serde_json::json;
    use smolpc_assistant_types::{
        AppMode, AssistantMessageDto, AssistantSendRequestDto, AssistantStreamEventDto,
        ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
    };
    use smolpc_engine_client::EngineChatMessage;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockProvider {
        tools: Vec<ToolDefinitionDto>,
        result: Option<ToolExecutionResultDto>,
    }

    #[async_trait]
    impl ToolProvider for MockProvider {
        async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            Ok(provider_state(mode, "connected", None, true, false))
        }

        async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            Ok(provider_state(mode, "connected", None, true, false))
        }

        async fn list_tools(&self, _mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
            Ok(self.tools.clone())
        }

        async fn execute_tool(
            &self,
            _mode: AppMode,
            _name: &str,
            _arguments: serde_json::Value,
        ) -> Result<ToolExecutionResultDto, String> {
            self.result
                .clone()
                .ok_or_else(|| "missing tool result".to_string())
        }

        async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
            Err("unsupported".to_string())
        }

        async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
            Ok(())
        }
    }

    struct MockPlanner {
        reply: String,
    }

    #[async_trait]
    impl TextPlanner for MockPlanner {
        async fn generate(&self, _messages: &[EngineChatMessage]) -> Result<String, String> {
            Ok(self.reply.clone())
        }
    }

    struct MockStreamer;

    #[async_trait]
    impl TextStreamer for MockStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            _state: &AssistantState,
            on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            on_token("Writer ".to_string());
            on_token("summary".to_string());
            Ok("Writer summary".to_string())
        }
    }

    struct FailingStreamer;

    #[async_trait]
    impl TextStreamer for FailingStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            _state: &AssistantState,
            _on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            Err("summary failed".to_string())
        }
    }

    struct CancellableStreamer;

    #[async_trait]
    impl TextStreamer for CancellableStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            state: &AssistantState,
            _on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            state.mark_cancelled();
            Err("ASSISTANT_CANCELLED".to_string())
        }
    }

    struct SlowStreamer;

    #[async_trait]
    impl TextStreamer for SlowStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            _state: &AssistantState,
            _on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            Ok("too late".to_string())
        }
    }

    fn request(mode: AppMode, text: &str) -> AssistantSendRequestDto {
        AssistantSendRequestDto {
            mode,
            chat_id: Some("chat".to_string()),
            messages: vec![AssistantMessageDto {
                role: "user".to_string(),
                content: text.to_string(),
            }],
            user_text: text.to_string(),
        }
    }

    fn writer_tool() -> ToolDefinitionDto {
        ToolDefinitionDto {
            name: "add_heading".to_string(),
            description: "Add a heading".to_string(),
            input_schema: json!({"type": "object"}),
        }
    }

    fn slides_tool() -> ToolDefinitionDto {
        ToolDefinitionDto {
            name: "add_slide".to_string(),
            description: "Add a slide".to_string(),
            input_schema: json!({"type": "object"}),
        }
    }

    fn ok_tool_result(name: &str) -> ToolExecutionResultDto {
        ToolExecutionResultDto {
            name: name.to_string(),
            ok: true,
            summary: format!("{name} completed"),
            payload: json!({
                "content": [
                    {
                        "type": "text",
                        "text": format!("{name} completed successfully.")
                    }
                ]
            }),
        }
    }

    #[tokio::test]
    async fn writer_request_executes_one_allowed_tool_and_streams_summary() {
        let provider = Arc::new(MockProvider {
            tools: vec![writer_tool()],
            result: Some(ok_tool_result("add_heading")),
        });
        let planner = MockPlanner {
            reply: "{\"tool_call\":{\"name\":\"add_heading\",\"arguments\":{\"text\":\"Hello\"}}}"
                .to_string(),
        };
        let streamer = MockStreamer;
        let state = AssistantState::default();
        let events = Arc::new(Mutex::new(Vec::<AssistantStreamEventDto>::new()));

        let response = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Writer, "Add a heading"),
            &state,
            {
                let events = Arc::clone(&events);
                move |event| {
                    events.lock().expect("events lock").push(event);
                }
            },
        )
        .await
        .expect("writer response");

        assert_eq!(response.reply, "Writer summary");
        assert_eq!(response.tool_results.len(), 1);
        let events = events.lock().expect("events lock");
        assert!(events
            .iter()
            .any(|event| matches!(event, AssistantStreamEventDto::Token { .. })));
    }

    #[tokio::test]
    async fn impress_request_uses_json_fallback_tool_parsing() {
        let provider = Arc::new(MockProvider {
            tools: vec![slides_tool()],
            result: Some(ok_tool_result("add_slide")),
        });
        let planner = MockPlanner {
            reply: "```json\n{\"tool_call\":{\"name\":\"add_slide\",\"arguments\":{\"title\":\"Intro\"}}}\n```"
                .to_string(),
        };
        let streamer = MockStreamer;
        let state = AssistantState::default();

        let response = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Impress, "Add a slide"),
            &state,
            |_| {},
        )
        .await
        .expect("slides response");

        assert_eq!(response.tool_results[0].name, "add_slide");
    }

    #[test]
    fn parse_tool_arguments_accepts_stringified_json_objects() {
        let tool_call = extract_tool_call(
            "{\"tool_call\":{\"name\":\"add_heading\",\"arguments\":\"{\\\"text\\\":\\\"Hello\\\"}\"}}",
        )
        .expect("tool call");

        assert_eq!(tool_call.name, "add_heading");
        assert_eq!(tool_call.arguments["text"], json!("Hello"));
    }

    #[tokio::test]
    async fn calc_mode_remains_unimplemented() {
        let provider = Arc::new(MockProvider::default());
        let planner = MockPlanner {
            reply: "no-op".to_string(),
        };
        let streamer = MockStreamer;
        let state = AssistantState::default();

        let error = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Calc, "Open a sheet"),
            &state,
            |_| {},
        )
        .await
        .expect_err("calc should remain scaffold-only");

        assert_eq!(error, "UNIFIED_ASSISTANT_NOT_IMPLEMENTED");
    }

    #[tokio::test]
    async fn summary_failure_falls_back_to_local_summary() {
        let provider = Arc::new(MockProvider {
            tools: vec![writer_tool()],
            result: Some(ToolExecutionResultDto {
                name: "list_documents".to_string(),
                ok: true,
                summary: "ok".to_string(),
                payload: json!({
                    "content": [
                        {
                            "type": "text",
                            "text": "Found 2 documents.\nName: alpha.odt\nName: beta.odt"
                        }
                    ]
                }),
            }),
        });
        let planner = MockPlanner {
            reply: "{\"tool_call\":{\"name\":\"add_heading\",\"arguments\":{\"text\":\"Hello\"}}}"
                .to_string(),
        };
        let streamer = FailingStreamer;
        let state = AssistantState::default();

        let response = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Writer, "List docs"),
            &state,
            |_| {},
        )
        .await
        .expect("fallback response");

        assert!(response.reply.contains("Found 2 document(s)."));
        assert_eq!(
            response.plan.expect("plan")["usedLocalFallbackSummary"],
            json!(true)
        );
    }

    #[tokio::test]
    async fn cancellation_after_tool_execution_uses_fallback_summary() {
        let provider = Arc::new(MockProvider {
            tools: vec![writer_tool()],
            result: Some(ok_tool_result("add_heading")),
        });
        let planner = MockPlanner {
            reply: "{\"tool_call\":{\"name\":\"add_heading\",\"arguments\":{\"text\":\"Hello\"}}}"
                .to_string(),
        };
        let streamer = CancellableStreamer;
        let state = AssistantState::default();

        let response = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Writer, "Add heading"),
            &state,
            |_| {},
        )
        .await
        .expect("fallback response");

        assert!(response
            .reply
            .contains("add_heading completed successfully"));
        assert_eq!(
            response.plan.expect("plan")["usedLocalFallbackSummary"],
            json!(true)
        );
    }

    #[tokio::test]
    async fn summary_timeout_uses_fallback_summary() {
        let provider = Arc::new(MockProvider {
            tools: vec![writer_tool()],
            result: Some(ok_tool_result("add_heading")),
        });
        let planner = MockPlanner {
            reply: "{\"tool_call\":{\"name\":\"add_heading\",\"arguments\":{\"text\":\"Hello\"}}}"
                .to_string(),
        };
        let streamer = SlowStreamer;
        let state = AssistantState::default();

        let response = execute_libreoffice_request(
            provider,
            &planner,
            &streamer,
            &request(AppMode::Writer, "Add heading"),
            &state,
            |_| {},
        )
        .await
        .expect("fallback response");

        assert!(response
            .reply
            .contains("add_heading completed successfully"));
        assert_eq!(
            response.plan.expect("plan")["usedLocalFallbackSummary"],
            json!(true)
        );
    }
}
