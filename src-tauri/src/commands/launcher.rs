use crate::launcher::catalog;
use crate::launcher::orchestrator::{launch_or_focus, LauncherState};
use crate::launcher::types::{LauncherAppSummary, LauncherLaunchResult};

#[tauri::command]
pub fn launcher_list_apps(app_handle: tauri::AppHandle) -> Result<Vec<LauncherAppSummary>, String> {
    catalog::list_apps(&app_handle)
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
