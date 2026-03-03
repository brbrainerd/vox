//! Scope guard — prevents agents from editing outside their assigned files.
//!
//! Uses the `FileAffinityMap` to validate that an agent only touches
//! files it has been assigned to. Emits `ScopeViolation` events when
//! an agent attempts to write outside its scope.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::events::{AgentEventKind, EventBus};
use crate::types::AgentId;

/// How strictly to enforce scope boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScopeEnforcement {
    /// Block the operation and return an error.
    Strict,
    /// Allow the operation but emit a warning event.
    #[default]
    Warn,
    /// No enforcement.
    Disabled,
}

/// Result of a scope check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCheckResult {
    /// The path is within the agent's scope.
    Allowed,
    /// The path is outside scope but enforcement is Warn (allowed with warning).
    Warned(String),
    /// The path is outside scope and enforcement is Strict (blocked).
    Denied(String),
}

impl ScopeCheckResult {
    /// Whether the operation should proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed | Self::Warned(_))
    }
}

/// Manages file scope assignments for agents.
///
/// Each agent is assigned a set of file paths or glob patterns
/// that define its scope. Operations outside this scope are
/// either blocked or warned depending on the enforcement level.
#[derive(Debug)]
pub struct ScopeGuard {
    /// Per-agent scope: set of file paths the agent is allowed to touch.
    scopes: HashMap<AgentId, HashSet<PathBuf>>,
    /// Enforcement level.
    enforcement: ScopeEnforcement,
}

impl ScopeGuard {
    /// Create a new scope guard with the given enforcement level.
    pub fn new(enforcement: ScopeEnforcement) -> Self {
        Self {
            scopes: HashMap::new(),
            enforcement,
        }
    }

    /// Assign a file to an agent's scope.
    pub fn assign_file(&mut self, agent_id: AgentId, path: impl Into<PathBuf>) {
        self.scopes.entry(agent_id).or_default().insert(path.into());
    }

    /// Assign multiple files to an agent's scope.
    pub fn assign_files(&mut self, agent_id: AgentId, paths: impl IntoIterator<Item = PathBuf>) {
        let scope = self.scopes.entry(agent_id).or_default();
        scope.extend(paths);
    }

    /// Remove a file from an agent's scope.
    pub fn revoke_file(&mut self, agent_id: AgentId, path: &Path) {
        if let Some(scope) = self.scopes.get_mut(&agent_id) {
            scope.remove(path);
        }
    }

    /// Clear all scope assignments for an agent.
    pub fn clear_scope(&mut self, agent_id: AgentId) {
        self.scopes.remove(&agent_id);
    }

    /// Check if an agent is allowed to write to the given path.
    pub fn check_write(
        &self,
        agent_id: AgentId,
        path: &Path,
        event_bus: &EventBus,
    ) -> ScopeCheckResult {
        if self.enforcement == ScopeEnforcement::Disabled {
            return ScopeCheckResult::Allowed;
        }

        // If agent has no scope defined, allow everything (unscoped agent).
        let scope = match self.scopes.get(&agent_id) {
            Some(s) if !s.is_empty() => s,
            _ => return ScopeCheckResult::Allowed,
        };

        // Check if the path is in the agent's scope (exact match or parent match).
        let in_scope = scope.iter().any(|allowed| {
            path == allowed || path.starts_with(allowed) || allowed.starts_with(path)
        });

        if in_scope {
            ScopeCheckResult::Allowed
        } else {
            let reason = format!(
                "Agent {} attempted to write to '{}' which is outside its assigned scope",
                agent_id,
                path.display()
            );

            event_bus.emit(AgentEventKind::ScopeViolation {
                agent_id,
                path: path.to_path_buf(),
                reason: reason.clone(),
            });

            match self.enforcement {
                ScopeEnforcement::Strict => ScopeCheckResult::Denied(reason),
                ScopeEnforcement::Warn => {
                    tracing::warn!("{}", reason);
                    ScopeCheckResult::Warned(reason)
                }
                ScopeEnforcement::Disabled => ScopeCheckResult::Allowed,
            }
        }
    }

    /// Get all files assigned to an agent.
    pub fn agent_scope(&self, agent_id: AgentId) -> Option<&HashSet<PathBuf>> {
        self.scopes.get(&agent_id)
    }

    /// Get the enforcement level.
    pub fn enforcement(&self) -> ScopeEnforcement {
        self.enforcement
    }

    /// Set the enforcement level.
    pub fn set_enforcement(&mut self, enforcement: ScopeEnforcement) {
        self.enforcement = enforcement;
    }
}

impl Default for ScopeGuard {
    fn default() -> Self {
        Self::new(ScopeEnforcement::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unscoped_agent_allowed_everywhere() {
        let bus = EventBus::new(16);
        let guard = ScopeGuard::new(ScopeEnforcement::Strict);
        let agent = AgentId(1);

        let result = guard.check_write(agent, Path::new("any/file.rs"), &bus);
        assert!(result.is_allowed());
    }

    #[test]
    fn scoped_agent_allowed_in_scope() {
        let bus = EventBus::new(16);
        let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
        let agent = AgentId(1);

        guard.assign_file(agent, PathBuf::from("src/parser.rs"));

        let result = guard.check_write(agent, Path::new("src/parser.rs"), &bus);
        assert!(result.is_allowed());
    }

    #[test]
    fn strict_denies_out_of_scope() {
        let bus = EventBus::new(16);
        let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
        let agent = AgentId(1);

        guard.assign_file(agent, PathBuf::from("src/parser.rs"));

        let result = guard.check_write(agent, Path::new("src/codegen.rs"), &bus);
        assert!(!result.is_allowed());
        match result {
            ScopeCheckResult::Denied(reason) => {
                assert!(reason.contains("outside its assigned scope"));
            }
            _ => panic!("expected Denied"),
        }
    }

    #[test]
    fn warn_allows_with_warning() {
        let bus = EventBus::new(16);
        let mut guard = ScopeGuard::new(ScopeEnforcement::Warn);
        let agent = AgentId(1);

        guard.assign_file(agent, PathBuf::from("src/parser.rs"));

        let result = guard.check_write(agent, Path::new("src/codegen.rs"), &bus);
        assert!(result.is_allowed());
        assert!(matches!(result, ScopeCheckResult::Warned(_)));
    }

    #[test]
    fn disabled_allows_everything() {
        let bus = EventBus::new(16);
        let mut guard = ScopeGuard::new(ScopeEnforcement::Disabled);
        let agent = AgentId(1);

        guard.assign_file(agent, PathBuf::from("src/parser.rs"));

        let result = guard.check_write(agent, Path::new("anywhere/else.rs"), &bus);
        assert!(result.is_allowed());
        assert!(matches!(result, ScopeCheckResult::Allowed));
    }

    #[test]
    fn assign_and_revoke() {
        let bus = EventBus::new(16);
        let mut guard = ScopeGuard::new(ScopeEnforcement::Strict);
        let agent = AgentId(1);

        guard.assign_file(agent, PathBuf::from("src/a.rs"));
        guard.assign_file(agent, PathBuf::from("src/b.rs"));
        assert_eq!(guard.agent_scope(agent).unwrap().len(), 2);

        guard.revoke_file(agent, Path::new("src/a.rs"));
        assert_eq!(guard.agent_scope(agent).unwrap().len(), 1);

        // b.rs still allowed, a.rs now denied
        assert!(guard
            .check_write(agent, Path::new("src/b.rs"), &bus)
            .is_allowed());
        assert!(!guard
            .check_write(agent, Path::new("src/a.rs"), &bus)
            .is_allowed());
    }
}
