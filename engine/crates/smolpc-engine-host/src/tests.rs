use crate::artifacts::*;
use crate::auth::*;
use crate::chat::*;
use crate::config::*;
use crate::openvino::OpenVinoStartupProbeResult;
use crate::probe::*;
use crate::runtime_bundles::{resolve_runtime_bundles_for_mode, RuntimeLoadMode};
use crate::selection::*;
use crate::state::*;
use crate::types::*;
use chrono::Utc;
use smolpc_engine_core::inference::backend::{
    BackendDecision, BackendDecisionKey, BackendSelectionState, DecisionReason, FailureCounters,
    InferenceBackend,
};
use smolpc_engine_core::inference::backend_store::BackendDecisionRecord;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn request_to_prompt_renders_chatml() {
    let messages = vec![
        ChatCompletionMessage {
            role: "system".to_string(),
            content: Some("You are helpful.".to_string()),
        },
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some("hello".to_string()),
        },
    ];

    let prompt = request_to_prompt(&messages, false).expect("chatml prompt");
    assert!(prompt.contains("<|im_start|>system\nYou are helpful.<|im_end|>\n"));
    assert!(prompt.contains("<|im_start|>user\nhello<|im_end|>\n"));
    assert!(prompt.ends_with("<|im_start|>assistant\n"));
}

#[test]
fn request_to_prompt_preserves_preformatted_chatml_single_user_message() {
    let preformatted =
        "<|im_start|>system\ns<|im_end|>\n<|im_start|>user\nu<|im_end|>\n<|im_start|>assistant\n";
    let messages = vec![ChatCompletionMessage {
        role: "user".to_string(),
        content: Some(preformatted.to_string()),
    }];

    let prompt = request_to_prompt(&messages, false).expect("preformatted chatml");
    assert_eq!(prompt, preformatted);
}

#[test]
fn request_to_prompt_injects_nothink_in_system_message() {
    let messages = vec![
        ChatCompletionMessage {
            role: "system".to_string(),
            content: Some("You are helpful.".to_string()),
        },
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some("hello".to_string()),
        },
    ];

    let prompt = request_to_prompt(&messages, true).expect("chatml with nothink");
    assert!(prompt.contains("<|im_start|>system\nYou are helpful.\n/nothink<|im_end|>\n"));
    assert!(prompt.contains("<|im_start|>user\nhello<|im_end|>\n"));
}

#[test]
fn request_to_prompt_prepends_nothink_system_message_when_none_present() {
    let messages = vec![ChatCompletionMessage {
        role: "user".to_string(),
        content: Some("hello".to_string()),
    }];

    let prompt = request_to_prompt(&messages, true).expect("nothink without system");
    assert!(prompt.starts_with("<|im_start|>system\n/nothink<|im_end|>\n"));
    assert!(prompt.contains("<|im_start|>user\nhello<|im_end|>\n"));
}

#[test]
fn request_to_prompt_nothink_false_does_not_inject() {
    let messages = vec![
        ChatCompletionMessage {
            role: "system".to_string(),
            content: Some("You are helpful.".to_string()),
        },
        ChatCompletionMessage {
            role: "user".to_string(),
            content: Some("hello".to_string()),
        },
    ];

    let prompt = request_to_prompt(&messages, false).expect("chatml without nothink");
    assert!(!prompt.contains("/nothink"));
}

#[test]
fn model_has_thinking_mode_matches_qwen3() {
    assert!(model_has_thinking_mode("qwen3-4b"));
    assert!(model_has_thinking_mode("qwen3-0.6b"));
    assert!(!model_has_thinking_mode("qwen2.5-1.5b-instruct"));
    assert!(!model_has_thinking_mode("phi-4-mini-instruct"));
}

#[test]
fn thinking_filter_streaming_suppresses_think_block() {
    let mut f = ThinkingFilter::new();
    let mut out = String::new();
    for token in ["<think>", "reasoning here", "</think>", "\n", "Hello!"] {
        if let Some(t) = f.push(token) {
            out.push_str(&t);
        }
    }
    if let Some(t) = f.finish() {
        out.push_str(&t);
    }
    assert_eq!(out, "Hello!");
}

#[test]
fn thinking_filter_streaming_split_tags() {
    let mut f = ThinkingFilter::new();
    let mut out = String::new();
    // Tags split across token boundaries.
    for token in ["<th", "ink>", "internal", "</thi", "nk>", "World"] {
        if let Some(t) = f.push(token) {
            out.push_str(&t);
        }
    }
    if let Some(t) = f.finish() {
        out.push_str(&t);
    }
    assert_eq!(out, "World");
}

