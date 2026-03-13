use std::path::PathBuf;

pub const LAUNCHER_REGISTRY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LauncherCatalog {
    pub apps: Vec<LauncherCatalogApp>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LauncherCatalogApp {
    pub app_id: String,
    pub display_name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default, alias = "min_engine_protocol_major")]
    pub min_engine_api_major: Option<u64>,
    #[serde(default)]
    pub installer: Option<LauncherInstallerSpec>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LauncherInstallerSpec {
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    pub kind: InstallerKind,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallerKind {
    Exe,
    Msi,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LauncherRegistry {
    pub schema_version: u32,
    #[serde(default)]
    pub apps: Vec<LauncherRegistryApp>,
}

impl Default for LauncherRegistry {
    fn default() -> Self {
        Self {
            schema_version: LAUNCHER_REGISTRY_SCHEMA_VERSION,
            apps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LauncherRegistryApp {
    pub app_id: String,
    pub exe_path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub launch_command: Option<Vec<String>>,
    #[serde(default)]
    pub focus_command: Option<Vec<String>>,
    pub installed_at: String,
    pub source: String,
}

impl LauncherRegistryApp {
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(&self.exe_path)
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LauncherInstallState {
    NotInstalled,
    Installed,
    Broken,
}

#[derive(Debug, Clone)]
pub struct ResolvedLauncherApp {
    pub catalog: LauncherCatalogApp,
    pub registration: Option<LauncherRegistryApp>,
    pub install_state: LauncherInstallState,
}

impl ResolvedLauncherApp {
    pub fn launchable(&self) -> Result<LauncherLaunchableApp, String> {
        let app_id = self.catalog.app_id.clone();
        let display_name = self.catalog.display_name.clone();
        let min_engine_api_major = self.catalog.min_engine_api_major;
        let Some(registration) = self.registration.clone() else {
            return Err(format!(
                "App '{display_name}' is not installed. Install it from launcher first."
            ));
        };

        let executable = registration.executable_path();
        if !executable.exists() {
            return Err(format!(
                "App '{display_name}' is registered but executable is missing: {}",
                executable.display()
            ));
        }

        Ok(LauncherLaunchableApp {
            app_id,
            display_name,
            exe_path: registration.exe_path,
            args: registration.args,
            launch_command: registration.launch_command,
            focus_command: registration.focus_command,
            min_engine_api_major,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LauncherLaunchableApp {
    pub app_id: String,
    pub display_name: String,
    pub exe_path: String,
    pub args: Vec<String>,
    pub launch_command: Option<Vec<String>>,
    pub focus_command: Option<Vec<String>>,
    pub min_engine_api_major: Option<u64>,
}

impl LauncherLaunchableApp {
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(&self.exe_path)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LauncherAppSummary {
    pub app_id: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub min_engine_api_major: Option<u64>,
    pub install_state: LauncherInstallState,
    pub exe_path: Option<String>,
    pub is_running: bool,
    pub has_launch_command: bool,
    pub has_focus_command: bool,
    pub can_install: bool,
    pub can_repair: bool,
    pub manual_registration_required: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherInstallOutcome {
    Installed,
    RetryRequired,
    ManualRequired,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LauncherInstallResult {
    pub app_id: String,
    pub outcome: LauncherInstallOutcome,
    pub message: String,
    pub exe_path: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_catalog_app() -> LauncherCatalogApp {
        LauncherCatalogApp {
            app_id: "codehelper".to_string(),
            display_name: "Code Helper".to_string(),
            icon: None,
            min_engine_api_major: Some(1),
            installer: None,
        }
    }

    #[test]
    fn launchable_rejects_not_installed_state() {
        let resolved = ResolvedLauncherApp {
            catalog: sample_catalog_app(),
            registration: None,
            install_state: LauncherInstallState::NotInstalled,
        };

        let error = resolved
            .launchable()
            .expect_err("not installed should block launch");
        assert!(error.contains("not installed"));
    }

    #[test]
    fn launchable_rejects_missing_registered_executable() {
        let missing_path = std::env::temp_dir()
            .join("smolpc-launcher-types-tests")
            .join("missing-app.exe");
        let resolved = ResolvedLauncherApp {
            catalog: sample_catalog_app(),
            registration: Some(LauncherRegistryApp {
                app_id: "codehelper".to_string(),
                exe_path: missing_path.display().to_string(),
                args: vec![],
                launch_command: None,
                focus_command: None,
                installed_at: "0".to_string(),
                source: "installer".to_string(),
            }),
            install_state: LauncherInstallState::Broken,
        };

        let error = resolved
            .launchable()
            .expect_err("missing executable should block launch");
        assert!(error.contains("executable is missing"));
    }
}
