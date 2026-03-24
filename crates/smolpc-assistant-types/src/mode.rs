use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Code,
    Gimp,
    Blender,
    Writer,
    Impress,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Local,
    Mcp,
    Hybrid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModeCapabilitiesDto {
    pub supports_tools: bool,
    pub supports_undo: bool,
    pub show_model_info: bool,
    pub show_hardware_panel: bool,
    pub show_export: bool,
    pub show_context_controls: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModeConfigDto {
    pub id: AppMode,
    pub label: String,
    pub subtitle: String,
    pub icon: String,
    pub provider_kind: ProviderKind,
    pub system_prompt_key: String,
    pub suggestions: Vec<String>,
    pub capabilities: ModeCapabilitiesDto,
}
