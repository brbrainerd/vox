# Semantic Behavior Map — `vox-plugin-publication`

Deterministically synthesized from 2 distinct proven-behavior claims (of 2 extracted) across 2 symbols. 0 symbols have an explicit error-path proof; **2 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `PublicationPlugin::id()`  (happy; EXTRACTED)
- [happy] PublicationPlugin's id() method returns the string "publication"  (crates/vox-plugin-publication/src/lib.rs)

### `manifest_json()`  (happy; EXTRACTED)
- [happy] manifest_json() returns a JSON string containing the publication ID "publication"  (crates/vox-plugin-publication/src/lib.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`PublicationPlugin::id()`** — only: _PublicationPlugin's id() method returns the string "publication"_
- **`manifest_json()`** — only: _manifest_json() returns a JSON string containing the publication ID "publication"_
