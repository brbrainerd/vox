//! Tauri bridge for the Repository surface's `IsolationPanel`.
//!
//! The GUI's transport is Tauri `invoke`, so it cannot reach `/api/v2` directly.
//! Live isolation state (the default strategy, per-agent overrides, and the
//! active conflict list) lives in the **one shared orchestrator** owned by the
//! `vox-orchestrator-d` daemon — the same instance the control-plane commands
//! drive. We therefore go through the daemon RPC
//! ([`orch_daemon_method::VCS_ISOLATION_STATUS`] /
//! [`orch_daemon_method::VCS_ISOLATION_SET_STRATEGY`]) rather than building a
//! second in-process `Orchestrator` (which would see neither live conflicts nor
//! prior overrides). This mirrors `control_plane.rs`.

use serde_json::Value;
use vox_cli_core::daemon_ipc::dispatch::call_daemon;
use vox_foundation::protocol::orch_daemon_method;

async fn call_orchestrator_daemon(method: &str, params: Value) -> Result<Value, String> {
    call_daemon("vox-orchestrator-d", method, params, false)
        .await
        .map_err(|e| e.to_string())
}

/// Build the RPC params for a strategy mutation, distinguishing "field absent"
/// from "field present with null" for the per-agent override clear path.
///
/// - `default: Some(s)` sets the baseline strategy.
/// - `agent_id: Some(id)` with `strategy: Some(s)` sets that agent's override;
///   with `strategy: None` it clears the override (serialized as JSON `null`).
/// - `agent_id: None` leaves overrides untouched (no `strategy` key emitted).
fn build_set_strategy_params(
    default: Option<String>,
    agent_id: Option<u64>,
    strategy: Option<String>,
) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(d) = default {
        obj.insert("strategy_default".to_string(), Value::String(d));
    }
    if let Some(id) = agent_id {
        obj.insert("agent_id".to_string(), Value::from(id));
        // Present `agent_id` => the `strategy` field is meaningful: a value sets
        // the override, explicit null clears it.
        obj.insert(
            "strategy".to_string(),
            match strategy {
                Some(s) => Value::String(s),
                None => Value::Null,
            },
        );
    }
    Value::Object(obj)
}

/// GET-equivalent: live isolation status (default + per-agent + active conflicts).
#[tauri::command]
pub async fn get_vcs_isolation() -> Result<Value, String> {
    call_orchestrator_daemon(
        orch_daemon_method::VCS_ISOLATION_STATUS,
        serde_json::json!({}),
    )
    .await
}

/// POST-equivalent: set the default and/or a per-agent override, returns fresh status.
#[tauri::command]
pub async fn set_vcs_isolation_strategy(
    default: Option<String>,
    agent_id: Option<u64>,
    strategy: Option<String>,
) -> Result<Value, String> {
    let params = build_set_strategy_params(default, agent_id, strategy);
    call_orchestrator_daemon(orch_daemon_method::VCS_ISOLATION_SET_STRATEGY, params).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_only_emits_strategy_default() {
        let p = build_set_strategy_params(Some("separate_branches".into()), None, None);
        assert_eq!(p["strategy_default"], "separate_branches");
        assert!(p.get("agent_id").is_none());
        assert!(p.get("strategy").is_none());
    }

    #[test]
    fn agent_override_set_emits_agent_and_strategy() {
        let p = build_set_strategy_params(None, Some(7), Some("split_changes".into()));
        assert_eq!(p["agent_id"], 7);
        assert_eq!(p["strategy"], "split_changes");
        assert!(p.get("strategy_default").is_none());
    }

    #[test]
    fn agent_override_clear_emits_explicit_null() {
        let p = build_set_strategy_params(None, Some(7), None);
        assert_eq!(p["agent_id"], 7);
        assert!(p.get("strategy").is_some());
        assert_eq!(p["strategy"], serde_json::Value::Null);
    }

    #[test]
    fn default_and_agent_can_coexist() {
        let p = build_set_strategy_params(
            Some("shared_branch".into()),
            Some(3),
            Some("separate_branches".into()),
        );
        assert_eq!(p["strategy_default"], "shared_branch");
        assert_eq!(p["agent_id"], 3);
        assert_eq!(p["strategy"], "separate_branches");
    }
}
