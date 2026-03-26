use serde_json::Value;
use smolpc_assistant_types::{AppMode, AssistantResponseDto, ToolExecutionResultDto};

pub fn payload_is_error(payload: &Value) -> bool {
    payload
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || payload
            .get("content")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|first| first.get("text"))
            .and_then(Value::as_str)
            .map(|text| text.starts_with("Error:"))
            .unwrap_or(false)
}

pub fn extract_primary_tool_text(payload: &Value) -> Option<String> {
    payload
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            payload
                .get("structuredContent")
                .and_then(|content| content.get("result"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

pub fn summarize_tool_result(name: &str, payload: &Value) -> String {
    if let Some(text) = extract_primary_tool_text(payload) {
        return text;
    }

    if payload_is_error(payload) {
        return format!("{name} reported an error");
    }

    format!("{name} completed")
}

pub fn build_tool_execution_result(name: &str, payload: Value) -> ToolExecutionResultDto {
    let ok = !payload_is_error(&payload);
    let summary = summarize_tool_result(name, &payload);

    ToolExecutionResultDto {
        name: name.to_string(),
        ok,
        summary,
        payload,
    }
}

pub fn build_local_fallback_summary(tool_result: &ToolExecutionResultDto) -> String {
    let text = extract_primary_tool_text(&tool_result.payload).unwrap_or_default();
    if text.trim().is_empty() {
        return if tool_result.summary.trim().is_empty() {
            format!("{} completed.", tool_result.name)
        } else {
            tool_result.summary.clone()
        };
    }

    let count_match = text
        .to_ascii_lowercase()
        .find("found ")
        .and_then(|_| regex_capture_document_count(&text));
    let name_matches = extract_named_entries(&text);

    if let Some(doc_count) = count_match {
        if !name_matches.is_empty() {
            return format!(
                "Found {} document(s). Example files: {}.",
                doc_count,
                name_matches.join(", ")
            );
        }

        return format!("Found {doc_count} document(s).");
    }

    if text.len() <= 280 {
        text
    } else {
        format!("{}...", text.chars().take(280).collect::<String>())
    }
}

pub fn build_libreoffice_response(
    mode: AppMode,
    reply: String,
    tool_name: Option<&str>,
    used_local_fallback: bool,
    tool_results: Vec<ToolExecutionResultDto>,
) -> AssistantResponseDto {
    let mode_name = match mode {
        AppMode::Writer => "writer",
        AppMode::Impress => "impress",
        _ => "unknown",
    };

    AssistantResponseDto {
        reply,
        explain: None,
        undoable: false,
        plan: Some(serde_json::json!({
            "mode": mode_name,
            "operation": if tool_name.is_some() { "tool_call_with_summary" } else { "direct_answer" },
            "tool": tool_name,
            "usedLocalFallbackSummary": used_local_fallback,
            "toolCallCount": tool_results.len(),
        })),
        tool_results,
    }
}

fn regex_capture_document_count(text: &str) -> Option<usize> {
    let mut number = String::new();
    let lower = text.to_ascii_lowercase();
    let start = lower.find("found ")? + "found ".len();
    for ch in lower[start..].chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
        } else if !number.is_empty() {
            break;
        } else if ch != ' ' {
            return None;
        }
    }
    number.parse::<usize>().ok()
}

fn extract_named_entries(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split_once("Name:"))
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .take(3)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{build_libreoffice_response, build_local_fallback_summary, payload_is_error};
    use serde_json::json;
    use smolpc_assistant_types::{AppMode, ToolExecutionResultDto};

    #[test]
    fn fallback_summary_uses_document_count_and_names() {
        let summary = build_local_fallback_summary(&ToolExecutionResultDto {
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
        });

        assert!(summary.contains("Found 2 document(s)."));
        assert!(summary.contains("alpha.odt"));
    }

    #[test]
    fn error_payload_marks_result_as_error() {
        assert!(payload_is_error(&json!({
            "content": [{ "text": "Error: missing helper" }]
        })));
    }

    #[test]
    fn libreoffice_response_marks_messages_non_undoable() {
        let response = build_libreoffice_response(
            AppMode::Writer,
            "Writer reply".to_string(),
            Some("add_heading"),
            false,
            Vec::new(),
        );

        assert!(!response.undoable);
        assert_eq!(response.plan.expect("plan")["mode"], "writer");
    }
}
