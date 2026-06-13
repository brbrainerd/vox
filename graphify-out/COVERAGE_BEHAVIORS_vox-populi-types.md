# Semantic Behavior Map — vox-populi-types

Source: `crates/vox-populi-types/src/node_record.rs`

## Summary

All 5 extracted Behavior claims come from a single test pair (`merge_unions_keeps_fresher_and_is_deterministic`, `merge_tie_resolves_to_incoming_authoritative`) and all target one symbol: `merge_registry_by_last_seen`. After dedup they collapse to five distinct happy-path properties of the merge function. No error-path, empty-input, or clock-skew invariant is proven for any symbol, and five other public symbols in this crate — including a state mutator, a stale-filter, a maintenance gate, and an HTTP error-classification surface — have no extracted coverage at all.

## merge_registry_by_last_seen

Distinct proven behaviors (happy path):
- Produces a union of node ids across local and incoming, sorted deterministically by id.
- On conflict, the record with the fresher (higher) `last_seen_unix_ms` wins.
- On an equal-`last_seen` tie, incoming (control plane) wins (authoritative).
- Merged `schema_version` is the max of the two inputs.
- Merged `queue_depth` adopts the incoming value.

Error path proven: none.
Edge/invariant proven: partial — the equal-timestamp tie is a genuine boundary case, and the "strictly fresher local is kept" sibling case is covered by `merge_keeps_strictly_fresher_local` (present in source, not in the claim set). Empty inputs, no-overlap-only inputs, and the `queue_depth` fallback-to-local branch (`incoming.queue_depth.or(local.queue_depth)`, line 181) are unproven.

## Symbols with no extracted coverage

- `filter_registry_by_max_stale_ms` — stale-node filter. Contract has explicit empty/no-op mode (`None`/`0` returns unchanged) and a `saturating_sub` clock-skew guard; nothing proven.
- `node_maintenance_blocks_new_work` — drain-gate predicate with three branches (not-in-maintenance, deadline-passed, still-blocking); nothing proven.
- `sweep_expired_maintenance_on_nodes` — in-place mutator that clears expired maintenance; nothing proven.
- `PopuliRegistryError::status_code` / `is_http_status` — error-classification helpers over the `HttpStatus` variant vs all others; nothing proven.

## Semantic gaps

These are symbols/branches proven only on the happy path (or not at all) whose contracts clearly have a failure, empty, or conflict mode:

1. **`sweep_expired_maintenance_on_nodes` — mutator with no test at all.** It mutates `NodeRecord` state (clears `maintenance` + deadline). No proof it (a) clears expired entries, (b) preserves un-expired or `None`-deadline maintenance, or (c) no-ops on an empty slice. Highest-priority gap: a state mutator with zero coverage.

2. **`filter_registry_by_max_stale_ms` — filter with no rejection/empty test.** The documented "`None` or `0` returns the file unchanged" no-op path, the threshold-boundary drop (`<= threshold` is inclusive), and the `saturating_sub` clock-skew guard (node `last_seen` in the future) are all unproven.

3. **`node_maintenance_blocks_new_work` — gate with only one of three branches reachable by inspection.** The deadline-expiry "returns false" branch and the early `maintenance != Some(true)` return are unproven.

4. **`PopuliRegistryError::status_code` / `is_http_status` — error-surface discrimination unproven.** The `Some(status)` vs `None` split across `HttpStatus` and the other variants is an integrity/error-classification surface with no test.

5. **`merge_registry_by_last_seen` — happy-path-only despite conflict/empty modes.** Untested: empty-on-both-sides union, the `queue_depth` fallback-to-local branch when incoming is `None`, and duplicate ids within a single input registry (last-writer-wins inside one `for` loop).
