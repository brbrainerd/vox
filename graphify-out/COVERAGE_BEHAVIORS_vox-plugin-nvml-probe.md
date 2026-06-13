# Semantic Behavior Map — `vox-plugin-nvml-probe`

Deterministically synthesized from 1 distinct proven-behavior claims (of 1 extracted) across 1 symbols. 0 symbols have an explicit error-path proof; **1 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `NvmlProbePlugin::id()`  (happy; EXTRACTED)
- [happy] id() returns the string "nvml-probe" when converted via as_str()  (crates/vox-plugin-nvml-probe/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`NvmlProbePlugin::id()`** — only: _id() returns the string "nvml-probe" when converted via as_str()_
