use crate::pid::Pid;
use crate::process::ProcessHandle;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Global registry mapping Pids and names to live process handles.
#[derive(Clone)]
pub struct ProcessRegistry {
    by_pid: Arc<RwLock<HashMap<Pid, ProcessHandle>>>,
    by_name: Arc<RwLock<HashMap<String, Pid>>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            by_pid: Arc::new(RwLock::new(HashMap::new())),
            by_name: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a process handle.
    pub fn register(&self, handle: ProcessHandle) {
        let pid = handle.pid;
        self.by_pid.write().unwrap().insert(pid, handle);
    }

    /// Register a named process.
    pub fn register_name(&self, name: String, handle: ProcessHandle) {
        let pid = handle.pid;
        self.by_name.write().unwrap().insert(name, pid);
        self.by_pid.write().unwrap().insert(pid, handle);
    }

    /// Lookup a process by Pid.
    pub fn lookup(&self, pid: &Pid) -> Option<ProcessHandle> {
        self.by_pid.read().unwrap().get(pid).cloned()
    }

    /// Lookup a process by name.
    pub fn lookup_name(&self, name: &str) -> Option<ProcessHandle> {
        let pid = self.by_name.read().unwrap().get(name).copied()?;
        self.lookup(&pid)
    }

    /// Remove a process from the registry.
    pub fn unregister(&self, pid: &Pid) {
        self.by_pid.write().unwrap().remove(pid);
        // Also remove from name registry
        let mut names = self.by_name.write().unwrap();
        names.retain(|_, v| v != pid);
    }

    /// Count of registered processes.
    pub fn len(&self) -> usize {
        self.by_pid.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ProcessRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::spawn_process;

    #[tokio::test]
    async fn test_register_and_lookup() {
        let registry = ProcessRegistry::new();
        let handle = spawn_process(|_ctx| async {});
        let pid = handle.pid;
        registry.register(handle);
        assert!(registry.lookup(&pid).is_some());
    }

    #[tokio::test]
    async fn test_named_lookup() {
        let registry = ProcessRegistry::new();
        let handle = spawn_process(|_ctx| async {});
        registry.register_name("worker".to_string(), handle);
        assert!(registry.lookup_name("worker").is_some());
    }
}
