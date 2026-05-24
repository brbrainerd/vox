//! `execute(activity_id, body)`: run-and-record on first execution; replay from
//! journal on resume. Phase 1 uses the in-memory test_support state behind a
//! cfg gate; Phase 3 swaps in `VoxDbTracker` for the production path.

use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;

/// Wrap an activity body. On first run, execute the body and persist the result
/// to the journal under `activity_id`. On replay (when the journal already has
/// a completed entry for `activity_id`), return the persisted value without
/// re-running the body.
///
/// Failed bodies (returning `Err`) are NOT recorded — they propagate to the
/// caller, who decides whether to retry or fail the workflow.
pub async fn execute<T, F>(activity_id: &str, body: F) -> Result<T, anyhow::Error>
where
    T: Serialize + DeserializeOwned + 'static,
    F: Future<Output = Result<T, anyhow::Error>>,
{
    #[cfg(any(test, feature = "test-support"))]
    {
        if let Some(seeded) = super::test_support::lookup_seeded(activity_id) {
            let value: T = serde_json::from_value(seeded)?;
            return Ok(value);
        }
    }

    let value = body.await?;

    #[cfg(any(test, feature = "test-support"))]
    {
        let json = serde_json::to_value(&value)?;
        super::test_support::record(activity_id, json);
    }

    // Production path (Phase 3): persist via VoxDbTracker. The codegen-emitted
    // call doesn't have the tracker in scope here — Phase 3 will route through
    // a runtime-side tracker registered at boot (mirroring the
    // current_hir_module pattern from Task 1.1).
    #[cfg(not(any(test, feature = "test-support")))]
    {
        let _ = activity_id;
    }

    Ok(value)
}
