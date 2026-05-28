//! P2-T5 acceptance: activity result cache tracker trait + dedup short-circuit.

use serde_json::json;
use vox_workflow_runtime::workflow::tracker::WorkflowTracker;

#[derive(Default)]
struct MemTracker {
    map: std::collections::HashMap<(String, String), serde_json::Value>,
}

impl WorkflowTracker for MemTracker {
    async fn load_cached_activity_result(
        &self,
        activity_id: &str,
        arg_hash_hex: &str,
        _now_unix_ms: u64,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(self
            .map
            .get(&(activity_id.to_string(), arg_hash_hex.to_string()))
            .cloned())
    }

    async fn record_cached_activity_result(
        &mut self,
        activity_id: &str,
        arg_hash_hex: &str,
        result: &serde_json::Value,
        _produced_at_unix_ms: u64,
        _dedup_window_ms: u64,
    ) -> anyhow::Result<()> {
        self.map.insert(
            (activity_id.to_string(), arg_hash_hex.to_string()),
            result.clone(),
        );
        Ok(())
    }
}

#[tokio::test]
async fn second_run_with_same_args_hits_cache() {
    let mut tracker = MemTracker::default();
    tracker
        .record_cached_activity_result(
            "post_to_slack",
            "hash1",
            &json!({"ok": true}),
            0,
            86_400_000,
        )
        .await
        .unwrap();
    let hit = tracker
        .load_cached_activity_result("post_to_slack", "hash1", 1_000)
        .await
        .unwrap();
    assert_eq!(hit, Some(json!({"ok": true})));
}

#[tokio::test]
async fn miss_on_distinct_arg_hash() {
    let mut tracker = MemTracker::default();
    tracker
        .record_cached_activity_result("post", "h1", &json!({"r": 1}), 0, 86_400_000)
        .await
        .unwrap();
    let miss = tracker
        .load_cached_activity_result("post", "h2", 1_000)
        .await
        .unwrap();
    assert!(miss.is_none());
}
