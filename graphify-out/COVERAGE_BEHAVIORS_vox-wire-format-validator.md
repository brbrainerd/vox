# Semantic Behavior Map — `vox-wire-format-validator`

Deterministically synthesized from 3 distinct proven-behavior claims (of 3 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **0 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `EXPECTED_SSOT_HASH`  (invariant; EXTRACTED)
- [invariant] EXPECTED_SSOT_HASH is exactly 64 characters long  (crates/vox-wire-format-validator/src/lib.rs)
- [invariant] All characters in EXPECTED_SSOT_HASH are valid ASCII hexadecimal digits  (crates/vox-wire-format-validator/src/lib.rs)

### `DRIFT_DIAGNOSTIC_ID`  (invariant; EXTRACTED)
- [invariant] DRIFT_DIAGNOSTIC_ID is set to 'vox/wire-format/spec-drift'  (crates/vox-wire-format-validator/src/lib.rs)

## Semantic gaps (proven happy-path only)

_None — every proven symbol has at least one error/edge/invariant claim._
