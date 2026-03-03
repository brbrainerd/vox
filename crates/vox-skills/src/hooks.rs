//! Hook system — event-driven hooks for the skill lifecycle.

use std::collections::HashMap;
use std::sync::Mutex;

use tracing::{debug, warn};

use crate::SkillError;

/// Events the hook system fires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookEvent {
    SkillInstalled,
    SkillUninstalling,
    PluginLoaded,
    PluginUnloading,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    BeforeCompaction,
    AfterCompaction,
    Custom(String),
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(s) => write!(f, "custom:{s}"),
            other => write!(f, "{other:?}"),
        }
    }
}

/// A hook function: takes context JSON string, returns optional output JSON.
pub type HookFn = Box<dyn Fn(&str) -> Result<Option<String>, SkillError> + Send + Sync>;

/// Registry of named hook functions keyed by event.
pub struct HookRegistry {
    hooks: Mutex<HashMap<String, Vec<(String, HookFn)>>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            hooks: Mutex::new(HashMap::new()),
        }
    }

    /// Register a hook for an event with an ID (for later removal).
    pub fn register(&self, event: HookEvent, hook_id: impl Into<String>, f: HookFn) {
        let key = event.to_string();
        let id = hook_id.into();
        let mut hooks = self.hooks.lock().unwrap_or_else(|e| e.into_inner());
        hooks.entry(key).or_default().push((id, f));
    }

    /// Deregister a hook by event + ID.
    pub fn deregister(&self, event: &HookEvent, hook_id: &str) -> bool {
        let key = event.to_string();
        let mut hooks = self.hooks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = hooks.get_mut(&key) {
            let before = entry.len();
            entry.retain(|(id, _)| id != hook_id);
            return entry.len() < before;
        }
        false
    }

    /// Fire all hooks for an event, passing context JSON.
    pub fn fire(&self, event: &HookEvent, context_json: &str) -> Vec<String> {
        let key = event.to_string();
        let hooks = self.hooks.lock().unwrap_or_else(|e| e.into_inner());
        let mut outputs = Vec::new();
        if let Some(entries) = hooks.get(&key) {
            for (id, f) in entries {
                debug!(hook_id = %id, event = %event, "Firing hook");
                match f(context_json) {
                    Ok(Some(out)) => outputs.push(out),
                    Ok(None) => {}
                    Err(e) => warn!(hook_id = %id, event = %event, "Hook error: {e}"),
                }
            }
        }
        outputs
    }

    pub fn count(&self, event: &HookEvent) -> usize {
        let key = event.to_string();
        let hooks = self.hooks.lock().unwrap_or_else(|e| e.into_inner());
        hooks.get(&key).map_or(0, |v| v.len())
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_fire() {
        let reg = HookRegistry::new();
        reg.register(
            HookEvent::TaskCompleted,
            "test-hook",
            Box::new(|ctx| Ok(Some(format!("processed:{ctx}")))),
        );
        let outputs = reg.fire(&HookEvent::TaskCompleted, r#"{"task":"fix-parser"}"#);
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("processed"));
    }

    #[test]
    fn deregister_removes_hook() {
        let reg = HookRegistry::new();
        reg.register(HookEvent::SkillInstalled, "h1", Box::new(|_| Ok(None)));
        assert_eq!(reg.count(&HookEvent::SkillInstalled), 1);
        let removed = reg.deregister(&HookEvent::SkillInstalled, "h1");
        assert!(removed);
        assert_eq!(reg.count(&HookEvent::SkillInstalled), 0);
    }

    #[test]
    fn multiple_hooks_fire_in_order() {
        let reg = HookRegistry::new();
        reg.register(
            HookEvent::TaskStarted,
            "h1",
            Box::new(|_| Ok(Some("first".into()))),
        );
        reg.register(
            HookEvent::TaskStarted,
            "h2",
            Box::new(|_| Ok(Some("second".into()))),
        );
        let outputs = reg.fire(&HookEvent::TaskStarted, "{}");
        assert_eq!(outputs, vec!["first", "second"]);
    }

    #[test]
    fn hook_error_does_not_abort_others() {
        let reg = HookRegistry::new();
        reg.register(
            HookEvent::TaskFailed,
            "bad",
            Box::new(|_| Err(crate::SkillError::Hook("oops".into()))),
        );
        reg.register(
            HookEvent::TaskFailed,
            "good",
            Box::new(|_| Ok(Some("done".into()))),
        );
        let outputs = reg.fire(&HookEvent::TaskFailed, "{}");
        assert_eq!(outputs, vec!["done"]);
    }
}
