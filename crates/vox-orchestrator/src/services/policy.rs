//! Policy engine: scope and lock checks before queueing tasks.
//!
//! Validates that an agent can acquire required locks and (optionally)
//! that writes fall within the agent's scope. Call before enqueueing
//! to fail fast and emit scope violations.

use crate::events::EventBus;
use crate::locks::{FileLockManager, LockConflict, LockKind};
use crate::scope::{ScopeCheckResult, ScopeGuard};
use crate::types::{AccessKind, AgentId, FileAffinity};

/// Result of a policy check before queueing a task.
#[derive(Debug, Clone)]
pub enum PolicyCheckResult {
    /// All checks passed; safe to enqueue.
    Allowed,
    /// A required lock could not be acquired.
    LockConflict(LockConflict),
    /// Scope guard denied write to a path (with reason).
    ScopeDenied(String),
}

impl PolicyCheckResult {
    /// Returns true if the operation is allowed to proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Stateless policy engine for pre-queue validation.
pub struct PolicyEngine;

impl PolicyEngine {
    /// Check whether the agent can acquire locks for all write files.
    /// Does not actually acquire locks; use for dry-run or pre-check.
    pub fn check_locks(
        lock_manager: &FileLockManager,
        manifest: &[FileAffinity],
        agent_id: AgentId,
    ) -> PolicyCheckResult {
        for fa in manifest {
            if fa.access != AccessKind::Write {
                continue;
            }
            if let Err(e) = lock_manager.try_acquire(&fa.path, agent_id, LockKind::Exclusive) {
                return PolicyCheckResult::LockConflict(e);
            }
        }
        PolicyCheckResult::Allowed
    }

    /// Run both lock and scope checks. Pass None for scope_guard to skip scope checks.
    /// When scope_guard is Some, pass event_bus so that ScopeViolation events are emitted.
    pub fn check_before_queue(
        lock_manager: &FileLockManager,
        scope_guard: Option<&ScopeGuard>,
        event_bus: &EventBus,
        manifest: &[FileAffinity],
        agent_id: AgentId,
    ) -> PolicyCheckResult {
        if let PolicyCheckResult::LockConflict(e) =
            Self::check_locks(lock_manager, manifest, agent_id)
        {
            return PolicyCheckResult::LockConflict(e);
        }
        if let Some(guard) = scope_guard {
            for fa in manifest {
                if fa.access != AccessKind::Write {
                    continue;
                }
                let result = guard.check_write(agent_id, &fa.path, event_bus);
                if !result.is_allowed() {
                    let reason = match &result {
                        ScopeCheckResult::Warned(s) => s.clone(),
                        ScopeCheckResult::Denied(s) => s.clone(),
                        _ => String::new(),
                    };
                    return PolicyCheckResult::ScopeDenied(reason);
                }
            }
        }
        PolicyCheckResult::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn check_before_queue_lock_conflict_when_other_agent_holds() {
        let lock_manager = FileLockManager::new();
        let event_bus = EventBus::new(16);
        let path = PathBuf::from("src/foo.rs");
        let manifest = vec![FileAffinity::write(&path)];
        let a1 = AgentId(1);
        let a2 = AgentId(2);
        let _ = lock_manager.try_acquire(&path, a1, LockKind::Exclusive);
        let r = PolicyEngine::check_before_queue(&lock_manager, None, &event_bus, &manifest, a2);
        assert!(!r.is_allowed());
        assert!(matches!(r, PolicyCheckResult::LockConflict(_)));
    }

    #[test]
    fn check_before_queue_allowed_when_same_agent_reentrant() {
        let lock_manager = FileLockManager::new();
        let event_bus = EventBus::new(16);
        let path = PathBuf::from("src/bar.rs");
        let manifest = vec![FileAffinity::write(&path)];
        let a1 = AgentId(1);
        let _ = lock_manager.try_acquire(&path, a1, LockKind::Exclusive);
        let r = PolicyEngine::check_before_queue(&lock_manager, None, &event_bus, &manifest, a1);
        assert!(r.is_allowed());
    }
}
