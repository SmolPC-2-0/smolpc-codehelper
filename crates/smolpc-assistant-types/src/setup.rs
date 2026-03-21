use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetupItemStateDto {
    Ready,
    Missing,
    NotPrepared,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupItemDto {
    pub id: String,
    pub label: String,
    pub state: SetupItemStateDto,
    pub detail: Option<String>,
    pub required: bool,
    pub can_prepare: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetupOverallStateDto {
    Ready,
    NeedsAttention,
    Error,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusDto {
    pub overall_state: SetupOverallStateDto,
    pub items: Vec<SetupItemDto>,
    pub last_error: Option<String>,
}
