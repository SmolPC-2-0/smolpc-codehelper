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
    pub icon: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub launch_command: Option<Vec<String>>,
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
    pub icon: Option<String>,
    pub is_running: bool,
    pub has_launch_command: bool,
    pub has_focus_command: bool,
    pub min_engine_api_major: Option<u64>,
}

impl LauncherAppSummary {
    pub fn from_manifest_app(app: &LauncherManifestApp, is_running: bool) -> Self {
        Self {
            app_id: app.app_id.clone(),
            display_name: app.display_name.clone(),
            exe_path: app.exe_path.clone(),
            icon: app.icon.clone(),
            is_running,
            has_launch_command: app
                .launch_command
                .as_ref()
                .is_some_and(|command| !command.is_empty()),
            has_focus_command: app
                .focus_command
                .as_ref()
                .is_some_and(|command| !command.is_empty()),
            min_engine_api_major: app.min_engine_api_major,
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineStatusSummary {
    pub reachable: bool,
    pub ready: bool,
    pub state: Option<String>,
    pub active_model: Option<String>,
}
