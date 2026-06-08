# Semantic Behavior Map — `vox-plugin-host`

Deterministically synthesized from 51 distinct proven-behavior claims (of 51 extracted) across 18 symbols. 0 symbols have an explicit error-path proof; **15 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `parse_skill_md`  (happy; EXTRACTED)
- [happy] parse_skill_md extracts manifest id from legacy skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest version from legacy skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest category from legacy skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest tools list from legacy skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest tags from legacy skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts SkillPermission::ReadFiles from legacy skill markdown permissions  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts SkillPermission::ShellExec from legacy skill markdown permissions  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts vox-id metadata into manifest id field from agentskills markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts name field from agentskills markdown metadata block  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest version from agentskills skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest category from agentskills skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- [happy] parse_skill_md extracts manifest tools from agentskills skill markdown  (crates/vox-plugin-host/src/skill_parser.rs)
- … +4 more claims

### `Registry`  (edge, happy, invariant; EXTRACTED)
- [happy] discover() successfully finds skill plugins in a directory structure and returns a registry containing them  (crates/vox-plugin-host/tests/discover_basics.rs)
- [edge] discover() returns an empty registry when root directory does not exist  (crates/vox-plugin-host/tests/discover_basics.rs)
- [edge] discover() returns empty registry when scanning directories without manifest files  (crates/vox-plugin-host/tests/discover_basics.rs)
- [happy] discover() detects code plugins after manifest and dylib are staged in a directory  (crates/vox-plugin-host/tests/load_noop_code.rs)
- [happy] discover() finds skill plugins after fixture files are copied to temporary directory  (crates/vox-plugin-host/tests/load_noop_skill.rs)
- [invariant] newly created Registry has empty list_ids() result  (crates/vox-plugin-host/tests/registry_basics.rs)
- [invariant] has() returns false for arbitrary ids in empty registry  (crates/vox-plugin-host/tests/registry_basics.rs)

### `Plugin`  (happy; EXTRACTED)
- [happy] discovered skill plugins are accessible via registry.skills.lookup() with correct exposed_tools list  (crates/vox-plugin-host/tests/discover_basics.rs)
- [happy] discovered skill plugins have body content loaded from their manifest skill-md file  (crates/vox-plugin-host/tests/discover_basics.rs)
- [happy] loaded code plugin trait object has correct id() and supports shutdown()  (crates/vox-plugin-host/tests/load_noop_code.rs)
- [happy] discovered skill plugin has correct exposed_tools, body content from markdown file, and format_version  (crates/vox-plugin-host/tests/load_noop_skill.rs)
- [happy] telemetry::discovered() logs a plugin.discovered event containing the plugin id  (crates/vox-plugin-host/tests/telemetry.rs)

### `SkillRegistry::install_bundle`  (happy; EXTRACTED)
- [happy] install_bundle returns result with correct skill id  (crates/vox-plugin-host/src/skill_registry.rs)
- [happy] install_bundle marks first install as not already_installed  (crates/vox-plugin-host/src/skill_registry.rs)
- [happy] install_bundle detects and flags duplicate installation of same version  (crates/vox-plugin-host/src/skill_registry.rs)

### `Plugin::lookup`  (happy; EXTRACTED)
- [happy] lookup returns LoadedSkill with correct exposed_tools  (crates/vox-plugin-host/tests/discover_basics.rs)
- [happy] lookup returns LoadedSkill with correct body content  (crates/vox-plugin-host/tests/discover_basics.rs)

### `SkillRegistry`  (happy; EXTRACTED)
- [happy] install() adds a skill to the registry and lookup() returns it by id  (crates/vox-plugin-host/tests/registry_basics.rs)
- [happy] installed skills appear in list_ids()  (crates/vox-plugin-host/tests/registry_basics.rs)

### `SkillRegistry::search`  (happy; EXTRACTED)
- [happy] search returns non-empty results for matching skill name  (crates/vox-plugin-host/src/skill_registry.rs)
- [happy] search returns correct skill id in results matching name query  (crates/vox-plugin-host/src/skill_registry.rs)

