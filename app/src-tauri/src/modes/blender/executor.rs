use super::prompts::build_question_prompts;
use super::rag::RagContext;
use super::response::{build_blender_response, parse_rag_contexts, parse_scene_snapshot};
use crate::assistant::state::AssistantState;
use smolpc_connector_common::ToolProvider;
use crate::modes::text_generation::TextStreamer;
use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantSendRequestDto, AssistantStreamEventDto,
};
use smolpc_engine_client::EngineChatMessage;
use std::sync::Arc;

const ASSISTANT_CANCELLED: &str = "ASSISTANT_CANCELLED";
const DEFAULT_RAG_RESULTS: usize = 3;
const SCENE_QUERY_HINTS: [&str; 14] = [
    "current scene",
    "scene right now",
    "in my scene",
    "on my scene",
    "what is in the scene",
    "what's in the scene",
    "whats in the scene",
    "scene contents",
    "list objects",
    "what objects",
    "which objects",
    "active object",
    "selected object",
    "scene summary",
];
const WORKFLOW_HINTS: [&str; 12] = [
    "how do i",
    "how to",
    "add ",
    "create ",
    "bevel",
    "modifier",
    "workflow",
    "fix ",
    "apply ",
    "rotate ",
    "extrude",
    "explain how",
];

fn ensure_not_cancelled(state: &AssistantState) -> Result<(), String> {
    if state.is_cancelled() {
        Err(ASSISTANT_CANCELLED.to_string())
    } else {
        Ok(())
    }
}

fn should_skip_rag_for_scene_query(question: &str) -> bool {
    let normalized = question.trim().to_ascii_lowercase();
    if WORKFLOW_HINTS
        .iter()
        .any(|needle| normalized.contains(needle))
    {
        return false;
    }

    SCENE_QUERY_HINTS
        .iter()
        .any(|needle| normalized.contains(needle))
}

