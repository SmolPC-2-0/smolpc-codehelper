use crate::commands::engine_client_adapter::engine_status;
use crate::commands::inference::InferenceState;
use crate::modes::config::list_mode_configs;
use crate::modes::registry::ModeProviderRegistry;
use crate::setup::host_apps::{detect_blender, detect_gimp, detect_libreoffice, HostAppDetection};
use crate::setup::launch::{
    launch_blender_if_needed, launch_gimp_if_needed, launch_libreoffice_if_needed,
};
use smolpc_assistant_types::{AppMode, ModeConfigDto, ModeStatusDto, ProviderStateDto};

fn build_mode_status_dto(
    mode: AppMode,
    engine_ready: bool,
    provider_state: ProviderStateDto,
    available_tools: Vec<smolpc_assistant_types::ToolDefinitionDto>,
    last_error: Option<String>,
) -> ModeStatusDto {
    ModeStatusDto {
        mode,
        engine_ready,
        provider_state,
        available_tools,
        last_error,
    }
}

fn detection_error_detail(detection: &HostAppDetection) -> String {
    detection.detail.clone().unwrap_or_else(|| {
        format!(
            "{} is not installed or could not be detected yet.",
            detection.label
        )
    })
}

fn open_host_app_for_mode(mode: AppMode) -> Result<(), String> {
    match mode {
        AppMode::Code => Err("Code mode does not have a host app to open.".to_string()),
        AppMode::Gimp => {
            let detection = detect_gimp(None);
            let path = detection
                .path
                .clone()
                .ok_or_else(|| detection_error_detail(&detection))?;
            launch_gimp_if_needed(&path)?;
            Ok(())
        }
        AppMode::Blender => {
            let detection = detect_blender(None);
            let path = detection
                .path
                .clone()
                .ok_or_else(|| detection_error_detail(&detection))?;
            launch_blender_if_needed(&path)?;
            Ok(())
        }
        AppMode::Writer | AppMode::Calc | AppMode::Impress => {
            let detection = detect_libreoffice(None);
            let path = detection
                .path
                .clone()
                .ok_or_else(|| detection_error_detail(&detection))?;
            launch_libreoffice_if_needed(&path)?;
            Ok(())
        }
    }
}

async fn collect_mode_status(
    mode: AppMode,
    app_handle: tauri::AppHandle,
    inference_state: tauri::State<'_, InferenceState>,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<ModeStatusDto, String> {
    let provider = registry.provider_for_mode(mode);
    let provider_state = provider.status(mode).await?;
    let available_tools = provider.list_tools(mode).await?;

    let (engine_ready, last_error) = match engine_status(app_handle, inference_state).await {
        Ok(readiness) => (
            readiness.state == "ready",
            if readiness.state == "failed" {
                readiness.error_message.or(readiness.error_code)
            } else {
                None
            },
        ),
        Err(error) => (false, Some(error)),
    };

    let merged_last_error = last_error.or_else(|| {
        if matches!(provider_state.state.as_str(), "disconnected" | "error") {
            provider_state.detail.clone()
        } else {
            None
        }
    });

    Ok(build_mode_status_dto(
        mode,
        engine_ready,
        provider_state,
        available_tools,
        merged_last_error,
    ))
}

#[tauri::command]
pub fn list_modes() -> Vec<ModeConfigDto> {
    list_mode_configs()
}

#[tauri::command]
pub async fn mode_status(
    mode: AppMode,
    app_handle: tauri::AppHandle,
    inference_state: tauri::State<'_, InferenceState>,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<ModeStatusDto, String> {
    collect_mode_status(mode, app_handle, inference_state, registry).await
}

#[tauri::command]
pub async fn mode_refresh_tools(
    mode: AppMode,
    app_handle: tauri::AppHandle,
    inference_state: tauri::State<'_, InferenceState>,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<ModeStatusDto, String> {
    // Phase 4: GIMP uses refresh to force a reconnect and live tool discovery.
    let provider = registry.provider_for_mode(mode);
    let _ = provider.disconnect_if_needed(mode).await;
    let _ = provider.connect_if_needed(mode).await;
    collect_mode_status(mode, app_handle, inference_state, registry).await
}

#[tauri::command]
pub fn mode_open_host_app(mode: AppMode) -> Result<(), String> {
    open_host_app_for_mode(mode)
}

#[cfg(test)]
mod tests {
    use super::build_mode_status_dto;
    use smolpc_assistant_types::{AppMode, ProviderStateDto};

    #[test]
    fn mode_status_dto_uses_camel_case_keys() {
        let dto = build_mode_status_dto(
            AppMode::Calc,
            false,
            ProviderStateDto {
                mode: AppMode::Calc,
                state: "disconnected".to_string(),
                detail: Some("Provider not integrated yet".to_string()),
                supports_tools: true,
                supports_undo: false,
            },
            Vec::new(),
            Some("engine offline".to_string()),
        );

        let value = serde_json::to_value(dto).expect("serialize mode status");
        assert_eq!(value["providerState"]["mode"], "calc");
        assert_eq!(value["engineReady"], false);
        assert_eq!(value["lastError"], "engine offline");
    }
}
