---
title: "Vox Skill Marketplace"
description: "Discovery roots, bundled interop skills, MCP tools, and GUI slash integration for the Vox skill ecosystem."
category: "Language Reference"
status: "current"
training_eligible: true
training_rationale: "Authoritative reference for skill discovery, installation, and agent-facing MCP tools."
---

# Vox Skill Marketplace

Vox skills are [agentskills.io](https://agentskills.io/specification)-format directories: each skill is `<root>/<name>/SKILL.md` with YAML frontmatter (`name`, `description`, optional `metadata.vox-*`). The runtime discovers them from standard interop roots, installs them into the skill registry, and exposes them through CLI, MCP, orchestrator prompts, and the GUI command palette.

## Two formats (do not conflate)

| Format | Location | Frontmatter | When to use |
|--------|----------|-------------|-------------|
| **Interop directory** | `.vox/skills/`, `.cursor/skills/`, `.agents/skills/`, `.claude/skills/`, `assets/skills/` | YAML (`name`, `description`) | Universal ecosystem skills; bundled library; user imports |
| **Plugin skill** | `crates/vox-plugin-skill-*/*.skill.md` | TOML + `vox-tools` tied to `Plugin.toml` | First-party skills that expose MCP tools via the plugin host |

Plugin skills win on id collision when installed before external discovery. External/bundled skills use `external:<id>` provenance.

## Discovery roots (precedence: high → low)

Resolved by `vox_config::paths::skill_search_roots()`:

1. `<workspace>/.vox/skills/`
2. `<workspace>/.cursor/skills/`
3. `<workspace>/.agents/skills/`
4. `<workspace>/.claude/skills/`
5. `~/.vox/skills/`, `~/.cursor/skills/`, `~/.agents/skills/`, `~/.claude/skills/`
6. `<workspace>/assets/skills/` (lowest — bundled, license-verified library)

First root wins for a given skill id. Search uses `vox skill search` / `vox_skill_search` (hybrid lexical + semantic via `vox-search`), not GUI directory walks.

## Bundled library (`assets/skills/`)

Shipped with the repo; provenance in `assets/skills/SOURCES.toml`; catalog SSOT in `crates/vox-plugin-catalog/catalog.toml` (`[[skill-bundle]]` rows).

| Upstream | License | Count |
|----------|---------|------:|
| [anthropics/skills](https://github.com/anthropics/skills) | Apache-2.0 | 12 |
| [obra/superpowers](https://github.com/obra/superpowers) | MIT | 14 |

Refresh vendored copies:

```bash
vox run scripts/vendor-skills.vox
vox run scripts/audit-skill-licenses.vox
```

**Not bundled:** Cursor built-in skills under `~/.cursor/skills-cursor/` (Cursor distribution — import locally only). Anthropic document skills (docx/pdf/pptx/xlsx) are proprietary and excluded per `SOURCES.toml`.

## Importing from Cursor

Cursor built-in skills are **not** committed to the Vox repo. Import them into your workspace interop root:

```bash
# Dry-run — lists source/target paths
vox run scripts/sync-cursor-skills.vox

# Copy into .agents/skills/<name>/
vox run scripts/sync-cursor-skills.vox -- --write

# Alternate target (higher precedence than .agents)
vox run scripts/sync-cursor-skills.vox -- --write --target .vox/skills
```

After import, run `vox skill discover` or restart the orchestrator so the registry picks up new skills.

## CLI

| Command | Purpose |
|---------|---------|
| `vox skill list` | Installed skills (registry SSOT) |
| `vox skill search <query>` | Keyword search over installed skills |
| `vox skill discover` | Re-scan discovery roots |
| `vox skill info <id>` | Manifest + body for one skill |
| `vox codex import-skill-bundle --file <bundle.json>` | Install from `VoxSkillBundle` JSON |

Registry bootstrap (`install_external_skills` in `crates/vox-cli/src/commands/extras/ars/registry.rs`) runs on daemon/MCP startup: plugin skills first, then external/bundled discovery (idempotent).

## MCP tools

| Tool | Description |
|------|-------------|
| `vox_skill_list` | List installed skills |
| `vox_skill_search` | Search installed skills by keyword |
| `vox_skill_info` | Detail for one skill by id |
| `vox_skill_use` | Load tier-2 skill body into agent context |
| `vox_skill_discover` | Re-scan discovery roots |
| `vox_skill_install` | Install from `VoxSkillBundle` JSON payload |
| `vox_skill_uninstall` | Remove an installed skill |
| `vox_skill_parse` | Preview a `SKILL.md` before installing |

Tier-1 catalog (max 64 descriptions) is injected into the orchestrator system prompt via `crates/vox-orchestrator-mcp/src/chat_tools/skill_catalog.rs`.

## GUI integration

- **Loquela slash commands:** Installed skills from `vox_skill_list` (not the CLI command catalog). Names must match `^[a-z0-9][a-z0-9-]*$` for slash expansion.
- **Command palette (`Cmd+K`):** Type `/` to filter installed skills; `@` for agents; default mode searches commands + skills.
- **Skills surface:** Browse and invoke skills from the GUI sidebar.

Implementation: `crates/vox-gui/ui/src/hooks/useInstalledSkills.ts`, `crates/vox-gui/ui/src/lib/installedSkills.ts`.

## First-party plugin skills

Tool-linked plugin skills live under `crates/vox-plugin-skill-*` and are declared in `crates/vox-plugin-catalog/catalog.toml` as `[[plugin]]` with `payload-kind = "skill"`. See `docs/src/reference/plugin-catalog.md` for the full plugin catalog.

## Related docs

- Research: `docs/src/architecture/skill-ecosystem-interop-research-2026-06-12.md`
- Plan: `docs/superpowers/plans/2026-06-16-universal-skill-bundle-cursor-import.md`
- AGENTS.md §Agent Skills
