---
category: "Architecture SSOTs"
title: "Graphify General Enhancement + GUI Information-Architecture Blueprint — Design"
date: 2026-06-26
status: design
---

# Graphify General Enhancement + GUI Information-Architecture Blueprint — Design

**Goal:** Make the **general, Rust-native Graphify engine** good enough to supply LLM tool-calls with trustworthy graph-based answers to "what is actually in this codebase and how is it wired" — codebase-wide — then use that enriched engine to produce a ratifiable **blueprint for an aggressive reorganization of the Vox GUI** (add / cut / merge / move / rename every surface, nav-group, and command, on evidence). The GUI is the first and highest-value application of the general engine, not a fork of it.

**Intended executor:** Claude Opus 4.8, via `subagent-driven-development`, in a later session. This document is the design; the implementation plan is its sibling under `docs/superpowers/plans/`.

## Why (motivation, grounded)

The existing Rust Graphify (`vox-graphify-reader` + `vox graphify` CLI + 5 MCP tools + `vox-config` freshness model + `contracts/retrieval/graphify-corpora.v1.yaml` corpus registry) builds a deterministic AST graph and is production-healthy. But a real rebuild of the GUI corpus (`vox graphify rebuild --corpus vox-gui-surface`, commit `f03d9f94b5`) returns **822 nodes / 465 edges / 490 communities, with 51% of nodes having zero edges.** The structural extractor captures only direct function *calls*; it cannot see React composition (`<Component/>`, hooks, imports), and — critically — it cannot cross the **string-dispatch boundaries** that define this codebase: `invoke('cmd')` → Tauri handler, `callTool('tool')` → MCP fn, clap subcommand → impl, event-bus topic → subscriber, `vox://` stream → producer. The TS and Rust halves of the GUI do not even connect in the graph.

This is not a GUI problem; it is a **general engine gap**. String-dispatch indirection is the single hardest thing for an agent to trace by grep across a monorepo, and it is everywhere in Vox. Fixing it in the general engine pays off codebase-wide (and for testing — mapping tests → code). The GUI is simply where the payoff is largest and most measurable first.

## Principles (best practices — recorded as SSOT)

These answer "what is Graphify best and worst at," and they constrain every design decision below.

**Graphify is BEST at (lean in):**
- **Structural recall for agents** — neighborhood expansion, "what calls X", shortest path A→B, "what is in module Y". Hands an LLM a map so it stops blind-grepping; highest payoff where the codebase exceeds an agent's context.
- **Tracing string-dispatch boundaries** (once added) — the indirection grep cannot follow.
- **Registry-vs-implementation coverage** — declared-thing joined to real-thing: orphans, gaps, dead-ends. Deterministic and trustworthy.
- **Cluster / blast-radius / change-impact** — cohesive subsystems, cross-cutting tangles, "what breaks if I touch this".

**Graphify is WORST at (never ask it these — route to the LLM/audit instead):**
- **Semantic intent / "why" / utility / UX judgment.** It is structural, not semantic. It cannot decide a name is confusing, a surface is low-value, or two surfaces should merge. The GUI reorg's *judgment* calls come from the LLM + the manual audit, never from the graph.
- **Dynamic / computed dispatch** (reflection, computed keys). Graphify **drops** what it cannot resolve, so it *under-reports rather than fabricates*. Safe for honesty; dangerous only if an agent assumes completeness.
- **Non-code** — CSS, tokens, runtime state, visual layout. Out of the structural core's scope.
- **Fuzzy "related-to"** — it links via *explicit* edges only; it will not infer two differently-named things are the same concept.

**The implied architecture (two layers — the answer to "a semantic map relating any feature to any other"):** keep a deterministic **structural core** (Graphify today — cheap, trustworthy, never hallucinates) and, *later and separately*, an optional **semantic overlay** (embeddings over nodes/docs + LLM-labeled relations) for fuzzy relations. **Never bake LLM-guessed edges into the structural graph** — that destroys the trust that makes it useful. The future "relate arbitrary features to any other" is that semantic overlay built *on top of* a strong structural core. **This project hardens the structural core; the semantic overlay is an explicit non-goal here** (it gets its own spec later).

## Architecture

Five components. Components 1–3 are **general engine** work (codebase-wide); Component 4 is the **GUI application**; Component 5 is the human gate + the hand-off to follow-on execution.

### Component 1 — General Graphify core enhancements (`vox-graphify-reader`, codebase-wide)

All additions are general capabilities, config-driven per corpus, each edge carrying a **confidence label** (`resolved` | `heuristic` | `declared`) so consumers know how much to trust it. They obey Graphify's existing honesty rule: drop ambiguous/unresolved, never invent. Concretely, `ExtractedEdge` (in `crates/vox-graphify-reader/src/ast.rs`) gains a `confidence` field and `ExtractedNode.kind` gains new values (`command`, `tool`, `surface`, `registry-entry`); the `graph.json` writer (`rebuild.rs`) emits both.

