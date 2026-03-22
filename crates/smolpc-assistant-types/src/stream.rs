use crate::assistant::AssistantResponseDto;
use crate::provider::ToolExecutionResultDto;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssistantStreamEventDto {
    Status {
        phase: String,
        detail: String,
    },
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    ToolResult {
        name: String,
        result: ToolExecutionResultDto,
    },
    Token {
        token: String,
    },
    Complete {
        response: AssistantResponseDto,
    },
    Error {
        code: String,
        message: String,
    },
}
