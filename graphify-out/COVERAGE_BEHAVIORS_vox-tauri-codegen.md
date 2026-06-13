## Semantic behavior map: `vox-tauri-codegen`

vox-tauri-codegen emits Tauri v2 packaging hints (`tauri-packaging/tauri.conf.json` + `README.md`) and, when a contracts repo root is supplied, a `runtime-capabilities.projection.json` projected from `contracts/capability/runtime-capabilities.v1.yaml`. The 10 extracted claims collapse to ~7 distinct behaviors over 3 symbols, all sourced from `crates/vox-tauri-codegen/src/lib.rs`. Coverage is shaped almost entirely as happy-path generation plus one genuine filter edge case; the failure, fallback, and empty/conflict modes that the code visibly contains are entirely unproven.

### `emit_tauri_packaging_hints()`
Proven behaviors (happy):
- Creates `tauri.conf.json` carrying `identifier` and `frontend_dist_relative` from params.
- Includes window label `"main"` in the output.
- Does NOT create `runtime-capabilities.projection.json` when `contracts_repo_root` is `None`.
- DOES create the projection when a contracts dir with `runtime-capabilities.v1.yaml` exists.
- Filters capabilities to `required_capabilities` ids when `required` is provided.

Error path: none. Edge/invariant: none direct (filtering edge proven via the projection helper).
Note: this is a mutator with `create_dir_all`/`write` calls (lines 232,235,264) and a `?`-propagated projection call (line 238) — none of those error paths are tested.

### `emit_runtime_capabilities_projection()` (private)
Proven behaviors:
- Preserves `schema_version` from the source YAML in the projection (happy; value 1 observed).
- Populates `tauri_permission_allow_list` from capability permissions (happy).
- Generates an allow-list containing only permissions from required ids (happy/filter).
- Excludes unrequired capabilities even when present in source YAML (edge — the one true edge proof: `caps.len()==1`, `notification:default` absent).

Error path: none. Edge/invariant beyond filtering: none.
Unproven despite existing in code: `fs::read_to_string` failure (165), `serde_yaml::from_str` parse failure (167), `schema_version.unwrap_or(1)` fallback (192), empty-match required_ids → empty projection, cross-row permission dedup via the `BTreeSet`.

### `find_contracts_repo_root()`
Proven behaviors (happy):
- Walks up the directory tree to the parent containing `contracts/capability/runtime-capabilities.v1.yaml`.

Error path / negative: none — the `None`-on-not-found branch (lines 103-107) is untested.

### Untested public surface
`tauri_desktop_config_value()`, `serialize_tauri_desktop_config()`, and `write_tauri_desktop_config()` have no direct tests. `write_tauri_desktop_config` has an explicit overwrite + mkdir-parent contract (lines 86-93) that is entirely unproven.

## Semantic gaps
The most actionable holes — every one is a symbol whose contract has a real failure/empty/conflict mode but is proven only on the happy path:

1. **Parse/IO rejection in the projection emitter** — `emit_runtime_capabilities_projection` parses external YAML (`serde_yaml::from_str`, line 167) and reads a file (line 165). Malformed or unreadable contract YAML is a realistic drift mode and should fail with context, not mis-project. No rejection test exists. (Highest priority: this is the integrity surface that gates Tauri permission allow-lists.)
2. **`schema_version` fallback invariant** — `unwrap_or(1)` (line 192) means an absent `schema_version` silently becomes `1`. Untested; the only YAML without a version (`find_contracts_repo_root` test) never produces a projection.
3. **Empty/no-match required filter** — when `required_ids` matches nothing, the projection writes empty `capabilities[]`/`allow_list`. This empty-output invariant is unproven and is exactly the kind of silent-empty-allow-list that weakens a security surface.
4. **Mutator write-failure paths** — `emit_tauri_packaging_hints` and `write_tauri_desktop_config` perform `create_dir_all`/`write` with `?`; no test forces a filesystem error to confirm errors propagate with their `with_context` messages.
5. **`find_contracts_repo_root` not-found** — the `None` branch (no ancestor holds the SSOT) is never exercised; the locator is proven to find but not to correctly fail to find.
6. **Cross-capability permission dedup** — the `BTreeSet` dedups permissions across rows; only single-row collection is observed. A duplicate-permission contract would not surface a regression today.