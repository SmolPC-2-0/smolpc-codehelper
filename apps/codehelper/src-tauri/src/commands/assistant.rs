use crate::assistant::state::AssistantState;
use crate::assistant::{MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION, UNIFIED_ASSISTANT_NOT_IMPLEMENTED};
use crate::commands::inference::{resolve_generation_client, InferenceState};
use crate::modes::gimp::{execute_gimp_request, EngineTextGenerator};
use crate::modes::registry::ModeProviderRegistry;
use smolpc_assistant_types::{
    AppMode, AssistantResponseDto, AssistantSendRequestDto, AssistantStreamEventDto,
};
use tauri::ipc::Channel;

#[tauri::command]
pub async fn assistant_send(
    request: AssistantSendRequestDto,
    on_event: Channel<AssistantStreamEventDto>,
    state: tauri::State<'_, AssistantState>,
    app_handle: tauri::AppHandle,
    inference_state: tauri::State<'_, InferenceState>,
    registry: tauri::State<'_, ModeProviderRegistry>,
) -> Result<AssistantResponseDto, String> {
    state.clear_cancelled();

    if request.mode != AppMode::Gimp {
        return Err(UNIFIED_ASSISTANT_NOT_IMPLEMENTED.to_string());
    }

    let provider = registry.provider_for_mode(AppMode::Gimp);
    let engine_client = resolve_generation_client(&app_handle, &inference_state).await?;
    let generator = EngineTextGenerator::new(engine_client);

    let result = execute_gimp_request(provider, &generator, &request, &state, |event| {
        if let Err(error) = on_event.send(event) {
            log::warn!("Failed to emit GIMP assistant event: {error}");
        }
    })
    .await;

    if let Err(message) = &result {
        let code = if message == "ASSISTANT_CANCELLED" {
            "ASSISTANT_CANCELLED"
        } else {
            "GIMP_ASSISTANT_FAILED"
        };
        if let Err(error) = on_event.send(AssistantStreamEventDto::Error {
            code: code.to_string(),
            message: message.clone(),
        }) {
            log::warn!("Failed to emit GIMP assistant error event: {error}");
        }
    }

    result
}

#[tauri::command]
pub fn assistant_cancel(state: tauri::State<'_, AssistantState>) -> Result<(), String> {
    state.mark_cancelled();
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
            MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string()
        }
    })
}