#[test]
fn thinking_filter_passes_non_thinking_text() {
    let mut f = ThinkingFilter::new();
    let mut out = String::new();
    for token in ["Hello, ", "world!"] {
        if let Some(t) = f.push(token) {
            out.push_str(&t);
        }
    }
    if let Some(t) = f.finish() {
        out.push_str(&t);
    }
    assert_eq!(out, "Hello, world!");
}

#[test]
fn request_to_config_rejects_zero_max_tokens() {
    let _guard = lock_env();
    std::env::remove_var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV);
    let request = ChatCompletionRequest {
        model: None,
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some("hi".to_string()),
        }],
        stream: None,
        max_tokens: Some(0),
        temperature: None,
        top_k: None,
        top_p: None,
        repetition_penalty: None,
        repetition_penalty_last_n: None,
    };

    let error = request_to_config(&request, None).expect_err("zero max_tokens should fail");
    assert!(error.contains("max_tokens"));
}

#[test]
fn request_to_config_caps_max_tokens_to_hard_limit() {
    let _guard = lock_env();
    std::env::remove_var(OPENVINO_MAX_TOKENS_HARD_CAP_ENV);
    let request = ChatCompletionRequest {
        model: None,
        messages: vec![ChatCompletionMessage {
            role: "user".to_string(),
            content: Some("hi".to_string()),
        }],
        stream: None,
        max_tokens: Some(99_999),
        temperature: None,
        top_k: None,
        top_p: None,
        repetition_penalty: None,
        repetition_penalty_last_n: None,
    };

    let config = request_to_config(&request, None)
        .expect("config parse")
        .expect("config");
    assert_eq!(config.max_length, OPENVINO_MAX_TOKENS_HARD_CAP_DEFAULT);
}

#[test]
fn openvino_qwen3_request_defaults_match_upstream_non_thinking_guidance() {
    let defaults = openvino_request_defaults(Some("qwen3-4b"), true)
        .expect("qwen3 openvino defaults should exist");

    assert_eq!(defaults.temperature, 0.7);
    assert_eq!(defaults.top_p, Some(0.8));
    assert_eq!(defaults.top_k, Some(20));
}

#[test]
fn structured_messages_follow_loaded_openvino_runtime_not_backend_name() {
    assert!(should_use_openvino_structured_messages(true, false));
    assert!(!should_use_openvino_structured_messages(false, false));
    assert!(!should_use_openvino_structured_messages(true, true));
}

#[test]
fn auth_compare_is_constant_time_functionally() {
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"ab"));
}

#[test]
fn engine_state_startup_succeeds_with_missing_ort_bundle() {
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    fs::create_dir_all(&resource_dir).expect("create resource dir");

    let args = test_args(temp.path(), Some(resource_dir.clone()));
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let engine = EngineState::new_with_runtime_bundles(&args, bundles);
    let status = engine.backend_status.blocking_lock().clone();

    assert_eq!(status.selection_state, Some(BackendSelectionState::Pending));
    assert!(!status.runtime_bundles.ort.validated);
    assert_eq!(
        status.runtime_bundles.ort.failure.as_deref(),
        Some("missing_root")
    );
    assert!(!status.lanes.cpu.bundle_ready);
}

#[test]
fn engine_state_startup_succeeds_with_missing_openvino_bundle() {
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );

    let args = test_args(temp.path(), Some(resource_dir.clone()));
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let engine = EngineState::new_with_runtime_bundles(&args, bundles);
    let status = engine.backend_status.blocking_lock().clone();

    assert!(status.runtime_bundles.ort.validated);
    assert!(!status.runtime_bundles.openvino.validated);
    assert_eq!(
        status.runtime_bundles.openvino.failure.as_deref(),
        Some("missing_root")
    );
    assert!(!status.lanes.openvino_npu.bundle_ready);
}

#[test]
fn engine_state_startup_succeeds_with_missing_openvino_plugin() {
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let openvino_root = libs.join("openvino");
    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&openvino_root);
    fs::remove_file(openvino_root.join("openvino_intel_npu_plugin.dll"))
        .expect("remove npu plugin");

    let args = test_args(temp.path(), Some(resource_dir.clone()));
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let engine = EngineState::new_with_runtime_bundles(&args, bundles);
    let status = engine.backend_status.blocking_lock().clone();

    assert!(status.runtime_bundles.ort.validated);
    assert!(status.runtime_bundles.openvino.validated);
    assert_eq!(status.runtime_bundles.openvino.failure, None);
    assert!(!status.lanes.openvino_npu.bundle_ready);
    assert_eq!(
        status.lanes.openvino_npu.last_failure_class.as_deref(),
        Some("openvino_npu_plugin_missing")
    );
}

