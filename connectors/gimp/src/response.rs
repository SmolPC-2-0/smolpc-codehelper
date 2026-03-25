use serde_json::Value;
use smolpc_assistant_types::ToolExecutionResultDto;

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
        || payload
            .get("structuredContent")
            .and_then(|content| content.get("result"))
            .and_then(Value::as_str)
            .map(|text| text.starts_with("Error:"))
            .unwrap_or(false)
}

pub fn summarize_tool_result(name: &str, payload: &Value) -> String {
    if let Some(text) = payload
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)
    {
        return text.to_string();
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

pub fn parse_gimp_info_reply(payload: &Value) -> Option<String> {
    let text = payload
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)?;
    let info: Value = serde_json::from_str(text).ok()?;
    let version = info
        .get("version")
        .and_then(|value| value.get("detected_version"))
        .and_then(Value::as_str)
        .unwrap_or("unknown version");
    let platform = info
        .get("system")
        .and_then(|value| value.get("platform"))
        .and_then(Value::as_str)
        .unwrap_or("unknown platform");

    Some(format!("You are using GIMP {version} on {platform}."))
}

pub fn parse_image_metadata_reply(payload: &Value) -> Option<String> {
    let text = payload
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)?;
    let metadata: Value = serde_json::from_str(text).ok()?;
    let basic = metadata.get("basic").unwrap_or(&Value::Null);
    let file = metadata.get("file").unwrap_or(&Value::Null);
    let width = basic
        .get("width")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let height = basic
        .get("height")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let base_type = basic
        .get("base_type")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let basename = file
        .get("basename")
        .and_then(Value::as_str)
        .unwrap_or("unknown image");

    Some(format!(
        "Your current image \"{basename}\" is {width}×{height} pixels with base type {base_type}."
    ))
}
