//! Tauri commands for GUI-driven edits to a live plan DAG. Writes go through
//! the same `upsert_plan_node` primitive the orchestrator's own plan-synthesis
//! code uses, so edits take effect automatically for any node the scheduler
//! hasn't dispatched yet.

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;
use vox_db::{PlanNodeRow, VoxDb};

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

#[derive(Debug, Deserialize)]
pub struct InsertPlanNodeInput {
    pub plan_session_id: String,
    pub plan_version: i64,
    pub node_id: String,
    pub description: String,
    pub depends_on: Vec<String>,
}

/// Insert a new intermediate step into the live plan. Joins the same
/// dependency graph `enqueue_runnable_plan_nodes` walks — becomes runnable
/// as soon as its `depends_on` entries complete, same as any agent-created
/// node.
///
/// Gated: if any existing node in this plan/version is still
/// `blocked_on_approval`, the plan hasn't been approved yet, so the new node
/// is inserted `blocked_on_approval` too — otherwise the GUI could create a
/// node the scheduler dispatches immediately, bypassing the approval gate
/// entirely.
#[tauri::command]
pub async fn insert_plan_node(
    pool: State<'_, GuiDbPool>,
    input: InsertPlanNodeInput,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let deps_json = serde_json::to_string(&input.depends_on).map_err(|e| e.to_string())?;
    let existing = db
        .load_plan_nodes_with_status(&input.plan_session_id, input.plan_version)
        .await
        .map_err(map_db_err)?;
    let plan_unapproved = existing.iter().any(|r| r.status == "blocked_on_approval");
    let status = if plan_unapproved {
        "blocked_on_approval"
    } else {
        "pending"
    };
    db.upsert_plan_node(
        &input.plan_session_id,
        input.plan_version,
        &input.node_id,
        &input.description,
        &deps_json,
        "{}",
        status,
        None,
    )
    .await
    .map_err(map_db_err)?;
    Ok(())
}

/// Read-only fetch of the current plan-node rows for a session/version, so
/// the GUI's plan panel can render the live DAG state.
#[tauri::command]
pub async fn list_plan_nodes(
    pool: State<'_, GuiDbPool>,
    plan_session_id: String,
    plan_version: i64,
) -> Result<Vec<PlanNodeRow>, String> {
    let db = pool_db(&pool)?;
    db.load_plan_nodes_with_status(&plan_session_id, plan_version)
        .await
        .map_err(map_db_err)
}

/// Batched open-task counts for the sidebar's task-count badges, one round trip for every
/// visible session instead of one `invoke` per session. Keyed by chat session id (matches
/// `ChatSessionDto::session_id`); a session absent from the returned map has zero open tasks.
#[tauri::command]
pub async fn plan_open_task_counts(
    pool: State<'_, GuiDbPool>,
    session_ids: Vec<String>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let db = pool_db(&pool)?;
    db.open_task_counts_for_sessions(&session_ids)
        .await
        .map_err(map_db_err)
}

/// Approve every `blocked_on_approval` node in a plan session so the scheduler
/// picks them up — the `PlanPanel` footer's "Approve" button. One-line
/// delegation to the same primitive `approve_plan_inner`
/// (`vox-orchestrator-mcp/src/chat_tools/plan.rs`) uses for the MCP/CLI path;
/// duplicated here rather than adding a `vox-gui` -> `vox-orchestrator-mcp`
/// crate edge for a single SQL call.
#[tauri::command]
pub async fn approve_plan_nodes(
    pool: State<'_, GuiDbPool>,
    plan_session_id: String,
) -> Result<u64, String> {
    let db = pool_db(&pool)?;
    db.approve_all_blocked_plan_nodes(&plan_session_id)
        .await
        .map_err(map_db_err)
}

