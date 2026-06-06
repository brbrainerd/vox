//! Write-path lock enforcement. Mirrors `scope_guard.rs` but consults the
//! orchestrator's `FileLockManager`. The check is purely READ-ONLY: it probes
//! the existing lock state and never acquires or modifies anything.

use crate::scope_guard::{PATH_ARG_KEYS, WRITE_TOOLS};
use crate::server_state::ServerState;
use std::path::Path;
use vox_orchestrator::locks::{FileLockManager, LockKind};
use vox_orchestrator_types::AgentId;

/// Pure read-only probe: is `path` exclusively locked by an agent OTHER than `agent_id`?
/// `None` = allowed (unlocked, or held by this same agent); `Some(msg)` = rejected.
pub(crate) fn evaluate_lock(
    lock_manager: &FileLockManager,
    agent_id: u64,
    path: &str,
) -> Option<String> {
    match lock_manager.holder(Path::new(path)) {
        Some((holder, LockKind::Exclusive)) if holder != AgentId(agent_id) => Some(format!(
            "LOCK_CONFLICT: '{path}' is exclusively locked by agent {holder}. \
             Wait for release or route this write to a non-overlapping path."
        )),
        _ => None,
    }
}

/// Returns `Some(rejection)` when a write tool targets a path locked by another agent.
pub(crate) fn check_lock(
    state: &ServerState,
    tool_name: &str,
    args: &serde_json::Value,
) -> Option<String> {
    if !WRITE_TOOLS.contains(&tool_name) {
        return None;
    }
    // Intentional: if `agent_id` or a path arg is absent we return `None` (allow) here rather
    // than hard-failing. This preflight is best-effort admission control; the AUTHORITATIVE
    // lock-conflict gate is queue admission (`vox_orchestrator::services::policy::check_before_queue`,
    // which calls `try_acquire(.., Exclusive)` per write path and returns `LockConflict`). So a
    // write tool that omits these args is not silently let past the lock system — it is still
    // blocked downstream if it conflicts.
    let raw = args.get("agent_id").or_else(|| args.get("vcs_agent_id"));
    let agent_id = raw.and_then(|v| v.as_u64()).or_else(|| {
        raw.and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
    })?;
    let path = PATH_ARG_KEYS
        .iter()
        .find_map(|key| args.get(*key).and_then(|v| v.as_str()))?;
    evaluate_lock(&state.orchestrator.lock_manager, agent_id, path)
}

#[cfg(test)]
mod tests {
    use super::evaluate_lock;
    use std::path::Path;
    use vox_orchestrator::locks::{FileLockManager, LockKind};
    use vox_orchestrator_types::AgentId;

    #[test]
    fn rejects_other_agent_allows_holder_and_unlocked() {
        let mgr = FileLockManager::new();
        // unlocked path: anyone may write
        assert!(evaluate_lock(&mgr, 1, "src/a.rs").is_none());
        // agent 1 takes the lock through the real acquire path
        mgr.try_acquire(Path::new("src/a.rs"), AgentId(1), LockKind::Exclusive)
            .unwrap();
        // holder may write (re-entrant), a different agent is rejected
        assert!(
            evaluate_lock(&mgr, 1, "src/a.rs").is_none(),
            "holder allowed"
        );
        assert!(
            evaluate_lock(&mgr, 2, "src/a.rs").is_some(),
            "other agent rejected"
        );
    }
}
