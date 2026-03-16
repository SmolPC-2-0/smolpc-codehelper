use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantStreamEventDto, ModeCapabilitiesDto, ModeConfigDto,
    ProviderKind, ToolExecutionResultDto,
};

#[test]
fn app_mode_serializes_to_expected_wire_values() {
    let value = serde_json::to_string(&AppMode::Impress).expect("serialize app mode");
    assert_eq!(value, "\"impress\"");
}

#[test]
fn mode_config_uses_camel_case_keys() {
    let dto = ModeConfigDto {
        id: AppMode::Code,
        label: "Code".to_string(),
        subtitle: "Coding help".to_string(),
        icon: "code".to_string(),
        provider_kind: ProviderKind::Local,
        system_prompt_key: "mode.code.default".to_string(),
        suggestions: vec!["Explain this error".to_string()],
        capabilities: ModeCapabilitiesDto {
            supports_tools: false,
            supports_undo: false,
            show_model_info: true,
            show_hardware_panel: true,
            show_benchmark_panel: true,
            show_export: true,
            show_context_controls: true,
        },
    };

    let value = serde_json::to_value(dto).expect("serialize mode config");
    assert_eq!(value["providerKind"], "local");
    assert_eq!(value["systemPromptKey"], "mode.code.default");
    assert_eq!(value["capabilities"]["showBenchmarkPanel"], true);
}

#[test]
fn assistant_stream_events_use_kind_tag() {
    let event = AssistantStreamEventDto::ToolResult {
        name: "resize_image".to_string(),
        result: ToolExecutionResultDto {
            name: "resize_image".to_string(),
            ok: true,
            summary: "Image resized".to_string(),
            payload: serde_json::json!({ "width": 800 }),
        },
    };

    let value = serde_json::to_value(event).expect("serialize stream event");
    assert_eq!(value["kind"], "tool_result");
    assert_eq!(value["name"], "resize_image");
    assert_eq!(value["result"]["payload"]["width"], 800);
}

#[test]
fn assistant_response_uses_tool_results_key() {
    let response = AssistantResponseDto {
        reply: "Done".to_string(),
        explain: Some("Used a provider".to_string()),
        undoable: false,
        plan: Some(serde_json::json!({ "type": "noop" })),
        tool_results: vec![],
    };

    let value = serde_json::to_value(response).expect("serialize response");
    assert!(value.get("toolResults").is_some());
    assert!(value.get("tool_results").is_none());
}
