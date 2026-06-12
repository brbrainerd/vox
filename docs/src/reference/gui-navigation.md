---
title: "GUI navigation"
description: "Nine top-level surfaces, inner tabs, keybinds, and unified search scopes for the Vox operator console."
category: "Language Reference"
---

# GUI navigation

The operator console compresses legacy sidebar entries into **nine surfaces** plus a **Settings** system row.

## Top-level surfaces

| Surface | Inner tabs (default first) |
|---------|---------------------------|
| Chat | Full-page sessions + docked composer |
| Agents | Dashboard · Agents (flow) · Routing (matrix) |
| Runs & Approvals | Runs · Approvals · Policies |
| Workspace | Repository · Browser · Quick Harness (composer redirect) |
| Commands | Browse & Run · MCP Skills & Plugins |
| Search | Unified search (Memory as a scope chip) |
| Knowledge | Research · Scientia · Review · Claims · Publications |
| Compute | Models · Mens · Populi · Oratio · Mesh |
| Settings | Orchestrator · Model routing · Mesh & peers · Signing · Secrets · Telemetry · Keybinds · Theme · Gamification (+ Coverage child surface) |

Legacy view keys (`approvals`, `flow`, `catalog`, …) still deep-link: they open the correct parent and inner tab.

## Keybinds

| Key | Action |
|-----|--------|
| ⌘/Ctrl+K | Quick search palette |
| ⌘/Ctrl+B | Cycle sidebar width (rail → default → wide) |
| ⌘/Ctrl+Shift+H | Cycle HUD (full → slim → hidden) |
| ⌘/Ctrl+\\ | Split panel (dockview) — **planned** |
| ⌘/Ctrl+W | Close focused panel — **planned** |

## Search scopes (user-facing)

`Code`, `Docs`, `Chats`, `Commands`, `Memory`, `Web`, `Settings` — mapped to backend corpora in the shared search controller.

## SSOT

Surface hierarchy is defined in `contracts/gui/surface-registry.v1.yaml` and regenerated with `vox ci gui-surface-registry --write`.
