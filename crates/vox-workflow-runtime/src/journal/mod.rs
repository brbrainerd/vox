//! Activity-body journal wrapper used by codegen-emitted code.
//!
//! `journal::execute(activity_id, body)` is the wrapper the codegen emits
//! around every `activity` body. It records the activity completion in the
//! journal and short-circuits to the recorded value on replay.
//!
//! Today (Phase 1), the production path is a no-op for non-test builds: it
//! always runs the body and never records. The actual VoxDbTracker-backed
//! persistence lands in Phase 3. The `test-support` feature gates the
//! in-memory recording/replay used by unit tests.

mod execute;
pub use execute::execute;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    //! In-memory journal state for tests. Resets between test runs; not used
    //! in production binaries.

    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::OnceLock;

    fn seeded() -> &'static Mutex<HashMap<String, Value>> {
        static S: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn recorded() -> &'static Mutex<HashMap<String, Vec<Value>>> {
        static R: OnceLock<Mutex<HashMap<String, Vec<Value>>>> = OnceLock::new();
        R.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// Clear seeded and recorded state. Call at the start of each test.
    pub fn reset() {
        seeded().lock().unwrap().clear();
        recorded().lock().unwrap().clear();
    }

    /// Seed a recorded completion for `activity_id`. Subsequent
    /// `journal::execute` calls for this activity will replay from this value
    /// without running the body.
    pub fn seed_completed(activity_id: &str, value: Value) {
        seeded()
            .lock()
            .unwrap()
            .insert(activity_id.to_string(), value);
    }

    /// Return the list of values recorded for `activity_id` by
    /// `journal::execute` (not by `seed_completed`).
    pub fn recorded_for(activity_id: &str) -> Vec<Value> {
        recorded()
            .lock()
            .unwrap()
            .get(activity_id)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn lookup_seeded(activity_id: &str) -> Option<Value> {
        seeded().lock().unwrap().get(activity_id).cloned()
    }

    pub(crate) fn record(activity_id: &str, value: Value) {
        recorded()
            .lock()
            .unwrap()
            .entry(activity_id.to_string())
            .or_default()
            .push(value);
    }
}
