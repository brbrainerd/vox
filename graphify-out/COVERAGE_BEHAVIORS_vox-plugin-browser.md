# Semantic Behavior Map — `vox-plugin-browser`

Deterministically synthesized from 2 distinct proven-behavior claims (of 2 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `BrowserPluginHost::id`  (happy; EXTRACTED)
- [happy] BrowserPluginHost.id() returns RString with value 'browser'  (crates/vox-plugin-browser/src/lib.rs)

### `manifest_json`  (happy; EXTRACTED)
- [happy] manifest_json() returns a JSON string containing the literal text '"browser"'  (crates/vox-plugin-browser/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`BrowserPluginHost::id`** — only: _BrowserPluginHost.id() returns RString with value 'browser'_
- **`manifest_json`** — only: _manifest_json() returns a JSON string containing the literal text '"browser"'_
