use crate::mode::AppMode;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStateDto {
    pub mode: AppMode,
    pub state: String,
    pub detail: Option<String>,
    pub supports_tools: bool,
    pub supports_undo: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinitionDto {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionResultDto {
    pub name: String,
    pub ok: bool,
    pub summary: String,
    pub payload: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModeStatusDto {
    pub mode: AppMode,
    pub engine_ready: bool,
    pub provider_state: ProviderStateDto,
    pub available_tools: Vec<ToolDefinitionDto>,
    pub last_error: Option<String>,
}
