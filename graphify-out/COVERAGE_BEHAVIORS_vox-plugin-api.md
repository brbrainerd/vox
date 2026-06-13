# Semantic Behavior Map — vox-plugin-api

Synthesized from 14 extracted Behavior claims (deduped to 5 distinct symbols). One paragraph summary: the ABI-compatibility predicate is the best-covered symbol, carrying both happy-path (in-range) and error-path (out-of-range, both sides) proof. `LogLevel` carries a serde round-trip invariant across every variant. The manifest-parsing surface — `PluginManifest`, `PluginPayload`, and `PluginPayload::Code` — is proven exclusively on the happy path against a single valid code manifest, with no rejection, missing-field, empty, or conflict coverage. Because this surface is the plugin trust/loading boundary, its happy-path-only status is the dominant semantic gap.

## abi_compatible()
File: `crates/vox-plugin-api/src/lib.rs` (signature `abi_compatible(plugin_abi: u32) -> bool`)

Distinct proven behaviors:
- Returns `true` at the minimum supported ABI version (range lower endpoint). [happy]
- Returns `true` at the current ABI version (range upper endpoint). [happy]
- Returns `false` for versions below the minimum supported version. [error]
- Returns `false` for versions above the current ABI version. [error]

Error path: yes (both below-min and above-current). Edge/invariant: endpoints are tested as the true range boundaries (effectively boundary-equality edges). Well-covered.

## LogLevel
File: `crates/vox-plugin-api/tests/errors_basic.rs`

Distinct proven behaviors:
- Serde round-trip through `serde_json` without data loss for every variant: `Trace`, `Debug`, `Info`, `Warn`, `Error`. [invariant]

Error path: none (no malformed-input deserialization test). Edge/invariant: strong — full-variant round-trip invariant. Adequate for an enum; the only missing piece is a deserialize-rejection test for an unknown level string.

## PluginManifest
File: `crates/vox-plugin-api/tests/manifest_parsing.rs`

Distinct proven behaviors:
- Parses TOML and reads `plugin.id` correctly from a valid code-plugin manifest. [happy]

Error path: none. Edge/invariant: none. This is a parser/validator with no rejection coverage.

## PluginPayload
File: `crates/vox-plugin-api/tests/manifest_parsing.rs`

Distinct proven behaviors:
- The `Code` variant is correctly deserialized from TOML. [happy]

Error path: none. Edge/invariant: none. Only one variant exercised; no test for an unrecognized or ambiguous payload kind.

## PluginPayload::Code
File: `crates/vox-plugin-api/tests/manifest_parsing.rs`

Distinct proven behaviors:
- `abi_version` is parsed from TOML. [happy]
- `provides.extension_points` is parsed as a vector. [happy]
- `artifacts` map is parsed with platform keys. [happy]

Error path: none. Edge/invariant: none. All fields proven only when present and well-formed.

## Semantic gaps

These symbols are proven on the happy path only, yet their contracts clearly carry a failure/empty/conflict mode. Most actionable first:

1. **PluginManifest (validator with no rejection test).** The TOML parser is proven only against a valid code manifest. No proof of behavior for missing `plugin.id`, malformed TOML, or an unknown payload kind. This is the plugin-loading trust boundary — a parser that has never been shown to reject anything is the sharpest hole here.
2. **PluginPayload (only one variant exercised).** Only `Code` deserialization is proven. There is no error path for an unrecognized or ambiguous payload kind, so the enum's discrimination logic is untested against bad input.
3. **PluginPayload::Code field coverage (no edge/empty/conflict).** `abi_version`, `provides.extension_points`, and `artifacts` are each proven only when present and well-formed. No coverage for a missing `abi_version`, an empty `extension_points` vector, or an invalid/duplicate platform key in `artifacts`.
4. **LogLevel deserialize rejection (low priority).** Round-trip invariant is strong, but an unknown log-level string has no proven rejection behavior.
5. **abi_compatible() (lowest priority — effectively covered).** Has full error-path proof; endpoints double as boundary edges, so no additional gap of substance.