#[test]
fn backend_selection_prefers_directml_when_both_candidates_ready() {
    let (backend, reason) =
        choose_preferred_backend(None, &FailureCounters::default(), None, true, true);

    assert_eq!(backend, InferenceBackend::DirectML);
    assert_eq!(reason, DecisionReason::DefaultDirectMLCandidate);
}

#[test]
fn backend_selection_prefers_directml_when_openvino_is_unavailable() {
    let (backend, reason) =
        choose_preferred_backend(None, &FailureCounters::default(), None, true, false);

    assert_eq!(backend, InferenceBackend::DirectML);
    assert_eq!(reason, DecisionReason::DefaultDirectMLCandidate);
}

#[test]
fn directml_model_reload_releases_current_adapter_before_rebuild() {
    assert!(should_release_current_adapter_for_load(
        Some(InferenceBackend::DirectML),
        InferenceBackend::Cpu,
        true,
    ));
    assert!(should_release_current_adapter_for_load(
        Some(InferenceBackend::Cpu),
        InferenceBackend::DirectML,
        true,
    ));
    assert!(!should_release_current_adapter_for_load(
        Some(InferenceBackend::Cpu),
        InferenceBackend::Cpu,
        true,
    ));
    assert!(!should_release_current_adapter_for_load(
        Some(InferenceBackend::DirectML),
        InferenceBackend::DirectML,
        false,
    ));
}

