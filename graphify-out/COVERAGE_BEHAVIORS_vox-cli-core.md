# Semantic Behavior Map — `vox-cli-core`

Deterministically synthesized from 11 distinct proven-behavior claims (of 11 extracted) across 6 symbols. 1 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `is_allowed_artifact_path()`  (error, happy; EXTRACTED)
- [happy] is_allowed_artifact_path() returns true for paths under canonical workspace target directory (root/target and subdirs)  (crates/vox-cli-core/src/artifact_policy.rs)
- [error] is_allowed_artifact_path() returns false for paths matching root-level target sprawl patterns (target-* or target_* siblings)  (crates/vox-cli-core/src/artifact_policy.rs)
- [happy] is_allowed_artifact_path() returns true for paths under system temp vox-targets directory  (crates/vox-cli-core/src/artifact_policy.rs)

### `ClaimReviewDecisionCli::as_stored()`  (happy, invariant; EXTRACTED)
- [happy] as_stored() maps Approve variant to 'approved', Reject to 'rejected', and Defer to 'deferred' strings  (crates/vox-cli-core/src/scientia.rs)
- [invariant] All ClaimReviewDecisionCli variants produce as_stored() values contained in vox_db::store::VALID_DECISIONS  (crates/vox-cli-core/src/scientia.rs)

### `GlobalOpts::quiet`  (happy; EXTRACTED)
- [happy] -q short flag parses and sets GlobalOpts.quiet field to true  (crates/vox-cli-core/src/lib.rs)
- [happy] --quiet long flag parses and sets GlobalOpts.quiet field to true  (crates/vox-cli-core/src/lib.rs)

### `process_is_running()`  (edge, happy; EXTRACTED)
- [happy] process_is_running() returns true when called with current process PID  (crates/vox-cli-core/src/daemon_ipc/process_supervision.rs)
- [edge] process_is_running() returns false for PID 0  (crates/vox-cli-core/src/daemon_ipc/process_supervision.rs)

### `DispatchRequest`  (happy; EXTRACTED)
- [happy] DispatchRequest with AI_GENERATE method and prompt param serializes and validates against contracts/dei/rpc-methods.schema.json  (crates/vox-cli-core/src/daemon_ipc/dispatch_protocol.rs)

### `dei_method`  (invariant; EXTRACTED)
- [invariant] All 9 daemon dei_method constants (AI_CHECK, AI_FIX, AI_REVIEW, AI_GENERATE, CONFIG_GET, AI_PLAN_NEW, AI_PLAN_REPLAN, AI_PLAN_STATUS, AI_PLAN_EXECUTE) are present in schema enum and schema enum count is exactly 9  (crates/vox-cli-core/src/daemon_ipc/dispatch_protocol.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`DispatchRequest`** — only: _DispatchRequest with AI_GENERATE method and prompt param serializes and validates against contracts/dei/rpc-methods.schema.json_
- **`GlobalOpts::quiet`** — only: _-q short flag parses and sets GlobalOpts.quiet field to true_
