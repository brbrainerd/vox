# Crate API: vox-skills

## Module: `vox-skills\src\builtins.rs`

Built-in skills that are always available in the Vox skill registry.

These are embedded at compile time via `include_str!` so the registry
works even without a filesystem. They are installed on first startup.


### `fn install_builtins`

Install all built-in skills into the registry if they are not already present.


### `fn builtin_bundles`

Return all built-in skill bundles without installing them.


## Module: `vox-skills\src\bundle.rs`

Skill bundle format — an in-memory or on-disk representation of a VoxSkill.

A `VoxSkillBundle` contains:
- The parsed `SkillManifest`
- Raw SKILL.md content (instructions for the runtime)
- Optional inline tool implementations (JSON arrays of MCP tool specs)
- Optional asset bytes


### `struct VoxSkillBundle`

A fully-loaded skill bundle ready for installation.


## Module: `vox-skills\src\clawhub.rs`

ClawHub bridge — HTTP client for the OpenClaw skill registry.

Enabled with feature `clawhub`. Without it, all methods return an error.


### `struct ClawHubResult`

A search result from the ClawHub registry.


### `struct ClawHubClient`

HTTP client for the ClawHub / OpenClaw skill marketplace.


## Module: `vox-skills\src\hooks.rs`

Hook system — event-driven hooks for the skill lifecycle.


### `enum HookEvent`

Events the hook system fires.


### `struct HookRegistry`

Registry of named hook functions keyed by event.


## Module: `vox-skills\src\lib.rs`

# vox-skills — Skill Marketplace and Plugin Architecture

Provides a typed skill registry, skill bundle format parsing,
plugin lifecycle management, and an optional ClawHub HTTP bridge.


### `enum SkillError`

Errors from the skill system.


## Module: `vox-skills\src\manifest.rs`

Skill manifest types — the metadata schema for a VoxSkill.


### `struct SkillManifest`

A complete skill manifest (equivalent to OpenClaw's skill.json / SKILL.md frontmatter).


### `enum SkillCategory`

Skill category for marketplace browsing and filtering.


### `enum SkillPermission`

Permissions a skill may require at install time.


## Module: `vox-skills\src\parser.rs`

SKILL.md format parser — extracts frontmatter + body from a SKILL.md file.

SKILL.md format:
```markdown
---
id: "vox.compiler"
name: "Compiler"
version: "0.1.0"
author: "vox"
description: "Compiles Vox programs"
category: "compiler"
tools:
- "vox_compile"
- "vox_check"
---

# Compiler Skill

... instructions ...
```


### `fn parse_skill_md`

Parse a full SKILL.md file into a `VoxSkillBundle`.


## Module: `vox-skills\src\plugin.rs`

Plugin system — Plugin trait, PluginManager, and plugin kinds.

A Plugin is a runtime-activatable unit: it can be a skill wrapper,
an external tool adapter, or a built-in Vox capability. The PluginManager
owns loading, unloading, and dispatching.


### `enum PluginKind`

Plugin kind discriminant.


### `struct PluginMeta`

Metadata about a loaded plugin.


### `trait Plugin`

Plugin lifecycle trait.


### `struct SkillPlugin`

A skill-backed plugin implementation.


### `struct PluginManager`

Manager for all loaded plugins.


## Module: `vox-skills\src\registry.rs`

Skill registry — install, uninstall, search, and list skills.


### `struct InstallResult`

Result of a skill installation.


### `struct UninstallResult`

Result of a skill uninstallation.


### `struct SkillRegistry`

In-memory skill registry with interior mutability for db field.


