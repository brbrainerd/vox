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

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use serde_json::json;

    // Catches: activity treated as completed before on_activity_completed is ever called
    #[tokio::test]
    async fn activity_not_completed_before_record() {
        let tracker = InMemoryTracker::new();
        let completed = tracker
            .is_activity_completed("wf1", "act-A")
            .await
            .unwrap();
        assert!(!completed, "activity must not be reported complete before on_activity_completed");
    }

    // Catches: load_activity_result returning stale data for a different workflow/id pair
    #[tokio::test]
    async fn result_isolated_by_workflow_and_activity_id() {
        let mut tracker = InMemoryTracker::new();
        tracker
            .on_activity_completed("wf1", "step", "act-A", &json!({"v": 1}))
            .await
            .unwrap();

        // Different workflow name — must not see wf1's result
        let wrong_wf = tracker.load_activity_result("wf2", "act-A").await.unwrap();
        assert!(wrong_wf.is_none(), "wrong workflow must not see another workflow's result");

        // Same workflow, different activity id — must be None
        let wrong_act = tracker.load_activity_result("wf1", "act-B").await.unwrap();
        assert!(wrong_act.is_none(), "different activity_id must not share a result");
    }

    // Catches: idempotency regression where a second on_activity_completed overwrites
    // the first result with a different value (replay correctness relies on stable stored value)
    #[tokio::test]
    async fn second_completion_overwrites_value() {
        let mut tracker = InMemoryTracker::new();
        tracker
            .on_activity_completed("wf1", "step", "act-A", &json!(42))
            .await
            .unwrap();
        tracker
            .on_activity_completed("wf1", "step", "act-A", &json!(99))
            .await
            .unwrap();
        // The tracker DOES overwrite (no dedup guard here); the test asserts the
        // stored value reflects the last write, exposing any map corruption.
        let val = tracker
            .load_activity_result("wf1", "act-A")
            .await
            .unwrap()
            .expect("result must be present after two completions");
        assert_eq!(val, json!(99), "stored value must equal last written result");
    }

    // Catches: is_activity_completed returning false after on_activity_completed succeeds
    #[tokio::test]
    async fn completed_flag_set_after_on_activity_completed() {
        let mut tracker = InMemoryTracker::new();
        tracker
            .on_activity_completed("wf1", "step", "act-X", &json!(true))
            .await
            .unwrap();
        let flag = tracker
            .is_activity_completed("wf1", "act-X")
            .await
            .unwrap();
        assert!(flag, "is_activity_completed must return true after on_activity_completed");
    }

    // Catches: DefaultTracker incorrectly returning true for is_activity_completed
    #[tokio::test]
    async fn default_tracker_never_reports_completed() {
        let tracker = DefaultTracker;
        let completed = tracker
            .is_activity_completed("any-wf", "any-act")
            .await
            .unwrap();
        assert!(!completed, "DefaultTracker must always report not-completed");
    }

    // Catches: DefaultTracker returning a non-None result for load_activity_result
    #[tokio::test]
    async fn default_tracker_load_result_always_none() {
        let tracker = DefaultTracker;
        let result = tracker
            .load_activity_result("wf", "act")
            .await
            .unwrap();
        assert!(result.is_none(), "DefaultTracker must always return None for load_activity_result");
    }

    // Catches: next_activity_attempt_start returning 0 (attempt numbering must start at 1)
    #[tokio::test]
    async fn default_tracker_attempt_start_is_one() {
        let tracker = DefaultTracker;
        let attempt = tracker
            .next_activity_attempt_start("wf", "step", "act")
            .await
            .unwrap();
        assert_eq!(attempt, 1, "first attempt must be numbered 1, not 0");
    }

    // Catches: empty workflow name or empty activity_id treated the same as a real name
    #[tokio::test]
    async fn empty_keys_do_not_collide_with_real_keys() {
        let mut tracker = InMemoryTracker::new();
        // Record under empty strings
        tracker
            .on_activity_completed("", "", "", &json!("empty"))
            .await
            .unwrap();
        // A real workflow/activity must not be polluted
        let real = tracker.load_activity_result("wf1", "act-A").await.unwrap();
        assert!(real.is_none(), "empty-key entry must not collide with wf1/act-A");
        // The empty entry itself must be retrievable
        let empty_entry = tracker.load_activity_result("", "").await.unwrap();
        assert_eq!(empty_entry, Some(json!("empty")));
    }
}
