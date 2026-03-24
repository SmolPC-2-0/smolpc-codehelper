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

/// Strip a multi-line MCP tool description to just the first non-empty line.
/// This removes the redundant `Args:` section that duplicates `inputSchema`,
/// keeping the planner prompt compact for the small local LLM.
fn first_line_description(description: &str) -> &str {
    description
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(description.trim())
}

/// Strip optional properties from a JSON Schema `inputSchema` so the planner
/// prompt only shows required parameters.  The small LLM struggles with many
/// optional empty-string fields and often garbles the JSON syntax.
fn strip_optional_properties(schema: &Value) -> Value {
    let Some(obj) = schema.as_object() else {
        return schema.clone();
    };
    let required: Vec<String> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    if required.is_empty() {
        return schema.clone();
    }

    let mut out = obj.clone();
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        let filtered: serde_json::Map<String, Value> = props
            .iter()
            .filter(|(key, _)| required.contains(key))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        out.insert("properties".to_string(), Value::Object(filtered));
    }
    Value::Object(out)
}

/// Keyword-based tool pre-filter. Small models can't reliably pick from 18+
/// tools — narrow the catalog to the best matches so the LLM only chooses
/// between 1-4 tools.  Falls back to the full catalog when nothing matches.
fn filter_tools_by_intent<'a>(
    user_text: &str,
    tools: &'a [ToolDefinitionDto],
) -> Vec<&'a ToolDefinitionDto> {
    let lower = user_text.to_ascii_lowercase();

    // Keyword → tool name mapping, checked in order (first match wins).
    let routing: &[(&[&str], &[&str])] = &[
        (
            &["open", "launch", "view in libre", "view in office"],
            &["open_in_libreoffice"],
        ),
        (
            &["list", "show all", "what documents", "what files"],
            &["list_documents"],
        ),
        (
            &["read", "show content", "what does it say", "contents of"],
            &["read_text_document", "read_presentation"],
        ),
        (
            &["properties", "info", "metadata", "word count", "stats"],
            &["get_document_properties"],
        ),
        (
            &["copy", "duplicate"],
            &["copy_document"],
        ),
        (
            &["create", "new", "blank", "make"],
            &["create_blank_document", "create_blank_presentation"],
        ),
        (
            &["heading"],
            &["add_heading"],
        ),
        (
            &["table"],
            &["add_table", "format_table"],
        ),
        (
            &["slide"],
            &["add_slide", "edit_slide_content", "edit_slide_title", "delete_slide"],
        ),
        (
            &["image", "picture", "photo"],
            &["insert_image", "insert_slide_image"],
        ),
        (
            &["page break"],
            &["insert_page_break"],
        ),
        (
            &["format", "bold", "italic", "font", "style", "color"],
            &[
                "format_text",
                "format_table",
                "format_slide_content",
                "format_slide_title",
                "apply_document_style",
                "apply_presentation_template",
            ],
        ),
        (
            &["add text", "add the text", "add paragraph", "add a paragraph", "append text"],
            &["add_text", "add_paragraph"],
        ),
        (
            &["search and replace", "find and replace", "replace all", "search for"],
            &["search_replace_text"],
        ),
        (
            &["delete", "remove"],
            &["delete_text", "delete_paragraph", "delete_slide"],
        ),
        (
            &["paragraph", "text"],
            &["add_paragraph", "add_text"],
        ),
    ];

    for (keywords, tool_names) in routing {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            let matched: Vec<&ToolDefinitionDto> = tools
                .iter()
                .filter(|t| tool_names.contains(&t.name.as_str()))
                .collect();
            if !matched.is_empty() {
                return matched;
            }
        }
    }

    // No keyword match — return all tools.
    tools.iter().collect()
}

