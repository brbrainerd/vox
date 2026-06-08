# Semantic Behavior Map — `vox-rename-registry`

Deterministically synthesized from 4 distinct proven-behavior claims (of 4 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **1 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `primitive_tags::is_primitive`  (happy; EXTRACTED)
- [happy] is_primitive returns true for the tag 'stack'  (crates/vox-rename-registry/src/lib.rs)
- [happy] is_primitive returns true for the tag 'button'  (crates/vox-rename-registry/src/lib.rs)
- [happy] is_primitive returns false for invalid tag names like 'not-a-real-primitive'  (crates/vox-rename-registry/src/lib.rs)

### `primitive_tags::all_primitives`  (invariant; EXTRACTED)
- [invariant] all_primitives returns a non-empty collection  (crates/vox-rename-registry/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`primitive_tags::is_primitive`** — only: _is_primitive returns true for the tag 'stack'_
