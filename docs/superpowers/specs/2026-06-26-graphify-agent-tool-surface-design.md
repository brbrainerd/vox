---
category: "Architecture SSOTs"
title: "Graphify Agent Tool Surface — Auto-Availability, Agent Steering, GUI Consumption, and Layer-Tool Registry Pattern"
date: 2026-06-26
status: design
---

# Graphify Agent Tool Surface Design

**Goal.** Make the Graphify code-intelligence tools available **automatically** to any AI agent on any harness running on Vox, steer those agents to **graph-first discovery** (call the graph before `grep`/`Glob`), and consume the **same single tool layer** from the Vox Axis GUI. One tool layer (the MCP dispatch), two consumers (agents + GUI). No per-agent setup.

This is a design SSOT. It is read-only with respect to code; nothing here is executed. It reserves names and patterns so that the data-flow/def-use and semantic-overlay agents (separate work) can add tools uniformly.

---

## 1. Current exposure (as built)

### 1.1 The five tools

Implemented in `crates/vox-orchestrator-mcp/src/graphify_tools.rs`, dispatched from a single `match name` in `crates/vox-orchestrator-mcp/src/dispatch.rs` (graphify arms at lines 627–641, all unconditional / no `#[cfg]` gate):

| Tool | Handler | Shape |
|---|---|---|
| `vox_graphify_status` | `graphify_status` | Read-only freshness report over the corpus registry (see §1.4). |
| `vox_graphify_search` | `graphify_search` | Lexical search over an on-disk graph; persists hits to `knowledge_nodes` (Turso) for agent recall. Supports `intent`-based corpus routing. |
| `vox_graphify_query` | `graphify_query` | BFS neighbor expansion from seed node IDs (`max_depth` ≤ 5). |
| `vox_graphify_path` | `graphify_path` | Shortest path between two node IDs. |
| `vox_graphify_compare` | `graphify_compare` | Node/edge/community delta between two corpus manifests. |

### 1.2 Where descriptions + schemas live (the listing seam)

`tools/list` is assembled in `crates/vox-orchestrator-mcp/src/registry.rs` from **two** sources, joined per tool name:

- **Descriptions + meta** (`vox_tier`, `vox_product_lane`, `vox_http_read_role_eligible`) come from the static generated `TOOL_REGISTRY` table (`vox-mcp-registry` crate, re-exported in `lib.rs:163`).
- **JSON input schemas** are inline raw-string literals in `crates/vox-orchestrator-mcp/src/input_schemas.rs`, function `tool_input_schema(name)` (graphify schemas at lines 471–485).

`server.rs::list_tools` (line 187) merges the static registry + federated workspace tools, filters by the `VOX_MCP_TIERS` env (default `"core"`; all graphify tools are `tier: core`), and appends skill macro tools.

### 1.3 The tool-registry SSOT chain

```
contracts/operations/catalog.v1.yaml      (authoritative operation catalog; graphify entries from line 6170)
        │  vox ci operations-sync --target mcp --write
        ▼
contracts/mcp/tool-registry.canonical.yaml (GENERATED; graphify tools lines 656–680, product_lane: platform, http_read_role_eligible: true, tier: core)
        │  codegen
        ▼
vox-mcp-registry::TOOL_REGISTRY            (static table → descriptions + meta at runtime)
```

The canonical YAML header says **GENERATED — do not hand-edit**. Each catalog entry is one operation, optionally carrying an `mcp:` block (`name`, `http_read_role_eligible`, `tier`) and/or a `cli:` block. This is the uniform extension point (see §5).

### 1.4 Freshness model (detection only — no auto-rerun today)

`crates/vox-config/src/graphify.rs`. `assess_corpus_status` computes `is_fresh = stale_reasons.is_empty()` from these signals:

- `graph_missing` — `graph.json` absent.
- `graph_corrupt` — JSON unparseable / no node-edge stats.
- `git_drift` — manifest `git_sha` ≠ supplied `head_git_sha`.
- `ttl_expired` — `now − built_at > ttl_days` (default 30; per-corpus override + env `VOX_GRAPHIFY_TTL_DAYS`).
- `lexical_lag` — `graph_json_sha256` ≠ `lexical_ingest_sha256` (Turso index behind the graph).
- Warnings (non-stale): `manifest_missing`, `node_count_drift`, `edge_count_drift`, `virtual_corpus`.

