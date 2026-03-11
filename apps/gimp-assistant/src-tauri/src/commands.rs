use serde_json;
use crate::plan_execute::StepResult;
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunPlanResponse {
    pub plan: Value,
    pub results: Vec<StepResult>,
}

// #[tauri::command(rename = "run_action_plan")]
// pub async fn run_action_plan(plan_json: String) -> Result<Vec<StepResult>, String> {
//     let plan = generate_action_plan(&plan_json)
//     .await
//     .map_err(|e| format!("plan generation failed: {e}"))?;

//     let plan: crate::plan_schema::ActionPlan =
//         serde_json::from_str(&plan_json)
//             .map_err(|e| format!("Invalid ActionPlan JSON: {e}"))?;

//     crate::plan_execute::execute_plan(plan)
// }
#[tauri::command(rename = "run_action_plan")]
pub async fn run_action_plan(user_text: String) -> Result<RunPlanResponse, String> {
    let plan = generate_action_plan(&user_text).await
        .map_err(|e| format!("plan generation failed: {e}"))?;

    // Convert the plan struct back to JSON so UI can display it
    let plan_json = serde_json::to_value(&plan)
        .map_err(|e| format!("failed to serialize plan: {e}"))?;

    let results = crate::plan_execute::execute_plan(plan)
        .map_err(|e| format!("plan execution failed: {e}"))?;

    Ok(RunPlanResponse { plan: plan_json, results })
}


// #[tauri::command(rename = "generate_action_plan")]
// pub async fn generate_action_plan(user_text: String) -> Result<crate::plan_schema::ActionPlan, String> {
//     crate::plan_llm::make_plan_from_text(&user_text).await
// }
pub async fn generate_action_plan(user_text: &str)
    -> Result<crate::plan_schema::ActionPlan, String>
{
    crate::plan_llm::make_plan_from_text(user_text).await
}
