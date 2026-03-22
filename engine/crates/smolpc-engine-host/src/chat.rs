use smolpc_engine_core::inference::types::InferenceChatMessage;
use smolpc_engine_core::GenerationConfig;

use crate::openvino::openvino_model_tuning_for_model;
use crate::types::*;

/// Returns true for model families that default to "thinking" mode (e.g. Qwen3).
pub(crate) fn model_has_thinking_mode(model_id: &str) -> bool {
    model_id.starts_with("qwen3")
}

/// Streaming filter that strips `<think>...</think>` blocks from generated text.
///
/// Qwen3 models may emit chain-of-thought reasoning wrapped in these tags even when
/// `/nothink` is present in the prompt.  This filter acts as a safety net so that
/// thinking output never reaches the user.
pub(crate) struct ThinkingFilter {
    buf: String,
    inside_think: bool,
    /// After `</think>`, skip one leading newline (it may arrive in a later token).
    skip_newline: bool,
}

impl ThinkingFilter {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::new(),
            inside_think: false,
            skip_newline: false,
        }
    }

    /// Feed a new token into the filter.  Returns any text that should be emitted
    /// to the user, or `None` if the token was suppressed / buffered.
    pub(crate) fn push(&mut self, token: &str) -> Option<String> {
        self.buf.push_str(token);

        // Strip a deferred newline left over from a previous </think>.
        if self.skip_newline && self.buf.starts_with('\n') {
            self.buf = self.buf[1..].to_string();
            self.skip_newline = false;
        } else if self.skip_newline && !self.buf.is_empty() {
            // Next char isn't '\n' -- stop waiting.
            self.skip_newline = false;
        }

        let mut output = String::new();

        loop {
            if self.inside_think {
                if let Some(end) = self.buf.find("</think>") {
                    let rest = end + "</think>".len();
                    self.buf = self.buf[rest..].to_string();
                    self.inside_think = false;
                    // Consume trailing newline if present; otherwise defer to next push.
                    if self.buf.starts_with('\n') {
                        self.buf = self.buf[1..].to_string();
                    } else {
                        self.skip_newline = true;
                    }
                    continue;
                }
                // Buffer might end with a partial "</think>" match -- keep it.
                let keep = partial_tag_suffix_len(&self.buf, "</think>");
                self.buf = self.buf[self.buf.len() - keep..].to_string();
                break;
            } else {
                if let Some(start) = self.buf.find("<think>") {
                    output.push_str(&self.buf[..start]);
                    self.buf = self.buf[start + "<think>".len()..].to_string();
                    self.inside_think = true;
                    continue;
                }
                // Emit everything except a potential partial "<think>" at the tail.
                let keep = partial_tag_suffix_len(&self.buf, "<think>");
                let safe = self.buf.len() - keep;
                if safe > 0 {
                    output.push_str(&self.buf[..safe]);
                    self.buf = self.buf[safe..].to_string();
                }
                break;
            }
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    /// Flush any remaining buffered text at end-of-stream.
    pub(crate) fn finish(&mut self) -> Option<String> {
        if self.inside_think || self.buf.is_empty() {
            self.buf.clear();
            return None;
        }
        Some(std::mem::take(&mut self.buf))
    }
}

/// Returns the length of the longest suffix of `haystack` that is a prefix of `needle`.
pub(crate) fn partial_tag_suffix_len(haystack: &str, needle: &str) -> usize {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    let max = n.len().min(h.len());
    for suffix_len in (1..=max).rev() {
        if h.ends_with(&n[..suffix_len]) {
            return suffix_len;
        }
    }
    0
}

pub(crate) fn looks_like_chatml_prompt(content: &str) -> bool {
    content.contains("<|im_start|>") && content.contains("<|im_end|>")
}

pub(crate) fn is_preformatted_chatml_single_user_message(
    messages: &[ChatCompletionMessage],
) -> bool {
    if messages.len() != 1 {
        return false;
    }
    let only = &messages[0];
    if !only.role.trim().eq_ignore_ascii_case("user") {
        return false;
    }
    let content = only.content.clone().unwrap_or_default();
    !content.trim().is_empty() && looks_like_chatml_prompt(&content)
}

pub(crate) fn request_to_prompt(
    messages: &[ChatCompletionMessage],
    disable_thinking: bool,
) -> Result<String, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }

    // Compatibility mode: older clients may already send a full ChatML prompt
    // as a single user message. Preserve that payload as-is.
    if messages.len() == 1 {
        let only = &messages[0];
        if only.role.trim().eq_ignore_ascii_case("user") {
            let content = only.content.clone().unwrap_or_default();
            if !content.trim().is_empty() && looks_like_chatml_prompt(&content) {
                return Ok(content);
            }
        }
    }

    let mut prompt = String::new();
    let mut system_seen = false;
    for m in messages {
        let content = m.content.clone().unwrap_or_default();
        if !content.is_empty() {
            let role = match m.role.trim().to_ascii_lowercase().as_str() {
                "system" => "system",
                "user" => "user",
                "assistant" => "assistant",
                other => return Err(format!("unsupported message role: {other}")),
            };
            prompt.push_str("<|im_start|>");
            prompt.push_str(role);
            prompt.push('\n');
            prompt.push_str(&content);
            if disable_thinking && role == "system" && !system_seen {
                prompt.push_str("\n/nothink");
                system_seen = true;
            }
            prompt.push_str("<|im_end|>\n");
        }
    }

    // If thinking should be disabled but no system message was present,
    // prepend a minimal system message with the /nothink directive.
    if disable_thinking && !system_seen {
        let rest = prompt.clone();
        prompt.clear();
        prompt.push_str("<|im_start|>system\n/nothink<|im_end|>\n");
        prompt.push_str(&rest);
    }

    if prompt.is_empty() {
        return Err("messages must contain at least one non-empty content item".to_string());
    }

    prompt.push_str("<|im_start|>assistant\n");
    Ok(prompt)
}

