//! Durable workflow execution tracker trait and no-op default.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// An engine tracker that allows the interpreted runner to persist durable states.
pub trait WorkflowTracker: Send + Sync {
    /// Check if a specific step was already completed in a prior, durable run.
    fn is_activity_completed(
        &self,
        _workflow_name: &str,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<bool>> + Send {
        async { Ok(false) }
    }

    /// Load the stored result payload for a completed durable step, when available.
    fn load_activity_result(
        &self,
        _workflow_name: &str,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<Value>>> + Send {
        async { Ok(None) }
    }

    /// Called when the workflow plan begins.
    fn on_workflow_started(
        &mut self,
        _workflow_name: &str,
        _plan_len: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when an activity starts execution.
    fn on_activity_started(
        &mut self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when one activity attempt starts under the execution boundary.
    fn on_activity_attempt_started(
        &mut self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
        _attempt: u32,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when one activity attempt fails.
    fn on_activity_attempt_failed(
        &mut self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
        _attempt: u32,
        _error: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when one activity attempt succeeds.
    fn on_activity_attempt_completed(
        &mut self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
        _attempt: u32,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Return the next attempt number to use for this activity.
    fn next_activity_attempt_start(
        &self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<u32>> + Send {
        async { Ok(1) }
    }

    /// Called when an activity fully completes.
    fn on_activity_completed(
        &mut self,
        _workflow_name: &str,
        _activity_name: &str,
        _activity_id: &str,
        _result: &Value,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Called when the workflow successfully completes all steps.
    fn on_workflow_completed(
        &mut self,
        _workflow_name: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// Load the previously recorded version for a `workflow.version` patch marker.
    fn load_workflow_patch(
        &self,
        _workflow_name: &str,
        _change_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<u32>>> + Send {
        async { Ok(None) }
    }

    /// Persist the chosen version for a `workflow.version` patch marker.
    fn record_workflow_patch(
        &mut self,
        _workflow_name: &str,
        _change_id: &str,
        _version: u32,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    /// P2-T5: try the activity result cache. `Ok(None)` for miss; `Ok(Some(_))`
    /// for hit (caller skips the body). Default: always miss.
    fn load_cached_activity_result(
        &self,
        _activity_id: &str,
        _arg_hash_hex: &str,
        _now_unix_ms: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<Value>>> + Send {
        async { Ok(None) }
    }

    /// P2-T5: upsert a cache entry. Default: no-op.
    fn record_cached_activity_result(
        &mut self,
        _activity_id: &str,
        _arg_hash_hex: &str,
        _result: &Value,
        _produced_at_unix_ms: u64,
        _dedup_window_ms: u64,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }
}

/// A default no-op tracker used if none is provided.
#[derive(Default)]
pub struct DefaultTracker;

impl WorkflowTracker for DefaultTracker {}

/// An in-memory [`WorkflowTracker`] for unit tests.
///
/// Stores activity completions in a process-local `HashMap` keyed by
/// `(workflow_name, activity_id)`. Avoids the `vox-db` dependency required by
/// [`crate::VoxDbTracker`] so workflow-runtime tests can exercise replay /
/// recording semantics without booting a database.
///
/// All other [`WorkflowTracker`] hooks (attempts, leases, patches, cache) fall
/// through to the trait's no-op defaults.
#[derive(Default)]
pub struct InMemoryTracker {
    /// `(workflow_name, activity_id) -> result_value`
    results: Mutex<HashMap<(String, String), Value>>,
}

impl InMemoryTracker {
    /// Create an empty in-memory tracker.
    pub fn new() -> Self {
        Self::default()
    }
}

impl WorkflowTracker for InMemoryTracker {
    fn is_activity_completed(
        &self,
        workflow_name: &str,
        activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<bool>> + Send {
        let key = (workflow_name.to_string(), activity_id.to_string());
        let hit = self
            .results
            .lock()
            .map(|m| m.contains_key(&key))
            .unwrap_or(false);
        async move { Ok(hit) }
    }

    fn load_activity_result(
        &self,
        workflow_name: &str,
        activity_id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<Value>>> + Send {
        let key = (workflow_name.to_string(), activity_id.to_string());
        let value = self.results.lock().ok().and_then(|m| m.get(&key).cloned());
        async move { Ok(value) }
    }

    fn on_activity_completed(
        &mut self,
        workflow_name: &str,
        _activity_name: &str,
        activity_id: &str,
        result: &Value,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        let key = (workflow_name.to_string(), activity_id.to_string());
        let value = result.clone();
        let outcome: anyhow::Result<()> = match self.results.lock() {
            Ok(mut m) => {
                m.insert(key, value);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("InMemoryTracker mutex poisoned: {e}")),
        };
        async move { outcome }
    }
}