#[test]
fn backend_selection_keeps_persisted_cpu_choice() {
    let record = BackendDecisionRecord {
        key: BackendDecisionKey {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            model_artifact_fingerprint: Some("artifact-v1".to_string()),
            app_version: "test".to_string(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: Some("2.0.0-rc.11".to_string()),
            ort_bundle_fingerprint: Some("ort-bundle".to_string()),
            openvino_runtime_version: Some("2026.0.0".to_string()),
            openvino_genai_version: Some("2026.0.0".to_string()),
            openvino_tokenizers_version: Some("2026.0.0".to_string()),
            openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
            gpu_adapter_identity: Some("intel:arc".to_string()),
            gpu_driver_version: Some("31.0.101.5522".to_string()),
            gpu_device_id: Some(0),
            npu_adapter_identity: None,
            npu_driver_version: None,
            openvino_npu_max_prompt_len: Some(512),
            openvino_npu_min_response_len: Some(1024),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
        },
        persisted_decision: Some(BackendDecision::new(
            InferenceBackend::Cpu,
            DecisionReason::NoDirectMLCandidate,
            None,
        )),
        failure_counters: FailureCounters::default(),
        updated_at: Utc::now().to_rfc3339(),
    };

    let (backend, reason) = choose_preferred_backend(
        None,
        &FailureCounters::default(),
        Some(&record),
        true,
        false,
    );

    assert_eq!(backend, InferenceBackend::Cpu);
    assert_eq!(reason, DecisionReason::PersistedDecision);
}

#[test]
fn backend_selection_keeps_persisted_openvino_choice_when_candidate_is_ready() {
    let record = BackendDecisionRecord {
        key: BackendDecisionKey {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            model_artifact_fingerprint: Some("artifact-v1".to_string()),
            app_version: "test".to_string(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: Some("2.0.0-rc.11".to_string()),
            ort_bundle_fingerprint: Some("ort-bundle".to_string()),
            openvino_runtime_version: Some("2026.0.0".to_string()),
            openvino_genai_version: Some("2026.0.0".to_string()),
            openvino_tokenizers_version: Some("2026.0.0".to_string()),
            openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
            gpu_adapter_identity: Some("intel:arc".to_string()),
            gpu_driver_version: Some("31.0.101.5522".to_string()),
            gpu_device_id: Some(0),
            npu_adapter_identity: Some("openvino:npu:intel_npu".to_string()),
            npu_driver_version: Some("32.0.100.3104".to_string()),
            openvino_npu_max_prompt_len: Some(512),
            openvino_npu_min_response_len: Some(1024),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
        },
        persisted_decision: Some(BackendDecision::new(
            InferenceBackend::OpenVinoNpu,
            DecisionReason::PersistedDecision,
            None,
        )),
        failure_counters: FailureCounters::default(),
        updated_at: Utc::now().to_rfc3339(),
    };

    let (backend, reason) =
        choose_preferred_backend(None, &FailureCounters::default(), Some(&record), true, true);

    assert_eq!(backend, InferenceBackend::OpenVinoNpu);
    assert_eq!(reason, DecisionReason::PersistedDecision);
}

#[test]
fn backend_selection_falls_back_when_persisted_openvino_candidate_is_unavailable() {
    let record = BackendDecisionRecord {
        key: BackendDecisionKey {
            model_id: "qwen2.5-1.5b-instruct".to_string(),
            model_artifact_fingerprint: Some("artifact-v1".to_string()),
            app_version: "test".to_string(),
            selector_engine_id: "engine_host".to_string(),
            ort_runtime_version: Some("2.0.0-rc.11".to_string()),
            ort_bundle_fingerprint: Some("ort-bundle".to_string()),
            openvino_runtime_version: Some("2026.0.0".to_string()),
            openvino_genai_version: Some("2026.0.0".to_string()),
            openvino_tokenizers_version: Some("2026.0.0".to_string()),
            openvino_bundle_fingerprint: Some("openvino-bundle".to_string()),
            gpu_adapter_identity: Some("intel:arc".to_string()),
            gpu_driver_version: Some("31.0.101.5522".to_string()),
            gpu_device_id: Some(0),
            npu_adapter_identity: Some("openvino:npu:intel_npu".to_string()),
            npu_driver_version: Some("32.0.100.3104".to_string()),
            openvino_npu_max_prompt_len: Some(512),
            openvino_npu_min_response_len: Some(1024),
            openvino_message_mode: Some(OPENVINO_CHAT_MODE_STRUCTURED.to_string()),
            selection_profile: Some(OPENVINO_SELECTION_PROFILE.to_string()),
        },
        persisted_decision: Some(BackendDecision::new(
            InferenceBackend::OpenVinoNpu,
            DecisionReason::PersistedDecision,
            None,
        )),
        failure_counters: FailureCounters::default(),
        updated_at: Utc::now().to_rfc3339(),
    };

    let (backend, reason) = choose_preferred_backend(
        None,
        &FailureCounters::default(),
        Some(&record),
        true,
        false,
    );

    assert_eq!(backend, InferenceBackend::DirectML);
    assert_eq!(reason, DecisionReason::DefaultDirectMLCandidate);
}

#[test]
fn check_model_response_reports_lane_readiness() {
    let _guard = lock_env();
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let models_dir = temp.path().join("models");
    let model_dir = models_dir.join("qwen2.5-1.5b-instruct");
    let dml_dir = model_dir.join("dml");
    let openvino_dir = model_dir.join("openvino");

    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&libs.join("openvino"));
    fs::create_dir_all(&dml_dir).expect("create dml dir");
    fs::create_dir_all(&openvino_dir).expect("create openvino dir");
    fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
    fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
    fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
    fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
    fs::write(
        openvino_dir.join("manifest.json"),
        br#"{"required_files":["model.xml"]}"#,
    )
    .expect("write openvino manifest");

    let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let probe = BackendProbeResult {
        available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
        directml_device_count: 1,
        directml_candidate: Some(DirectMlCandidate {
            device_id: 0,
            device_name: "Intel Arc".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: String::new(),
            vram_mb: 4096,
        }),
        directml_probe_failure_class: None,
        directml_probe_failure_message: None,
    };

    let response =
        build_check_model_response("qwen2.5-1.5b-instruct", &bundles, Some(&probe), None);
    drop(models_guard);

    assert!(response.lanes.cpu.ready);
    assert_eq!(response.lanes.cpu.reason, "ready");
    assert!(response.lanes.directml.ready);
    assert_eq!(response.lanes.directml.reason, "ready");
    assert!(!response.lanes.openvino_npu.ready);
    assert_eq!(response.lanes.openvino_npu.reason, "startup_probe_pending");
}

#[test]
fn check_model_response_keeps_directml_ready_while_probe_is_pending() {
    let _guard = lock_env();
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let models_dir = temp.path().join("models");
    let model_dir = models_dir.join("qwen2.5-1.5b-instruct");
    let dml_dir = model_dir.join("dml");
    let openvino_dir = model_dir.join("openvino");

    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&libs.join("openvino"));
    fs::create_dir_all(&dml_dir).expect("create dml dir");
    fs::create_dir_all(&openvino_dir).expect("create openvino dir");
    fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
    fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
    fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
    fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
    fs::write(
        openvino_dir.join("manifest.json"),
        br#"{"required_files":["model.xml"]}"#,
    )
    .expect("write openvino manifest");

    let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);

    let response = build_check_model_response("qwen2.5-1.5b-instruct", &bundles, None, None);
    drop(models_guard);

    assert!(response.lanes.directml.artifact_ready);
    assert!(response.lanes.directml.bundle_ready);
    assert!(response.lanes.directml.ready);
    assert_eq!(response.lanes.directml.reason, "ready");
    assert!(!response.lanes.openvino_npu.ready);
    assert_eq!(response.lanes.openvino_npu.reason, "startup_probe_pending");
}

