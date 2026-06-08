# vox-package — Semantic Behavior Map

Synthesized from 13 extracted Behavior claims (12 distinct after dedup), grouped by the symbol under test. The crate covers two subsystems: a **build artifact cache** (`artifact_cache.rs`) and a **content-addressed bundle/model store** (`model_bundle.rs`, `tests/bundle_cas.rs`). Proven behavior is dense on happy-path round-trips with three genuine invariants (hash determinism, serde-stable bundle hash, idempotent `put`) and one edge case (miss → `None`). The notable holes are on the integrity and mutation surfaces: the bundle-hash verifier is never exercised on a bad hash, and neither `put()` nor `prune()` has a failure path despite obvious conflict/IO failure modes.

## ArtifactCache.lookup()
Proven behaviors:
- Returns `CacheLookup::Miss` when no entry exists for a hash. *(happy)*
- Returns `CacheLookup::Hit` with manifest + `artifact_dir` after `record_build()`. *(happy)*
- Returns `Miss` again after `prune()` removes the entry. *(happy)*

Error path: none. Edge/invariant: none (the post-prune miss is happy-path state transition, not a corrupt-entry edge).

## CacheLookup::Hit
Proven behaviors:
- `Hit` carries `input_hash`, `description`, and `artifact_dir` matching the recorded build metadata. *(happy)*

Error path: n/a (data carrier). Edge/invariant: none.

## ArtifactCache.compute_input_hash()
Proven behaviors:
- Deterministic: identical input files + options → identical hash. *(invariant)*
- Content-sensitive: changed file content → different hash. *(happy)*

Error path: none. Edge/invariant: has the determinism invariant; no proof for missing/unreadable inputs or option-change sensitivity.

## ArtifactCache.prune()
Proven behaviors:
- Removes entries older than the threshold and returns the count removed. *(happy)*

Error path: none. Edge/invariant: none.

## ModelBundle / verify_bundle_hash()
Proven behaviors:
- `verify_bundle_hash()` returns `true` when `bundle_hash` was correctly computed via `compute_model_bundle_content_hash()`. *(happy)*
- `ModelBundle` serde round-trip preserves `bundle_hash` and still passes verification. *(invariant)*

Error path: none. Edge/invariant: has the serde-stability invariant; **the verifier's negative case (tampered/mismatched hash → false) is unproven.**

## BundleStore.put()
Proven behaviors:
- Stores a bundle and returns a `BundleRef` with matching `fn_hash`. *(happy)*
- Idempotent: two `put()` calls with the same bundle both succeed and return the same `fn_hash`. *(invariant)*

Error path: none. Edge/invariant: has the idempotency invariant; no IO-failure or hash-collision-with-differing-bytes proof.

## BundleStore.lookup()
Proven behaviors:
- Retrieves a bundle by `BundleRef`, returning the same `fn_hash` and bytes that were stored. *(happy)*
- Returns `None` (not an error) when the bundle is absent. *(edge)*

Error path: none beyond the not-found edge. Edge/invariant: covered for miss; no corrupt-entry path.

## Semantic gaps

Symbols proven only on the happy path whose contracts have an obvious failure/empty/conflict mode, ordered by actionability:

1. **`ModelBundle.verify_bundle_hash()` — integrity surface, untested negative.** Proven only to return `true` for a valid hash. There is no test feeding a tampered/mismatched `bundle_hash` to confirm it returns `false`. A verifier that is never seen rejecting is the highest-value gap — a regression silently turning it into "always true" would pass every current test.
2. **`BundleStore.put()` — mutator with no failure path.** Happy + idempotent are covered, but there is no test for IO failure or, critically, a `fn_hash` collision where the stored bytes differ from the new bytes (does it overwrite, dedup, or error?). CAS conflict semantics are unspecified by the suite.
3. **`ArtifactCache.prune()` — mutator, only the removing case.** No proof for an empty cache, all-entries-fresh (returns 0, removes nothing), the exact threshold boundary, or an IO failure mid-removal.
4. **`ArtifactCache.compute_input_hash()` — no input-error proof.** Determinism and content-sensitivity exist, but behavior on missing/unreadable input files and on options-only changes is unproven.
5. **`ArtifactCache.lookup()` — no corrupt-entry path.** Only clean miss/hit transitions are proven; a partially-written or unreadable manifest is untested.