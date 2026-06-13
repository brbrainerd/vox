# Semantic Behavior Map — `voxup`

Synthesized from 11 extracted Behavior claims (deduped to distinct behaviors across 3 symbols in `crates/voxup/src/install.rs` and `crates/voxup/src/manifest.rs`).

The single-binary establishment surface is the strongest-covered part of the crate: `establish_single_binary()` is proven on the happy path, a real error path (no binary found), and a genuine invariant (Unix inode identity / content sync), and `is_real_binary()` has both happy and edge (missing-path) coverage. The manifest deserializer is the weakest: `WorkspaceToolchain::parse()` is proven only on well-formed minimal YAML, with no rejection test for the failure mode its `Result` signature advertises.

## `is_real_binary()` — `install.rs`

Proven behaviors:
- Rejects files smaller than `MIN_REAL_BINARY_BYTES` (64 KiB) — e.g. a tiny shell/batch stub (happy).
- Accepts files `>= MIN_REAL_BINARY_BYTES` as real binaries (happy).
- Returns `false` for non-existent paths (edge).

Error path: n/a (returns `bool`, no `Result`). Edge/invariant: yes (missing-path edge). **Adequately covered** for its contract — the only thing untested is the directory-not-file branch (`m.is_file()`), a minor edge.

## `establish_single_binary()` — `install.rs`

Proven behaviors:
- Seeds canonical path as a real binary when only the secondary path holds one; copies content from secondary to canonical (happy).
- Overwrites a stub at the canonical path with the real binary from secondary (happy).
- Errors with a message containing `"no real vox binary"` when neither path holds a real binary (error).
- Hard-links canonical and secondary to the same inode on Unix (invariant).
- Keeps canonical and secondary byte-identical / content-synchronized (invariant).

Error path: yes. Edge/invariant: yes. **Strongest-covered symbol.** Untested: the `link_or_copy` cross-volume fallback (hard-link fails → copy + drift warning) and `replace_file`'s `remove_file` failure — both real branches with no coverage.

## `WorkspaceToolchain::parse()` — `manifest.rs`

Proven behaviors:
- Deserializes the `schema` field correctly from YAML (happy).
- Deserializes an empty `versions` map from YAML (happy).

Error path: **none**. Edge/invariant: **none**. The signature is `Result<Self, serde_yaml::Error>` and `targets`/`components` are required non-`Option` fields, so malformed or field-missing YAML is a real, advertised failure mode that is entirely unproven.

## Semantic gaps

Symbols proven only on the happy path whose contract has an obvious failure/empty/conflict mode:

1. **`WorkspaceToolchain::parse()` — validator/deserializer with no rejection test (most actionable).** Returns `serde_yaml::Error` and has required fields (`schema`, `versions`, `targets`, `components`), yet every test feeds well-formed YAML. There is no proof that malformed YAML, a missing required field, or a wrong-typed field is actually rejected. Add a test asserting `parse()` returns `Err` on garbage input and on YAML missing `targets`/`components`.

2. **`establish_single_binary()` copy-fallback drift path (`link_or_copy`).** The hard-link-fails → copy-and-warn branch (cross-volume install, the exact scenario the doc comment and `vox doctor` SSOT check care about) is never exercised. The inode invariant is only proven for the success case; the documented "the two may drift" degradation is unverified.

3. **`establish_single_binary()` overwrite/removal failure (`replace_file`).** The `remove_file` step when the canonical path already exists (e.g. permission-denied / locked file on Windows) has no failure test; only successful overwrite of a stub is proven.