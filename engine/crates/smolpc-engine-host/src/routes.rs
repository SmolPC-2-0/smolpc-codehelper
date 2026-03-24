use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;
use smolpc_engine_core::models::ModelRegistry;
use std::convert::Infallible;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::artifacts::build_check_model_response;
use crate::auth::auth;
use crate::chat::{
    humanize_generation_error, is_preformatted_chatml_single_user_message, model_has_thinking_mode,
    openvino_request_defaults, request_to_config, request_to_prompt,
    request_to_structured_messages, should_use_openvino_structured_messages, stream_error_code,
    ThinkingFilter,
};
use crate::config::{epoch_ms, with_memory_pressure_hint};
use crate::state::AppState;
use crate::types::{
    ApiError, AudioSpeechRequest, CancelOnDrop, ChatCompletionRequest, CheckModelRequest,
    CompletionInput, EnsureStartedOutcome, EnsureStartedRequest, ErrorResponse, LoadRequest,
    ReadinessState, StartupError, StartupMode, StreamMessage, UnloadRequest, ENGINE_API_VERSION,
    ENGINE_PROTOCOL_VERSION, OPENVINO_CHAT_MODE_LEGACY_PROMPT, OPENVINO_CHAT_MODE_STRUCTURED,
};

pub(crate) async fn health(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let readiness = state.engine.readiness.lock().await;
    let state_name = format!("{:?}", readiness.state).to_ascii_lowercase();
    if matches!(readiness.state, ReadinessState::Failed) {
        Ok((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "state": state_name})),
        ))
    } else {
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "state": state_name})),
        ))
    }
}

pub(crate) async fn meta(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    Ok(Json(serde_json::json!({
        "ok": true,
        "protocol_version": ENGINE_PROTOCOL_VERSION,
        "engine_api_version": ENGINE_API_VERSION,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
        "busy": state.engine.generating.load(Ordering::SeqCst),
    })))
}

pub(crate) async fn status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let payload = state.engine.current_readiness_payload(true, None).await;
    Ok(Json(serde_json::json!(payload)))
}

pub(crate) async fn ensure_started(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<EnsureStartedRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    let outcome = state
        .engine
        .ensure_started(req.mode, req.startup_policy.clone())
        .await;
    let (http_status, ok, override_error) = match outcome {
        EnsureStartedOutcome::Ready => (StatusCode::OK, true, None),
        EnsureStartedOutcome::Failed => (StatusCode::SERVICE_UNAVAILABLE, false, None),
        EnsureStartedOutcome::Conflict => (
            StatusCode::CONFLICT,
            false,
            Some(StartupError {
                phase: ReadinessState::Ready,
                code: "STARTUP_POLICY_CONFLICT",
                message: "Engine is already ready under a different startup mode/policy. Perform explicit shutdown and restart.".to_string(),
                retryable: false,
            }),
        ),
    };
    let payload = state
        .engine
        .current_readiness_payload(ok, override_error)
        .await;
    Ok((http_status, Json(payload)).into_response())
}

pub(crate) async fn load(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<LoadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state
        .engine
        .load_model(req.model_id.clone(), StartupMode::Auto)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: with_memory_pressure_hint(&e, Some(&req.model_id)),
                }),
            )
        })?;
    state
        .engine
        .mark_ready_after_external_load(req.model_id)
        .await;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn unload(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<UnloadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state
        .engine
        .unload_model(req.force.unwrap_or(false))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e }),
            )
        })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn cancel(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state.engine.cancel();
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn shutdown(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    state.shutdown.notify_waiters();
    Ok(Json(serde_json::json!({"ok": true})))
}

pub(crate) async fn check_model(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CheckModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let startup_probe = state.engine.startup_probe.lock().await.clone();
    let openvino_probe = state.engine.openvino_startup_probe.lock().await.clone();
    let readiness = build_check_model_response(
        &req.model_id,
        state.engine.runtime_bundles(),
        startup_probe.as_ref(),
        openvino_probe.as_ref(),
    );
    match serde_json::to_value(readiness) {
        Ok(value) => Ok(Json(value)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to serialize check-model response: {e}"),
            }),
        )),
    }
}

pub(crate) async fn v1_models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);
    let data = ModelRegistry::available_models()
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "object": "model",
                "owned_by": "smolpc",
                "min_ram_gb": m.min_ram_gb,
                "estimated_runtime_ram_gb": m.estimated_runtime_ram_gb,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({"object": "list", "data": data})))
}