### `VoxSkillBundle::content_hash`  (happy, invariant; EXTRACTED)
- [invariant] content_hash produces identical output when called multiple times on the same bundle  (crates/vox-plugin-host/src/skill_bundle.rs)
- [happy] content_hash returns a 64-character string (SHA3-256 hex format)  (crates/vox-plugin-host/src/skill_bundle.rs)

### `VoxSkillBundle::from_json`  (happy; EXTRACTED)
- [happy] from_json correctly reconstructs manifest id from serialized JSON  (crates/vox-plugin-host/src/skill_bundle.rs)
- [happy] from_json correctly reconstructs manifest version from serialized JSON  (crates/vox-plugin-host/src/skill_bundle.rs)

### `discover()`  (edge; EXTRACTED)
- [edge] discover() succeeds without error when given a non-existent root directory path  (crates/vox-plugin-host/tests/discover_basics.rs)
- [edge] discover() skips directories that do not contain a Plugin.toml manifest file  (crates/vox-plugin-host/tests/discover_basics.rs)

### `Loader`  (happy; EXTRACTED)
- [happy] Loader::load() successfully loads a code plugin dylib and creates a trait object  (crates/vox-plugin-host/tests/load_noop_code.rs)

### `SkillRegistry::get`  (happy; EXTRACTED)
- [happy] get returns None after skill is uninstalled  (crates/vox-plugin-host/src/skill_registry.rs)

### `SkillRegistry::get_backend`  (happy; EXTRACTED)
- [happy] get_backend returns None for newly created registry  (crates/vox-plugin-host/src/skill_registry.rs)

### `SkillRegistry::list`  (happy; EXTRACTED)
- [happy] list returns the installed skill after install_bundle  (crates/vox-plugin-host/src/skill_registry.rs)

### `SkillRegistry::list_ids`  (happy; EXTRACTED)
- [happy] list_ids returns installed skill ids after install  (crates/vox-plugin-host/src/skill_registry.rs)

### `SkillRegistry::lookup`  (happy; EXTRACTED)
- [happy] lookup returns LoadedSkill with correct plugin_id  (crates/vox-plugin-host/src/skill_registry.rs)

### `SkillRegistry::uninstall`  (happy; EXTRACTED)
- [happy] uninstall returns result with was_installed=true for installed skill  (crates/vox-plugin-host/src/skill_registry.rs)

### `discover`  (happy; EXTRACTED)
- [happy] discover finds skill plugin in directory with Plugin.toml manifest  (crates/vox-plugin-host/tests/discover_basics.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`Loader`** — only: _Loader::load() successfully loads a code plugin dylib and creates a trait object_
- **`Plugin`** — only: _discovered skill plugins are accessible via registry.skills.lookup() with correct exposed_tools list_
- **`Plugin::lookup`** — only: _lookup returns LoadedSkill with correct exposed_tools_
- **`SkillRegistry`** — only: _install() adds a skill to the registry and lookup() returns it by id_
- **`SkillRegistry::get`** — only: _get returns None after skill is uninstalled_
- **`SkillRegistry::get_backend`** — only: _get_backend returns None for newly created registry_
- **`SkillRegistry::install_bundle`** — only: _install_bundle returns result with correct skill id_
- **`SkillRegistry::list`** — only: _list returns the installed skill after install_bundle_
- **`SkillRegistry::list_ids`** — only: _list_ids returns installed skill ids after install_
- **`SkillRegistry::lookup`** — only: _lookup returns LoadedSkill with correct plugin_id_
- **`SkillRegistry::search`** — only: _search returns non-empty results for matching skill name_
- **`SkillRegistry::uninstall`** — only: _uninstall returns result with was_installed=true for installed skill_
- **`VoxSkillBundle::from_json`** — only: _from_json correctly reconstructs manifest id from serialized JSON_
- **`discover`** — only: _discover finds skill plugin in directory with Plugin.toml manifest_
- **`parse_skill_md`** — only: _parse_skill_md extracts manifest id from legacy skill markdown_
