# Vox Skill Marketplace

The Vox skill marketplace (`vox-skills` crate) provides a plugin system for extending agent capabilities with installable, versioned skills.

## What is a Skill?

A skill is a self-contained bundle containing:
- A `SKILL.md` manifest (TOML frontmatter + markdown body)
- Optional code or instructions
- Declared dependencies and permissions

## SKILL.md Format

```markdown
---
name = "web-search"
version = "1.0.0"
description = "Adds the ability to search the web"
author = "vox-team"
tags = ["search", "web"]
permissions = ["network"]
---

## Instructions

Use this skill to perform web searches...
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `vox_skill_install` | Install a skill from a VoxSkillBundle JSON payload |
| `vox_skill_uninstall` | Uninstall an installed skill by ID |
| `vox_skill_list` | List all installed skills |
| `vox_skill_search` | Search installed skills by keyword |
| `vox_skill_info` | Get detailed info on a specific skill by ID |
| `vox_skill_parse` | Preview a SKILL.md manifest before installing |

## Built-in Skills

The following skills ship pre-installed in `vox-skills/skills/`:

| File | Purpose |
|------|---------|
| `compiler.SKILL.md` | Vox compiler integration |
| `testing.SKILL.md` | Test runner integration |
| `docs.SKILL.md` | Documentation generation |
| `deploy.SKILL.md` | Deployment automation |
| `refactor.SKILL.md` | Code refactoring helper |

## Plugin System

Skills are backed by the `Plugin` trait and managed by `PluginManager`:

```rust
trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn on_event(&self, event: &HookEvent) -> Result<(), PluginError>;
}
```

## Hook System

Skills can register lifecycle hooks via `HookRegistry`:

```rust
registry.register(HookEvent::TaskCompleted, |event| {
    // react to task completion
});
```

Available events: `TaskCompleted`, `TaskFailed`, `AgentStarted`, `AgentStopped`, `MemoryFlushed`.
