# Semantic Behavior Map — `vox-shell-stdlib-types`

Deterministically synthesized from 3 distinct proven-behavior claims (of 3 extracted) across 1 symbols. 0 symbols have an explicit error-path proof; **1 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `VoxFileRecord`  (happy; EXTRACTED)
- [happy] Clone trait produces a value that equals the original via PartialEq  (crates/vox-shell-stdlib-types/src/lib.rs)
- [happy] PartialEq correctly compares VoxFileRecord instances for equality  (crates/vox-shell-stdlib-types/src/lib.rs)
- [happy] Struct construction and field assignment work correctly with is_file=true and is_dir=false  (crates/vox-shell-stdlib-types/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`VoxFileRecord`** — only: _Clone trait produces a value that equals the original via PartialEq_
