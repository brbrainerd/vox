# SP-1 — Directed Call Queries over the Native Graph

**Date:** 2026-06-30
**Status:** Design (adversarially re-audited 2026-06-30), ready for implementation plan
**Scope:** `vox-graph-reader` (reader + traversal), `vox-orchestrator-mcp` (graph tools), `contracts/operations/catalog.v1.yaml` (tool SSOT)

## Problem

The user's headline need is "what calls what — traversable at any time, yielded to every AI
harness." An audit of the live graph found the directional data is already present and the
code discards it; a second adversarial audit found the data is **directionally sound but not
a clean call graph**, which this spec now states honestly rather than overselling.

### What the live graph actually is (verified)

- The production graph is **System B** (native Rust), at
  `.vox/cache/graphify/repo-code-graph/graph.json` — 95,818 nodes / 83,436 links, zero
  Python, zero LLM. (The Python `graphify-out/` artifact is **System A**, dead w.r.t. the MCP
  tools; do not conflate.)
- Links are stored `{source, target, confidence}`. **Direction is sound:**
  `source = caller, target = callee` is structurally guaranteed at every emit site and is
  never swapped through `rebuild.rs::resolve_edges` (verified; `rebuild.rs:82-125`, locked by
  its `resolve_tests`). Empirical sample: source is 100% plain symbols; boundary nodes
  (`cmd:`/`tool:`) only ever appear as targets (sinks).
- **Direction is destroyed at two layers:** the writer omits any `directed` flag, and the
  reader (`lib.rs:131-134`) builds a single **symmetric** adjacency (pushes both `s→d` and
  `d→s`), so `bfs_from_seeds` / `shortest_path` answer "structurally connected," not "calls."

### What the graph is NOT (the honesty section — read before trusting results)

These are properties an agent or downstream tool must know, or it will over-trust empty/noisy
answers. They are **not** fixed by restoring direction:

1. **Rust method calls are not extracted.** `RustVisitor` implements `visit_expr_call`
   (`foo()`) but has **no `visit_expr_method_call`** (`ast.rs:78-91`; `ExprMethodCall` appears
   nowhere in the crate). So `x.foo()`, `self.bar()`, `vec.push()`, `.await` receivers — the
   dominant Rust call shape — emit zero edges. **Callers/callees over `.rs` are substantially
   incomplete**, not merely imperfect.
2. **Constructor/variant noise dominates in-degree.** ~13.8% of live edges (11,488 / 83,436)
   target `Ok`/`Err`/`Some`/`None`/`From`/`Into`; `Ok` alone is the #1 node at **11,470
   in-edges**. `callers-of(Ok)` is meaningless, and these appear as noise callees of nearly
   every function.
3. **TS/JS edges are not all calls.** The tree-sitter path also emits JSX-composition edges
   (`<Component/>`) and declared `cmd:`/`tool:` boundary edges (`ast.rs:204-274`). All are
   correctly `source→target`, but "what calls what" over TS mixes call + compose + boundary.

## Goal

Make "what calls what" (callers-of / callees-of) directly answerable as a small, additive,
backward-compatible change — restoring the direction the data already has — **and** raise
answer quality with one cheap in-scope filter, while stating the known incompleteness so
results are not over-trusted.

### In scope

- Reader: directed `forward`/`reverse` indexes alongside the existing symmetric one.
- Traversal: a `Direction` selector (`Out`=callees, `In`=callers, `Both`=legacy default).
- MCP: optional `direction` param on `vox_search_neighbors` / `vox_search_path` (default
  `both`, unchanged) + two discoverable wrappers `vox_search_callers` / `vox_search_callees`.
- **Precision filter (false-positive cut):** the two wrappers drop constructor/variant nodes
  (`Ok`/`Err`/`Some`/`None`) from their hits. Minimal stop-set, extensible.

### Decision point for the user (scope boundary)

- **Method-call extraction** (`visit_expr_method_call`) is the single biggest *true-positive*
  win, but it changes extraction: it requires bumping `EXTRACTOR_VERSION` and a one-time
  `vox graphify rebuild` for existing corpora to reflect method edges. It is carried as an
  **optional, clearly-delineated final task** so it can be kept or cut without disturbing the
  direction spine. Recommendation: include it — a Rust "call graph" that omits all method
  calls is misleading. Default the plan to including it; the user may cut it.

### Non-goals (deferred)

- Freshness automation / scheduling (SP-2).
- Cross-harness `.mcp.json` registration / `vox mcp install` (SP-3).
- Dataflow "what yields what", constructor *re-typing*, and a general `relation` edge field
  (SP-4 / W6).

## Design

### 1. Reader: directed indexes alongside the symmetric one

`GraphifyReader` keeps its symmetric `adjacency` (clustering, community, god-node degree
legitimately want undirected — unchanged) and additionally builds, in the same edge pass:

- `forward: source -> [targets]` — **callees**
- `reverse: target -> [sources]` — **callers**

Storage order is already caller→callee, so this is purely "stop merging the two directions."
No file-format change; existing on-disk corpora gain direction immediately, no rebuild.

### 2. Traversal: a `Direction` parameter

`bfs_from_seeds` / `shortest_path` (the reader methods) take a `Direction` enum and select
which adjacency to pass to the existing `bfs.rs` free functions (those are unchanged — they
already take an `adjacency: &HashMap` ref):

