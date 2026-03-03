# .opencode Configuration

This directory contains **OpenCode AI IDE** configuration for the Vox programming language project.

## Structure

```
.opencode/
├── agents/      # Specialized agent definitions (markdown frontmatter)
├── skills/      # Reusable skill instructions for agents
└── README.md    # This file
```

## Agents

Each `.md` file in `agents/` defines a specialized AI agent with:
- **scope** — files/crates the agent is allowed to modify
- **model** — preferred LLM (overrides project default)
- **tools** — which OpenCode tools the agent can use
- **permissions** — per-agent permission overrides

## Skills

Skill files in `skills/` provide reusable instructions that any agent can load on-demand for specialized tasks like "adding a language feature" or "writing tests."

## MCP Integration

The `opencode.json` at the project root registers `vox-mcp` as an MCP server, exposing the Vox orchestrator's task queue, file locks, and inter-agent messaging to all agents.