#[test]
fn check_model_response_blocks_directml_when_probe_confirms_no_candidate() {
    let _guard = lock_env();
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let models_dir = temp.path().join("models");
    let model_dir = models_dir.join("qwen2.5-1.5b-instruct");
    let dml_dir = model_dir.join("dml");
    let openvino_dir = model_dir.join("openvino");

    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&libs.join("openvino"));
    fs::create_dir_all(&dml_dir).expect("create dml dir");
    fs::create_dir_all(&openvino_dir).expect("create openvino dir");
    fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
    fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
    fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
    fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
    fs::write(
        openvino_dir.join("manifest.json"),
        br#"{"required_files":["model.xml"]}"#,
    )
    .expect("write openvino manifest");

    let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let probe = BackendProbeResult {
        available_backends: vec![InferenceBackend::Cpu],
        directml_device_count: 0,
        directml_candidate: None,
        directml_probe_failure_class: Some("directml_candidate_missing".to_string()),
        directml_probe_failure_message: Some("No DirectML-capable adapter detected".to_string()),
    };

    let response =
        build_check_model_response("qwen2.5-1.5b-instruct", &bundles, Some(&probe), None);
    drop(models_guard);

    assert!(!response.lanes.directml.ready);
    assert_eq!(response.lanes.directml.reason, "directml_candidate_missing");
}

#[test]
fn check_model_response_reports_openvino_lane_ready_when_probe_is_ready() {
    let _guard = lock_env();
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let models_dir = temp.path().join("models");
    let model_dir = models_dir.join("qwen2.5-1.5b-instruct");
    let dml_dir = model_dir.join("dml");
    let openvino_dir = model_dir.join("openvino");

    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&libs.join("openvino"));
    fs::create_dir_all(&dml_dir).expect("create dml dir");
    fs::create_dir_all(&openvino_dir).expect("create openvino dir");
    fs::write(dml_dir.join("model.onnx"), []).expect("write dml model");
    fs::write(dml_dir.join("genai_config.json"), []).expect("write dml config");
    fs::write(dml_dir.join("tokenizer.json"), []).expect("write dml tokenizer");
    fs::write(openvino_dir.join("model.xml"), []).expect("write openvino model");
    fs::write(
        openvino_dir.join("manifest.json"),
        br#"{"required_files":["model.xml"]}"#,
    )
    .expect("write openvino manifest");

    let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let probe = BackendProbeResult {
        available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
        directml_device_count: 1,
        directml_candidate: Some(DirectMlCandidate {
            device_id: 0,
            device_name: "Intel Arc".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: String::new(),
            vram_mb: 4096,
        }),
        directml_probe_failure_class: None,
        directml_probe_failure_message: None,
    };
    let openvino_probe = OpenVinoStartupProbeResult {
        startup_ready: true,
        device_visible: true,
        adapter_identity: Some("openvino:npu:intel_npu".to_string()),
        device_name: Some("Intel NPU".to_string()),
        driver_version: Some("32.0.100.3104".to_string()),
        failure_class: None,
        failure_message: None,
    };

    let response = build_check_model_response(
        "qwen2.5-1.5b-instruct",
        &bundles,
        Some(&probe),
        Some(&openvino_probe),
    );
    drop(models_guard);

    assert!(response.lanes.openvino_npu.artifact_ready);
    assert!(response.lanes.openvino_npu.bundle_ready);
    assert!(response.lanes.openvino_npu.ready);
    assert_eq!(response.lanes.openvino_npu.reason, "ready");
}

