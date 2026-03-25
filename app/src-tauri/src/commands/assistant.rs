use crate::assistant::state::AssistantState;
use crate::assistant::UNIFIED_ASSISTANT_NOT_IMPLEMENTED;
use smolpc_connector_common::MODE_UNDO_NOT_SUPPORTED;
use crate::engine::EngineSupervisorHandle;
use smolpc_connector_blender::execute_blender_request;
use smolpc_connector_gimp::{execute_gimp_request, EngineTextGenerator};
use smolpc_connector_libreoffice::{execute_libreoffice_request, EngineTextPlanner};
use crate::modes::registry::ModeProviderRegistry;
use smolpc_connector_common::EngineTextStreamer;
use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantSendRequestDto, AssistantStreamEventDto,
};
use std::time::Duration;
use tauri::ipc::Channel;

#[tauri::command]
pub async fn assistant_send(
    request: AssistantSendRequestDto,
    on_event: Channel<AssistantStreamEventDto>,
    state: tauri::State<'_, AssistantState>,
    supervisor: tauri::State<'_, EngineSupervisorHandle>,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<AssistantResponseDto, String> {
    state.clear_cancelled();

    let result = match request.mode {
        AppMode::Gimp => {
            let provider = registry.provider_for_mode(AppMode::Gimp);
            let engine_client = supervisor.get_client(Duration::from_secs(60)).await?;
            let generator = EngineTextGenerator::new(engine_client);

            execute_gimp_request(provider, &generator, &request, &*state, |event| {
                if let Err(error) = on_event.send(event) {
                    log::warn!("Failed to emit GIMP assistant event: {error}");
                }
            })
            .await
        }
        AppMode::Blender => {
            let provider = registry.provider_for_mode(AppMode::Blender);
            let engine_client = supervisor.get_client(Duration::from_secs(60)).await?;
            let generator = EngineTextStreamer::new(engine_client);

            execute_blender_request(provider, &generator, &request, &*state, |event| {
                if let Err(error) = on_event.send(event) {
                    log::warn!("Failed to emit Blender assistant event: {error}");
                }
            })
            .await
        }
        AppMode::Writer | AppMode::Impress => {
            let provider = registry.provider_for_mode(request.mode);
            let engine_client = supervisor.get_client(Duration::from_secs(60)).await?;
            let planner = EngineTextPlanner::new(engine_client.clone());
            let generator = EngineTextStreamer::new(engine_client);

            execute_libreoffice_request(provider, &planner, &generator, &request, &*state, |event| {
                if let Err(error) = on_event.send(event) {
                    log::warn!("Failed to emit LibreOffice assistant event: {error}");
                }
            })
            .await
        }
        _ => Err(UNIFIED_ASSISTANT_NOT_IMPLEMENTED.to_string()),
    };

    if let Err(message) = &result {
        let code = if message == "ASSISTANT_CANCELLED" {
            "ASSISTANT_CANCELLED"
        } else if message == UNIFIED_ASSISTANT_NOT_IMPLEMENTED {
            "UNIFIED_ASSISTANT_NOT_IMPLEMENTED"
        } else if request.mode == AppMode::Blender {
            "BLENDER_ASSISTANT_FAILED"
        } else if matches!(request.mode, AppMode::Writer | AppMode::Impress) {
            "LIBREOFFICE_ASSISTANT_FAILED"
        } else {
            "GIMP_ASSISTANT_FAILED"
        };
        if let Err(error) = on_event.send(AssistantStreamEventDto::Error {
            code: code.to_string(),
            message: message.clone(),
        }) {
            log::warn!("Failed to emit assistant error event: {error}");
        }
    }

    result
}

#[tauri::command]
pub async fn assistant_cancel(
    state: tauri::State<'_, AssistantState>,
    supervisor: tauri::State<'_, EngineSupervisorHandle>,
) -> Result<(), String> {
    state.mark_cancelled();

    if let Some(client) = supervisor.get_client_if_ready() {
        let _ = client.cancel().await;
    }

    Ok(())
}

#[tauri::command]
pub async fn mode_undo(
    mode: AppMode,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<(), String> {
    let provider = registry.provider_for_mode(mode);
    provider.undo_last_action(mode).await.map_err(|error| {
        if mode == AppMode::Gimp {
            error
        } else {
            MODE_UNDO_NOT_SUPPORTED.to_string()
        }
    })
}