Registry (`GraphifyCorporaRegistry`): `default_corpus_id` = `repo-code-graph`; `select_corpus_for_intent` returns the first corpus whose `default_for_intents` contains the intent. Defined corpora (contract `contracts/retrieval/graphify-corpora.v1.yaml`): `repo-code-graph`, `vox-gui-surface`, `vox-config-graph`, `config-audit`, `crate-map`, `graphify-search-log` (`is_virtual: true`).

**Critical gap:** there is **no auto-rerun/regeneration trigger anywhere.** On staleness the GUI panel only displays a copy-able `vox graphify rebuild --corpus <id>` string; the human runs it.

### 1.5 Server entry + connection (who can connect today)

- **Transport: stdio only.** `run_stdio_server_blocking()` in `lifecycle.rs:27`, launched as the `vox mcp` subcommand (`crates/vox-cli/src/commands/mcp.rs:6`). An optional HTTP+WebSocket gateway exists (`spawn_http_gateway_if_enabled`, `lifecycle.rs:56`) for remote/mobile control.
- **No `.mcp.json` is shipped** anywhere in the repo. Today a harness can connect **only if the human externally configures it** to spawn `vox mcp` over stdio. Claude Code would need a hand-written `.mcp.json`; Gemini/Antigravity and deployed/headless agents have **no** wiring at all.

**Net:** the tools exist and are tier-`core`, but availability is **manual and per-harness**, and nothing steers agents to use them first. Those are the two problems this design solves.

---

## 2. Auto-availability — every Vox-hosted agent gets Graphify, no setup

The principle: **Graphify is not an optional connected server; it is part of the Vox agent runtime.** Two cases.

### 2.1 In-process agents (orchestrator-hosted, GUI chat, MENS, deployed/headless)

These already run **inside** `vox-orchestrator-mcp` and dispatch through the same `handle_tool_call` → `match name` path. The graphify arms are unconditional, so **every in-process agent already has the tools** — there is nothing to "connect." The only missing piece is steering (§3) and freshness (§3.4). This covers headless/deployed agents for free, because they share the dispatcher.

**Action:** none for availability; confirm no tier filter hides them (they are `core`; `VOX_MCP_TIERS` default includes core). Add a CI assertion that the five (and future layer-tools) are present in the default tier set so a future tiering change can't silently drop them.

### 2.2 External harnesses (Claude Code, Gemini/Antigravity, third-party MCP clients)

These connect over stdio and need a client config pointed at `vox mcp`. Make this **automatic and arbitrary** via a generated, shipped client config — produced from the same catalog SSOT so it never drifts:

