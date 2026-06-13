## Semantic Behavior Map — `vox-mesh-policy`

One paragraph summary: The 10 extracted claims cover two surfaces of `vox-mesh-policy`: the policy round-trip in `src/lib.rs` (`parse_source()` + `pretty_print()`) and the in-memory model registry in `src/models/registry.rs` (`upsert_node()`, `remove_node()`, read via `all_models()`). The parse/print pair is the best-covered area — it carries two genuine invariant proofs (round-trip structural equality and byte-level determinism), a happy path on slot + scalar extraction, and a minimal-input edge case (scalars only, no slots). The registry mutators, by contrast, are proven exclusively on the happy path: add-then-read and remove-then-read, with no failure, overwrite, empty, or no-op edges in the claim set. The sharpest semantic hole is `parse_source()`: it is a validator with a three-variant `ParseError` (`Parse`, `MissingField`, `Io`) yet not one claim drives a rejection.

### `parse_source()` — `crates/vox-mesh-policy/src/lib.rs` (impl in `src/parse.rs`)
Distinct proven behaviors:
- Happy: parses source with `slot` declarations, extracting correct `TaskKind`, `max_concurrent`, and `weight_pct` per slot.
- Happy: extracts the four scalar fields (`nsfw_allowed`, `max_job_duration_secs`, `public_mesh_opt_in`, `min_priority`).
- Edge: parses a minimal policy with scalars only and no slot declarations (`slots.is_empty()`).
- (Participates in invariant proofs below as the re-parse half of the round trip.)

Error path proven: **No.** Edge/invariant proven: **Yes** (minimal-input edge; round-trip invariant via `pretty_print`). Despite the `ParseError::{Parse, MissingField, Io}` contract, no malformed-input or missing-field rejection is exercised.

### `pretty_print()` — `crates/vox-mesh-policy/src/lib.rs` (impl in `src/print.rs`)
Distinct proven behaviors:
- Invariant: output re-parses via `parse_source()` to a structurally equal `WorkerDonationPolicy` (`policy1 == policy2`).
- Invariant: byte-identical output across consecutive invocations on the same policy (determinism).

Error path proven: N/A (infallible signature). Edge/invariant proven: **Yes** (two strong invariants). Well-covered for its contract.

### `upsert_node()` — `crates/vox-mesh-policy/src/models/registry.rs`
Distinct proven behaviors:
- Happy: inserting a node's model inventory makes it retrievable via `all_models()` (correct `tag` and `node_id`).

Error path proven: N/A (infallible). Edge/invariant proven: **No.** The replace-existing-node semantics (`HashMap::insert` overwrites) and the empty-`models` vector case are not proven by the claim set.

### `remove_node()` — `crates/vox-mesh-policy/src/models/registry.rs`
Distinct proven behaviors:
- Happy: removing a node drops its inventory; `all_models()` then returns only the remaining nodes' models.

Error path proven: N/A (infallible). Edge/invariant proven: **No.** Remove-nonexistent-node (silent no-op via `HashMap::remove`) and remove-last-node-empties-registry are unproven here.

### `all_models()` — `crates/vox-mesh-policy/src/models/registry.rs`
Distinct proven behaviors (as the read view in the above claims):
- Happy: returns the model with correct `tag`/`node_id` after `upsert_node()`.
- Happy: returns only non-removed nodes' models after `remove_node()`.

Error path proven: N/A. Edge/invariant proven: **Not in this claim set.** The documented dedup/merge invariant (same `tag` + `kind` merged, `nodes` lists unioned, output sorted by tag) and the empty-registry case exist as sibling tests in the file but are absent from the extracted claims.

## Semantic gaps

Symbols proven only on the happy path whose contract clearly has a failure/empty/conflict mode:

1. **`parse_source()` — validator with no rejection test (most actionable).** It returns `Result<_, ParseError>` with `Parse` and `MissingField` variants, but every claim asserts `.expect(...)` success. There is no proof that malformed source yields `ParseError::Parse`, that a missing required scalar yields `ParseError::MissingField`, or how a bad literal (e.g. non-integer `max_concurrent`, non-bool `nsfw_allowed`) is handled. The "unknown keys ignored for forward compatibility" promise is also unverified. Add negative tests for each rejection mode.

2. **`upsert_node()` — mutator with no overwrite/empty edge.** Backed by `HashMap::insert`, a second upsert on the same `node_id` replaces the prior inventory (not merges). No claim proves this replace-not-accumulate semantics, nor the empty-`models` vector case. This is a conflict mode (re-report on heartbeat) worth a test.

3. **`remove_node()` — mutator with no no-op / empty path.** Removing a `node_id` that was never registered must be a silent no-op, and removing the last node must empty the registry. Neither is proven; both are realistic mesh-churn paths.

4. **`all_models()` — integrity/merge invariant not in the tracked set.** The dedup-and-union merge is the function's core correctness contract and an aggregation integrity surface. A sibling test exists in-file but it is outside the extracted claims, so the semantic map shows it as unproven; surface/track it.