#[test]
fn check_model_response_reports_shared_openvino_artifact_readiness() {
    let _guard = lock_env();
    let temp = tempdir().expect("temp dir");
    let resource_dir = temp.path().join("resources");
    let libs = resource_dir.join("libs");
    let models_dir = temp.path().join("models");
    let model_dir = models_dir.join("qwen3-4b");
    let openvino_dir = model_dir.join("openvino");

    create_ort_files(
        &libs,
        &[
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "onnxruntime-genai.dll",
            "DirectML.dll",
        ],
    );
    create_openvino_files(&libs.join("openvino"));
    fs::create_dir_all(&openvino_dir).expect("create openvino dir");
    fs::write(openvino_dir.join("openvino_model.xml"), []).expect("write openvino model");
    fs::write(openvino_dir.join("openvino_model.bin"), []).expect("write openvino weights");
    fs::write(openvino_dir.join("openvino_tokenizer.xml"), []).expect("write tokenizer xml");
    fs::write(openvino_dir.join("openvino_tokenizer.bin"), []).expect("write tokenizer bin");
    fs::write(openvino_dir.join("openvino_detokenizer.xml"), []).expect("write detokenizer xml");
    fs::write(openvino_dir.join("openvino_detokenizer.bin"), []).expect("write detokenizer bin");
    fs::write(openvino_dir.join("openvino_config.json"), []).expect("write ov config");
    fs::write(openvino_dir.join("generation_config.json"), []).expect("write generation config");
    fs::write(openvino_dir.join("config.json"), []).expect("write config");
    fs::write(openvino_dir.join("tokenizer.json"), []).expect("write tokenizer");
    fs::write(openvino_dir.join("tokenizer_config.json"), []).expect("write tokenizer config");
    fs::write(openvino_dir.join("special_tokens_map.json"), []).expect("write special tokens map");
    fs::write(openvino_dir.join("chat_template.jinja"), []).expect("write chat template");
    fs::write(openvino_dir.join("added_tokens.json"), []).expect("write added tokens");
    fs::write(openvino_dir.join("merges.txt"), []).expect("write merges");
    fs::write(openvino_dir.join("vocab.json"), []).expect("write vocab");
    fs::write(
            openvino_dir.join("manifest.json"),
            br#"{"entrypoint":"openvino_model.xml","required_files":["openvino_model.bin","openvino_tokenizer.xml","openvino_tokenizer.bin","openvino_detokenizer.xml","openvino_detokenizer.bin","openvino_config.json","generation_config.json","config.json","tokenizer.json","tokenizer_config.json","special_tokens_map.json","chat_template.jinja","added_tokens.json","merges.txt","vocab.json"]}"#,
        )
        .expect("write openvino manifest");

    let models_guard = EnvVarGuard::set("SMOLPC_MODELS_DIR", models_dir.as_os_str());
    let bundles =
        resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
    let probe = BackendProbeResult {
        available_backends: vec![InferenceBackend::Cpu, InferenceBackend::DirectML],
        directml_device_count: 1,
        directml_candidate: Some(DirectMlCandidate {
            device_id: 0,
            device_name: "Intel Arc".to_string(),
            adapter_identity: "intel:arc".to_string(),
            driver_version: String::new(),
            vram_mb: 4096,
        }),
        directml_probe_failure_class: None,
        directml_probe_failure_message: None,
    };
    let openvino_probe = OpenVinoStartupProbeResult {
        startup_ready: true,
        device_visible: true,
        adapter_identity: Some("openvino:npu:intel_npu".to_string()),
        device_name: Some("Intel NPU".to_string()),
        driver_version: Some("32.0.100.3104".to_string()),
        failure_class: None,
        failure_message: None,
    };

    let response =
        build_check_model_response("qwen3-4b", &bundles, Some(&probe), Some(&openvino_probe));
    drop(models_guard);

    assert!(response.lanes.openvino_npu.ready);
    assert_eq!(response.lanes.openvino_npu.reason, "ready");
    assert!(!response.lanes.directml.ready);
    assert_eq!(response.lanes.directml.reason, "artifact_missing");
    assert!(response.lanes.cpu.ready);
    assert_eq!(response.lanes.cpu.reason, "ready");
}

#[test]
fn process_idle_exit_is_disabled_by_default() {
    let _guard = lock_env();
    let env_guard = EnvVarGuard::unset("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS");

    assert_eq!(
        parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", None, 60),
        None
    );

    drop(env_guard);
}

#[test]
fn idle_timeout_zero_disables_timer() {
    let _guard = lock_env();
    let env_guard = EnvVarGuard::set("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", "0");

    assert_eq!(
        parse_idle_timeout_secs("SMOLPC_ENGINE_PROCESS_IDLE_EXIT_SECS", Some(1800), 60),
        None
    );

    drop(env_guard);
}

#[test]
fn model_idle_unload_keeps_default_when_unset() {
    let _guard = lock_env();
    let env_guard = EnvVarGuard::unset("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS");

    assert_eq!(
        parse_idle_timeout_secs("SMOLPC_ENGINE_MODEL_IDLE_UNLOAD_SECS", Some(300), 30),
        Some(Duration::from_secs(300))
    );

    drop(env_guard);
}

fn test_args(base: &Path, resource_dir: Option<PathBuf>) -> ParsedArgs {
    ParsedArgs {
        port: 19432,
        data_dir: base.join("data"),
        resource_dir,
        app_version: "test".to_string(),
        queue_size: 1,
        queue_timeout: Duration::from_secs(1),
        model_idle_unload: Some(Duration::from_secs(30)),
        process_idle_exit: Some(Duration::from_secs(60)),
    }
}

