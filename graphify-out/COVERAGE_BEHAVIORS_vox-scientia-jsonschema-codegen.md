# Semantic Behavior Map — `vox-scientia-jsonschema-codegen`

Deterministically synthesized from 1 distinct proven-behavior claims (of 1 extracted) across 1 symbols. 0 symbols have an explicit error-path proof; **1 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `module_name()`  (happy; EXTRACTED)
- [happy] module_name() normalizes dots and hyphens in file stem to underscores, converting 'foo.bar-baz.schema' to 'foo_bar_baz_schema'  (crates/vox-scientia-jsonschema-codegen/src/main.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`module_name()`** — only: _module_name() normalizes dots and hyphens in file stem to underscores, converting 'foo.bar-baz.schema' to 'foo_bar_baz_schema'_
