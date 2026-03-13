use crate::launcher::orchestrator::LauncherState;
use crate::launcher::types::EngineStatusSummary;

/// Peek at the cached engine client (does not spawn) and return its status.
/// If no client is cached or the engine is unreachable, returns a "not connected" summary.
#[tauri::command]
pub async fn engine_status(
    state: tauri::State<'_, LauncherState>,
) -> Result<EngineStatusSummary, String> {
    let Some(client) = state.peek_client().await else {
        return Ok(EngineStatusSummary {
            reachable: false,
            ready: false,
            state: None,
            active_model: None,
        });
    };

    let reachable = client.health().await.unwrap_or(false);
    if !reachable {
        return Ok(EngineStatusSummary {
            reachable: false,
            ready: false,
            state: None,
            active_model: None,
        });
    }

    match client.status().await {
        Ok(status) => Ok(EngineStatusSummary {
            reachable: true,
            ready: status.is_ready(),
            state: status.state.or(status.startup_phase),
            active_model: status.active_model_id,
        }),
        Err(_) => Ok(EngineStatusSummary {
            reachable: false,
            ready: false,
            state: None,
            active_model: None,
        }),
    }
}