pub(crate) async fn v1_chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    let queue_permit = state
        .queue_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "Engine queue is full".to_string(),
                }),
            )
        })?;

    let gen_permit = timeout(
        state.queue_timeout,
        state.generation_semaphore.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Queued request timed out".to_string(),
            }),
        )
    })
    .and_then(|r| {
        r.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Generation semaphore closed".to_string(),
                }),
            )
        })
    })?;

    drop(queue_permit);

    let current_model_id = state.engine.current_model.lock().await.clone();
    let openvino_runtime_loaded = state.engine.uses_openvino_genai_runtime().await;
    let config = request_to_config(
        &req,
        openvino_request_defaults(current_model_id.as_deref(), openvino_runtime_loaded),
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
    let disable_thinking = current_model_id
        .as_deref()
        .is_some_and(model_has_thinking_mode);
    let use_legacy_prompt = is_preformatted_chatml_single_user_message(&req.messages);
    let use_structured_messages =
        should_use_openvino_structured_messages(openvino_runtime_loaded, use_legacy_prompt);
    let completion_input = if use_structured_messages {
        let messages = request_to_structured_messages(&req.messages)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
        (
            OPENVINO_CHAT_MODE_STRUCTURED,
            CompletionInput::Messages(messages),
        )
    } else {
        let prompt = request_to_prompt(&req.messages, disable_thinking)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })))?;
        let mode = if use_legacy_prompt {
            OPENVINO_CHAT_MODE_LEGACY_PROMPT
        } else {
            OPENVINO_CHAT_MODE_STRUCTURED
        };
        (mode, CompletionInput::Prompt(prompt))
    };
    if openvino_runtime_loaded {
        let mut backend_status = state.engine.backend_status.lock().await;
        backend_status.openvino_message_mode = Some(completion_input.0.to_string());
    }
    let completion_input = completion_input.1;
    let model_name = req.model.unwrap_or_else(|| "smolpc-engine".to_string());
    let request_id = format!("chatcmpl-{}", Utc::now().timestamp_millis());
    let created = Utc::now().timestamp();

    if req.stream.unwrap_or(false) {
        let (tx, mut rx) = mpsc::unbounded_channel::<StreamMessage>();
        let engine = state.engine.clone();
        let activity = state.last_activity_ms.clone();
        let input = completion_input;
        tokio::spawn(async move {
            let _permit = gen_permit;
            let result = match input {
                CompletionInput::Prompt(prompt) => {
                    engine
                        .generate_stream(&prompt, config, |t| {
                            let _ = tx.send(StreamMessage::Token(t));
                        })
                        .await
                }
                CompletionInput::Messages(messages) => {
                    engine
                        .generate_stream_messages(&messages, config, |t| {
                            let _ = tx.send(StreamMessage::Token(t));
                        })
                        .await
                }
            };
            match result {
                Ok(metrics) => {
                    let _ = tx.send(StreamMessage::Metrics(metrics));
                    let _ = tx.send(StreamMessage::Done);
                }
                Err(e) => {
                    let _ = tx.send(StreamMessage::Error {
                        code: stream_error_code(&e),
                        message: humanize_generation_error(&e),
                    });
                    let _ = tx.send(StreamMessage::Done);
                }
            }
            activity.store(epoch_ms(), Ordering::SeqCst);
        });

        let stream = async_stream::stream! {
            let _cancel_guard = CancelOnDrop { engine: state.engine.clone() };
            let mut think_filter = if disable_thinking {
                Some(ThinkingFilter::new())
            } else {
                None
            };
            let start = serde_json::json!({
                "id": request_id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model_name,
                "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": serde_json::Value::Null}],
            });
            yield Ok::<Event, Infallible>(Event::default().data(start.to_string()));

            while let Some(item) = rx.recv().await {
                match item {
                    StreamMessage::Token(token) => {
                        let filtered = if let Some(ref mut filter) = think_filter {
                            filter.push(&token)
                        } else {
                            Some(token)
                        };
                        if let Some(text) = filtered {
                            let chunk = serde_json::json!({
                                "id": request_id,
                                "object": "chat.completion.chunk",
                                "created": created,
                                "model": model_name,
                                "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": serde_json::Value::Null}],
                            });
                            yield Ok(Event::default().data(chunk.to_string()));
                        }
                    }
                    StreamMessage::Metrics(metrics) => {
                        let metrics_event = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.metrics",
                            "created": created,
                            "model": model_name,
                            "smolpc_metrics": metrics,
                        });
                        yield Ok(Event::default().data(metrics_event.to_string()));
                    }
                    StreamMessage::Error { message, code } => {
                        let error_type = if code == "INFERENCE_GENERATION_CANCELLED" {
                            "cancelled"
                        } else {
                            "runtime_error"
                        };
                        let error_event = serde_json::json!({
                            "error": {
                                "message": message,
                                "code": code,
                                "type": error_type
                            }
                        });
                        yield Ok(Event::default().data(error_event.to_string()));
                    }
                    StreamMessage::Done => {
                        // Flush any remaining buffered text from the thinking filter.
                        if let Some(ref mut filter) = think_filter {
                            if let Some(tail) = filter.finish() {
                                let chunk = serde_json::json!({
                                    "id": request_id,
                                    "object": "chat.completion.chunk",
                                    "created": created,
                                    "model": model_name,
                                    "choices": [{"index": 0, "delta": {"content": tail}, "finish_reason": serde_json::Value::Null}],
                                });
                                yield Ok(Event::default().data(chunk.to_string()));
                            }
                        }
                        let done = serde_json::json!({
                            "id": request_id,
                            "object": "chat.completion.chunk",
                            "created": created,
                            "model": model_name,
                            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                        });
                        yield Ok(Event::default().data(done.to_string()));
                        yield Ok(Event::default().data("[DONE]"));
                        break;
                    }
                }
            }
        };

        Ok(Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Non-streaming completions are not supported. Set \"stream\": true."
                    .to_string(),
            }),
        ))
    }
}

