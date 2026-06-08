# Semantic Behavior Map — `vox-plugin-sdk`

Synthesized from 4 extracted Behavior claims (deduped to 4 distinct, grouped across 2 symbols). This crate is a thin authoring surface over the stable plugin ABI: it re-exports `vox_plugin_api`/`abi_stable`, provides the `wrap()` erasure helper, and provides the `declare_plugin!` macro that emits dylib export glue. The proven surface is narrow — only the runtime helper `wrap` and a manifest-parsing helper are tested; the macro that is the crate's stated reason for existing is entirely unproven.

## `wrap`

Erases a concrete `VoxPlugin` into the host-facing stable-ABI trait object (`VoxPlugin_TO::from_value(plugin, TD_Opaque)`).

Proven behaviors (happy path):
- Erases a concrete `VoxPlugin` while preserving `id()` calls through the trait object (`"test-plugin"` round-trips).
- Erases a concrete `VoxPlugin` while preserving `shutdown()` calls returning `Ok`.

Coverage: happy-path only. No error-path proof, no edge/invariant proof.

## `manifest_id_version_json()`

Parses a plugin TOML manifest and projects `id` + `version` into JSON.

Proven behaviors:
- Happy: parses a valid TOML manifest and extracts `id` and `version` into JSON.
- Edge: returns fallback JSON with empty `id` and version `0.0.0` when given invalid TOML.

Coverage: happy path + one edge (malformed-input fallback). The fallback proves the fully-invalid case; the partial/missing-field case is not covered.

## Semantic gaps

Symbols whose contract has a clear failure/empty/conflict mode but are proven only on the happy path (or not at all):

1. **`declare_plugin!` macro — completely unproven (highest priority).** This is the SDK's core deliverable: it emits `root_module`/`manifest_json`/`init`, stamps `VOX_PLUGIN_ABI_VERSION` (not a hard-coded number), and coerces `init` to a non-capturing `fn` pointer. The headline invariants — "byte-identical exports vs hand-written glue" and "ABI version is stamped from the constant" — are asserted only in doc comments, with no compile/expansion/trybuild test. A macro-expansion or trybuild test (including the documented "must declare `abi_stable` as a direct dependency" failure mode and the no-captures rejection) is the most actionable missing coverage.

2. **`wrap` — no error-path proof.** `shutdown()` returns `RResult<(), RBoxError>`, yet only the `ROk` branch is exercised. Add a `TestPlugin` variant whose `shutdown()` returns `RErr` to prove the error value survives trait-object erasure intact.

3. **`manifest_id_version_json()` — missing partial-input edge.** Fallback is proven only for wholly-invalid TOML. The realistic conflict mode — valid TOML present but `id` or `version` field absent — is untested, so per-field defaulting behavior is unverified.