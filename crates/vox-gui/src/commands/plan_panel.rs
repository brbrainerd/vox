//! Tauri commands for GUI-driven edits to a live plan DAG. Writes go through
//! the same `upsert_plan_node` primitive the orchestrator's own plan-synthesis
//! code uses, so edits take effect automatically for any node the scheduler
//! hasn't dispatched yet.

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use vox_db::VoxDb;

use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

fn pool_db(pool: &GuiDbPool) -> Result<Arc<VoxDb>, String> {
    pool.handle()
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlanNodeInput {
    pub plan_session_id: String,
    pub plan_version: i64,
    pub node_id: String,
    pub description: String,
}

/// Edit a not-yet-dispatched plan node's description from the GUI. Writes
/// through the same `upsert_plan_node` primitive the orchestrator's own
/// plan-synthesis code uses — `enqueue_runnable_plan_nodes` re-reads current
/// DB state immediately before dispatching each node, so this edit takes
/// effect automatically for any node the scheduler hasn't reached yet.
#[tauri::command]
pub async fn update_plan_node(
    pool: State<'_, GuiDbPool>,
    input: UpdatePlanNodeInput,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let rows = db
        .load_plan_nodes_with_status(&input.plan_session_id, input.plan_version)
        .await
        .map_err(map_db_err)?;
    let existing = rows
        .iter()
        .find(|r| r.node_id == input.node_id)
        .ok_or_else(|| format!("plan node {} not found", input.node_id))?;
    db.upsert_plan_node(
        &input.plan_session_id,
        input.plan_version,
        &input.node_id,
        &input.description,
        &existing.dependencies_json,
        &existing.execution_policy_json,
        &existing.status,
        existing.workflow_invocation.as_deref(),
    )
    .await
    .map_err(map_db_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::gui_db_pool::GuiDbPool;
    use tauri::Manager;

    #[tokio::test]
    async fn update_plan_node_writes_through_to_the_db() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let db = pool.handle().unwrap();

        db.create_plan_session("ps1", None, "test goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("ps1", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node(
            "ps1",
            1,
            "n1",
            "original description",
            "[]",
            "{}",
            "pending",
            None,
        )
        .await
        .unwrap();

        update_plan_node(
            pool,
            UpdatePlanNodeInput {
                plan_session_id: "ps1".to_string(),
                plan_version: 1,
                node_id: "n1".to_string(),
                description: "edited description".to_string(),
            },
        )
        .await
        .unwrap();

        let rows = db.load_plan_nodes_with_status("ps1", 1).await.unwrap();
        let edited = rows.iter().find(|r| r.node_id == "n1").unwrap();
        assert_eq!(edited.description, "edited description");
    }
}
