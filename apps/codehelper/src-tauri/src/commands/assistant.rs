use crate::assistant::state::AssistantState;
use crate::assistant::{MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION, UNIFIED_ASSISTANT_NOT_IMPLEMENTED};
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
) -> Result<AssistantResponseDto, String> {
    state.clear_cancelled();
    let _ = (request, on_event, state.is_cancelled());
    Err(UNIFIED_ASSISTANT_NOT_IMPLEMENTED.to_string())
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
    provider
        .undo_last_action()
        .await
        .map_err(|_| MODE_UNDO_NOT_SUPPORTED_IN_FOUNDATION.to_string())
}
