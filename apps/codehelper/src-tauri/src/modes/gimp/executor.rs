use super::heuristics::{detect_direct_tool, detect_fast_path, DirectToolKind};
use super::planner::{
    answer_without_tool, plan_call_api, select_tool, SelectedTool, TextGenerator,
};
use super::response::{parse_gimp_info_reply, parse_image_metadata_reply};
use crate::assistant::state::AssistantState;
use crate::modes::provider::ToolProvider;
use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantSendRequestDto, AssistantStreamEventDto,
};
use std::sync::Arc;

const ASSISTANT_CANCELLED: &str = "ASSISTANT_CANCELLED";

fn ensure_not_cancelled(state: &AssistantState) -> Result<(), String> {
    if state.is_cancelled() {
        Err(ASSISTANT_CANCELLED.to_string())
    } else {
        Ok(())
    }
}

pub async fn execute_gimp_request<F>(
    provider: Arc<dyn ToolProvider>,
    generator: &dyn TextGenerator,
    request: &AssistantSendRequestDto,
    state: &AssistantState,
    mut emit: F,
) -> Result<AssistantResponseDto, String>
where
    F: FnMut(AssistantStreamEventDto),
{
    emit(AssistantStreamEventDto::Status {
        phase: "selecting_action".to_string(),
        detail: "Selecting the best GIMP action for this request.".to_string(),
    });
    ensure_not_cancelled(state)?;

    if let Some(fast_path) = detect_fast_path(&request.user_text) {
        emit(AssistantStreamEventDto::Status {
            phase: "connecting".to_string(),
            detail: "Connecting to GIMP.".to_string(),
        });
        provider.connect_if_needed(AppMode::Gimp).await?;
        ensure_not_cancelled(state)?;

        emit(AssistantStreamEventDto::ToolCall {
            name: fast_path.tool_name.clone(),
            arguments: fast_path.arguments.clone(),
        });
        let tool_result = provider
            .execute_tool(
                AppMode::Gimp,
                &fast_path.tool_name,
                fast_path.arguments.clone(),
            )
            .await?;
        emit(AssistantStreamEventDto::ToolResult {
            name: fast_path.tool_name.clone(),
            result: tool_result.clone(),
        });

        if !tool_result.ok {
            return Err(tool_result.summary);
        }

        let response = AssistantResponseDto {
            reply: fast_path.reply,
            explain: fast_path.explain,
            undoable: fast_path.undoable,
            plan: Some(fast_path.plan),
            tool_results: vec![tool_result],
        };
        emit(AssistantStreamEventDto::Complete {
            response: response.clone(),
        });
        return Ok(response);
    }

    if let Some(direct_tool) = detect_direct_tool(&request.user_text) {
        return execute_direct_tool(provider, direct_tool, state, &mut emit).await;
    }

    let selection = select_tool(generator, &request.user_text).await?;
    ensure_not_cancelled(state)?;

    match selection.tool {
        SelectedTool::None => {
            let reply = answer_without_tool(generator, &request.user_text).await?;
            let response = AssistantResponseDto {
                reply,
                explain: None,
                undoable: false,
                plan: Some(serde_json::json!({
                    "thought": "Tool selector chose none.",
                    "toolSelection": {
                        "tool": "none",
                        "reason": selection.reason,
                    },
                    "steps": []
                })),
                tool_results: Vec::new(),
            };
            emit(AssistantStreamEventDto::Complete {
                response: response.clone(),
            });
            Ok(response)
        }
        SelectedTool::GetGimpInfo => {
            execute_direct_tool(provider, DirectToolKind::GimpInfo, state, &mut emit).await
        }
        SelectedTool::GetImageMetadata => {
            execute_direct_tool(provider, DirectToolKind::ImageMetadata, state, &mut emit).await
        }
        SelectedTool::CallApi => {
            emit(AssistantStreamEventDto::Status {
                phase: "planning".to_string(),
                detail: "Planning the requested GIMP edit.".to_string(),
            });
            let plan = plan_call_api(generator, &request.user_text).await?;
            ensure_not_cancelled(state)?;

            emit(AssistantStreamEventDto::Status {
                phase: "connecting".to_string(),
                detail: "Connecting to GIMP.".to_string(),
            });
            provider.connect_if_needed(AppMode::Gimp).await?;

            let mut tool_results = Vec::new();
            for step in plan
                .get("steps")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
            {
                ensure_not_cancelled(state)?;
                let tool_name = step
                    .get("tool")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("call_api")
                    .to_string();
                let arguments = step
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                emit(AssistantStreamEventDto::ToolCall {
                    name: tool_name.clone(),
                    arguments: arguments.clone(),
                });
                let tool_result = provider
                    .execute_tool(AppMode::Gimp, &tool_name, arguments)
                    .await?;
                emit(AssistantStreamEventDto::ToolResult {
                    name: tool_name,
                    result: tool_result.clone(),
                });

                if !tool_result.ok {
                    return Err(tool_result.summary);
                }

                tool_results.push(tool_result);
            }

            let explain = plan
                .get("explain")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let reply = plan
                .get("thought")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|thought| !thought.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| "Done! Changes applied to the image.".to_string());
            let response = AssistantResponseDto {
                reply,
                explain,
                undoable: true,
                plan: Some(plan),
                tool_results,
            };
            emit(AssistantStreamEventDto::Complete {
                response: response.clone(),
            });
            Ok(response)
        }
    }
}

