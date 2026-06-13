## Semantic Behavior Map: `vox-capability-registry`

All 16 extracted claims originate from a single suite, `crates/vox-capability-registry/tests/registry_test.rs`. After deduplication they describe **6 distinct symbols**. The proven surface is the *success* surface: registry contents, exposure invariants, parameter-schema shape, and OpenAI-function shape are all confirmed for well-formed inputs. There is **no error-path, missing-key, malformed-input, or empty-result proof anywhere in the suite** — every behavior assumes the happy case or asserts a shape invariant on valid data. The sharpest gaps are the two name-keyed lookup functions and the YAML-backed CLI-path filter, all of which have obvious failure/empty/conflict modes that go untested.

### `default_registry()`
- Exposes at least one Mens-chat capability. *(happy)*
- Exposes `mcp.vox_oratio_transcribe`. *(happy)*
- Exposes `mcp.vox_oratio_status`. *(happy)*
- Error path: none. Edge/invariant: none (no no-duplicate or ordering invariant proven).

### `mens_chat_capabilities()`
- Yields only capabilities with `PopuliExposure::Auto`. *(invariant)*
- Error path: none. Edge: empty-result case and count-completeness unproven.

### `PopuliExposure::Auto`
- Auto-exposed capabilities have a defined `mcp_tool` name. *(invariant)*
- Error path: n/a (enum-level invariant). Reasonably covered for its contract.

### `mens_chat_parameters()`
- For `mcp.vox_oratio_transcribe`: returns an object schema, includes a `path` property, and marks `path` as required. *(happy + invariant)*
- For `mcp.vox_oratio_status`: returns an (empty) object schema. *(happy)*
- Error path: **none** — no test for an unknown/nonexistent tool name. This is a name-keyed lookup with an obvious missing-key mode.

### `active_vox_cli_paths_from_command_registry_yaml()`
- Filters to vox-cli commands with active status only. *(happy)*
- Excludes retired vox-cli commands. *(edge)*
- Excludes non-vox-cli surfaces even when active. *(edge)*
- Error path: **none** — empty/malformed YAML and duplicate-path behavior unproven.

### `capability_to_openai_function()`
- Output `type` is `"function"`. *(happy)*
- `function.name` populated from the provided tool name. *(happy)*
- `function.description` populated from the provided description. *(happy)*
- Preserves the parameters object in `function.parameters`. *(invariant)*
- Error path: **none** — no proof for missing/empty description or malformed parameters.

## Semantic gaps

The whole crate is proven happy-path-only; these are the symbols whose contract clearly has a failure/empty/conflict mode that is currently untested, ordered by actionability:

1. **`mens_chat_parameters()` — unknown-name lookup (most actionable).** A name-keyed accessor proven only for two known names. Its behavior on a nonexistent capability (return `None`, empty schema, or error?) is undefined by the tests. Add a rejection/None test.
2. **`capability_to_openai_function()` — degenerate inputs.** A shape-mapper proven only on valid input. No test pins down behavior with an empty/missing description or a malformed/absent parameters object — the integrity of the emitted tool spec depends on this.
3. **`active_vox_cli_paths_from_command_registry_yaml()` — parse/empty/conflict path.** This is the riskiest surface because it reads external YAML. Filtering is well covered (active-only, retired-excluded, surface-excluded), but empty registry, malformed YAML, and duplicate command paths are unproven.
4. **`default_registry()` — integrity invariant.** Contents are spot-checked by membership only; no proof that the registry contains no duplicate capability IDs (a real conflict mode for a registry) or a stable ordering.
5. **`mens_chat_capabilities()` — completeness/empty.** Proven to be a correct *subset* (Auto-only) but not complete (every Auto capability is included) and untested when no Auto capabilities exist.