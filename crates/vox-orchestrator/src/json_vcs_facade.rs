//! JSON payloads shared by MCP VCS tools and `vox dei` CLI (parity surface).

use serde_json::{Value, json};

use crate::Orchestrator;
use crate::snapshot::SnapshotId;
use crate::types::AgentId;

/// List recent snapshots (same shape as MCP `vox_snapshot_list`).
pub fn snapshot_list_json(orch: &Orchestrator, agent_id: Option<u64>, limit: usize) -> Value {
    let agent = agent_id.map(AgentId);
    let handle = orch.snapshot_store_handle();
    let store = crate::sync_lock::rw_read(&*handle);
    let snaps = store.list(agent, limit);
    let items: Vec<Value> = snaps
        .iter()
        .map(|s| {
            json!({
                "id": s.id.to_string(),
                "agent_id": s.agent_id.0.to_string(),
                "timestamp_ms": s.timestamp_ms,
                "description": s.description,
                "file_count": s.files.len(),
            })
        })
        .collect();
    json!({ "snapshots": items })
}

/// Diff two snapshots by numeric id (same shape as MCP `vox_snapshot_diff`).
pub fn snapshot_diff_json(orch: &Orchestrator, before_id: u64, after_id: u64) -> Value {
    let handle = orch.snapshot_store_handle();
    let store = crate::sync_lock::rw_read(&*handle);
    let before = store.get(SnapshotId(before_id)).cloned();
    let after = store.get(SnapshotId(after_id)).cloned();
    match (before, after) {
        (Some(b), Some(a)) => {
            let diffs = crate::snapshot::SnapshotStore::diff(&b, &a);
            let items: Vec<Value> = diffs
                .iter()
                .map(|d| {
                    json!({
                        "path": d.path.display().to_string(),
                        "kind": format!("{:?}", d.kind),
                    })
                })
                .collect();
            json!({ "diffs": items })
        }
        _ => json!({
            "error": "one_or_both_snapshots_missing",
            "before": before_id,
            "after": after_id,
        }),
    }
}

/// Restore filesystem state from a snapshot (`S-` prefix optional in `snapshot_id_str`).
pub async fn snapshot_restore_json(
    orch: &Orchestrator,
    snapshot_id_str: &str,
) -> Result<Value, String> {
    let sid = snapshot_id_str
        .strip_prefix("S-")
        .unwrap_or(snapshot_id_str)
        .parse::<u64>()
        .map(SnapshotId)
        .map_err(|_| "invalid snapshot_id: expected numeric or S-<digits>".to_string())?;
    orch.restore_fs_snapshot(sid)
        .await
        .map_err(|e| e.to_string())?;
    Ok(json!({
        "restored": true,
        "snapshot_id": sid.to_string(),
    }))
}

/// Create agent workspace (MCP `vox_workspace_create`).
pub fn workspace_create_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let snap_handle = orch.snapshot_store_handle();
    let base_id = {
        let mut store = crate::sync_lock::rw_write(&*snap_handle);
        store.take_snapshot(AgentId(agent_id), &[], "workspace base".to_string())
    };
    let ws_handle = orch.workspace_manager_handle();
    let mut mgr = crate::sync_lock::rw_write(&*ws_handle);
    let ws = mgr.create_workspace(AgentId(agent_id), base_id).clone();
    json!({
        "workspace_created": true,
        // Raw-u64 string ("7"), matching snapshot_list_json / oplog_list_json and
        // the `agent_id: u64` input — NOT the "A-07" Display form, which is reserved
        // for human-facing markdown handoffs. Keeps agent_id parity across the facade.
        "agent_id": ws.agent_id.0.to_string(),
        "base_snapshot": base_id.to_string(),
    })
}

/// Workspace status (MCP `vox_workspace_status`).
pub fn workspace_status_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let ws_handle = orch.workspace_manager_handle();
    let mgr = crate::sync_lock::rw_read(&*ws_handle);
    match mgr.get_workspace(AgentId(agent_id)) {
        Some(ws) => {
            let paths: Vec<String> = ws
                .modified_paths()
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            json!({
                "has_workspace": true,
                "modified_files": paths,
                "modified_count": ws.modified_count(),
                "base_snapshot": ws.base_snapshot.to_string(),
                "active_change": ws.active_change.map(|c| c.to_string()),
            })
        }
        None => json!({ "has_workspace": false }),
    }
}