pub(crate) fn request_to_structured_messages(
    messages: &[ChatCompletionMessage],
) -> Result<Vec<InferenceChatMessage>, String> {
    if messages.is_empty() {
        return Err("messages cannot be empty".to_string());
    }

    let mut out = Vec::new();
    for message in messages {
        let content = message.content.clone().unwrap_or_default();
        if content.is_empty() {
            continue;
        }

        let role = match message.role.trim().to_ascii_lowercase().as_str() {
            "system" => "system",
            "user" => "user",
            "assistant" => "assistant",
            other => return Err(format!("unsupported message role: {other}")),
        };
        out.push(InferenceChatMessage {
            role: role.to_string(),
            content,
        });
    }

    if out.is_empty() {
        return Err("messages must contain at least one non-empty content item".to_string());
    }
    Ok(out)
}

pub(crate) fn max_tokens_hard_cap() -> usize {
    std::env::var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT)
}

pub(crate) fn openvino_request_defaults(
    model_id: Option<&str>,
    runtime_is_openvino_genai: bool,
) -> Option<GenerationConfig> {
    if !runtime_is_openvino_genai {
        return None;
    }

    model_id
        .map(openvino_model_tuning_for_model)
        .and_then(|tuning| tuning.request_defaults)
}

pub(crate) fn should_use_openvino_structured_messages(
    runtime_is_openvino_genai: bool,
    use_legacy_prompt: bool,
) -> bool {
    runtime_is_openvino_genai && !use_legacy_prompt
}

pub(crate) fn request_to_config(
    request: &ChatCompletionRequest,
    default_config: Option<GenerationConfig>,
) -> Result<Option<GenerationConfig>, String> {
    let mut changed = default_config.is_some();
    let mut c = default_config.unwrap_or_default();
    if let Some(v) = request.max_tokens {
        if v == 0 {
            return Err("max_tokens must be greater than zero".to_string());
        }
        let hard_cap = max_tokens_hard_cap();
        c.max_length = v.min(hard_cap);
        if v > hard_cap {
            log::info!("Capping max_tokens from {v} to backend hard cap {hard_cap}");
        }
        changed = true;
    }
    if let Some(v) = request.temperature {
        c.temperature = v;
        changed = true;
    }
    if let Some(v) = request.top_k {
        c.top_k = Some(v);
        changed = true;
    }
    if let Some(v) = request.top_p {
        c.top_p = Some(v);
        changed = true;
    }
    if let Some(v) = request.repetition_penalty {
        c.repetition_penalty = v;
        changed = true;
    }
    if let Some(v) = request.repetition_penalty_last_n {
        c.repetition_penalty_last_n = v;
        changed = true;
    }
    if changed {
        Ok(Some(c))
    } else {
        Ok(None)
    }
}

pub(crate) fn stream_error_code(error: &str) -> &'static str {
    if error.contains("INFERENCE_GENERATION_CANCELLED") {
        "INFERENCE_GENERATION_CANCELLED"
    } else {
        "ENGINE_STREAM_ERROR"
    }
}
