use crate::process::ProcessHandle;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing;

/// Restart strategy for supervised processes.
#[derive(Debug, Clone, Copy)]
pub enum RestartStrategy {
    /// Restart only the crashed child.
    OneForOne,
    /// Restart all children if any crashes.
    OneForAll,
    /// Restart the crashed child and all children started after it.
    RestForOne,
}

/// Specification for a supervised child process.
pub struct ChildSpec {
    pub name: String,
    pub start: Box<dyn Fn() -> ProcessHandle + Send + Sync>,
}

/// A supervisor managing a set of child actor processes.
pub struct Supervisor {
    strategy: RestartStrategy,
    children: Arc<RwLock<Vec<ChildEntry>>>,
    max_restarts: u32,
}

struct ChildEntry {
    name: String,
    handle: ProcessHandle,
    start: Arc<dyn Fn() -> ProcessHandle + Send + Sync>,
    restart_count: u32,
}

impl Supervisor {
    pub fn new(strategy: RestartStrategy) -> Self {
        Self {
            strategy,
            children: Arc::new(RwLock::new(Vec::new())),
            max_restarts: 5,
        }
    }

    pub fn max_restarts(mut self, count: u32) -> Self {
        self.max_restarts = count;
        self
    }

    /// Add a child specification and start the process.
    pub async fn add_child(&self, spec: ChildSpec) {
        let handle = (spec.start)();
        let entry = ChildEntry {
            name: spec.name.clone(),
            handle,
            start: Arc::new(spec.start),
            restart_count: 0,
        };
        self.children.write().await.push(entry);
        tracing::info!("Supervisor started child: {}", spec.name);
    }

    /// Monitor all children and apply restart strategy when any dies.
    pub async fn monitor_loop(&self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let mut children = self.children.write().await;
            let mut needs_restart = Vec::new();

            for (i, child) in children.iter().enumerate() {
                if !child.handle.is_alive() {
                    tracing::warn!("Child '{}' has died", child.name);
                    needs_restart.push(i);
                }
            }

            for idx in needs_restart.into_iter().rev() {
                let child = &mut children[idx];
                if child.restart_count >= self.max_restarts {
                    tracing::error!("Child '{}' exceeded max restarts ({})", child.name, self.max_restarts);
                    continue;
                }

                match self.strategy {
                    RestartStrategy::OneForOne => {
                        tracing::info!("Restarting child '{}' (attempt {})", child.name, child.restart_count + 1);
                        let new_handle = (child.start)();
                        child.handle = new_handle;
                        child.restart_count += 1;
                    }
                    RestartStrategy::OneForAll => {
                        tracing::info!("Restarting all children due to '{}' failure", child.name);
                        for c in children.iter_mut() {
                            let new_handle = (c.start)();
                            c.handle = new_handle;
                            c.restart_count += 1;
                        }
                        break;
                    }
                    RestartStrategy::RestForOne => {
                        tracing::info!("Restarting '{}' and subsequent children", child.name);
                        for c in children[idx..].iter_mut() {
                            let new_handle = (c.start)();
                            c.handle = new_handle;
                            c.restart_count += 1;
                        }
                        break;
                    }
                }
            }
        }
    }
}
