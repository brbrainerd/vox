---
title: "Plugin Manifest (Plugin.toml)"
description: "Schema for the Plugin.toml file every Vox plugin ships."
category: "Language Reference"
status: "current"
training_eligible: true
---

# Plugin Manifest (Plugin.toml)

Every Vox plugin ships a `Plugin.toml` in its install directory describing what the plugin is, what it provides, and what the host needs to know to load it. This page is the schema reference. For the design rationale see [Plugin System Redesign (2026)](../architecture/plugin-system-redesign-2026.md).

## Common header

```toml
[plugin]
id = "<short-hyphenated-id>"
name = "<human-readable name>"
version = "<semver>"
description = "<one-line description>"
authors = ["..."]
license = "<SPDX identifier>"
homepage = "<url>"

[plugin.host]
min-vox-version = "<semver>"
```

## Payload kinds

The `[plugin.payload]` block discriminates on `kind`:

- `code` — ships a `cdylib` per OS/arch and provides one or more code extension points.
- `skill` — ships a `SKILL.md` and registers MCP tools.
- `composite` — ships both.

### Code payload

```toml
[plugin.payload]
kind = "code"
abi-version = 1

[plugin.payload.provides]
extension-points = ["MlBackend"]

[plugin.payload.requires]
os = ["windows", "linux"]
arch = ["x86_64"]
native-libs = [
    { name = "cudart", min-version = "12.0" },
    { name = "cublas" },
]

[plugin.payload.artifacts]
"windows-x86_64" = "vox_plugin_<id>.dll"
"linux-x86_64"   = "libvox_plugin_<id>.so"
"macos-aarch64"  = "libvox_plugin_<id>.dylib"
```

### Skill payload

```toml
[plugin.payload]
kind = "skill"
format-version = 1
skill-md = "<filename>.skill.md"

[plugin.payload.tools]
exposes = ["vox_tool_one", "vox_tool_two"]
```

### Composite payload

```toml
[plugin.payload]
kind = "composite"

[plugin.payload.code]
abi-version = 1
provides.extension-points = ["MeshDriver"]
artifacts."linux-x86_64" = "libvox_plugin_<id>.so"

[plugin.payload.skill]
format-version = 1
skill-md = "<filename>.skill.md"
tools.exposes = ["vox_tool_one"]
```

## AgentSkills Compliance

Vox skill plugins implement the [AgentSkills open standard](https://agentskills.io/specification), which allows them to be loaded by Claude Code, OpenAI Codex CLI, Gemini CLI, GitHub Copilot, Cursor, JetBrains, and any other tool that follows the spec.

### SKILL.md frontmatter schema

The AgentSkills spec requires two top-level fields: `name` (lowercase + hyphens, matches the directory short-id) and `description`. All Vox-specific fields live under a `metadata` block prefixed with `vox-`:

```toml
---
name = "skill-compiler"
description = "Compiles Vox source files and runs cargo check/build for the workspace."

[metadata]
"vox-id" = "vox.compiler"
"vox-version" = "0.1.0"
"vox-author" = "vox-team"
"vox-category" = "compiler"
"vox-tools" = ["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
"vox-tags" = ["compile", "build", "cargo"]
"vox-permissions" = ["read_files", "shell_exec"]
---
```

**Field mapping:**

| AgentSkills field | Vox usage |
|---|---|
| `name` | Spec-required; lowercase-hyphen id matching the plugin directory short-name (e.g. `skill-compiler`) |
| `description` | Spec-required; one-paragraph description shown in tool marketplaces |
| `metadata.vox-id` | Internal dot-notation id used by the Vox orchestrator bridge (e.g. `vox.compiler`) |
| `metadata.vox-version` | Semver version string |
| `metadata.vox-author` | Author or publisher identifier |
| `metadata.vox-category` | Primary skill category (see `SkillCategory` enum) |
| `metadata.vox-tools` | MCP tool IDs this skill exposes |
| `metadata.vox-tags` | Search tags |
| `metadata.vox-permissions` | Permissions required at install time |

### Parser compatibility

The `vox-skills` parser reads `metadata.vox-*` fields first, then falls back to the legacy top-level field names (`id`, `version`, `author`, etc.) for backward compatibility. This means older SKILL.md files continue to work without migration.

If neither `metadata.vox-id` nor the legacy `id` field is present, the parser derives the id from the `name` field.

### CI enforcement

The `vox ci agentskills-compliance` guard runs on every PR and enforces the AgentSkills frontmatter contract for all `*.skill.md` files under `crates/vox-plugin-*`:

- Frontmatter block (`---` … `---`) must be present.
- `name` must be present, match `^[a-z0-9][a-z0-9-]{0,63}$`, and equal the crate directory short-name (directory name with the `vox-plugin-` prefix stripped).
- `description` must be present and ≤ 1024 chars.

Run locally with:

```bash
vox ci agentskills-compliance
```

## Validation

The manifest is parsed at host startup and validated against this schema. Failures are reported by `vox plugin doctor` with the offending field path.