async fn execute_direct_tool<F>(
    provider: Arc<dyn ToolProvider>,
    tool: DirectToolKind,
    state: &AssistantState,
    emit: &mut F,
) -> Result<AssistantResponseDto, String>
where
    F: FnMut(AssistantStreamEventDto),
{
    ensure_not_cancelled(state)?;
    emit(AssistantStreamEventDto::Status {
        phase: "connecting".to_string(),
        detail: "Connecting to GIMP.".to_string(),
    });
    provider.connect_if_needed(AppMode::Gimp).await?;
    ensure_not_cancelled(state)?;

    let (tool_name, thought) = match tool {
        DirectToolKind::GimpInfo => ("get_gimp_info", "Query GIMP environment details."),
        DirectToolKind::ImageMetadata => ("get_image_metadata", "Query current image metadata."),
    };
    let arguments = serde_json::json!({});

    emit(AssistantStreamEventDto::ToolCall {
        name: tool_name.to_string(),
        arguments: arguments.clone(),
    });
    let tool_result = provider
        .execute_tool(AppMode::Gimp, tool_name, arguments)
        .await?;
    emit(AssistantStreamEventDto::ToolResult {
        name: tool_name.to_string(),
        result: tool_result.clone(),
    });

    if !tool_result.ok {
        return Err(tool_result.summary);
    }

    let reply = match tool {
        DirectToolKind::GimpInfo => parse_gimp_info_reply(&tool_result.payload)
            .unwrap_or_else(|| "I fetched the current GIMP environment details.".to_string()),
        DirectToolKind::ImageMetadata => parse_image_metadata_reply(&tool_result.payload)
            .unwrap_or_else(|| "I fetched the current image metadata.".to_string()),
    };
    let response = AssistantResponseDto {
        reply,
        explain: None,
        undoable: false,
        plan: Some(serde_json::json!({
            "thought": thought,
            "steps": [
                {
                    "tool": tool_name,
                    "arguments": {}
                }
            ]
        })),
        tool_results: vec![tool_result],
    };
    emit(AssistantStreamEventDto::Complete {
        response: response.clone(),
    });
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::execute_gimp_request;
    use crate::assistant::state::AssistantState;
    use crate::modes::provider::{provider_state, ToolProvider};
    use async_trait::async_trait;
    use serde_json::json;
    use smolpc_assistant_types::{
        AppMode, AssistantMessageDto, AssistantSendRequestDto, AssistantStreamEventDto,
        ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
    };
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockProvider {
        results: Mutex<VecDeque<ToolExecutionResultDto>>,
    }

    #[async_trait]
    impl ToolProvider for MockProvider {
        async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            Ok(provider_state(mode, "connected", None, true, true))
        }

        async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            Ok(provider_state(mode, "connected", None, true, true))
        }

        async fn list_tools(&self, _mode: AppMode) -> Result<Vec<ToolDefinitionDto>, String> {
            Ok(Vec::new())
        }

        async fn execute_tool(
            &self,
            _mode: AppMode,
            name: &str,
            _arguments: serde_json::Value,
        ) -> Result<ToolExecutionResultDto, String> {
            self.results
                .lock()
                .expect("results lock")
                .pop_front()
                .map(|mut result| {
                    result.name = name.to_string();
                    result
                })
                .ok_or_else(|| "no mock result".to_string())
        }

        async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
            Ok(())
        }

        async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
            Ok(())
        }
    }

    struct MockGenerator {
        responses: Mutex<VecDeque<String>>,
    }

    #[async_trait]
    impl super::super::planner::TextGenerator for MockGenerator {
        async fn generate(&self, _prompt: &str) -> Result<String, String> {
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or_else(|| "no mock response".to_string())
        }
    }

    fn request(user_text: &str) -> AssistantSendRequestDto {
        AssistantSendRequestDto {
            mode: AppMode::Gimp,
            chat_id: Some("chat-1".to_string()),
            messages: vec![AssistantMessageDto {
                role: "user".to_string(),
                content: user_text.to_string(),
            }],
            user_text: user_text.to_string(),
        }
    }

    #[tokio::test]
    async fn gimp_request_succeeds_for_info_query() {
        let provider = Arc::new(MockProvider {
            results: Mutex::new(VecDeque::from(vec![ToolExecutionResultDto {
                name: "get_image_metadata".to_string(),
                ok: true,
                summary: "metadata returned".to_string(),
                payload: json!({
                    "content": [
                        {
                            "text": "{\"basic\":{\"width\":640,\"height\":480,\"base_type\":\"RGB\"},\"file\":{\"basename\":\"photo.png\"}}"
                        }
                    ]
                }),
            }])),
        });
        let generator = MockGenerator {
            responses: Mutex::new(VecDeque::new()),
        };
        let mut events = Vec::new();

        let response = execute_gimp_request(
            provider,
            &generator,
            &request("What image is open right now?"),
            &AssistantState::default(),
            |event| events.push(event),
        )
        .await
        .expect("response");

        assert!(response.reply.contains("photo.png"));
        assert!(!response.undoable);
        assert!(events.iter().any(|event| matches!(event, AssistantStreamEventDto::ToolCall { name, .. } if name == "get_image_metadata")));
    }

    #[tokio::test]
    async fn gimp_request_succeeds_for_fast_path_edit() {
        let provider = Arc::new(MockProvider {
            results: Mutex::new(VecDeque::from(vec![ToolExecutionResultDto {
                name: "call_api".to_string(),
                ok: true,
                summary: "edit applied".to_string(),
                payload: json!({ "content": [{ "text": "ok" }] }),
            }])),
        });
        let generator = MockGenerator {
            responses: Mutex::new(VecDeque::new()),
        };

        let response = execute_gimp_request(
            provider,
            &generator,
            &request("Crop this image to a square"),
            &AssistantState::default(),
            |_| {},
        )
        .await
        .expect("response");

        assert!(response.reply.contains("square"));
        assert!(response.undoable);
    }

    #[tokio::test]
    async fn gimp_request_succeeds_for_planned_call_api_edit() {
        let provider = Arc::new(MockProvider {
            results: Mutex::new(VecDeque::from(vec![ToolExecutionResultDto {
                name: "call_api".to_string(),
                ok: true,
                summary: "edit applied".to_string(),
                payload: json!({ "content": [{ "text": "ok" }] }),
            }])),
        });
        let generator = MockGenerator {
            responses: Mutex::new(VecDeque::from(vec![
                r#"{"thought":"Rotate the image 90 degrees clockwise.","explain":"To do this yourself in GIMP: open Image → Transform → Rotate 90° clockwise.","steps":[{"tool":"call_api","arguments":{"api_path":"exec","args":["pyGObject-console",["from gi.repository import Gimp, Gegl","image = Gimp.get_images()[0]","layer = image.flatten()","w = image.get_width()","h = image.get_height()","drawable = layer","image.rotate(Gimp.RotationType.DEGREES90)","Gimp.displays_flush()"]],"kwargs":{}}}]}"#.to_string(),
                r#"{"tool":"call_api","reason":"This is an edit request"}"#.to_string(),
            ])),
        };

        let response = execute_gimp_request(
            provider,
            &generator,
            &request("Rotate the image 90 degrees clockwise"),
            &AssistantState::default(),
            |_| {},
        )
        .await
        .expect("response");

        assert!(response.reply.contains("Rotate"));
        assert!(response.undoable);
    }
}
