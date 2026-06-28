---
title: "Graphify GUI Corpus-Health Surface (2026-06-25)"
description: "Per-corpus freshness/health cards in vox-gui driven by the vox_graphify_status MCP tool, treating freshness as a progress/health indicator with a Rebuild action affordance; documents why health-cards were chosen over a full node-link renderer, which was deferred."
category: "Architecture SSOTs"
---

# Graphify GUI Corpus-Health Surface

> Status: **Landed (GUI only).** Component:
> `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx`.
> Surfaced via the existing `graphify` surface case in
> `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (no new
> registry entry required — the surface was already wired).

## What it is

The Graphify surface renders one **health card per corpus** from the
registry. Each card is a progress/health indicator, not a graph view:

- **Health badge** — `Fresh` (emerald) vs `Stale` (amber), driven by
  `is_fresh` from `vox_graphify_status`.
- **Node / Edge counts** — `node_count` / `edge_count` (em dash when the
  graph has never been built).
- **Built** — `built_at` rendered as a coarse relative time ("3h ago"),
  with the full RFC3339 timestamp on hover.
- **Stale reasons** — `stale_reasons[]` shown as chips so the user knows
  *why* a corpus needs attention.
- **Rebuild action** — a per-stale-corpus `Rebuild` button (plus the
  copy-paste `vox graphify rebuild --corpus <id>` command for terminal
  users).

## Data source

The panel reads the existing read-only `vox_graphify_status` MCP tool
(`crates/vox-orchestrator-mcp/src/graphify_tools.rs`) via the
`vox_graphify_status` Tauri command alias
(`transport.ts::getGraphifyStatus` → `useGraphifyStatus` react-query hook,
60s refetch). The response shape (`CorpusStatus` in
`crates/vox-config/src/graphify.rs`) already carries everything the cards
need: `corpus_id`, `title`, `node_count`, `edge_count`, `built_at`,
`is_fresh`, `stale_reasons`.

The `Rebuild` button calls the `vox_graphify_rebuild` MCP tool **by name**
through the generic `invoke('invoke_mcp_tool', { tool, args })` path and
invalidates the status query on success so the card flips `Fresh` once the
rebuild finishes. That tool is being added by a parallel workstream; until
it lands, a runtime 404 surfaces inline and the rest of the panel keeps
working (marked with a `// ponytail:` debt comment).

## Why health-cards, not a node-link renderer

A full interactive node-link graph renderer is **expensive** (layout
engine, virtualization for 60k+ node corpora, hit-testing, MCP streaming of
edges) and **low marginal value** for the day-to-day question users
actually have: *"is my corpus fresh, and if not, what do I do?"* That
question is fully answered by freshness + counts + stale reasons + a
rebuild affordance, all of which the `vox_graphify_status` tool already
returns. Building the cards is high-value / low-cost; the renderer is
low-value / high-cost.

## Deferred: interactive node-link renderer

A node-link graph view (BFS expansion, path highlighting, community
coloring) is **deferred**. The read-side primitives already exist on the
backend — `vox_graphify_query` (BFS), `vox_graphify_path` (shortest path),
and `vox_graphify_compare` (manifest diff) in `graphify_tools.rs` — so a
future renderer can be added as a second tab on this same surface without
new backend work. It was intentionally left out of this pass to keep the
surface cheap and focused on the health signal.
