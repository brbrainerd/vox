# Semantic Behavior Map — `vox-plugin-runtime-wasm`

Deterministically synthesized from 2 distinct proven-behavior claims (of 2 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `RuntimeWasmPlugin::id`  (happy; EXTRACTED)
- [happy] RuntimeWasmPlugin.id() returns the string 'runtime-wasm'  (crates/vox-plugin-runtime-wasm/src/lib.rs)

### `manifest_json`  (happy; EXTRACTED)
- [happy] manifest_json() returns a JSON string containing the substring '"runtime-wasm"'  (crates/vox-plugin-runtime-wasm/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`RuntimeWasmPlugin::id`** — only: _RuntimeWasmPlugin.id() returns the string 'runtime-wasm'_
- **`manifest_json`** — only: _manifest_json() returns a JSON string containing the substring '"runtime-wasm"'_
