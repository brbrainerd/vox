# Semantic Behavior Map — `vox-build-meta`

Deterministically synthesized from 1 distinct proven-behavior claims (of 1 extracted) across 1 symbols. 0 symbols have an explicit error-path proof; **1 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `git_stdout`  (happy; EXTRACTED)
- [happy] git_stdout returns None when called with a non-existent git subcommand instead of panicking  (crates/vox-build-meta/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`git_stdout`** — only: _git_stdout returns None when called with a non-existent git subcommand instead of panicking_
