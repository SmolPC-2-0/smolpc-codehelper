use crate::launcher::catalog;
use crate::launcher::orchestrator::{
    install_app, is_app_running_in_snapshot, launch_or_focus, register_manual_path, LauncherState,
};
use crate::launcher::types::{
    LauncherAppSummary, LauncherInstallResult, LauncherInstallState, LauncherLaunchResult,
};

#[tauri::command]
pub async fn launcher_list_apps(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, LauncherState>,
) -> Result<Vec<LauncherAppSummary>, String> {
    let catalog_doc = catalog::load_catalog(&app_handle)?;
    let registry_doc = catalog::load_registry(&app_handle)?;
    let manual_required = state.manual_required_apps().await;
    let running_names = state.running_process_names().await;

    Ok(
        catalog::merge_catalog_and_registry(&catalog_doc, &registry_doc)
            .into_iter()
            .map(|resolved| {
                let app_id = resolved.catalog.app_id.clone();
                let registration = resolved.registration.as_ref();
                let exe_path = registration.map(|entry| entry.exe_path.clone());
                let has_launch_command = registration
                    .and_then(|entry| entry.launch_command.as_ref())
                    .is_some_and(|command| !command.is_empty());
                let has_focus_command = registration
                    .and_then(|entry| entry.focus_command.as_ref())
                    .is_some_and(|command| !command.is_empty());
                let is_running = match (registration, resolved.install_state.clone()) {
                    (Some(entry), LauncherInstallState::Installed) => {
                        is_app_running_in_snapshot(&entry.executable_path(), &running_names)
                    }
                    _ => false,
                };
                let manual_registration_required = manual_required.contains(&app_id);

                LauncherAppSummary {
                    app_id,
                    display_name: resolved.catalog.display_name,
                    icon: resolved.catalog.icon,
                    min_engine_api_major: resolved.catalog.min_engine_api_major,
                    install_state: resolved.install_state.clone(),
                    exe_path,
                    is_running,
                    has_launch_command,
                    has_focus_command,
                    can_install: resolved.catalog.installer.is_some(),
                    can_repair: resolved.install_state != LauncherInstallState::Installed
                        || manual_registration_required,
                    manual_registration_required,
                }
            })
            .collect(),
    )
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

#[tauri::command]
pub async fn launcher_install_app(
    app_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, LauncherState>,
) -> Result<LauncherInstallResult, String> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err("app_id cannot be empty".to_string());
    }

    install_app(app_id, &app_handle, state.inner()).await
}

#[tauri::command]
pub async fn launcher_register_manual_path(
    app_id: String,
    exe_path: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, LauncherState>,
) -> Result<LauncherInstallResult, String> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return Err("app_id cannot be empty".to_string());
    }
    let exe_path = exe_path.trim();
    if exe_path.is_empty() {
        return Err("exe_path cannot be empty".to_string());
    }

    register_manual_path(app_id, exe_path, &app_handle, state.inner()).await
}

#[tauri::command]
pub async fn launcher_pick_manual_exe() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| {
        Ok::<Option<String>, String>(
            rfd::FileDialog::new()
                .set_title("Select App Executable")
                .add_filter("Executable", &["exe"])
                .pick_file()
                .map(|path| path.display().to_string()),
        )
    })
    .await
    .map_err(|error| format!("Failed to join manual picker task: {error}"))?
}
