//! Workflow planning types: mens control ops and planned activity descriptors.

/// Control-plane sub-step for a [`PopuliActivity`] (URL always comes from env / `Vox.toml`, not source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopuliHttpOp {
    /// `POST` heartbeat with the current node record.
    Heartbeat,
    /// Log only; still runs local registry publish when mens is enabled.
    Noop,
    /// `POST /v1/populi/join` for this process record.
    Join,
    /// `GET /v1/populi/nodes` (counts in journal only; no arbitrary URLs).
    Snapshot,
    /// `POST /v1/populi/dispatch` for remote task execution.
    Dispatch,
    /// `GET /v1/populi/dispatch/result/{dispatch_id}` for remote task polling.
    Wait,
}

/// One planned activity invocation extracted from workflow HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedActivity {
    /// Activity name as referenced in the workflow body.
    pub name: String,
    /// When true, run the mens / Populi HTTP step (`execute_populi_step` when feature `mens` is on).
    pub mens: bool,
    /// Idempotency / journal key from `with { activity_id: "…" }` when set.
    pub activity_id: Option<String>,
    /// Wall-clock timeout for mens HTTP sub-steps from `with { timeout: … }` (milliseconds).
    pub timeout_ms: Option<u64>,
    /// Additional attempts after the first one for interpreted mesh activity execution.
    pub retries: u32,
    /// Delay before the first retry after a failed interpreted mesh activity attempt.
    pub initial_backoff_ms: Option<u64>,
    /// Populi control-plane operation when [`Self::mens`] is true.
    pub populi_op: PopuliHttpOp,
    /// Optional labels for mesh routing (e.g. `gpu`, `region=us-east-1`).
    pub required_labels: Option<Vec<String>>,
    /// When true, dispatch as a detached task and poll for completion.
    pub is_detached: bool,
    /// P2-T5: structural argument values for dedup-cache keying. Empty when planning-time args are unavailable.
    pub arguments: Vec<serde_json::Value>,
    /// P2-T5: dedup window from `@activity(dedup = "…")` in milliseconds. Default: 24h.
    pub dedup_window_ms: Option<u64>,
}

/// P2-T5: derive a hex hash of the canonicalized argument list, used to key
/// the dedup cache. Implementation detail: blake3 because it's already a
/// workspace dep and gives the same hex-string shape `vox-db::hash`
/// produced before this crate became feature-gated for mobile.
pub fn compute_structural_arg_hash(args: &[serde_json::Value]) -> String {
    let canonical = serde_json::Value::Array(args.to_vec()).to_string();
    blake3::hash(canonical.as_bytes()).to_hex().to_string()
}

/// Replay-oriented node for interpreted durable workflow execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayNode {
    /// Execute one activity step and persist/replay by `activity_id`.
    Activity(PlannedActivity),
    /// `workflow.version("change-id", min, max)` patch-marker (P2-T2).
    WorkflowPatch {
        /// Stable identifier for this version gate (e.g. `"add-retry-v2"`).
        change_id: String,
        /// Minimum workflow version that takes the new code path.
        min: u32,
        /// Maximum workflow version that takes the new code path (inclusive).
        max: u32,
    },
}

/// Linear replay IR produced from workflow HIR for the interpreted runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowReplayIr {
    /// Ordered replay nodes for deterministic interpreted execution.
    pub nodes: Vec<ReplayNode>,
}

/// P2-T5: unit tests for compute_structural_arg_hash and related types.
#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use serde_json::json;

    // Catches: hash function returning the same hex for different argument lists
    // (collision in the dedup-cache key causes incorrect replay)
    #[test]
    fn different_args_produce_different_hashes() {
        let h1 = compute_structural_arg_hash(&[json!(1), json!("a")]);
        let h2 = compute_structural_arg_hash(&[json!(2), json!("b")]);
        assert_ne!(
            h1, h2,
            "distinct argument lists must hash to different hex strings"
        );
    }

    // Catches: hash being non-deterministic across calls (non-stable sort or RNG)
    #[test]
    fn same_args_produce_same_hash_on_repeated_call() {
        let args = vec![json!({"x": 1}), json!([1, 2, 3])];
        let h1 = compute_structural_arg_hash(&args);
        let h2 = compute_structural_arg_hash(&args);
        assert_eq!(h1, h2, "hash must be deterministic for identical arguments");
    }

    // Catches: empty argument slice producing wrong or panicking output
    #[test]
    fn empty_args_hash_is_stable_non_empty_hex() {
        let h = compute_structural_arg_hash(&[]);
        assert!(
            !h.is_empty(),
            "empty-args hash must still produce a non-empty hex string"
        );
        // blake3 hex is always 64 chars
        assert_eq!(h.len(), 64, "blake3 hex must be 64 characters");
    }

    // Catches: arg ordering being ignored (hash([A,B]) == hash([B,A]))
    // which would treat order-distinct calls as duplicates
    #[test]
    fn arg_order_affects_hash() {
        let h_ab = compute_structural_arg_hash(&[json!("A"), json!("B")]);
        let h_ba = compute_structural_arg_hash(&[json!("B"), json!("A")]);
        assert_ne!(h_ab, h_ba, "argument order must affect the hash");
    }

    // Catches: PopuliHttpOp variants not being distinct (enum layout bug)
    #[test]
    fn populi_http_op_variants_are_distinct() {
        let ops = [
            PopuliHttpOp::Heartbeat,
            PopuliHttpOp::Noop,
            PopuliHttpOp::Join,
            PopuliHttpOp::Snapshot,
            PopuliHttpOp::Dispatch,
            PopuliHttpOp::Wait,
        ];
        // All pairs must be unequal
        for (i, a) in ops.iter().enumerate() {
            for (j, b) in ops.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "PopuliHttpOp variants {i} and {j} must not be equal");
                }
            }
        }
    }
}

/// Mens-tagged activity (name convention: `mesh_*`, plus [`PopuliHttpOp`]).
#[derive(Debug, Clone)]
pub struct PopuliActivity {
    /// Activity name from source.
    pub name: String,
    /// Resolved mens HTTP operation.
    pub populi_op: PopuliHttpOp,
    /// Timeout for populi HTTP client (defaults inside `execute_populi_step` when unset).
    pub timeout_ms: Option<u64>,
    /// Stable id for journal / idempotency (`with { activity_id }` or generated).
    pub activity_id: String,
    /// Mesh routing labels.
    pub required_labels: Option<Vec<String>>,
    /// Asynchronous execution.
    pub is_detached: bool,
}
