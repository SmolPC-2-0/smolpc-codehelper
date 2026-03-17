use crate::setup::{collect_setup_status, prepare_setup, SetupState};
use smolpc_assistant_types::SetupStatusDto;

#[tauri::command]
pub async fn setup_status(state: tauri::State<'_, SetupState>) -> Result<SetupStatusDto, String> {
    Ok(collect_setup_status(&state).await)
}

#[tauri::command]
pub async fn setup_prepare(state: tauri::State<'_, SetupState>) -> Result<SetupStatusDto, String> {
    Ok(prepare_setup(&state).await)
}
