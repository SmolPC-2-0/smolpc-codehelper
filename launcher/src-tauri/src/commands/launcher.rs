use crate::launcher::catalog;
use crate::launcher::orchestrator::{is_app_running, launch_or_focus, LauncherState};
use crate::launcher::types::{LauncherAppSummary, LauncherLaunchResult};

#[tauri::command]
pub fn launcher_list_apps(
    app_handle: tauri::AppHandle,
) -> Result<Vec<LauncherAppSummary>, String> {
    let manifest = catalog::load_manifest(&app_handle)?;
    Ok(manifest
        .apps
        .iter()
        .map(|app| {
            let running = is_app_running(&app.executable_path());
            LauncherAppSummary::from_manifest_app(app, running)
        })
        .collect())
}

#[tauri::command]
pub async fn launcher_launch_or_focus(
    app_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, LauncherState>,
) -> Result<LauncherLaunchResult, String> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err("app_id cannot be empty".to_string());
    }

    launch_or_focus(app_id, &app_handle, state.inner()).await
}
