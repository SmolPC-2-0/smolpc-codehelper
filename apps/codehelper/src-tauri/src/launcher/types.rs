use std::path::PathBuf;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LauncherManifest {
    pub apps: Vec<LauncherManifestApp>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LauncherManifestApp {
    pub app_id: String,
    pub display_name: String,
    pub exe_path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub focus_command: Option<Vec<String>>,
    #[serde(default, alias = "min_engine_protocol_major")]
    pub min_engine_api_major: Option<u64>,
}

impl LauncherManifestApp {
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(&self.exe_path)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LauncherAppSummary {
    pub app_id: String,
    pub display_name: String,
    pub exe_path: String,
    pub has_focus_command: bool,
    pub min_engine_api_major: Option<u64>,
}

impl From<&LauncherManifestApp> for LauncherAppSummary {
    fn from(value: &LauncherManifestApp) -> Self {
        Self {
            app_id: value.app_id.clone(),
            display_name: value.display_name.clone(),
            exe_path: value.exe_path.clone(),
            has_focus_command: value
                .focus_command
                .as_ref()
                .is_some_and(|command| !command.is_empty()),
            min_engine_api_major: value.min_engine_api_major,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchAction {
    Launched,
    Focused,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineApiGateInfo {
    pub required_major: Option<u64>,
    pub actual_version: String,
    pub actual_major: Option<u64>,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LauncherLaunchResult {
    pub app_id: String,
    pub action: LaunchAction,
    pub readiness_state: Option<String>,
    pub readiness_attempt_id: Option<String>,
    pub engine_api_gate: EngineApiGateInfo,
}
