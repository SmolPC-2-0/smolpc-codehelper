use serde_json;
use crate::plan_execute::StepResult;
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlanResponse {
    pub plan: Value,
    pub results: Vec<StepResult>,
}

#[tauri::command(rename = "run_action_plan")]
pub async fn run_action_plan(
    user_text: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, crate::engine::EngineState>,
) -> Result<RunPlanResponse, String> {
    let client = crate::engine::resolve_client(&app_handle, &state).await?;

    let plan = crate::plan_llm::make_plan_from_text(&client, &user_text).await
        .map_err(|e| format!("plan generation failed: {e}"))?;

    let plan_json = serde_json::to_value(&plan)
        .map_err(|e| format!("failed to serialize plan: {e}"))?;

    let results = crate::plan_execute::execute_plan(plan)
        .map_err(|e| format!("plan execution failed: {e}"))?;

    Ok(RunPlanResponse { plan: plan_json, results })
}