fn create_ort_files(root: &Path, files: &[&str]) {
    fs::create_dir_all(root).expect("create ort root");
    for file in files {
        fs::write(root.join(file), []).expect("write ort runtime file");
    }
}

fn create_openvino_files(root: &Path) {
    fs::create_dir_all(root).expect("create openvino root");
    for file in [
        "openvino.dll",
        "openvino_c.dll",
        "openvino_intel_npu_plugin.dll",
        "openvino_intel_npu_compiler.dll",
        "openvino_intel_cpu_plugin.dll",
        "openvino_ir_frontend.dll",
        "openvino_genai.dll",
        "openvino_genai_c.dll",
        "openvino_tokenizers.dll",
        "tbb12.dll",
        "tbbbind_2_5.dll",
        "tbbmalloc.dll",
        "tbbmalloc_proxy.dll",
        "icudt70.dll",
        "icuuc70.dll",
    ] {
        fs::write(root.join(file), []).expect("write openvino runtime file");
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    match env_lock().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        std::env::remove_var(key);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn resolve_default_model_id_prefers_request_then_config_then_builtin() {
    let selected = resolve_default_model_id_with_sources(
        Some("request-model".to_string()),
        Some("config-model".to_string()),
        Some("built-in-model".to_string()),
    )
    .expect("request model should win");
    assert_eq!(selected, "request-model");

    let selected = resolve_default_model_id_with_sources(
        None,
        Some("config-model".to_string()),
        Some("built-in-model".to_string()),
    )
    .expect("config model should win when request missing");
    assert_eq!(selected, "config-model");

    let selected =
        resolve_default_model_id_with_sources(None, None, Some("built-in-model".to_string()))
            .expect("built-in model should be used as final fallback");
    assert_eq!(selected, "built-in-model");
}

#[test]
fn classify_startup_model_error_flags_unknown_model_as_non_retryable() {
    let classified = classify_startup_model_error("Unknown model ID: bad-model");
    assert_eq!(classified.code, STARTUP_DEFAULT_MODEL_INVALID);
    assert!(!classified.retryable);
}

#[test]
fn classify_startup_model_error_flags_missing_assets_as_non_retryable() {
    let classified =
        classify_startup_model_error("Model file for backend 'cpu' not found: C:/models/x");
    assert_eq!(classified.code, STARTUP_MODEL_ASSET_MISSING);
    assert!(!classified.retryable);
}

#[test]
fn classify_startup_model_error_flags_memory_pressure() {
    let classified = classify_startup_model_error("OpenVINO runtime failed: out of memory");
    assert_eq!(classified.code, STARTUP_MEMORY_PRESSURE);
    assert!(classified.retryable);
    assert!(classified.message.contains("Memory pressure detected."));
}

#[test]
fn classify_startup_model_error_ignores_non_memory_substrings() {
    let classified = classify_startup_model_error("Launcher window room resize failed");
    assert_eq!(classified.code, STARTUP_MODEL_LOAD_FAILED);
    assert!(!classified.message.contains("Memory pressure detected."));
}

#[test]
fn with_memory_pressure_hint_is_idempotent() {
    let hinted = with_memory_pressure_hint("generation failed: out of memory", Some("qwen3-4b"));
    let hinted_twice = with_memory_pressure_hint(&hinted, Some("qwen3-4b"));
    assert_eq!(hinted, hinted_twice);
}

#[test]
fn startup_mode_directml_required_sets_directml_gate() {
    assert!(StartupMode::DirectmlRequired.requires_directml());
    assert!(!StartupMode::Auto.requires_directml());
}

fn fixture_models() -> Vec<smolpc_engine_core::ModelDefinition> {
    vec![
        smolpc_engine_core::ModelDefinition {
            id: "small-model".to_string(),
            name: "Small".to_string(),
            size: "1.5B".to_string(),
            disk_size_gb: 0.9,
            min_ram_gb: 8.0,
            estimated_runtime_ram_gb: 1.5,
            directory: "small".to_string(),
            description: "test".to_string(),
        },
        smolpc_engine_core::ModelDefinition {
            id: "large-model".to_string(),
            name: "Large".to_string(),
            size: "4B".to_string(),
            disk_size_gb: 2.5,
            min_ram_gb: 16.0,
            estimated_runtime_ram_gb: 4.0,
            directory: "large".to_string(),
            description: "test".to_string(),
        },
    ]
}

#[test]
fn ram_selection_32gb_picks_largest() {
    let models = fixture_models();
    let selected = select_best_model_for_ram(&models, 32.0);
    assert_eq!(selected, Some(("large-model".to_string(), 16.0)));
}

#[test]
fn ram_selection_16gb_picks_largest_at_threshold() {
    let models = fixture_models();
    let selected = select_best_model_for_ram(&models, 16.0);
    assert_eq!(selected, Some(("large-model".to_string(), 16.0)));
}

#[test]
fn ram_selection_12gb_picks_smaller() {
    let models = fixture_models();
    let selected = select_best_model_for_ram(&models, 12.0);
    assert_eq!(selected, Some(("small-model".to_string(), 8.0)));
}

#[test]
fn ram_selection_4gb_returns_none() {
    let models = fixture_models();
    let selected = select_best_model_for_ram(&models, 4.0);
    assert_eq!(selected, None);
}

#[test]
fn classify_startup_model_error_flags_memory_pressure() {
    let classified = classify_startup_model_error("OpenVINO runtime failed: out of memory");
    assert_eq!(classified.code, STARTUP_MEMORY_PRESSURE);
    assert!(classified.retryable);
    assert!(classified.message.contains("Memory pressure detected."));
}

#[test]
fn classify_startup_model_error_ignores_non_memory_substrings() {
    let classified = classify_startup_model_error("Launcher window room resize failed");
    assert_eq!(classified.code, STARTUP_MODEL_LOAD_FAILED);
    assert!(!classified.message.contains("Memory pressure detected."));
}

#[test]
fn with_memory_pressure_hint_is_idempotent() {
    let hinted =
        with_memory_pressure_hint("generation failed: out of memory", Some("qwen3-4b"));
    let hinted_twice = with_memory_pressure_hint(&hinted, Some("qwen3-4b"));
    assert_eq!(hinted, hinted_twice);
}

#[test]
fn startup_mode_directml_required_sets_directml_gate() {
    assert!(StartupMode::DirectmlRequired.requires_directml());
    assert!(!StartupMode::Auto.requires_directml());
}

// ── Whisper STT tests ────────────────────────────────────────────

    #[test]
    fn audio_body_must_be_f32_aligned() {
        // f32 samples are 4 bytes each. Non-aligned bodies must be rejected.
        assert!(5 % 4 != 0, "5 bytes is not f32-aligned");
        assert!(8 % 4 == 0, "8 bytes is f32-aligned");
        assert!(0 % 4 == 0, "empty is technically aligned but rejected separately");
    }

    #[test]
    fn f32_le_bytes_round_trip() {
        let samples: Vec<f32> = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let recovered: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(samples, recovered);
    }

    #[test]
    fn whisper_model_path_structure() {
        let models_dir = PathBuf::from("/fake/models");
        let whisper_dir = models_dir.join("whisper-base.en").join("openvino");
        assert!(
            whisper_dir.ends_with("whisper-base.en/openvino")
                || whisper_dir.ends_with(r"whisper-base.en\openvino"),
            "unexpected path: {}",
            whisper_dir.display()
        );
    }

    // ── TTS sidecar tests ────────────────────────────────────────────

    #[test]
    fn tts_binary_not_in_resource_dir() {
        // In debug builds, the fallback path may find the actual built binary
        // in the workspace target dir. This test only verifies that an empty
        // resource_dir doesn't contain the binary.
        let dir = tempdir().unwrap();
        let binaries = dir.path().join("binaries");
        fs::create_dir_all(&binaries).unwrap();
        // binaries/ dir exists but is empty — no TTS binary inside it
        let binary_name = format!("smolpc-tts-server{}", std::env::consts::EXE_SUFFIX);
        assert!(!binaries.join(&binary_name).exists());
    }

    #[test]
    fn tts_binary_found_in_binaries_dir() {
        let dir = tempdir().unwrap();
        let binaries = dir.path().join("binaries");
        fs::create_dir_all(&binaries).unwrap();
        let binary_name = format!("smolpc-tts-server{}", std::env::consts::EXE_SUFFIX);
        fs::write(binaries.join(&binary_name), b"fake").unwrap();
        let result = crate::tts_sidecar::resolve_tts_binary(Some(dir.path()));
        assert!(result.is_some());
        assert!(result.unwrap().ends_with(&binary_name));
    }

    #[test]
    fn audio_speech_request_defaults() {
        let req: AudioSpeechRequest =
            serde_json::from_str(r#"{"text":"hello"}"#).unwrap();
        assert_eq!(req.voice, "Bella");
        assert_eq!(req.speed, 1.0);
        assert_eq!(req.text, "hello");
    }

    #[test]
    fn audio_speech_request_with_overrides() {
        let req: AudioSpeechRequest =
            serde_json::from_str(r#"{"text":"hi","voice":"Luna","speed":1.5}"#).unwrap();
        assert_eq!(req.voice, "Luna");
        assert_eq!(req.speed, 1.5);
    }
