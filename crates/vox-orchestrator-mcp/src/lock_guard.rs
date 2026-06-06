//! Write-path lock enforcement. Mirrors `scope_guard.rs` but consults the
//! orchestrator's `FileLockManager`. `try_acquire` is re-entrant for the
//! lock holder and rejects any other agent, which is exactly the gate we want.

use crate::scope_guard::{PATH_ARG_KEYS, WRITE_TOOLS};
use crate::server_state::ServerState;
use std::path::Path;
use vox_orchestrator::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

/// Pure core: try to take an exclusive lock for `agent_id` on `path`.
/// `None` = allowed (acquired or re-entrant); `Some(msg)` = rejected.
pub(crate) fn evaluate_lock(
    lock_manager: &FileLockManager,
    agent_id: u64,
    path: &str,
) -> Option<String> {
    match lock_manager.try_acquire(Path::new(path), AgentId(agent_id), LockKind::Exclusive) {
        Ok(()) => None,
        Err(conflict) => Some(format!(
            "LOCK_CONFLICT: '{path}' is locked by another agent ({conflict:?}). \
             Wait for release or route this write to a non-overlapping path."
        )),
    }
}

/// Returns `Some(rejection)` when a write tool targets a path locked by another agent.
pub fn check_lock(
    state: &ServerState,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    if !WRITE_TOOLS.iter().any(|t| *t == tool_name) {
        return None;
    }
    let agent_id = args
        .get("agent_id")
        .or_else(|| args.get("vcs_agent_id"))
        .and_then(|v| v.as_u64())?;
    let path = PATH_ARG_KEYS
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))?;
    evaluate_lock(&state.orchestrator.lock_manager, agent_id, path)
}

#[cfg(test)]
mod tests {
    use super::evaluate_lock;
    use vox_orchestrator::locks::FileLockManager;

    #[test]
    fn holder_is_reentrant_others_are_rejected() {
        let mgr = FileLockManager::new();
        assert!(
            evaluate_lock(&mgr, 1, "src/a.rs").is_none(),
            "agent 1 should acquire"
        );
        assert!(
            evaluate_lock(&mgr, 1, "src/a.rs").is_none(),
            "same agent re-entrant"
        );
        assert!(
            evaluate_lock(&mgr, 2, "src/a.rs").is_some(),
            "agent 2 must be rejected"
        );
    }
}