1. **String-dispatch / boundary edges.** A general resolver that links a string-keyed call site to the definition it names, configured per "boundary kind" via a declarative rule (caller pattern → target namespace):
   - Tauri IPC: TS `invoke('<cmd>')` / `voxTransport.<wrapper>` → Rust `#[tauri::command] fn <cmd>`.
   - MCP: `callTool('<tool>')` / `invoke('invoke_mcp_tool', { tool })` → MCP tool handler.
   - (Generalizes to clap subcommand routing, event-bus topics, `vox://` streams — added as further boundary rules, not GUI-specific code.)
   - Edges labeled `declared` (string match) — honest about being a name-resolution, not an AST call.
2. **Composition / usage edges.** Extend the TS/TSX extractor (the tree-sitter walk in `ast.rs`) to emit edges for JSX element usage (`<Component/>` → component def), hook calls, and ES `import` relations. This is what collapses the 51%-island problem.
3. **Registry-ingest nodes.** A general capability to ingest a declared registry/SSOT as typed nodes joined to their implementations, driven by a per-registry adapter config: each adapter says how to read the registry and how to match entries to graph nodes. Instances: the GUI surface registry, `get_command_catalog`, the clap command tree, the MCP tool registry. (Generalizes to the policy registry, `layers.toml`, etc.)
4. **Edge-confidence + node-kind extension.** The schema change above, so the graph can represent boundaries and declarations, not just `fn`/`struct`.

Feature-gated; each extractor/adapter has fixture unit tests. Output remains the existing `graph.json` + manifest under `.vox/cache/graphify/<corpus>/`, with the new edge/node kinds.

### Component 2 — Coverage & wiring computation (general "registry-vs-impl coverage" capability)

A general, deterministic pass over an enriched graph that produces a **coverage report** for a corpus: for a chosen registry node-set, classify each entry's implementation status and reachability. Exposed as a new `vox graphify coverage --corpus <id> --registry <name>` subcommand (mirroring the existing `GraphifyCmd` variants). For the GUI it yields:
- **Wiring map**: interactive element → handler → IPC target → backend command/tool → {exists, registered, reachable}; flags dead-ends and orphan-backends.
- **Command-coverage scorecard**: union of (clap CLI ∪ MCP tools ∪ command catalog) × GUI presence/fidelity ∈ {none, cli-only, surfaced-partial, surfaced-full} — "what's met and at what level."
- **Orphan-nav report**: surfaces present in the registry but absent from `navigation.ts` reachability (already known: `needs-you`, `mission-control`, `sub-agents`, `activity`, `search`, `graphify`).

### Component 3 — Join with the manual audit (complete coverage)

The Phase-1 GUI honesty audit (32 per-surface findings JSON, the surface registry tiers, the visual/a11y findings) carries **semantic verdicts the graph cannot produce**. Component 3 joins the deterministic coverage artifacts (C2) with those findings into one per-surface evidence record. This is the literal realization of "use both source systems for complete coverage": structure from Graphify, judgment from the audit/LLM.

### Component 4 — GUI Information-Architecture analysis + aggressive reorg blueprint

The judgment layer (agent + audit driven, NOT graph-derived). Over the joined evidence (C3) it produces, **per surface AND per nav-group AND per command**, a recommendation across the full rubric — **ADD** (a high-utility command/flow with no GUI home), **CUT** (low-utility / dead-end / redundant), **MERGE/COMBINE** (surfaces hitting the same commands/data), **MOVE/REGROUP** (cohesion-driven), **RENAME** (Latin/opaque labels: `mens`, `populi`, `oratio`, `scientia`), **CONDENSE/EXPAND**, **KEEP** — each with rationale + evidence links (graph path + audit finding). It also proposes a **new nav taxonomy from first principles**, derived from command-group cohesion and graph community structure rather than the current accreted tree, presented as a before/after.

Analysis dimensions (the rubric): wiring completeness, command-coverage, redundancy (overlapping data/commands), utility (backend richness + user-facing value + any usage signal), semantic clarity (label↔content match), structural cohesion (does the surface's graph neighborhood match its nav group), and reachability. Each candidate recommendation is **adversarially re-checked** (as in the honesty audit's G1) before entering the table.

### Component 5 — Ratification gate + follow-on hand-off

The blueprint is a per-item decision table (`docs/agents/gui-ia-blueprint.md`) presented for **human ratification** before any reorg code change — the proven G2-style gate. Ratified decisions feed the follow-on execution program (the original "thread 2/3"): the caveat completions (vox-gui Rust compile-verification path, Playwright/visual proof, the 109 visual/DS-token/a11y findings) are folded in there, scoped to the surfaces that *survive* the blueprint (no hardening a surface slated to merge or cut). That execution is a separate plan, gated on ratification.

## Data flow