- `Out` → `forward` (callees) · `In` → `reverse` (callers) · `Both` → `adjacency` (legacy)

`Both` is the default at every existing call site, so existing behavior is byte-identical.

### 3. MCP surface

- **Extend** `vox_search_neighbors` (`graphify_query`) and `vox_search_path` (`graphify_path`)
  with optional `direction: "in" | "out" | "both"` (default `"both"`). Backward-compatible:
  omitting it reproduces current output. The handler echoes the resolved `direction` into the
  `data` payload (it must add this field — not emitted today).
- **Two dedicated wrappers** for agent discoverability (agents pick tools by name/description):
  - `vox_search_callers` — pinned `In`, constructor filter on
  - `vox_search_callees` — pinned `Out`, constructor filter on
  Each delegates to a shared `graphify_query_core(state, params, forced_dir, filter_noise)`.

### 4. Registration reality (verified — the canonical YAML is generated)

`contracts/mcp/tool-registry.canonical.yaml` is a **generated artifact**. The hand-edited SSOT
is `contracts/operations/catalog.v1.yaml`; the canonical registry + capability-registry are
projected from it and CI fails on drift. Therefore:

- Add two operation rows to `catalog.v1.yaml` (template: the `graph.neighbors` / `graph.path`
  rows at `:6289`/`:6311`). Set `http_read_role_eligible: false` to avoid the HTTP-read-role
  governance coupling (two gates require eligible tools to be listed in
  `http-read-role-governance.yaml`). `false` is safe: harnesses reach these via local stdio
  `vox mcp`, not the HTTP read gateway, so nothing is lost.
- Regenerate derived registries with `vox ci operations-sync --target all --write` (never
  hand-edit the generated YAMLs).
- Add a `tool_input_schema` arm and a `dispatch.rs` match arm per tool.

### 5. Deliberate simplifications (YAGNI, with upgrade paths)

- **No `relation` edge field.** The graph is single-emit-site today; a `relation` field earns
  its keep only when a second *typed* edge kind lands. `ponytail:` add it then.
- **No `directed:true` rewrite of `graph.json`.** Direction is recovered from existing
  `source→target` order; format untouched, old corpora keep working.
- **No dataflow / "yields."** SP-4.
- **Minimal constructor stop-set** (`Ok`/`Err`/`Some`/`None`) rather than a broad std list —
  these four cover the measured 13.8% noise; broadening risks dropping real user symbols.
  `ponytail:` extend the set if a future measurement shows new dominant noise.

## Edge cases & correctness

- **Self-loops** (`A` calls `A`): present in both `forward[A]` and `reverse[A]`; visited-set
  prevents infinite loops (assert it).
- **Constructor seeds:** `callers-of(Ok)` still returns its (huge) caller set — the filter
  drops constructor *hits*, not constructor *seeds*; a constructor seed is a user choosing a
  bad query. Acceptable; documented.
- **Unresolved/`dangling` targets:** treated as ordinary target nodes (today's behavior).
- **Empty result:** `callees(f)` legitimately `[]` (common given the method-call gap) — a
  valid answer, not an error. The tool description states the method-call limitation so empty
  is not misread as "calls nothing."
- **Missing/unknown `direction`:** default `Both`; the schema `enum` rejects malformed values.

## Testing

- **Reader-level unit test** (lightest, where the logic lives — mirrors
  `bfs.rs::tests::multi_seed_reports_nearest_seed_depth`): fixture `A→B→C` + stray `D→B`;
  assert `callees(B)=={C}`, `callers(B)=={A,D}`, directed path `A→C` (out) reachable, `C→A`
  (in) reachable, `A→C` (in) **not** reachable, and `Both` still returns the legacy undirected
  neighborhood `{A,C,D}` (the regression guard against symmetrization leaking back).
- **Handler-level test** (locks the `direction` echo): the existing
  `graph_tools.rs::tests` idiom — `tempfile::tempdir()` + `write_registry(tmp)` +
  hand-written `graph.json` + `test_state_for_repo(tmp)` → `ServerState::test_stub`; call
  `graphify_query` with `direction:"out"`, assert `parsed["success"]==true` and
  `parsed["data"]["direction"]=="out"`. One more asserts `graphify_callers`/`graphify_callees`
  pin direction and drop a constructor hit.
- **Filter unit test:** a graph where `f` calls both `g` and `Ok`; assert `callees(f)`
  contains `g`, excludes `Ok`.
- **Method-call test** (only if the optional task is kept): fixture with `x.foo()`; assert the
  resulting edge `caller→foo` exists.

## Scope boundary (files)

- `crates/vox-graph-reader/src/bfs.rs` (`Direction` enum), `…/lib.rs` (directed indexes +
  method signatures)
- `crates/vox-orchestrator-mcp/src/graph_tools.rs` (params field, `_core` refactor, wrappers,
  filter, tests), `…/input_schemas.rs` (schemas), `…/dispatch.rs` (two arms)
- `contracts/operations/catalog.v1.yaml` (two rows) → regen via `operations-sync`
- *(optional task only)* `crates/vox-graph-reader/src/ast.rs` (`visit_expr_method_call`,
  `EXTRACTOR_VERSION` bump) + one-time `vox graphify rebuild`

No GUI, no scheduling, no `.mcp.json`. With the optional task excluded, no rebuild is required.
