use crate::mode::AppMode;
use crate::provider::ToolExecutionResultDto;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessageDto {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSendRequestDto {
    pub mode: AppMode,
    pub chat_id: Option<String>,
    pub messages: Vec<AssistantMessageDto>,
    pub user_text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponseDto {
    pub reply: String,
    pub explain: Option<String>,
    pub undoable: bool,
    pub plan: Option<serde_json::Value>,
    pub tool_results: Vec<ToolExecutionResultDto>,
}