enrich extractor (C1) → `vox graphify rebuild --corpus vox-gui-surface` (now connected) → `vox graphify coverage` (C2) → join with audit findings (C3) → per-dimension analysis agents + adversarial recheck (C4) → synthesized blueprint table + new-nav proposal → **ratify** (C5) → follow-on execution plan.

## Deliverables

- **General engine:** enhanced `vox-graphify-reader` (boundary edges, composition edges, registry-ingest, edge-confidence), the `vox graphify coverage` subcommand, fixture tests, and updated `graphify-corpora.v1.yaml` (GUI lens config + registry adapters). Re-runnable on any corpus.
- **GUI artifacts:** `graphify-out/gui-coverage/` — wiring map, command-coverage scorecard, orphan/gap/redundancy lists, graph exports/HTML.
- **Blueprint:** `docs/agents/gui-ia-blueprint.md` — the ratification table + the new-nav before/after.
- **This spec** + the implementation plan + (next) the follow-on execution plan.

## Honesty / error handling

- Deterministic extraction with drop-on-ambiguity; every non-AST edge labeled `declared`/`heuristic` so no consumer mistakes a name-match for a proven call.
- Coverage scorecard cross-checked against the canonical command registries (the tool-registry SSOT) so a "surfaced→nonexistent command" claim cannot survive.
- Analysis recommendations adversarially re-checked before ratification.
- Graph freshness respected (existing model); a stale GUI graph fails the coverage step rather than misleading.

## Testing

- Unit fixtures per new extractor/adapter (a TSX file with `invoke('x')` produces the IPC edge to a Rust `#[tauri::command] fn x`; a `<Foo/>` produces a composition edge; a registry adapter yields the expected nodes/joins).
- A regression/parity test: the GUI wiring map contains zero "surfaced element → nonexistent command" entries (a permanent honesty gate, complementary to `vox ci gui-honesty`).
- Coverage scorecard validated against a known command registry snapshot.

## Decomposition (this is a long program — three sub-plans)

This spec is intentionally large; the implementation is decomposed into three sequential plans, each producing working software:

- **Plan 1 — General Graphify core (Components 1–2):** boundary edges + composition edges + registry-ingest + edge-confidence + `vox graphify coverage`, with the GUI corpus as the first proof and a non-GUI corpus as a generality check. Ships a stronger general engine independent of any GUI reorg.
- **Plan 2 — GUI coverage + IA blueprint (Components 3–4–5 up to ratification):** run the enriched engine on the GUI, join with the audit, produce + adversarially verify the blueprint, ratify. Ships the decision artifact; no GUI code changes.
- **Plan 3 — Follow-on execution (post-ratification):** apply the ratified reorg + fold in the caveat completions, scoped to surviving surfaces. Separate plan, separate spec section, gated on Plan 2's ratification.

The writing-plans output for *this* spec covers **Plan 1 and Plan 2** (the general engine + the blueprint). Plan 3 is authored after ratification, when its scope is known.

## Non-goals

- **No semantic/embedding overlay** in this project (the future "relate any feature to any other" — its own spec later).
- **No GUI-specific extractor fork** — all extraction lives in the general engine; the GUI is a lens/config + adapters.
- **No reorg code changes before ratification** — Plans 1–2 change Graphify and produce a blueprint; they do not move/cut/rename GUI surfaces.
- **No CSS/visual modeling in the structural core** — visual findings stay in the audit/overlay layer.
- **No Python construction path** — Rust-native only.

## Dependencies / assumptions

- Builds conceptually on the GUI honesty/wiring work (branch `claude/jolly-jackson-f4b3fb`, the base of this branch): the wiring it added is what makes the GUI worth mapping and what the coverage map should reflect. The follow-on (Plan 3) assumes that work is merged. NOTE: `main` currently does not compile (`vox-cli` `db_cli` WIP breakage at `07ef88d7e2`); this project is based on the compiling honesty branch instead.
- Canonical command/tool registries exist (the tool-registry SSOT, clap tree, `get_command_catalog`) and are the coverage source-of-truth.
- `vox` CLI + the Rust Graphify path are healthy (verified this session).

## Success criteria

1. The enriched general Graphify connects the GUI graph: zero-edge-node share drops sharply from 51% and the TS↔Rust halves are linked via labeled boundary edges — and the same capabilities run on at least one non-GUI corpus (generality proven, no fork).
2. `vox graphify coverage` produces the GUI wiring map + command-coverage scorecard, cross-checked against the command registries with no false "wired" claims.
3. A complete per-item IA blueprint (every surface, nav-group, command) with evidence and an adversarially-verified recommendation, plus a from-first-principles new-nav proposal — ratified by the user.
4. A permanent regression gate that the GUI wiring map stays honest.
5. The general engine improvements are usable by arbitrary agent tool-calls codebase-wide (documented, re-runnable), establishing the structural core the future semantic overlay will build on.
