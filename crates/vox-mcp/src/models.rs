use schemars::JsonSchema;
use serde::Deserialize;
use vox_orchestrator::types::TaskCategory;

use crate::{ServerState, ToolResult};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListModelsParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestModelParams {
    pub task_category: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetModelParams {
    pub agent_id: u64,
    pub model_id: String,
}

pub async fn list_models(state: &ServerState, _params: ListModelsParams) -> String {
    let orch = state.orchestrator.lock().await;
    let models = orch.models().list_models();
    ToolResult::ok(models).to_json()
}

pub async fn suggest_model(state: &ServerState, params: SuggestModelParams) -> String {
    let orch = state.orchestrator.lock().await;

    // Parse task_category from string
    let category = match params.task_category.to_lowercase().as_str() {
        "parsing" => TaskCategory::Parsing,
        "typechecking" => TaskCategory::TypeChecking,
        "debugging" => TaskCategory::Debugging,
        "research" => TaskCategory::Research,
        "testing" => TaskCategory::Testing,
        "codegen" => TaskCategory::CodeGen,
        "review" => TaskCategory::Review,
        _ => TaskCategory::CodeGen, // Default fallback
    };

    let preference = orch.config().cost_preference;
    let complexity = 5; // Default for interactive suggestions
    if let Some(model) = orch.models().best_for(category, complexity, preference) {
        ToolResult::ok(model).to_json()
    } else {
        ToolResult::<String>::err("No suitable model found for category").to_json()
    }
}

pub async fn set_model(state: &ServerState, params: SetModelParams) -> String {
    let mut orch = state.orchestrator.lock().await;

    if orch.models().get(&params.model_id).is_some() {
        orch.models_mut()
            .set_override(params.agent_id, params.model_id.clone());
        ToolResult::ok(format!(
            "Successfully overridden model to {} for agent {}",
            params.model_id, params.agent_id
        ))
        .to_json()
    } else {
        ToolResult::<String>::err(format!("Model {} not found in registry", params.model_id))
            .to_json()
    }
}
