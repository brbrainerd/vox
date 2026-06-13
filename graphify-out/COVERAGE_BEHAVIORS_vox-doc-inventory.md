# Semantic Behavior Map — `vox-doc-inventory`

## Summary
Four `EXTRACTED` behavior claims were collected for `vox-doc-inventory`, each mapping to a distinct symbol in `crates/vox-doc-inventory/src/lib.rs` (tests in the inline `#[cfg(test)] mod tests`). All four are **happy-path** assertions; none exercises an error path, edge case, or invariant. Every symbol with a clear empty/conflict/missing-input contract is therefore semantically under-proven. The two pure transforms — `strip_generated_at` and `rust_symbol_hints` — are the highest-value gaps because their contracts have obvious untested modes (field absent; doc comment with no following item).

## Per-symbol proven behaviors

### `relevance_score()`
- Proven: returns a strictly higher score for a `FileEntry` with `hotspot_tier=1` than `hotspot_tier=0` when all other fields are identical (monotonic in hotspot tier).
- Error path: none. Edge/invariant: none.

### `strip_generated_at()`
- Proven: removes the `generated_at` field from a JSON object that contains it.
- Error path: none. Edge/invariant: none (no proof of no-op behavior when the field is absent or input is not an object).

### `normalize_json_value()` (`verify_normalize`)
- Proven: sorts object keys alphabetically at the top level (`a` before `z`), exercised on an object with one nested object.
- Error path: none. Edge/invariant: none (no array, deep-nesting, empty-object, or idempotence proof).

### `hints::rust_symbol_hints()`
- Proven: parses a `///` doc comment and emits a symbol hint whose `item_preview` references the immediately following `fn` name.
- Error path: none. Edge/invariant: none.

## Semantic gaps
All four symbols are proven on the happy path only. Most actionable:

1. **`strip_generated_at()` — mutator with no no-op/failure path.** Only the "field present, gets removed" case is proven. The contract clearly admits an empty/missing mode: an object without `generated_at`, a non-object JSON value, or nested occurrences. Add a test asserting it is a no-op when the field is absent and does not panic on non-object input.
2. **`hints::rust_symbol_hints()` — parser/validator with no empty-result or conflict proof.** Only "`///` directly above a fn" is proven. Untested: a doc comment with no following item, blank lines or attributes between doc and item, non-`fn` items (struct/impl/enum), and input containing zero doc comments (should yield an empty result). This is an integrity-adjacent surface (it drives the inventory) and deserves an empty/negative case.
3. **`normalize_json_value()` — normalizer with no idempotence or structural-edge proof.** Sorting is the whole point of a normalizer, yet arrays, deep nesting, empty objects, and the idempotence invariant (`normalize(normalize(x)) == normalize(x)`) are unproven.
4. **`relevance_score()` — scoring function with only a single monotonicity point.** No tie behavior, zero-doc-density floor, or ordering stability across multiple tiers/fields is proven; the single assertion does not pin the contract.