1. **Ship a repo-root `.mcp.json`** (Claude Code's native discovery file) that registers the Vox server:
   ```json
   { "mcpServers": { "vox": { "command": "vox", "args": ["mcp"] } } }
   ```
   Generated by a new `vox ci mcp-client-config --write` step from `catalog.v1.yaml` (transport + binary name are SSOT-derived, not literals). Because Claude Code auto-discovers `.mcp.json` at the workspace root, **any agent the user runs in a Vox checkout gets all Vox MCP tools — including Graphify — with zero setup.**

2. **`vox mcp install <harness>`** writes the equivalent client entry into each harness's own config location (Claude Code user settings, Gemini/Antigravity config, generic MCP client). One generator, multiple emitters (mirrors the existing `operations-sync --target` pattern). This handles harnesses that do **not** read a repo-root `.mcp.json`.

3. **Deployed/headless** agents launched by Vox itself are in-process (§2.1) and need nothing. Headless agents launched under a foreign harness are covered by `vox mcp install`.

**Design fork F1 (per-harness config vs universal default):** option (a) ship only the repo-root `.mcp.json` (universal for `.mcp.json`-aware harnesses, lowest surface); option (b) also auto-run `vox mcp install` for every detected harness at `vox` first-run (truly zero-touch but writes into user-global configs). **Recommendation:** ship (a) always; make (b) an explicit opt-in (`vox mcp install --all`) so Vox never mutates a user's global harness config without consent. **Needs human ratify.**

---

## 3. Agent steering — make them USE Graphify for discovery (graph-first)

Availability ≠ usage. Four reinforcing mechanisms; each alone is weak, together they make graph-first the path of least resistance.

### 3.1 Tool descriptions that route ("call before grep")

The description string is the single highest-leverage steering surface (it is in every `tools/list` an agent sees). Edit the **descriptions in `catalog.v1.yaml`** (then regenerate the canonical YAML + `TOOL_REGISTRY`) so each graphify tool's description is **imperative and comparative**, e.g.:

- `vox_graphify_search`: *"Find where a concept/symbol/module lives by meaning, across the whole repo, in one call. PREFER THIS over grep/Glob for 'where is X' / 'what handles Y' questions — it returns ranked graph nodes (files, symbols, crates) with stable IDs you can feed to vox_graphify_query/path. Falls back to grep only when the graph misses."*
- `vox_graphify_query`: *"Expand outward from a known node to its neighbors (callers, callees, imports, siblings). Use to understand blast radius before editing. Cheaper and more complete than reading files one by one."*
- `vox_graphify_path`: *"Show how two parts of the codebase connect. Use before asking 'does A reach B' — answers structurally instead of by reading."*
- `vox_graphify_status`: *"Check graph freshness. Call once at session start; if stale, the result tells you the rebuild command — but search/query still work on the last build."*

Constraint: descriptions are GENERATED, so the comparative framing must live in the catalog's `description` (or a new `description_human`/`agent_hint` field — see §5) — never hand-edited into the canonical YAML.

### 3.2 A default discovery skill (auto-disclosed at session start)

Skills are **all disclosed at Tier-1** at session start: `build_system_prompt_with_skill` injects every installed skill's `name + description` via `skill_catalog::render_skill_catalog` (`chat_tools/mod.rs:144–157`). There is no per-skill "always-loaded body" flag, but pinned skills get their full body injected.

Add a skill `graph-first-discovery` shipped under `crates/vox-skills/skills/` (discovered + `install_bundle`'d at boot — no code change):

- **Frontmatter `description`** is itself the steering one-liner ("Before exploring an unfamiliar codebase, query the Graphify knowledge graph first — `vox_graphify_search` to locate, `vox_graphify_query` to expand, `vox_graphify_path` to connect — and only fall back to grep when the graph misses."), so even with body unloaded the Tier-1 catalog nudges graph-first.
- **Body** = the discovery playbook (the call-order recipe, when to fall back, how to chain `search→query→path`, how to read the freshness result).

**Make it pinned-by-default** for exploration sessions so its body is injected, not just its name. The cleanest hook: when session-start context detects an unfamiliar/large repo (or always, behind a config flag), pin `graph-first-discovery`. **Design fork F2** below governs whether this is always-on.

### 3.3 Auto-context injection — a code-map summary at session start

Mirror how `MEMORY.md` is injected. The seam is `build_system_prompt_with_skill` in `chat_tools/mod.rs`; insert **immediately after the MEMORY.md block (after line 137, before the `## Environment` block at 139)**, using the same `prompt.push_str("## ...\n\n")` + `vox_bounded_fs::read_utf8_path_capped` pattern. There is **no** code-map injection in this chain today, so this is net-new.

Inject a compact, size-capped **`## Repository code map (Graphify)`** block built from the default corpus:

- Top-N god nodes / highest-degree modules (the structural spine).
- Community labels (the natural subsystem partition).
- Node/edge counts + **freshness line** (`fresh as of <built_at> @ <git_sha>`, or a one-line staleness note).
- A pointer sentence: *"Use `vox_graphify_search`/`_query`/`_path` to drill in; this map is a summary, not the whole graph."*

Source it from the existing reader (`vox_graphify_reader`) over the `repo-code-graph` corpus, capped (e.g. 1–2 KB) to keep the cache prefix stable. This gives every agent — including prompt-only MENS models that can't call tools mid-turn — a baseline mental model **and** primes them that the graph exists. For external harnesses that don't go through this assembler, the same summary is available on demand via a new lightweight `vox_graphify_status` field (or a `summary: true` param) so the discovery skill can fetch it in its first call.

### 3.4 Freshness → auto-rerun (never stale)

Detection exists (§1.4); the trigger does not. Design the auto-rerun so results are never silently stale:

- **Lazy, on-read regeneration (recommended default).** Wrap the graph-reading tools (`search`/`query`/`path`) with a freshness pre-check: call `assess_corpus_status`; if `is_fresh`, proceed; if stale for a **cheap-to-fix** reason (`lexical_lag`, `ttl_expired` on a small corpus), trigger a regenerate **before** answering and tag the response `regenerated_at`. For **expensive** reasons (`git_drift` on the full repo graph), do **not** block the call — answer on the last build but stamp the response `stale: true` + `stale_reasons` + the rebuild command, and **enqueue** a background rebuild (debounced, single-flight per corpus) so the next call is fresh.
- **Event-driven invalidation.** Subscribe a graphify freshness watcher to the post-commit / HEAD-change signal (the same git seam `resolve_head_sha` already uses) so `git_drift` is detected at commit time and a debounced background rebuild is enqueued — not discovered later by an agent.
- **Single-flight + debounce.** A rebuild registry keyed by `corpus_id` prevents N agents from each spawning a rebuild; concurrent readers share one in-flight rebuild and either wait (cheap corpora) or read-stale-and-stamp (expensive corpora).
- **Status surfaces the truth.** `vox_graphify_status` and the GUI panel report `last_rebuild_started_at` / `rebuild_in_progress` so both consumers see the same freshness state.

This converts the existing read-only freshness model into a **self-healing** one without changing the assessment logic — only adding a trigger + a single-flight rebuild queue. (An implementation plan already exists as a draft: `docs/superpowers/plans/2026-06-18-graphify-run-lifecycle-rerun-plan-C.md`.)

**Design fork F3 (block vs answer-stale).** Always-block-until-fresh (simplest correctness, worst latency) vs the tiered block-cheap / answer-stale-expensive policy above (best UX, more moving parts). **Recommendation:** tiered. **Needs human ratify** for the cheap/expensive cutoff (corpus size threshold).

---

## 4. GUI consumption — a Graphify exploration surface in Vox Axis

**Single tool layer, two consumers.** The GUI must call the **same MCP tools** the agents use, via the already-proven `invoke_mcp_tool` Tauri command (`voxTransport.invokeMcpTool(tool, args)` in `transport.ts:439–444`, used today for `vox_pending_approvals`, `vox_feedback_list`, etc.). It must **not** re-implement graph logic in a Tauri command — that is exactly the split-brain to avoid (status already has one: `getGraphifyStatus()` → bespoke `vox_graphify_status` in `crates/vox-gui/src/commands/graphify.rs`, separate from the MCP tool).

### 4.1 What the surface shows

A `Graphify` surface (working name) with these panes, each backed 1:1 by an MCP tool:

- **Code map / overview** — communities + god nodes + node/edge counts + the freshness banner (from `vox_graphify_status`, reusing the §3.3 summary). The default landing pane.
- **Search** — a query box → ranked hits (`vox_graphify_search`), each hit clickable to seed the neighborhood pane. Surfaces the `knowledge_id` so a hit can be pinned to memory.
- **Neighborhood** — pick a node, set depth → BFS graph view (`vox_graphify_query`), rendered as an interactive node-link view.
- **Path** — from/to node pickers → the structural route (`vox_graphify_path`), with `reachable: false` shown honestly.
- **Coverage / communities** — community list with sizes; orphan/dead-end/zero-edge counts (ties into the existing coverage subcommand work on this branch). Lets a human see what the graph does **not** cover (the honesty surface).
- **Compare** (secondary) — corpus-A vs corpus-B deltas (`vox_graphify_compare`), e.g. before/after a refactor.

### 4.2 Registration (per the ratified IA)

Registration is a generated registry + switch dispatch (no runtime panelRegistry):

1. `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` already has `case 'graphify': return <GraphifyStatusPanel />;` — **but `graphify` is an orphan**: it is absent from `navigation.ts` maps and `surfaceRegistry.generated.ts`. Fix the orphan as part of this work.
2. Add the `graphify` view key to `crates/vox-gui/ui/src/lib/navigation.ts` and regenerate `surfaceRegistry.generated.ts` via `vox ci gui-surface-registry --write`.
3. **Ratified-nav placement.** The ratified IA blueprint (`docs/agents/gui-ia-blueprint.md`, RATIFIED 2026-06-26) retires `search` into **Knowledge** and merges claims+knowledge into `scientia`. Graphify is code-intelligence, so it belongs under **Knowledge** (the de-Latinized `scientia`) as a child surface — making "find code by meaning" a first-class part of the knowledge area, co-located with the retired free-text search it supersedes. Promote `GraphifyStatusPanel` into a tabbed `GraphifyPanel` hosting the §4.1 panes.

This keeps the GUI on the **same MCP dispatch** as agents: every pane is `invokeMcpTool('vox_graphify_*', …)`. New layer-tools (§5) appear in the GUI for free by adding a pane that calls them — no backend duplication.

---

## 5. Tool-registry pattern for new layers (uniform extension)

The data-flow/def-use layer and the semantic overlay (separate agents) will add tools. The pattern must make adding a layer-tool **mechanical**. Reserve names + schemas now:

| Reserved tool | Layer | Purpose (one-liner for the description SSOT) |
|---|---|---|
| `vox_graphify_dataflow` | data-flow/def-use | Trace how a value flows: defs → uses → transforms for a symbol/node. |
| `vox_graphify_callers` | data-flow/def-use | Who calls this (reverse call edges), with depth. |
| `vox_graphify_callers_ignoring_result` | data-flow/def-use | Callers that invoke X but **discard its result** (the must-use / dropped-error finder). |
| `vox_graphify_defuse` | data-flow/def-use | Def-use chains for a binding within a scope. |
| `vox_graphify_semantic_related` | semantic overlay | Semantically related nodes (embedding/overlay), not just structurally adjacent. |
| `vox_graphify_explain` | semantic overlay | Natural-language explanation of a node/community from the semantic overlay. |

These are reservations; their schemas are owned by the layer agents. This doc fixes the **names** so the two layers don't collide and the GUI/skill can reference them.

### 5.1 The uniform "add a layer-tool" recipe

Adding any new graphify tool is exactly these steps (no other touch points):

1. **Catalog SSOT** — add an operation entry to `contracts/operations/catalog.v1.yaml` with an `mcp:` block (`name: vox_graphify_<x>`, `http_read_role_eligible: true`, `tier: core`, `product_lane: platform`, `intent_tags: [retrieval, graph, <layer>]`).
2. **Regenerate** the canonical registry: `vox ci operations-sync --target mcp --write` (updates `contracts/mcp/tool-registry.canonical.yaml` → `TOOL_REGISTRY` → descriptions + meta).
3. **Schema** — add the inline JSON schema arm in `crates/vox-orchestrator-mcp/src/input_schemas.rs::tool_input_schema`.
4. **Handler + dispatch** — add the handler fn in `graphify_tools.rs` (or a sibling `graphify_dataflow_tools.rs`) and one `"vox_graphify_<x>" => …` arm in `dispatch.rs`, alongside the existing five.
5. **(Optional) GUI** — add a pane calling `invokeMcpTool('vox_graphify_<x>', …)`. No backend duplication.

Because availability is unconditional in the dispatcher and the registry is generated from one SSOT, a new layer-tool is **automatically** available to every in-process agent and every external harness on next build — the same auto-availability guarantee as the original five.

### 5.2 SSOT enrichment for steering (one optional field)

To carry the "call before grep" comparative framing without polluting structural descriptions, add an optional `agent_hint` field to the catalog `mcp:` block, emitted into `TOOL_REGISTRY` and appended to the tool description at registry-assembly time (`registry.rs`). This keeps the steering copy in the SSOT, regenerable, and uniform across all graphify (and future) tools.

---

## 6. Summary of design forks needing the human

- **F1 — Auto-availability scope (per-harness vs universal).** Ship a repo-root `.mcp.json` always (universal for discovery-aware harnesses); make global `vox mcp install --all` (which writes into users' harness configs) an explicit opt-in. *Confirm: is auto-writing global harness configs acceptable, or strictly opt-in?*
- **F2 — Auto-context injection: always-on vs opt-in.** Inject the Graphify code-map summary + pin the `graph-first-discovery` skill body for **every** session (max steering, ~1–2 KB prompt cost + a freshness read per session) vs only for large/unfamiliar repos behind a config flag. *Confirm: always-on or gated?*
- **F3 — Freshness policy: block vs answer-stale.** Block-until-fresh (simple, slower) vs tiered block-cheap / answer-stale-expensive with background single-flight rebuild (recommended). *Confirm the cheap/expensive cutoff (corpus-size threshold).*

A secondary, lower-stakes decision: whether to add the `agent_hint` SSOT field (§5.2) now or fold the steering copy into existing `description` strings.