fn build_planner_messages(
    mode: AppMode,
    request: &AssistantSendRequestDto,
    tools: &[ToolDefinitionDto],
) -> Vec<EngineChatMessage> {
    let label = libreoffice_profile(mode)
        .map(|profile| profile.label)
        .unwrap_or("LibreOffice");

    // Pre-filter the tool catalog to 1-4 tools based on user intent keywords.
    // The small LLM can't reliably pick from 18 tools but handles 1-4 well.
    let filtered = filter_tools_by_intent(&request.user_text, tools);
    let tool_catalog = filtered
        .iter()
        .map(|tool| {
            format!(
                "- {}: {} inputSchema: {}",
                tool.name,
                first_line_description(&tool.description),
                strip_optional_properties(&tool.input_schema)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_prompt = format!(
        "You are the LibreOffice {} assistant.\n\
Reply with a JSON tool call. Use this shape:\n\
{{\"tool_call\":{{\"name\":\"TOOL_NAME\",\"arguments\":{{...}}}}}}\n\
No markdown. No natural language. JSON only.\n\
IMPORTANT: The tool name MUST be exactly one of the tool names listed below. \
Never use a filename, document title, or any other string as the tool name.\n\
For file_path arguments: pass the filename exactly as the user wrote it. \
Do NOT construct absolute paths yourself. \
If the user message ends with a (Context: ...) note, copy that path verbatim.\n\
Available tools:\n{}",
        label, tool_catalog
    );

    let mut messages = vec![EngineChatMessage {
        role: "system".to_string(),
        content: system_prompt,
    }];

    // Only send the current user message to the planner — conversation history
    // contains natural-language summaries that prime the small model to respond
    // in natural language instead of JSON.  File references like "that document"
    // are handled by scanning the history for the most recent file path.
    let user_content = enrich_user_text_with_file_context(
        &request.user_text,
        &request.messages,
    );
    messages.push(EngineChatMessage {
        role: "user".to_string(),
        content: user_content,
    });
    messages
}

/// If the user message references a file implicitly ("it", "that document",
/// "the document", or a bare filename without a path), scan the conversation
/// history for the most recently mentioned absolute file path and append it
/// as context so the planner can fill in the `file_path` argument.
fn enrich_user_text_with_file_context(
    user_text: &str,
    history: &[smolpc_assistant_types::AssistantMessageDto],
) -> String {
    // Only enrich if the user didn't already provide a full path.
    if user_text.contains('\\') || user_text.contains('/') {
        return user_text.to_string();
    }

    // Scan history (newest first) for an absolute file path.
    let path = history
        .iter()
        .rev()
        .filter(|m| m.role == "assistant")
        .find_map(|m| extract_file_path_from_text(&m.content));

    // Also check user messages — the path may have been mentioned by the user
    // (e.g., "Create a document called X.docx") and echoed back in the
    // assistant reply with the full resolved path.
    let path = path.or_else(|| {
        history
            .iter()
            .rev()
            .filter(|m| m.role == "user")
            .find_map(|m| extract_file_path_from_text(&m.content))
    });

    match path {
        Some(p) => format!("{user_text}\n(Context: the current file is {p})"),
        // No context path found — return user text unchanged.
        // The MCP server handles bare filenames by resolving them to Documents.
        None => user_text.to_string(),
    }
}

fn extract_file_path_from_text(text: &str) -> Option<String> {
    // Document extensions we recognise (lowercase, with dot).
    const EXTENSIONS: &[&str] = &[
        ".odt", ".docx", ".doc", ".rtf", ".odp", ".pptx", ".ppt", ".ods",
        ".xlsx", ".xls", ".pdf", ".txt", ".csv",
    ];

    // Scan for Windows absolute paths like `C:\...\file.ext`.  Paths may
    // contain spaces (e.g. OneDrive folders), so we can't split on whitespace.
    // Strategy: find each `X:\` drive-letter start, then scan forward to the
    // first known document extension and take the whole substring.
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i + 2 < len {
        // Look for a drive letter followed by `:\`
        if chars[i].is_ascii_alphabetic()
            && chars[i + 1] == ':'
            && chars[i + 2] == '\\'
        {
            // Find the end: scan forward for a known extension.
            let start = i;
            let rest = &text[start..];
            let lower = rest.to_ascii_lowercase();
            if let Some((ext_end, _ext)) = EXTENSIONS
                .iter()
                .filter_map(|ext| lower.find(ext).map(|pos| (pos + ext.len(), *ext)))
                .min_by_key(|(pos, _)| *pos)
            {
                let path = rest[..ext_end].trim_end_matches(|c: char| {
                    c == '.' || c == ',' || c == ';' || c == ')' || c == '"'
                });
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
            // No extension found — skip past this drive letter.
            i += 3;
        } else {
            i += 1;
        }
    }

    // Fallback: check for Unix absolute paths (simple case, no spaces).
    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| c == '.' || c == ',' || c == ':' || c == '"');
        if cleaned.starts_with('/') && cleaned.len() > 3 {
            return Some(cleaned.to_string());
        }
    }

    None
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
                "You are the unified LibreOffice {label} assistant. A document tool has already run successfully. Summarize the result for the user in 2 or 3 short sentences. Plain text only. Do not suggest replaying the action. Do not mention undo."
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

/// Find the substring from the first `{` to the matching closing `}` by
/// tracking brace depth.  Small LLMs sometimes emit trailing garbage (extra
/// `}`, whitespace, or stray characters) after otherwise valid JSON — this
/// recovers the balanced object without relying on serde accepting the whole
/// input.
fn extract_balanced_braces(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;
    for (i, &byte) in bytes.iter().enumerate().skip(start) {
        if escape_next {
            escape_next = false;
            continue;
        }
        match byte {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => depth += 1,
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_json_candidates(raw_text: &str) -> Vec<String> {
    let trimmed = raw_text.trim();
    let mut candidates = Vec::new();

    // First try the full trimmed text (handles the common case cleanly).
    if !trimmed.is_empty() {
        candidates.push(trimmed.to_string());
    }

    // Then try the balanced-brace substring to handle trailing garbage.
    if let Some(balanced) = extract_balanced_braces(trimmed) {
        if balanced != trimmed {
            candidates.push(balanced.to_string());
        }
    }

    // Finally extract from markdown code fences.
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
                // Also try balanced extraction within the fence.
                if let Some(balanced) = extract_balanced_braces(candidate) {
                    if balanced != candidate {
                        candidates.push(balanced.to_string());
                    }
                }
            }
            remaining = &remaining[end + 3..];
        } else {
            break;
        }
    }

    candidates
}

/// Attempt light repairs on common small-LLM JSON mistakes.
/// e.g. `"title","value"` → `"title":"value"` (comma used instead of colon
/// between key and value).
fn repair_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        // Inside a string literal, copy verbatim until closing quote.
        if chars[i] == '"' {
            out.push('"');
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            if i < len {
                out.push('"'); // closing quote
                i += 1;
                // If the next non-whitespace char is `"` but we expect `:`,
                // the LLM used `,"` or just `"` where `:` was needed.
                let mut j = i;
                while j < len && chars[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < len && chars[j] == ',' {
                    // Peek past the comma — if the next token is `"`, this
                    // is likely a key,value pair with comma instead of colon.
                    let mut k = j + 1;
                    while k < len && chars[k].is_ascii_whitespace() {
                        k += 1;
                    }
                    if k < len && chars[k] == '"' {
                        // Check if this looks like a key-value pair: the value
                        // after the comma-separated quote should be followed by
                        // another comma or `}`.  Heuristic: if the string after
                        // the comma is short (empty or short value) and followed
                        // by `,` or `}`, treat this as a `:` replacement.
                        out.push(':');
                        i = j + 1; // skip the comma
                        continue;
                    }
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

fn extract_tool_call(raw_text: &str) -> Option<ToolCall> {
    for candidate in extract_json_candidates(raw_text) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&candidate) {
            if let Some(tool_call) = normalize_tool_calls(&parsed).into_iter().next() {
                return Some(tool_call);
            }
        }
    }

    // Last resort: try repairing common JSON mistakes (e.g. comma-for-colon).
    let repaired = repair_json(raw_text);
    if repaired != raw_text {
        for candidate in extract_json_candidates(&repaired) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&candidate) {
                if let Some(tool_call) = normalize_tool_calls(&parsed).into_iter().next() {
                    return Some(tool_call);
                }
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
    // If the extracted tool name isn't in the allowlist, try to recover:
    // the small LLM sometimes emits a mangled filename instead of a tool name.
    // Attempt a case-insensitive prefix/substring match against allowed tools.
    let tool_call = if allowlist.iter().any(|name| *name == tool_call.name) {
        tool_call
    } else {
        let lower_name = tool_call.name.to_ascii_lowercase();
        let fuzzy_match = allowlist.iter().find(|allowed| {
            let lower_allowed = allowed.to_ascii_lowercase();
            lower_allowed.starts_with(&lower_name) || lower_name.starts_with(&lower_allowed)
        });
        match fuzzy_match {
            Some(matched) => {
                log::warn!(
                    "Planner emitted '{}' which is not a valid tool; \
                     fuzzy-matched to '{}'",
                    tool_call.name,
                    matched
                );
                ToolCall {
                    name: matched.to_string(),
                    arguments: tool_call.arguments,
                }
            }
            None => {
                return Err(format!(
                    "I couldn't choose a safe {} action for that request. \
                     Try rephrasing as a single supported step.",
                    profile.label
                ));
            }
        }
    };

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

    #[test]
    fn extract_tool_call_recovers_from_trailing_brace_garbage() {
        // Exact failure observed in E2E: LLM emits an extra closing brace.
        let raw = r#"{"tool_call":{"name":"create_blank_presentation","arguments":{"filename":"demo-pitch.odp","author":"","keywords":"","subject":"","title":""}}}}"#;
        let tool_call = extract_tool_call(raw).expect("should recover from trailing brace");
        assert_eq!(tool_call.name, "create_blank_presentation");
        assert_eq!(tool_call.arguments["filename"], json!("demo-pitch.odp"));
    }

    #[test]
    fn extract_tool_call_recovers_from_trailing_whitespace_and_text() {
        let raw = r#"{"tool_call":{"name":"add_heading","arguments":{"text":"Hello"}}}
I hope this helps!"#;
        let tool_call = extract_tool_call(raw).expect("should recover from trailing text");
        assert_eq!(tool_call.name, "add_heading");
        assert_eq!(tool_call.arguments["text"], json!("Hello"));
    }

    #[test]
    fn extract_tool_call_repairs_comma_used_as_colon() {
        // Exact failure from E2E: LLM used commas instead of colons for some
        // key-value pairs in the arguments object.
        let raw = r#"{"tool_call":{"name":"create_blank_presentation","arguments":{"filename":"test.odp","author":"","title","","keywords","","subject":""}}}"#;
        let tool_call = extract_tool_call(raw).expect("should repair comma-for-colon");
        assert_eq!(tool_call.name, "create_blank_presentation");
        assert_eq!(tool_call.arguments["filename"], json!("test.odp"));
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