/// Merge workspace into mainline (MCP `vox_workspace_merge`).
/// Records overlap conflicts (as data) before destroying the workspace.
pub fn workspace_merge_json(orch: &Orchestrator, agent_id: u64) -> Value {
    let merging = AgentId(agent_id);
    let ws_handle = orch.workspace_manager_handle();
    let mut mgr = crate::sync_lock::rw_write(&*ws_handle);

    // Detect overlaps against every other active workspace, before mutating.
    let merging_base = mgr.get_workspace(merging).map(|w| w.base_snapshot);
    let others: Vec<(AgentId, SnapshotId, Vec<std::path::PathBuf>)> = mgr
        .list_workspaces()
        .iter()
        .filter(|w| w.agent_id != merging)
        .map(|w| {
            (
                w.agent_id,
                w.base_snapshot,
                mgr.overlapping_paths(merging, w.agent_id),
            )
        })
        .filter(|(_, _, paths)| !paths.is_empty())
        .collect();

    let conflicts_recorded = if let Some(base) = merging_base {
        // LOCK ORDER: workspace_manager → conflict_manager (always acquire in this
        // order; the `mgr` write lock above is still held here). Any future code that
        // touches both locks MUST follow this order to avoid an ABBA deadlock.
        let mut cm = crate::sync_lock::rw_write(&*orch.conflict_manager);
        crate::merge_conflicts::record_overlap_conflicts(&mut cm, (merging, base), &others).len()
    } else {
        0
    };

    match mgr.destroy_workspace(merging) {
        Some(ws) => {
            let count = ws.modified_count();
            json!({
                "merged": true,
                "files_merged": count,
                "conflicts_recorded": conflicts_recorded,
            })
        }
        None => json!({
            "merged": false,
            "error": "no_active_workspace",
            "conflicts_recorded": 0,
        }),
    }
}

/// Recent oplog entries (MCP `vox_oplog`).
pub async fn oplog_list_json(orch: &Orchestrator, agent_id: Option<u64>, limit: usize) -> Value {
    let agent = agent_id.map(AgentId);
    let ops = orch.list_recent_operations(agent, limit).await;
    let items: Vec<Value> = ops
        .into_iter()
        .map(|e| {
            json!({
                "id": e.id.to_string(),
                "agent_id": e.agent_id.0.to_string(),
                "timestamp_ms": e.timestamp_ms,
                "kind": format!("{:?}", e.kind),
                "description": e.description,
                "undone": e.undone,
            })
        })
        .collect();
    json!({ "operations": items })
}

/// Single JSON bundle for human handoff: repo identity + workspace + short snapshot/oplog tails.
/// CLI: `vox dei takeover-status`; mirrors fields agents need alongside MCP tool calls.
pub async fn takeover_handoff_json(
    orch: &Orchestrator,
    repo_root_display: &str,
    repository_id: &str,
    agent_id: u64,
) -> Value {
    json!({
        "schema": "vox_takeover_handoff_v1",
        "schema_version": 1,
        "repository": {
            "root": repo_root_display,
            "repository_id": repository_id,
        },
        // String form for parity with the embedded snapshots/oplog agent_id fields
        // (which are raw-u64 strings); a bare number here would mismatch its own bundle.
        "agent_id": agent_id.to_string(),
        "workspace": workspace_status_json(orch, agent_id),
        "snapshots": snapshot_list_json(orch, Some(agent_id), 5),
        "oplog": oplog_list_json(orch, Some(agent_id), 5).await,
    })
}

