//! Policy engine: scope and lock checks before queueing tasks.
//!
//! Validates that an agent can acquire required locks and (optionally)
//! that writes fall within the agent's scope. Call before enqueueing
//! to fail fast and emit scope violations.
//!
//! **LLM premature-completion governance** is anchored at
//! `contracts/operations/completion-policy.v1.yaml` and enforced in CI via
//! `vox ci completion-audit` / `completion-gates`; runtime completion attestation
//! and placeholder heuristics live in `crate::orchestrator::task_dispatch::complete`.

use crate::events::EventBus;
use crate::locks::{FileLockManager, LockConflict, LockKind};
use crate::scope::{ScopeCheckResult, ScopeGuard};
use crate::types::{AccessKind, AgentId, AgentTask, CompletionAttestation, FileAffinity};

/// Optional relaxation of strict scope using Codex `agent_reliability` (see [`PolicyEngine::check_before_queue`]).
#[derive(Debug, Clone)]
pub struct PolicyTrustRelax {
    /// When set, a strict scope denial may be allowed if [`Self::agent_reliability`] is high enough.
    pub relax_scope_strict_on_high_reliability: bool,
    pub agent_reliability: Option<f64>,
    pub min_reliability: f64,
}

impl Default for PolicyTrustRelax {
    fn default() -> Self {
        Self {
            relax_scope_strict_on_high_reliability: false,
            agent_reliability: None,
            min_reliability: 0.85,
        }
    }
}

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
    fn has_placeholder_marker(text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        [
            "todo",
            "tbd",
            "placeholder",
            "stub",
            "not implemented",
            "coming soon",
        ]
        .iter()
        .any(|m| lower.contains(m))
    }

    /// Completion policy for no-write tasks: require a concrete attestation and reject
    /// obvious placeholder markers unless `force_risky` is explicitly set with a reason.
    pub fn check_completion_before_complete(
        task: Option<&AgentTask>,
        attestation: Option<&CompletionAttestation>,
    ) -> PolicyCheckResult {
        let Some(task) = task else {
            return PolicyCheckResult::Allowed;
        };
        let write_manifest: std::collections::HashSet<_> =
            task.write_files().iter().cloned().collect();
        let is_write_task = !write_manifest.is_empty();

        let Some(att) = attestation else {
            return PolicyCheckResult::ScopeDenied(
                "Completion policy denied: no-write task requires completion attestation"
                    .to_string(),
            );
        };
        if att.force_risky {
            let reason_ok = match &att.force_risky_reason {
                Some(r) => !r.trim().is_empty(),
                None => false,
            };
            if !reason_ok {
                return PolicyCheckResult::ScopeDenied(
                    "Completion policy denied: force_risky requires non-empty force_risky_reason"
                        .to_string(),
                );
            }
            return PolicyCheckResult::Allowed;
        }

        if is_write_task {
            let actual_touched: std::collections::HashSet<_> = att
                .artifact_paths
                .iter()
                .map(std::path::PathBuf::from)
                .collect();
            let mut untouched = Vec::new();
            for planned in &write_manifest {
                if !actual_touched.contains(planned.as_path()) {
                    untouched.push(planned.display().to_string());
                }
            }
            if !untouched.is_empty() {
                return PolicyCheckResult::ScopeDenied(format!(
                    "Completion policy denied: {} planned files not in artifact_paths: {:?}. Use force_risky to bypass.",
                    untouched.len(),
                    untouched
                ));
            }
        } else {
            if !att.declared_non_placeholder {
                return PolicyCheckResult::ScopeDenied(
                    "Completion policy denied: declared_non_placeholder must be true for no-write tasks"
                        .to_string(),
                );
            }
        }
        let summary_ok = match &att.completion_summary {
            Some(s) => s.trim().len() >= 24 && !Self::has_placeholder_marker(s),
            None => false,
        };
        if !summary_ok {
            return PolicyCheckResult::ScopeDenied(
                "Completion policy denied: completion_summary is missing/too short or includes placeholder markers".to_string(),
            );
        }
        let has_evidence = !att.artifact_paths.is_empty() || !att.checks_passed.is_empty();
        if !has_evidence {
            let task_type = if is_write_task {
                "write task"
            } else {
                "no-write task"
            };
            return PolicyCheckResult::ScopeDenied(format!(
                "Completion policy denied: {} requires artifact_paths or checks_passed evidence",
                task_type
            ));
        }
        PolicyCheckResult::Allowed
    }

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
        trust: PolicyTrustRelax,
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
                    if matches!(result, ScopeCheckResult::Denied(_))
                        && trust.relax_scope_strict_on_high_reliability
                        && trust
                            .agent_reliability
                            .is_some_and(|r| r >= trust.min_reliability)
                    {
                        tracing::warn!(
                            target: "vox_orchestrator::policy",
                            agent_id = agent_id.0,
                            reliability = ?trust.agent_reliability,
                            path = %fa.path.display(),
                            "trust gate relax: allowing enqueue despite strict scope denial"
                        );
                        continue;
                    }
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
    use crate::types::{AgentTask, CompletionAttestation, TaskId, TaskPriority};
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
        let r = PolicyEngine::check_before_queue(
            &lock_manager,
            None,
            &event_bus,
            &manifest,
            a2,
            PolicyTrustRelax::default(),
        );
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
        let r = PolicyEngine::check_before_queue(
            &lock_manager,
            None,
            &event_bus,
            &manifest,
            a1,
            PolicyTrustRelax::default(),
        );
        assert!(r.is_allowed());
    }

    #[test]
    fn no_write_completion_requires_attestation() {
        let task = AgentTask::new(TaskId(1), "no write task", TaskPriority::Normal, vec![]);
        let r = PolicyEngine::check_completion_before_complete(Some(&task), None);
        assert!(!r.is_allowed());
    }

    #[test]
    fn no_write_completion_allows_valid_attestation() {
        let task = AgentTask::new(TaskId(2), "no write task", TaskPriority::Normal, vec![]);
        let att = CompletionAttestation {
            completion_summary: Some(
                "Validated documentation output and emitted artifacts.".into(),
            ),
            checks_passed: vec!["schema-verify".into()],
            evidence_citations: vec![],
            artifact_paths: vec![],
            declared_non_placeholder: true,
            force_risky: false,
            force_risky_reason: None,
            ..Default::default()
        };
        let r = PolicyEngine::check_completion_before_complete(Some(&task), Some(&att));
        assert!(r.is_allowed());
    }

    #[test]
    fn write_completion_requires_all_planned_files_or_force_risky() {
        use crate::types::FileAffinity;
        let path = PathBuf::from("src/foo.rs");
        let manifest = vec![FileAffinity::write(&path)];
        let task = AgentTask::new(TaskId(3), "write task", TaskPriority::Normal, manifest);

        let att_missing = CompletionAttestation {
            completion_summary: Some("Wrote code without artifacts.".into()),
            checks_passed: vec!["cargo check".into()],
            evidence_citations: vec![],
            artifact_paths: vec![], // Missing the planned file
            declared_non_placeholder: true,
            force_risky: false,
            force_risky_reason: None,
            ..Default::default()
        };
        let r1 = PolicyEngine::check_completion_before_complete(Some(&task), Some(&att_missing));
        assert!(!r1.is_allowed());

        let att_present = CompletionAttestation {
            completion_summary: Some("Wrote code and got artifacts.".into()),
            checks_passed: vec!["cargo check".into()],
            evidence_citations: vec![],
            artifact_paths: vec!["src/foo.rs".into()],
            declared_non_placeholder: true,
            force_risky: false,
            force_risky_reason: None,
            ..Default::default()
        };
        let r2 = PolicyEngine::check_completion_before_complete(Some(&task), Some(&att_present));
        assert!(r2.is_allowed());
    }

    #[test]
    fn check_before_queue_allows_scope_denial_when_reliability_is_high() {
        use crate::scope::{ScopeEnforcement, ScopeGuard};
        let lock_manager = FileLockManager::new();
        let event_bus = EventBus::new(16);
        let path = PathBuf::from("src/outside.rs");
        let manifest = vec![FileAffinity::write(&path)];
        let a1 = AgentId(1);

        let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
        // Agent 1 is only allowed in src/inside.rs
        guard.assign_file(a1, PathBuf::from("src/inside.rs"));

        let trust_high = PolicyTrustRelax {
            relax_scope_strict_on_high_reliability: true,
            agent_reliability: Some(0.95),
            min_reliability: 0.90,
        };

        let r = PolicyEngine::check_before_queue(
            &lock_manager,
            Some(&guard),
            &event_bus,
            &manifest,
            a1,
            trust_high,
        );

        assert!(
            r.is_allowed(),
            "Should be allowed due to high reliability relaxation"
        );
    }

    #[test]
    fn check_before_queue_denies_scope_denial_when_reliability_is_low() {
        use crate::scope::{ScopeEnforcement, ScopeGuard};
        let lock_manager = FileLockManager::new();
        let event_bus = EventBus::new(16);
        let path = PathBuf::from("src/outside.rs");
        let manifest = vec![FileAffinity::write(&path)];
        let a1 = AgentId(1);

        let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
        guard.assign_file(a1, PathBuf::from("src/inside.rs"));

        let trust_low = PolicyTrustRelax {
            relax_scope_strict_on_high_reliability: true,
            agent_reliability: Some(0.85), // Lower than 0.90
            min_reliability: 0.90,
        };

        let r = PolicyEngine::check_before_queue(
            &lock_manager,
            Some(&guard),
            &event_bus,
            &manifest,
            a1,
            trust_low,
        );

        assert!(!r.is_allowed(), "Should be denied due to low reliability");
    }
}