pub async fn execute_blender_request<F>(
    provider: Arc<dyn ToolProvider>,
    generator: &dyn TextStreamer,
    request: &AssistantSendRequestDto,
    state: &AssistantState,
    mut emit: F,
) -> Result<AssistantResponseDto, String>
where
    F: FnMut(AssistantStreamEventDto) + Send,
{
    emit(AssistantStreamEventDto::Status {
        phase: "starting_blender_request".to_string(),
        detail: "Starting the Blender tutoring request.".to_string(),
    });
    ensure_not_cancelled(state)?;

    provider.connect_if_needed(AppMode::Blender).await?;
    ensure_not_cancelled(state)?;

    emit(AssistantStreamEventDto::ToolCall {
        name: "scene_current".to_string(),
        arguments: serde_json::json!({}),
    });
    let scene_result = provider
        .execute_tool(AppMode::Blender, "scene_current", serde_json::json!({}))
        .await?;
    emit(AssistantStreamEventDto::ToolResult {
        name: "scene_current".to_string(),
        result: scene_result.clone(),
    });
    if !scene_result.ok {
        return Err(scene_result.summary);
    }

    let scene_snapshot = parse_scene_snapshot(&scene_result.payload);
    let scene_context = scene_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.scene_data.clone());
    let mut rag_contexts = Vec::<RagContext>::new();
    let mut tool_results = vec![scene_result];

    if !should_skip_rag_for_scene_query(&request.user_text) {
        let rag_arguments = serde_json::json!({
            "query": request.user_text,
            "nResults": DEFAULT_RAG_RESULTS,
        });
        emit(AssistantStreamEventDto::ToolCall {
            name: "retrieve_rag_context".to_string(),
            arguments: rag_arguments.clone(),
        });
        let rag_result = provider
            .execute_tool(AppMode::Blender, "retrieve_rag_context", rag_arguments)
            .await?;
        emit(AssistantStreamEventDto::ToolResult {
            name: "retrieve_rag_context".to_string(),
            result: rag_result.clone(),
        });
        if !rag_result.ok {
            return Err(rag_result.summary);
        }

        rag_contexts = parse_rag_contexts(&rag_result.payload);
        tool_results.push(rag_result);
    }

    ensure_not_cancelled(state)?;

    let (system_prompt, user_prompt) =
        build_question_prompts(&request.user_text, scene_context.as_ref(), &rag_contexts);
    let messages = vec![
        EngineChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        },
        EngineChatMessage {
            role: "user".to_string(),
            content: user_prompt,
        },
    ];

    emit(AssistantStreamEventDto::Status {
        phase: "generating_answer".to_string(),
        detail: "Generating a Blender tutoring answer.".to_string(),
    });

    let mut emitted_tokens = false;
    let reply = generator
        .generate_stream(&messages, state, &mut |token| {
            emitted_tokens = true;
            emit(AssistantStreamEventDto::Token { token });
        })
        .await?;

    let response = build_blender_response(
        if emitted_tokens || !reply.trim().is_empty() {
            reply
        } else {
            "I’m ready to help with your Blender question.".to_string()
        },
        scene_context.is_some(),
        !rag_contexts.is_empty(),
        rag_contexts.len(),
        tool_results,
    );
    emit(AssistantStreamEventDto::Complete {
        response: response.clone(),
    });
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::execute_blender_request;
    use crate::assistant::state::AssistantState;
    use smolpc_connector_common::{provider_state, ToolProvider};
    use crate::modes::text_generation::TextStreamer;
    use async_trait::async_trait;
    use serde_json::json;
    use smolpc_assistant_types::{
        AppMode, AssistantMessageDto, AssistantSendRequestDto, AssistantStreamEventDto,
        ProviderStateDto, ToolDefinitionDto, ToolExecutionResultDto,
    };
    use smolpc_engine_client::EngineChatMessage;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct MockProvider {
        calls: Mutex<Vec<String>>,
        results: Mutex<VecDeque<ToolExecutionResultDto>>,
    }

    #[async_trait]
    impl ToolProvider for MockProvider {
        async fn connect_if_needed(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            self.calls
                .lock()
                .expect("calls lock")
                .push("connect_if_needed".to_string());
            Ok(provider_state(mode, "connected", None, true, false))
        }

        async fn status(&self, mode: AppMode) -> Result<ProviderStateDto, String> {
            Ok(provider_state(mode, "connected", None, true, false))
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
            self.calls
                .lock()
                .expect("calls lock")
                .push(name.to_string());
            self.results
                .lock()
                .expect("results lock")
                .pop_front()
                .ok_or_else(|| "no tool result".to_string())
        }

        async fn undo_last_action(&self, _mode: AppMode) -> Result<(), String> {
            Err("unsupported".to_string())
        }

        async fn disconnect_if_needed(&self, _mode: AppMode) -> Result<(), String> {
            Ok(())
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
            on_token("Blender ".to_string());
            on_token("answer".to_string());
            Ok("Blender answer".to_string())
        }
    }

    struct BlockingStreamer;

    #[async_trait]
    impl TextStreamer for BlockingStreamer {
        async fn generate_stream(
            &self,
            _messages: &[EngineChatMessage],
            state: &AssistantState,
            _on_token: &mut (dyn FnMut(String) + Send),
        ) -> Result<String, String> {
            loop {
                if state.is_cancelled() {
                    return Err("ASSISTANT_CANCELLED".to_string());
                }

                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }
    }

    fn request(question: &str) -> AssistantSendRequestDto {
        AssistantSendRequestDto {
            mode: AppMode::Blender,
            chat_id: Some("chat-1".to_string()),
            messages: vec![AssistantMessageDto {
                role: "user".to_string(),
                content: question.to_string(),
            }],
            user_text: question.to_string(),
        }
    }

    #[tokio::test]
    async fn scene_query_skips_rag_lookup() {
        let provider = Arc::new(MockProvider {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from(vec![ToolExecutionResultDto {
                name: "scene_current".to_string(),
                ok: true,
                summary: "Scene snapshot available".to_string(),
                payload: json!({
                    "connected": true,
                    "scene_data": {
                        "object_count": 1,
                        "active_object": "Cube",
                        "mode": "OBJECT",
                        "render_engine": "BLENDER_EEVEE",
                        "objects": []
                    },
                    "message": null,
                    "last_update": 10
                }),
            }])),
        });
        let state = AssistantState::default();

        let response = execute_blender_request(
            provider.clone(),
            &MockStreamer,
            &request("What is in my scene right now?"),
            &state,
            |_| {},
        )
        .await
        .expect("blender response");

        let calls = provider.calls.lock().expect("calls");
        assert!(calls.iter().any(|call| call == "scene_current"));
        assert!(!calls.iter().any(|call| call == "retrieve_rag_context"));
        assert!(!response.undoable);
        assert_eq!(response.tool_results.len(), 1);
    }

    #[tokio::test]
    async fn workflow_question_uses_rag_and_emits_tokens() {
        let provider = Arc::new(MockProvider {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from(vec![
                ToolExecutionResultDto {
                    name: "scene_current".to_string(),
                    ok: true,
                    summary: "No scene".to_string(),
                    payload: json!({
                        "connected": false,
                        "scene_data": null,
                        "message": "No scene data",
                        "last_update": null
                    }),
                },
                ToolExecutionResultDto {
                    name: "retrieve_rag_context".to_string(),
                    ok: true,
                    summary: "Retrieved 1 Blender reference context(s).".to_string(),
                    payload: json!({
                        "contexts": [{
                            "text": "Use Add Modifier > Bevel",
                            "signature": "bpy.types.BevelModifier",
                            "url": "/bpy.types.BevelModifier.html",
                            "similarity": 0.9
                        }],
                        "ragEnabled": true
                    }),
                },
            ])),
        });
        let state = AssistantState::default();
        let events = Arc::new(Mutex::new(Vec::<AssistantStreamEventDto>::new()));

        let response = execute_blender_request(
            provider,
            &MockStreamer,
            &request("How do I add a bevel to the selected object?"),
            &state,
            {
                let events = Arc::clone(&events);
                move |event| events.lock().expect("events lock").push(event)
            },
        )
        .await
        .expect("blender response");

        assert_eq!(response.tool_results.len(), 2);
        assert_eq!(response.plan.expect("plan")["ragUsed"], true);
        assert!(events.lock().expect("events").iter().any(|event| matches!(
            event,
            AssistantStreamEventDto::Token { token } if token == "Blender "
        )));
    }

    #[tokio::test]
    async fn request_returns_cancelled_when_generation_is_stopped() {
        let provider = Arc::new(MockProvider {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from(vec![ToolExecutionResultDto {
                name: "scene_current".to_string(),
                ok: true,
                summary: "No scene".to_string(),
                payload: json!({
                    "connected": false,
                    "scene_data": null,
                    "message": "No scene data",
                    "last_update": null
                }),
            }])),
        });
        let state = Arc::new(AssistantState::default());

        let task = tokio::spawn({
            let provider = provider.clone();
            let state = state.clone();
            async move {
                execute_blender_request(
                    provider,
                    &BlockingStreamer,
                    &request("What is in my scene right now?"),
                    state.as_ref(),
                    |_| {},
                )
                .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        state.mark_cancelled();

        let error = task
            .await
            .expect("join handle")
            .expect_err("request cancelled");
        assert_eq!(error, "ASSISTANT_CANCELLED");
    }
}