// ── Whisper STT ──────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub(crate) async fn v1_audio_transcriptions(
    headers: HeaderMap,
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    // Validate body: must be f32-aligned and non-empty.
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Empty audio body".to_string(),
            }),
        ));
    }
    if body.len() % 4 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Body length must be a multiple of 4 (f32 PCM samples)".to_string(),
            }),
        ));
    }

    // Acquire voice semaphore with 30s timeout.
    let permit = timeout(
        Duration::from_secs(30),
        state.voice_semaphore.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Voice transcription queue timed out".to_string(),
            }),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Voice semaphore closed".to_string(),
            }),
        )
    })?;

    // Convert bytes to f32 samples (little-endian).
    let audio: Vec<f32> = body
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    let engine = state.engine.clone();

    let text = tokio::task::spawn_blocking(move || {
        let _permit = permit; // Hold permit for the duration of transcription.
        ensure_whisper_loaded(&engine)?;
        let pipeline_guard = engine.whisper_pipeline.blocking_lock();
        let pipeline = pipeline_guard
            .as_ref()
            .ok_or_else(|| "WhisperPipeline not loaded".to_string())?;
        pipeline.transcribe(&audio)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Transcription worker error: {e}"),
            }),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: e }),
        )
    })?;

    Ok(Json(serde_json::json!({ "text": text })))
}

/// Lazy-load the WhisperPipeline if not already loaded.
/// Called inside `spawn_blocking` — uses `blocking_lock()`.
#[cfg(target_os = "windows")]
fn ensure_whisper_loaded(engine: &crate::state::EngineState) -> Result<(), String> {
    let mut guard = engine.whisper_pipeline.blocking_lock();
    if guard.is_some() {
        return Ok(());
    }

    let openvino_bundle = &engine.runtime_bundles().openvino;

    let model_dir = smolpc_engine_core::models::ModelLoader::models_dir()
        .join("whisper-base.en")
        .join("openvino");

    if !model_dir.exists() {
        return Err(format!(
            "Whisper model not found at {}",
            model_dir.display()
        ));
    }

    log::info!("Loading WhisperPipeline from {}", model_dir.display());
    let pipeline = smolpc_engine_core::inference::genai::whisper::WhisperPipeline::new(
        openvino_bundle,
        &model_dir,
    )?;
    *guard = Some(pipeline);
    log::info!("WhisperPipeline loaded successfully");
    Ok(())
}

// ── TTS proxy ────────────────────────────────────────────────────────

pub(crate) async fn v1_audio_speech(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AudioSpeechRequest>,
) -> Result<impl IntoResponse, ApiError> {
    auth(&headers, &state.token)?;
    state.last_activity_ms.store(epoch_ms(), Ordering::SeqCst);

    if req.text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Text is empty".to_string(),
            }),
        ));
    }

    // Acquire voice semaphore with 30s timeout.
    let _permit = timeout(
        Duration::from_secs(30),
        state.voice_semaphore.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::GATEWAY_TIMEOUT,
            Json(ErrorResponse {
                error: "Voice synthesis queue timed out".to_string(),
            }),
        )
    })?
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Voice semaphore closed".to_string(),
            }),
        )
    })?;

    // Lazy health check — only when a TTS request arrives.
    let tts = &state.tts;
    if !tts.check_health().await {
        tts.attempt_respawn().await;
        if !tts.check_health().await {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "TTS service unavailable".to_string(),
                }),
            ));
        }
    }

    // Forward to sidecar POST /synthesize.
    let sidecar_url = format!("http://127.0.0.1:{}/synthesize", tts.port());
    let response = tts
        .http_client()
        .post(&sidecar_url)
        .header("Authorization", format!("Bearer {}", state.token))
        .json(&serde_json::json!({
            "text": req.text,
            "voice": req.voice,
            "speed": req.speed,
        }))
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("TTS sidecar request failed: {e}"),
                }),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("TTS sidecar error ({status}): {body}"),
            }),
        ));
    }

    let wav_bytes = response.bytes().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to read TTS response: {e}"),
            }),
        )
    })?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "audio/wav")],
        wav_bytes,
    ))
}
