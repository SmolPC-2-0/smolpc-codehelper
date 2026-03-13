use crate::engine_integration;
use crate::ollama;
use crate::prompts::{build_question_prompts, build_scene_analysis_prompts};
use crate::rag::types::RagContext;
use crate::state::{BackendState, GenerationBackend, SceneData};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::State;

const MAX_QUESTION_LEN: usize = 10_000;
const MAX_GOAL_LEN: usize = 500;
const DEFAULT_N_RESULTS: usize = 3;
const DEFAULT_TEMPERATURE: f64 = 0.7;
const RETRY_TEMPERATURE: f64 = 0.35;
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

#[derive(Debug, Clone, Deserialize)]
pub struct AskRequest {
    pub question: String,
    pub scene_context: Option<SceneData>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskResponse {
    pub answer: String,
    pub contexts_used: usize,
    pub rag_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneAnalysisRequest {
    pub goal: Option<String>,
    #[serde(default, alias = "scene_data")]
    pub scene_context: Option<SceneData>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneAnalysisResponse {
    pub suggestions: Vec<String>,
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagRetrieveResponse {
    pub contexts: Vec<RagContext>,
    pub rag_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantStatusResponse {
    pub status: String,
    pub connected: bool,
    pub backend: String,
    pub model: String,
    pub generating: bool,
    pub rag_enabled: bool,
    pub rag_docs: usize,
    pub rag_error: Option<String>,
}

pub fn resolve_scene_context(
    state: &BackendState,
    scene_context: Option<SceneData>,
) -> Option<SceneData> {
    if scene_context.is_some() {
        return scene_context;
    }

    match state.scene_cache.lock() {
        Ok(cache) => cache.latest_scene(),
        Err(poisoned) => poisoned.into_inner().latest_scene(),
    }
}

pub fn retrieve_contexts(
    state: &BackendState,
    query: &str,
    n_results: usize,
) -> Result<RagRetrieveResponse, String> {
    let guard = match state.rag_index.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if !guard.is_loaded() {
        return Ok(RagRetrieveResponse {
            contexts: Vec::new(),
            rag_enabled: false,
        });
    }

    let contexts = guard.retrieve_context(query, n_results)?;
    Ok(RagRetrieveResponse {
        contexts,
        rag_enabled: true,
    })
}

fn rag_is_loaded(state: &BackendState) -> bool {
    match state.rag_index.lock() {
        Ok(index) => index.is_loaded(),
        Err(poisoned) => poisoned.into_inner().is_loaded(),
    }
}

fn should_skip_rag_for_scene_query(question: &str) -> bool {
    let normalized = question.trim().to_ascii_lowercase();
    SCENE_QUERY_HINTS
        .iter()
        .any(|needle| normalized.contains(needle))
}

pub fn retrieve_contexts_for_question(
    state: &BackendState,
    question: &str,
    n_results: usize,
) -> Result<RagRetrieveResponse, String> {
    if should_skip_rag_for_scene_query(question) {
        return Ok(RagRetrieveResponse {
            contexts: Vec::new(),
            rag_enabled: rag_is_loaded(state),
        });
    }

    retrieve_contexts(state, question, n_results)
}

pub async fn ask_internal(
    state: &BackendState,
    request: AskRequest,
) -> Result<AskResponse, String> {
    let question = request.question.trim();
    if question.is_empty() {
        return Err("No question provided".to_string());
    }
    if question.len() > MAX_QUESTION_LEN {
        return Err("Question too long (max 10,000 characters)".to_string());
    }

    let scene_context = resolve_scene_context(state, request.scene_context);
    let rag = retrieve_contexts_for_question(state, question, DEFAULT_N_RESULTS)?;
    let (system_prompt, user_prompt) =
        build_question_prompts(question, scene_context.as_ref(), &rag.contexts);
    let model_override = request.model.clone();
    let mut contexts_used = rag.contexts.len();
    let mut answer = run_chat_completion(
        state,
        &system_prompt,
        &user_prompt,
        model_override.clone(),
        DEFAULT_TEMPERATURE,
        "ask",
    )
    .await?;

    if is_degenerate_response(&answer) {
        log::warn!(
            "[Assistant] Detected low-quality repetitive answer; retrying without RAG context"
        );
        let (retry_system_prompt, retry_user_prompt) =
            build_question_prompts(question, scene_context.as_ref(), &[]);

        match run_chat_completion(
            state,
            &retry_system_prompt,
            &retry_user_prompt,
            model_override.clone(),
            RETRY_TEMPERATURE,
            "ask retry",
        )
        .await
        {
            Ok(retry_answer) => {
                answer = retry_answer;
                contexts_used = 0;
            }
            Err(err) => {
                log::warn!(
                    "[Assistant] Retry after repetitive answer failed; returning original response: {}",
                    err
                );
            }
        }
    }

    Ok(AskResponse {
        answer,
        contexts_used,
        rag_enabled: rag.rag_enabled,
    })
}

pub async fn analyze_scene_internal(
    state: &BackendState,
    request: SceneAnalysisRequest,
) -> Result<SceneAnalysisResponse, String> {
    let goal = request
        .goal
        .unwrap_or_else(|| "learning blender".to_string())
        .trim()
        .to_string();
    if goal.len() > MAX_GOAL_LEN {
        return Err("Goal too long (max 500 characters)".to_string());
    }

    let scene_context = resolve_scene_context(state, request.scene_context)
        .ok_or_else(|| "No scene context available".to_string())?;

    let (system_prompt, user_prompt) = build_scene_analysis_prompts(&scene_context, &goal);
    let model_override = request.model.clone();
    let response = run_chat_completion(
        state,
        &system_prompt,
        &user_prompt,
        model_override,
        DEFAULT_TEMPERATURE,
        "scene analysis",
    )
    .await?;
    let suggestions = parse_suggestions(&response);

    Ok(SceneAnalysisResponse {
        suggestions,
        analysis: response,
    })
}

pub async fn assistant_status_internal(
    state: &BackendState,
    generating: bool,
) -> AssistantStatusResponse {
    let (rag_enabled, rag_docs, rag_error) = match state.rag_index.lock() {
        Ok(index) => (
            index.is_loaded(),
            index.document_count(),
            index.load_error().map(|s| s.to_string()),
        ),
        Err(poisoned) => {
            let index = poisoned.into_inner();
            (
                index.is_loaded(),
                index.document_count(),
                index.load_error().map(|s| s.to_string()),
            )
        }
    };

    let backend = state.get_generation_backend();
    let (connected, model) = match backend {
        GenerationBackend::Ollama => (
            ollama::is_ollama_available().await,
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| ollama::DEFAULT_MODEL.to_string()),
        ),
        GenerationBackend::SharedEngine => match engine_integration::engine_status().await {
            Ok(info) => (
                info.connected,
                info.current_model
                    .or_else(|| state.get_loaded_model_id())
                    .unwrap_or_else(|| "shared_engine".to_string()),
            ),
            Err(err) => {
                log::debug!("[SharedEngine] Status check failed: {}", err);
                (
                    false,
                    state
                        .get_loaded_model_id()
                        .unwrap_or_else(|| "shared_engine (disconnected)".to_string()),
                )
            }
        },
    };

    AssistantStatusResponse {
        status: if connected { "ok" } else { "error" }.to_string(),
        connected,
        backend: backend.as_str().to_string(),
        model,
        generating,
        rag_enabled,
        rag_docs,
        rag_error,
    }
}

#[tauri::command]
pub async fn assistant_ask(
    request: AskRequest,
    state: State<'_, BackendState>,
) -> Result<AskResponse, String> {
    ask_internal(&state, request).await
}

#[tauri::command]
pub async fn assistant_analyze_scene(
    request: SceneAnalysisRequest,
    state: State<'_, BackendState>,
) -> Result<SceneAnalysisResponse, String> {
    analyze_scene_internal(&state, request).await
}

#[tauri::command]
pub async fn retrieve_rag_context(
    query: String,
    n_results: Option<usize>,
    state: State<'_, BackendState>,
) -> Result<RagRetrieveResponse, String> {
    retrieve_contexts(&state, &query, n_results.unwrap_or(DEFAULT_N_RESULTS))
}

#[tauri::command]
pub async fn assistant_status(
    state: State<'_, BackendState>,
    generation_state: State<'_, super::generation::GenerationState>,
) -> Result<AssistantStatusResponse, String> {
    Ok(assistant_status_internal(&state, generation_state.is_generating()).await)
}

async fn run_chat_completion(
    state: &BackendState,
    system_prompt: &str,
    user_prompt: &str,
    model_override: Option<String>,
    temperature: f64,
    operation: &str,
) -> Result<String, String> {
    match state.get_generation_backend() {
        GenerationBackend::Ollama => {
            ollama::chat_once(system_prompt, user_prompt, model_override, temperature).await
        }
        GenerationBackend::SharedEngine => {
            let mut engine_result =
                engine_integration::chat_once(system_prompt, user_prompt, temperature).await;

            if let Err(err) = &engine_result {
                if engine_integration::is_model_not_loaded_error(err) {
                    log::info!(
                        "[SharedEngine] Model not loaded during {}, attempting autoload",
                        operation
                    );
                    match engine_integration::ensure_model_loaded().await {
                        Ok(model_id) => {
                            state.set_loaded_model_id(Some(model_id.clone()));
                            log::info!(
                                "[SharedEngine] Model autoload succeeded ('{}'), retrying {}",
                                model_id,
                                operation
                            );
                            engine_result = engine_integration::chat_once(
                                system_prompt,
                                user_prompt,
                                temperature,
                            )
                            .await;
                        }
                        Err(load_err) => {
                            return Err(format!("{} (model autoload failed: {})", err, load_err));
                        }
                    }
                }
            }

            match engine_result {
                Ok(answer) => Ok(answer),
                Err(err) if engine_integration::is_engine_connection_error(&err) => {
                    engine_integration::invalidate_availability_cache();
                    if crate::state::allow_ollama_fallback() {
                        log::info!(
                            "[SharedEngine] Engine unreachable during {}, falling back to Ollama",
                            operation
                        );
                        if ollama::is_ollama_available().await {
                            state.set_generation_backend(GenerationBackend::Ollama);
                            ollama::chat_once(
                                system_prompt,
                                user_prompt,
                                model_override,
                                temperature,
                            )
                            .await
                        } else {
                            Err(err)
                        }
                    } else {
                        log::info!(
                            "[SharedEngine] Engine unreachable during {}; Ollama fallback is disabled",
                            operation
                        );
                        Err(err)
                    }
                }
                Err(err) => Err(err),
            }
        }
    }
}

fn is_degenerate_response(text: &str) -> bool {
    let tokens: Vec<String> = text
        .split_whitespace()
        .map(normalize_word)
        .filter(|token| !token.is_empty())
        .collect();

    if tokens.len() < 48 {
        return false;
    }

    let mut longest_run = 1usize;
    let mut current_run = 1usize;
    for window in tokens.windows(2) {
        if window[0] == window[1] {
            current_run += 1;
            longest_run = longest_run.max(current_run);
        } else {
            current_run = 1;
        }
    }
    if longest_run >= 8 {
        return true;
    }

    let unique_tokens: HashSet<&str> = tokens.iter().map(String::as_str).collect();
    if unique_tokens.len() * 4 < tokens.len() {
        return true;
    }

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for token in &tokens {
        *counts.entry(token.as_str()).or_insert(0) += 1;
    }

    let max_count = counts.values().copied().max().unwrap_or(0);
    max_count >= 14 && max_count * 100 >= tokens.len() * 28
}

fn normalize_word(token: &str) -> String {
    token
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_ascii_lowercase()
}

fn parse_suggestions(response: &str) -> Vec<String> {
    let mut suggestions: Vec<String> = response
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(strip_leading_number)
        .collect();

    if suggestions.is_empty() && !response.trim().is_empty() {
        suggestions.push(response.trim().to_string());
    }

    suggestions.truncate(5);
    suggestions
}

fn strip_leading_number(line: &str) -> Option<String> {
    let mut chars = line.chars().peekable();
    let mut consumed_digits = false;

    while let Some(ch) = chars.peek() {
        if ch.is_ascii_digit() {
            consumed_digits = true;
            chars.next();
        } else {
            break;
        }
    }

    if consumed_digits {
        while let Some(ch) = chars.peek() {
            if *ch == '.' || *ch == ')' || *ch == ':' || *ch == '-' || ch.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
    }

    if !consumed_digits {
        return None;
    }

    let cleaned: String = chars.collect::<String>().trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_repetitive_response_pattern() {
        let repeated = format!(
            "{} {}",
            "explanation in simple short paragraph.",
            "an ".repeat(80).trim_end()
        );
        assert!(is_degenerate_response(&repeated));
    }

    #[test]
    fn keeps_normal_instructional_response() {
        let normal = "To retopologize a sculpt, start by adding a new mesh object and a Shrinkwrap modifier targeting your sculpt. In Edit Mode, use Poly Build and loop cuts to lay down clean quad loops around deformation areas like shoulders, elbows, and knees. Keep edge flow consistent, then add a Mirror modifier and test deformations with simple armature bends before final cleanup.";
        assert!(!is_degenerate_response(normal));
    }
}
