# Semantic Behavior Map — `vox-skills`

Deterministically synthesized from 33 distinct proven-behavior claims (of 33 extracted) across 21 symbols. 3 symbols have an explicit error-path proof; **17 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `ApprovalGuard::check`  (error, happy; EXTRACTED)
- [happy] ApprovalGuard::check returns Ok(()) when trust level is Trusted, regardless of the approved flag  (crates/vox-skills/src/sandbox/policy.rs)
- [error] ApprovalGuard::check returns Err when trust level is Untrusted, regardless of the approved flag  (crates/vox-skills/src/sandbox/policy.rs)
- [happy] Returns Ok(()) when Community-trust skill has approval=true  (crates/vox-skills/src/sandbox/policy.rs)

### `SkillRegistry::install_bundle()`  (happy; EXTRACTED)
- [happy] install_bundle() returns a result with id matching the skill  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] install_bundle() sets already_installed to false on first install  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] install_bundle() sets already_installed to true for duplicate version  (crates/vox-skills/tests/skill_registry_tests.rs)

### `SkillRegistry::list`  (happy; EXTRACTED)
- [happy] Returns all installed skills when called with None filter  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] Filters returned skills by the specified SkillCategory when provided  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] Returns empty list after all installed skills are uninstalled  (crates/vox-skills/tests/skill_registry_tests.rs)

### `resolve_policy`  (edge, happy; EXTRACTED)
- [happy] Returns SandboxPolicy::Permissive for trusted Document skills  (crates/vox-skills/src/sandbox/policy.rs)
- [edge] Returns SandboxPolicy::Container for Shell skills even when Trusted  (crates/vox-skills/src/sandbox/policy.rs)
- [happy] Returns SandboxPolicy::Container for Community-trust Tool skills  (crates/vox-skills/src/sandbox/policy.rs)

### `ArsSkill`  (happy; EXTRACTED)
- [happy] ArsSkill can be serialized to JSON and deserialized back with field integrity  (crates/vox-skills/tests/domain_test.rs)
- [happy] ArsSkill.description field can be None  (crates/vox-skills/tests/domain_test.rs)

### `SandboxPolicy`  (happy; EXTRACTED)
- [happy] resolve_policy(Document, Community) returns SandboxPolicy::Container  (crates/vox-skills/tests/openclaw_fallback_test.rs)
- [happy] resolve_policy(Tool, Trusted) returns SandboxPolicy::Permissive  (crates/vox-skills/tests/openclaw_fallback_test.rs)

### `SkillRegistry::search`  (happy; EXTRACTED)
- [happy] Finds skills by substring match in their id field  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] Returns empty result when no skills match the search query  (crates/vox-skills/tests/skill_registry_tests.rs)

### `SkillRegistry::uninstall`  (happy; EXTRACTED)
- [happy] Removes installed skill from registry and returns was_installed=true  (crates/vox-skills/tests/skill_registry_tests.rs)
- [happy] Returns was_installed=false when uninstalling a skill that was never installed  (crates/vox-skills/tests/skill_registry_tests.rs)

### `ArsRuntime::execute_skill()`  (error; EXTRACTED)
- [error] execute_skill() denies execution when skill requests unknown secrets  (crates/vox-skills/tests/openclaw_fallback_test.rs)

### `ArsRuntimeError`  (error; EXTRACTED)
- [error] execute_skill() returns InvalidRun error for unauthorized secrets  (crates/vox-skills/tests/openclaw_fallback_test.rs)

### `HookRegistry::count()`  (happy; EXTRACTED)
- [happy] count() decrements after deregister() is called  (crates/vox-skills/src/hooks.rs)

### `HookRegistry::deregister()`  (happy; EXTRACTED)
- [happy] deregister() removes a registered hook and returns true  (crates/vox-skills/src/hooks.rs)

### `PluginKind`  (happy; EXTRACTED)
- [happy] SkillPlugin defaults to PluginKind::Skill  (crates/vox-skills/src/plugin.rs)

### `PluginManager::all_tool_ids()`  (happy; EXTRACTED)
- [happy] all_tool_ids() aggregates tool_ids from all loaded plugins  (crates/vox-skills/src/plugin.rs)

### `PluginManager::is_loaded()`  (happy; EXTRACTED)
- [happy] is_loaded() returns false after unload()  (crates/vox-skills/src/plugin.rs)

### `PluginManager::list()`  (happy; EXTRACTED)
- [happy] list() returns all loaded plugins with correct metadata  (crates/vox-skills/src/plugin.rs)

### `PluginManager::unload()`  (happy; EXTRACTED)
- [happy] unload() removes a loaded plugin  (crates/vox-skills/src/plugin.rs)

### `SkillKind`  (happy; EXTRACTED)
- [happy] SkillKind is preserved through JSON serialization round-trip  (crates/vox-skills/tests/domain_test.rs)

### `SkillRegistry::get`  (happy; EXTRACTED)
- [happy] Returns None when querying a skill that was never installed  (crates/vox-skills/tests/skill_registry_tests.rs)

### `SkillRegistry::get()`  (happy; EXTRACTED)
- [happy] get() returns installed skill manifest after install_bundle()  (crates/vox-skills/tests/skill_registry_tests.rs)

### `install_builtins()`  (happy; EXTRACTED)
- [happy] install_builtins() returns 0 (no builtins to install)  (crates/vox-skills/src/builtins.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ArsSkill`** — only: _ArsSkill can be serialized to JSON and deserialized back with field integrity_
- **`HookRegistry::count()`** — only: _count() decrements after deregister() is called_
- **`HookRegistry::deregister()`** — only: _deregister() removes a registered hook and returns true_
- **`PluginKind`** — only: _SkillPlugin defaults to PluginKind::Skill_
- **`PluginManager::all_tool_ids()`** — only: _all_tool_ids() aggregates tool_ids from all loaded plugins_
- **`PluginManager::is_loaded()`** — only: _is_loaded() returns false after unload()_
- **`PluginManager::list()`** — only: _list() returns all loaded plugins with correct metadata_
- **`PluginManager::unload()`** — only: _unload() removes a loaded plugin_
- **`SandboxPolicy`** — only: _resolve_policy(Document, Community) returns SandboxPolicy::Container_
- **`SkillKind`** — only: _SkillKind is preserved through JSON serialization round-trip_
- **`SkillRegistry::get`** — only: _Returns None when querying a skill that was never installed_
- **`SkillRegistry::get()`** — only: _get() returns installed skill manifest after install_bundle()_
- **`SkillRegistry::install_bundle()`** — only: _install_bundle() returns a result with id matching the skill_
- **`SkillRegistry::list`** — only: _Returns all installed skills when called with None filter_
- **`SkillRegistry::search`** — only: _Finds skills by substring match in their id field_
- **`SkillRegistry::uninstall`** — only: _Removes installed skill from registry and returns was_installed=true_
- **`install_builtins()`** — only: _install_builtins() returns 0 (no builtins to install)_