/// Live multi-agent isolation status (spec §5.1/§5.4): the strategy default,
/// per-agent overrides, and active conflicts — one bundle for the GUI VCS panel.
///
/// Reads the **live** `IsolationPlan` (the orchestrator's `isolation_policy`
/// handle) and the **live** `ConflictManager` (`active_conflicts()`). Agent ids
/// are raw-u64 strings for parity with the rest of this facade.
pub fn isolation_status_json(orch: &Orchestrator) -> Value {
    let policy_handle = orch.isolation_policy_handle();
    let plan = crate::sync_lock::rw_read(&policy_handle);

    // Strategy serializes via the snake_case serde rename; unwrap the JSON string.
    let strategy_str = |s: crate::isolation::IsolationStrategy| -> Value {
        serde_json::to_value(s).unwrap_or(Value::Null)
    };

    let per_agent: serde_json::Map<String, Value> = plan
        .per_agent
        .iter()
        .map(|(agent, strategy)| (agent.0.to_string(), strategy_str(*strategy)))
        .collect();

    let conflict_handle = orch.conflict_manager_handle();
    let cm = crate::sync_lock::rw_read(&conflict_handle);
    let active_conflicts: Vec<Value> = cm
        .active_conflicts()
        .iter()
        .map(|c| {
            let sides: Vec<Value> = c
                .sides
                .iter()
                .map(|s| Value::String(s.agent_id.0.to_string()))
                .collect();
            json!({
                "id": c.id.to_string(),
                "path": c.path.display().to_string(),
                "sides": sides,
                "created_ms": c.created_ms,
            })
        })
        .collect();

    json!({
        "strategy_default": strategy_str(plan.default),
        "per_agent": Value::Object(per_agent),
        "active_conflicts": active_conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Orchestrator;
    use crate::config::OrchestratorConfig;

    #[test]
    fn isolation_status_json_reports_default_and_conflicts() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = isolation_status_json(&orch);
        assert_eq!(v["strategy_default"], "shared_branch");
        assert_eq!(v["per_agent"].as_object().map(|m| m.len()), Some(0));
        assert_eq!(v["active_conflicts"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn isolation_status_json_reflects_per_agent_override() {
        let mut cfg = OrchestratorConfig::default();
        cfg.isolation_per_agent
            .insert(9, crate::isolation::IsolationStrategy::SeparateBranches);
        let orch = Orchestrator::new(cfg);
        let v = isolation_status_json(&orch);
        assert_eq!(v["per_agent"]["9"], "separate_branches");
    }

    #[test]
    fn snapshot_list_json_empty_store() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = snapshot_list_json(&orch, None, 5);
        assert_eq!(v["snapshots"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn workspace_status_json_no_workspace() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = workspace_status_json(&orch, 0);
        assert_eq!(v["has_workspace"], false);
    }

    #[tokio::test]
    async fn takeover_handoff_json_has_core_keys() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        // Give the agent a workspace so the bundle embeds a real snapshot with an agent_id.
        workspace_create_json(&orch, 1);
        let v = takeover_handoff_json(&orch, "/repo", "rid", 1).await;
        assert_eq!(v["schema"], "vox_takeover_handoff_v1");
        assert!(v.get("repository").is_some());
        assert!(v.get("workspace").is_some());
        assert!(v.get("snapshots").is_some());
        assert!(v.get("oplog").is_some());

        // agent_id parity within a single bundle: the top-level field and the
        // agent_id embedded in the snapshot list must be the SAME representation
        // ("1"), so a consumer can correlate them. (Regression guard for the
        // earlier "A-07" vs "1" vs number-1 three-way mismatch.)
        assert_eq!(v["agent_id"], "1");
        let snap_agent = &v["snapshots"]["snapshots"][0]["agent_id"];
        assert_eq!(
            *snap_agent,
            serde_json::json!("1"),
            "embedded snapshot agent_id must match top-level"
        );
    }

    // ── Data-in → data-out parity ─────────────────────────────────────────────
    // These assert that state written through the orchestrator comes back out of
    // the JSON facade faithfully: the same ids, agent, description and counts.
    // This is the contract the MCP VCS tools and `vox dei` CLI both depend on.

    #[test]
    fn create_then_list_and_status_round_trip_the_base_snapshot() {
        let orch = Orchestrator::new(OrchestratorConfig::default());

        // Data IN: creating a workspace takes a base snapshot labelled "workspace base".
        let created = workspace_create_json(&orch, 7);
        assert_eq!(created["workspace_created"], true);
        assert_eq!(created["agent_id"], "7");
        let base_id = created["base_snapshot"]
            .as_str()
            .expect("base_snapshot is a string id")
            .to_string();

        // Data OUT (list): the base snapshot is visible for agent 7 with the exact
        // id, agent and description that went in.
        let listed = snapshot_list_json(&orch, Some(7), 10);
        let snaps = listed["snapshots"].as_array().expect("snapshots array");
        assert_eq!(snaps.len(), 1, "exactly the base snapshot should be listed");
        assert_eq!(
            snaps[0]["id"], base_id,
            "snapshot id round-trips create -> list"
        );
        assert_eq!(snaps[0]["agent_id"], "7");
        assert_eq!(snaps[0]["description"], "workspace base");
        assert_eq!(snaps[0]["file_count"], 0);

        // Data OUT (status): the workspace reports the same base snapshot id, with
        // a clean (zero-modified) initial state.
        let status = workspace_status_json(&orch, 7);
        assert_eq!(status["has_workspace"], true);
        assert_eq!(
            status["base_snapshot"], base_id,
            "base snapshot id round-trips create -> status"
        );
        assert_eq!(status["modified_count"], 0);
        assert_eq!(
            status["modified_files"].as_array().map(|a| a.len()),
            Some(0)
        );
    }

    #[test]
    fn list_scopes_snapshots_by_agent() {
        // Parity guard: a per-agent query must only surface that agent's data.
        let orch = Orchestrator::new(OrchestratorConfig::default());
        workspace_create_json(&orch, 1);
        workspace_create_json(&orch, 2);

        let a1 = snapshot_list_json(&orch, Some(1), 10);
        let a1_snaps = a1["snapshots"].as_array().unwrap();
        assert!(
            a1_snaps.iter().all(|s| s["agent_id"] == "1"),
            "agent-1 query must not leak agent-2 snapshots"
        );

        // The unscoped query sees both agents' base snapshots.
        let all = snapshot_list_json(&orch, None, 10);
        assert_eq!(all["snapshots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn merge_without_workspace_reports_no_active_workspace() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = workspace_merge_json(&orch, 99);
        assert_eq!(v["merged"], false);
        assert_eq!(v["error"], "no_active_workspace");
        assert_eq!(v["conflicts_recorded"], 0);
    }

    #[test]
    fn diff_with_missing_snapshots_is_a_structured_error() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        let v = snapshot_diff_json(&orch, 1234, 5678);
        assert_eq!(v["error"], "one_or_both_snapshots_missing");
        assert_eq!(v["before"], 1234);
        assert_eq!(v["after"], 5678);
    }
}

#[cfg(test)]
mod semcov_wave1_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::{Orchestrator, OrchestratorConfig};

    #[test]
    fn merge_existing_lone_workspace_succeeds_with_no_conflicts() {
        let orch = Orchestrator::new(OrchestratorConfig::default());
        // Create a workspace so the success (Some) branch of destroy_workspace fires.
        let created = workspace_create_json(&orch, 5);
        assert_eq!(created["workspace_created"], true);

        let v = workspace_merge_json(&orch, 5);
        assert_eq!(v["merged"], true);
        // Freshly created workspace has zero modified files.
        assert_eq!(v["files_merged"], 0);
        // Only one workspace exists, so there are no overlaps to record.
        assert_eq!(v["conflicts_recorded"], 0);

        // The workspace is destroyed by the merge: status must now report none.
        let status = workspace_status_json(&orch, 5);
        assert_eq!(status["has_workspace"], false);
    }

    #[test]
    fn diff_of_two_present_snapshots_reports_modified_and_added() {
        use std::path::PathBuf;
        let orch = Orchestrator::new(OrchestratorConfig::default());

        // Seed two snapshots directly through the store handle (same handle the
        // facade reads from), so snapshot_diff_json takes the (Some, Some) branch.
        let (before_id, after_id) = {
            let handle = orch.snapshot_store_handle();
            let mut store = crate::sync_lock::rw_write(&*handle);
            let before = store.take_snapshot_in_memory(
                AgentId(1),
                vec![(PathBuf::from("a.txt"), b"v1".to_vec())],
                "before".to_string(),
            );
            let after = store.take_snapshot_in_memory(
                AgentId(1),
                vec![
                    (PathBuf::from("a.txt"), b"v2".to_vec()), // content changed -> Modified
                    (PathBuf::from("b.txt"), b"new".to_vec()), // not in before -> Added
                ],
                "after".to_string(),
            );
            (before.0, after.0)
        };

        let v = snapshot_diff_json(&orch, before_id, after_id);
        let diffs = v["diffs"].as_array().expect("diffs array present");
        assert_eq!(diffs.len(), 2, "one modified + one added file");

        let kinds: std::collections::BTreeSet<&str> =
            diffs.iter().map(|d| d["kind"].as_str().unwrap()).collect();
        assert!(
            kinds.contains("Modified"),
            "changed file reported as Modified"
        );
        assert!(kinds.contains("Added"), "new file reported as Added");

        // The modified file's path is surfaced as the Display string.
        let modified_path = diffs
            .iter()
            .find(|d| d["kind"] == "Modified")
            .map(|d| d["path"].as_str().unwrap().to_string());
        assert_eq!(modified_path.as_deref(), Some("a.txt"));
    }
}