/// The most recently updated `plan_sessions` row linked to a chat session, if any — used to
/// pick which plan DAG the sidebar's task badge opens when a chat session has dispatched more
/// than one goal (each dispatch mints its own `plan_sessions` row; see `goal.rs`).
#[tauri::command]
pub async fn latest_plan_session_for_chat(
    pool: State<'_, GuiDbPool>,
    session_id: String,
) -> Result<Option<String>, String> {
    let db = pool_db(&pool)?;
    db.latest_plan_session_id_for_origin(&session_id)
        .await
        .map_err(map_db_err)
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

    #[tokio::test]
    async fn insert_plan_node_adds_a_new_pending_node() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let db = pool.handle().unwrap();

        db.create_plan_session("ps2", None, "test goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("ps2", 1, None, None, None)
            .await
            .unwrap();

        insert_plan_node(
            pool,
            InsertPlanNodeInput {
                plan_session_id: "ps2".to_string(),
                plan_version: 1,
                node_id: "n-new".to_string(),
                description: "a step the user added".to_string(),
                depends_on: vec![],
            },
        )
        .await
        .unwrap();

        let rows = db.load_plan_nodes_with_status("ps2", 1).await.unwrap();
        let added = rows.iter().find(|r| r.node_id == "n-new").unwrap();
        assert_eq!(added.description, "a step the user added");
        assert_eq!(added.status, "pending");
    }

    #[tokio::test]
    async fn insert_plan_node_is_gated_when_the_plan_is_still_unapproved() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let db = pool.handle().unwrap();

        db.create_plan_session("ps-gate", None, "test goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("ps-gate", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node(
            "ps-gate",
            1,
            "n1",
            "first step",
            "[]",
            "{}",
            "blocked_on_approval",
            None,
        )
        .await
        .unwrap();

        insert_plan_node(
            pool,
            InsertPlanNodeInput {
                plan_session_id: "ps-gate".to_string(),
                plan_version: 1,
                node_id: "n-new".to_string(),
                description: "a step added before approval".to_string(),
                depends_on: vec![],
            },
        )
        .await
        .unwrap();

        let rows = db.load_plan_nodes_with_status("ps-gate", 1).await.unwrap();
        let added = rows.iter().find(|r| r.node_id == "n-new").unwrap();
        assert_eq!(
            added.status, "blocked_on_approval",
            "a node inserted into an unapproved plan must not be immediately runnable"
        );
    }

    #[tokio::test]
    async fn list_plan_nodes_returns_the_current_rows() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let db = pool.handle().unwrap();

        db.create_plan_session("ps3", None, "test goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("ps3", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("ps3", 1, "n1", "first step", "[]", "{}", "pending", None)
            .await
            .unwrap();

        let rows = list_plan_nodes(pool, "ps3".to_string(), 1).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].node_id, "n1");
        assert_eq!(rows[0].description, "first step");
    }

    #[tokio::test]
    async fn plan_open_task_counts_batches_across_sessions() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();
        let db = pool.handle().unwrap();

        db.create_plan_session("ps-count-1", Some("chat-x"), "goal", "sequential")
            .await
            .unwrap();
        db.append_plan_version("ps-count-1", 1, None, None, None)
            .await
            .unwrap();
        db.upsert_plan_node("ps-count-1", 1, "n1", "step", "[]", "{}", "pending", None)
            .await
            .unwrap();

        let counts = plan_open_task_counts(pool, vec!["chat-x".to_string(), "chat-y".to_string()])
            .await
            .unwrap();
        assert_eq!(counts.get("chat-x").copied(), Some(1));
        assert_eq!(counts.get("chat-y"), None);
    }

    #[tokio::test]
    async fn latest_plan_session_for_chat_returns_none_for_a_session_with_no_dispatched_goals() {
        let app = tauri::test::mock_app();
        app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
        let pool = app.state::<GuiDbPool>();

        let result = latest_plan_session_for_chat(pool, "chat-with-no-tasks".to_string())
            .await
            .unwrap();
        assert_eq!(result, None);
    }
}
