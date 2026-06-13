# Semantic Behavior Map — `vox-distributed-training`

5 extracted claims collapse into behaviors over 4 distinct symbols across three modules (`checkpoint.rs`, `strategy/data_parallel.rs`, `strategy/mock.rs`). All proven behavior is happy-path: a serde round-trip, a single step increment, a resume restore, a step-result field match, and a concurrent multi-rank step. **No claim exercises an error path, an edge case, or an invariant** — so the failure, empty, and conflict modes that each of these contracts clearly has are entirely unverified.

## `OperationKind` — `checkpoint.rs`
- **Proven (happy):** `CheckpointBundle::to_operation_kind()` output round-trips through `serde_json` serialize→deserialize with equality preserved.
- Error path: none. Edge/invariant: none.
- Contract implies a failure mode (deserializing malformed / unknown-variant JSON) that is untested.

## `DataParallelSession::step` — `strategy/data_parallel.rs`
- **Proven (happy):** `step(Batch)` increments the step counter `0 → 1`.
- **Proven (happy):** the returned `StepResult.step` field matches internal session state.
- Error path: none. Edge/invariant: none.
- No proof of monotonicity across many steps, nor of stepping after a resume.

## `DataParallelSession::resume` — `strategy/data_parallel.rs`
- **Proven (happy):** restores `step_index` from a (valid) checkpoint bundle.
- Error path: none. Edge/invariant: none.
- This is an integrity-bearing mutator: restoring from a corrupt, empty, or version-mismatched bundle is unverified.

## `MockDistributedSession` (step + all_reduce) — `strategy/mock.rs`
- **Proven (happy):** in a 3-rank cluster, `step()` and `all_reduce()` run concurrently and the session reports step index `1` after execution.
- Error path: none. Edge/invariant: none.
- No proof for rank divergence, rank-count mismatch, or partial-rank failure during the collective.

## Semantic gaps

Every symbol in this set is happy-path-only, and each guards a contract with an obvious unhappy mode. Most actionable, in priority order:

1. **`DataParallelSession::resume` (integrity/mutator surface).** Restores state from a checkpoint with no rejection test for corrupt, empty, or version-mismatched bundles. A bad resume silently seeds a training run from garbage state — highest-value gap.
2. **`OperationKind` serde (validator surface).** Round-trip is proven only for well-formed input; deserializing an unknown/malformed `OperationKind` has no rejection proof.
3. **`MockDistributedSession::all_reduce` (collective/conflict surface).** Proven only when all 3 ranks agree; no rank-divergence, rank-count-mismatch, or partial-failure conflict test — exactly where a distributed collective is supposed to detect inconsistency.
4. **`DataParallelSession::step` (invariant).** Only the `0 → 1` increment is checked; the step-counter monotonicity invariant and step-after-resume edge are unproven.