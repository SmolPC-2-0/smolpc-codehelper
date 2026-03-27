use super::rag::RagContext;
use super::state::SceneSnapshot;
use smolpc_assistant_types::{AssistantResponseDto, ToolExecutionResultDto};

pub fn parse_scene_snapshot(payload: &serde_json::Value) -> Option<SceneSnapshot> {
    match serde_json::from_value(payload.clone()) {
        Ok(snapshot) => Some(snapshot),
        Err(error) => {
            log::warn!("Failed to parse scene snapshot: {error}");
            None
        }
    }
}

pub fn parse_rag_contexts(payload: &serde_json::Value) -> Vec<RagContext> {
    payload
        .get("contexts")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .map(|contexts| {
            contexts
                .into_iter()
                .filter_map(|context| serde_json::from_value(context).ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn build_blender_response(
    reply: String,
    scene_available: bool,
    rag_used: bool,
    contexts_used: usize,
    tool_results: Vec<ToolExecutionResultDto>,
) -> AssistantResponseDto {
    AssistantResponseDto {
        reply,
        explain: None,
        undoable: false,
        plan: Some(serde_json::json!({
            "mode": "blender",
            "operation": "scene_aware_tutoring",
            "sceneAvailable": scene_available,
            "ragUsed": rag_used,
            "contextsUsed": contexts_used,
        })),
        tool_results,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_blender_response, parse_rag_contexts};
    use serde_json::json;

    #[test]
    fn build_response_marks_blender_messages_non_undoable() {
        let response = build_blender_response("answer".to_string(), true, true, 2, Vec::new());
        assert!(!response.undoable);
        assert_eq!(response.plan.expect("plan")["mode"], "blender");
    }

    #[test]
    fn parse_rag_contexts_returns_context_list() {
        let contexts = parse_rag_contexts(&json!({
            "contexts": [
                {
                    "text": "context",
                    "signature": "sig",
                    "url": "/sig",
                    "similarity": 0.8
                }
            ]
        }));

        assert_eq!(contexts.len(), 1);
        assert_eq!(contexts[0].signature, "sig");
    }
}
