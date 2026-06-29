---
title: "Plan VG-1 — Vox Graph rename + skill + content-manifest emission (TDD)"
category: "Architecture SSOTs"
date: 2026-06-27
status: plan
plan_id: VG-1
spec: docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md
sources:
  - docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md
  - docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md
  - docs/superpowers/plans/2026-06-26-vox-search-absorption-and-cli-ingest.md
---

# Plan VG-1 — Vox Graph Rename + Skill + Content-Manifest Emission

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is **write-through-workflow**: it ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui add … && git -C /c/Users/Owner/vox-graphify-gui commit …` (add + commit only — **never** `push`, `reset`, `rebase`, `checkout --`, `clean`, or `commit --amend`). The workflow performs the final integration commit. On a `.git/index.lock` collision, wait ~20s and retry the `git` command once.
>
> Each task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`. Independent tasks are grouped into explicit **fan-out batches** a workflow can dispatch concurrently. Read the *Workflow Batch Plan* table before dispatching.

## Goal

Retire the residual word *graphify* from crate names, file names, cache paths, contracts, and GUI hooks — extending vs1's external-surface rename with the **internal** rename — and ship the **build-time `gui-content-manifest.json`**: a per-surface index of label, route, subsection headings, static on-screen copy, and invoked command names, emitted by the same Vox Graph walk that already builds the surface graph. The manifest is the new capability VG-2's Omnibar consumes for its ON-SCREEN facet.

**Scope of this plan:**
1. Rename crate `vox-graphify-reader` → `vox-graph-reader`; `graphify_tools.rs` → `graph_tools.rs` (the legacy internal file vs1 left in place); `.vox/cache/graphify/` → `.vox/cache/vox-graph/` with a **one-release back-compat read of the old path** + a test for the fallback; `contracts/retrieval/graphify-corpora.v1.yaml` → `vox-graph-corpora.v1.yaml`; GUI `useGraphifyStatus.ts`/`GraphifyStatusPanel.tsx` → `useVoxGraphStatus.ts`/`VoxGraphStatusPanel.tsx`.
2. Reconcile CLI verb nesting with vs1: the graph-specific verbs (`status`, `ingest`, `rebuild`, `index`, `refresh`, `gc`, `crate-map`) are canonically reached as `vox search <verb>` — vs1's renamed `Cli::Search` group over the **unchanged** inner `GraphifyCmd` enum already provides exactly this. **There is no `graph` infix and no `SearchCmd` enum.** An earlier draft proposed `vox search graph <verb>`, but that would duplicate `vox search rebuild` with `vox search graph rebuild` (no canonical winner) and contradict vs1. **Decision: drop the `graph` infix** — G6 becomes a verify + cli.md-regen reconciliation, not a code-adding task. **Do not re-derive the vs1 rename here** — vs1 owns `Cli::Graphify → Cli::Search` and `#[command(alias = "graphify")]`.
3. Pinned Vox skill `vox-graph` under `assets/skills/vox-graph/SKILL.md` steering agents to graph-first discovery via `vox_search`/`vox_discover` before grep.
4. New capability: `gui-content-manifest.json` emitted by the Vox Graph walk. Per surface: `view_key`, `nav_label`, `nav_group`, `route` (derived as `#view=<view_key>`), `headings` (extracted from the surface's component file — **resolved via the graph's surface→module edge, not a filename guess** — scanning `<h[1-6]` and `aria-label`), `commands` (all `cmd:` neighbor ids for the surface node, read from the graph's **`links`** edge array with an `edges` fallback), `notes` from the YAML registry, and `docs: []` (empty in VG-1, present for VG-2 type parity). Golden test: fixtures for the `links` key, the legacy `edges` key (parity), and a multi-word kebab view key with a real heading.

**VG-1 extends vs1** — it does NOT duplicate the external MCP tool rename (`vox_graphify_* → vox_search_*`) or the CLI top-level variant rename (`Cli::Graphify → Cli::Search`). Those are vs1's scope. VG-1's only CLI touch is the `graph` subgroup wire-up.

## Base branch note (precondition — read before any task)

- **Branch:** all work lands on `claude/graphify-general-gui-ia`. Confirm before the first commit:

```bash
git -C /c/Users/Owner/vox-graphify-gui rev-parse --abbrev-ref HEAD
# must print: claude/graphify-general-gui-ia
```

- **Rebase target:** this plan assumes the branch is rebased onto `main @ 063a3c3235` (the spec/INDEX §5 base). The transport seam `30a46cc88d` (`GraphifyStatusPanel` → `voxTransport`) **already landed on main** — VG-1 G9/G10 must **consume** that seam (rename the hook/panel that already call `voxTransport.getGraphifyStatus()`), **not redo** the Tauri→transport migration (that behavior change is vs1's §4 scope).
- **vs1 dependency:** only G6 (CLI `graph` subgroup) hard-blocks on vs1's `Cli::Search` rename. Phases A–C, E, F are independent of vs1.

## Final surface-key table (single source of truth — prevents double re-key)

Three surface-key strings are in play across vs1 / VG-1 / VG-2. This table is the canonical ownership map; **no task may re-key a surface owned by another plan.**

| Surface key | Final disposition | Owner | VG-1 action |
|---|---|---|---|
| `graphify` | re-keyed → `vox-search` (clap group + registry `view_key`) | **vs1** | **none** — VG-1 does NOT touch the `case` key; G10 renames only the *component* behind it |
| `search` | redirect target → Omnibar surface annotation | **VG-2** (O8) | none |
| `vox-search` | the post-vs1 canonical key for the graph status surface | **vs1** | none |

**G10 rule:** keep `case 'graphify': return <VoxGraphStatusPanel />;` verbatim except the component name. If vs1 has already re-keyed `graphify → vox-search` when G10 runs, G10's `case 'graphify'` arm is updated by vs1's re-key landing — **do not** add a parallel `case 'vox-search'` arm in VG-1 (that is vs1's edit). G10 is component-rename-only; the key edit is vs1's.

## Architecture

**No new engine.** The Vox Graph walk already produces the structural graph in `crates/vox-graphify-reader/src/rebuild.rs::rebuild_graph`. The manifest is a **post-walk emit**: after `rebuild_graph` writes `graph.json`, a new `emit_content_manifest` function reads the finished graph (nodes + edges), joins it with the surface-registry YAML, and writes `gui-content-manifest.json` alongside `graph.json` in the cache dir. The surface components are walked once for headings (a lightweight regex scan, not tree-sitter — the heading text is static, not code-structure).

**Back-compat cache path** (`caches/graphify/ → .vox/cache/vox-graph/`): the CLI commands that construct `cache_dir` (in `crates/vox-cli/src/commands/graphify/mod.rs`) currently hardcode `.vox/cache/graphify/{corpus_id}`. VG-1 changes the primary path to `.vox/cache/vox-graph/{corpus_id}`. On first startup after upgrade, the old path is checked as a fallback; if it exists, a one-line migration notice is printed and the legacy path is resolved (read-only; the function resolves the path, it does not move files). After one release, the fallback is deleted. The fallback is a pure runtime check — no config change. The `vox-config` `CORPORA_REL_PATH` constant is updated to point at `contracts/retrieval/vox-graph-corpora.v1.yaml` after the file rename; `load_graphify_corpora` also gains a fallback read of the old path for one release. A **second** hardcoded legacy cache path — `REGISTERED_REL_PATH = ".vox/cache/graphify/registered.v1.json"` (`vox-config/src/graphify.rs:16`) — is migrated by G4 with the same one-release fallback (see G4 Step 4b).

> **Cross-plan cache-path race (X3) — load-bearing.** This rename is **not** purely additive to vs1: sibling plans **vs3** (`...fusion-discover.md`) and **vs4** hardcode the **legacy** `.vox/cache/graphify/repo-code-graph` path for *writes* and *test fixtures*. VG-1's fallback is read-only, so if vs3/vs4 land after VG-1 a fresh rebuild writes to the deprecated dir and the one-release deletion window starts mis-counted. **Resolution:** VG-1 must either (a) run *after* vs3/vs4 land, or (b) co-update vs3/vs4's cache-path string sites in the same landing. The reconcile phase that owns the INDEX must add cache-path (not just registry-regen) to the cross-plan serialization list in §2.3/§3.1. Flag this to the human gate before dispatching G3.

**Manifest vs. existing surface indices (DRY note).** `gui-content-manifest.json` is **content/text per surface** (label, route, headings, on-screen copy, invoked command names) — it is *not* a competing index to the existing `contracts/gui/omnisearch-index.v1.yaml` (which is an index-*kind policy* over `kind: surface` rows from `SURFACE_REGISTRY`) nor to `scripts/coverage-graph/manifest_writer.py`. The manifest = per-surface content lane consumed by VG-2's ON-SCREEN facet; omnisearch = index-kind policy. VG-2 must not build a second surface index from this manifest. This note exists so VG-2 doesn't duplicate the omnisearch surface lane.

**GUI hooks** (`useGraphifyStatus.ts` → `useVoxGraphStatus.ts`): the hook already calls `getGraphifyStatus()` (transport seam `30a46cc88d`, already on main) → Tauri command `vox_graphify_status`. vs1 has already (or will have) renamed the MCP side to `vox_search_status`; the Tauri command in `crates/vox-gui/src/commands/graphify.rs` (`vox_graphify_status`) is an internal split-brain that vs1's §4 retires by switching the GUI to `invokeMcpTool('vox_search_status')`. VG-1 only renames the **hook file** and **panel component** — the behavior change (Tauri → MCP transport) is vs1 scope. **VG-1's rename is byte-for-byte body-preserving:** the real hook carries `staleTime: 30_000` and `refetchInterval: 60_000` (and `queryKey`/`queryFn`) — all four `useQuery` options must be copied verbatim; only the export name + query key change. The real panel is **113 lines** of working corpus-health UI (`GraphifyStatusPanel.tsx`) — G10 `git mv`s it and renames only the symbol/import; **it does NOT author a replacement component** (a stub would delete working UI — a gui-honesty regression and data loss). The panel's test (`GraphifyStatusPanel.test.tsx`, which `vi.mock`s `../../../hooks/useGraphifyStatus`) is renamed in lockstep.

**Crate rename** (`vox-graphify-reader` → `vox-graph-reader`): rename `[package].name` in `Cargo.toml`, update the crate's `Cargo.toml` description, update all reverse-dependency crates that list `vox-graphify-reader` in their `[dependencies]`, and update the workspace `Cargo.toml` member list if present. Because Rust crate-name identifiers use underscores, library code that imports `vox_graphify_reader::` must be updated to `vox_graph_reader::`.

**Contracts rename** (`graphify-corpora.v1.yaml` → `vox-graph-corpora.v1.yaml`): update `CORPORA_REL_PATH` in `crates/vox-config/src/graphify.rs`, add a one-release fallback read of the old path, rename the file on disk.

**Manifest emission** (`gui-content-manifest.json`): a new module `crates/vox-graph-reader/src/manifest.rs` (post-rename path) with `pub fn emit_content_manifest(graph_json: &str, surface_registry_yaml: &str, surface_dir: &Path, out_path: &Path)`. It: (1) parses the graph JSON for all `surface:` nodes, (2) for each surface node reads the YAML registry entry (label, nav_group, notes), (3) derives headings (see below), (4) collects `cmd:` neighbor ids from the graph edges, (5) writes one manifest entry per surface. Called from `rebuild_graph` in gui-wiring mode (same condition gate as `surface_nodes`).

> **Edge key — `links`, with `edges` fallback (X1, Critical).** The real `graph.json` **writer** emits the edge array under the key `"links"` (`rebuild.rs:333`: `"links": links_val`; `node_count`/`edge_count` read `["links"]`). The downstream readers `coverage.rs:57-58` and `lens.rs:19-20` use `graph.get("links").or_else(|| graph.get("edges"))`. The manifest emitter **must mirror that**: read `graph["links"]` and fall back to `graph["edges"]`. Keying only on `"edges"` would make every surface's `commands` array silently empty in production — a fabricated-empty result (gui-honesty regression). Edge fields are `source`/`target` (confirmed in the writer). The golden test must use a `"links"` fixture **and** a second `"edges"` fixture so both shapes are covered.

> **Headings keyed by graph edge, not filename guess (X5).** The graph already maps each `surface:<vk>` node to its component module via a surface→module edge. Headings must be keyed off that edge (the surface node's module-neighbor file), **not** a TSX filename heuristic (`SubAgentsView.tsx → "subagents" ≠ "sub-agents"` mis-keys every multi-word kebab view key). If the surface→module edge is unavailable for a surface, `headings` is `[]` (honest empty, not a wrong best-effort match). The golden test must include a multi-word view-key fixture with a real heading to exercise the join.

> **`docs` field (X11).** The manifest emits a `docs: []` field per surface (empty for VG-1) so VG-2's `ContentManifestEntry.docs: string[]` type matches the artifact. The DOCS facet itself is sourced from the federated index, not this manifest; the field exists only for type-shape parity.

## Tech Stack

Rust (`syn`, `serde`/`serde_json`, `walkdir`, `anyhow`); `vox-graph-reader` (renamed crate), `vox-cli`, `vox-config`; `vox-gui` Tauri backend + React/TS frontend; pnpm + vitest (GUI tests). Windows fmt rule: **never `cargo fmt --all`**; use `cargo fmt -p <crate>`. Per-crate builds only: `cargo test -p vox-graph-reader`, `cargo build -p vox-cli`, `cargo test -p vox-config`. GUI tests: `pnpm -C crates/vox-gui/ui vitest run <file>`.

## Spec

Primary: `docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md` §1 (naming), §2.1 (content manifest), §7 (testing — rename tests + manifest golden test), §8 (scope/decomposition — VG-1). **Spec §1 reconcile flag:** §1 currently reads "folded into `vox search graph <verb>`" — this contradicts vs1's `vox search <verb>` and the G6 decision to drop the `graph` infix. The reconcile/spec phase must amend §1 to `vox search <verb>`; do **not** edit the spec from this plan. Umbrella: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md`. Read vs1 plan (`2026-06-26-vox-search-absorption-and-cli-ingest.md`) Key-internals section before touching any shared file.

## Key internals (verified against the code — exact)

- **`crates/vox-graphify-reader/Cargo.toml`** — `name = "vox-graphify-reader"` (rename to `vox-graph-reader`). No `vox-cli` or `vox-config` dep (dependency direction is `vox-cli → vox-graphify-reader`); do not add those deps. `dev-dependencies = [tempfile]`.
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `pub struct RebuildMeta { corpus_id, git_sha, scope_path, extraction_mode, built_at_rfc3339 }` (line ~131). `pub fn rebuild_graph(_repo_root, source_dir, output_file, cache_dir, meta)` (line ~139). In gui-wiring mode (`meta.extraction_mode == Some("gui-wiring")`), it already runs `surface_nodes(&content)` when `module_id.ends_with("surfaceRegistry.generated.ts")` (line ~208). The final graph object is written as `{"nodes": nodes_val, "links": links_val}` (lines 332-333 — **the edge array is keyed `"links"`, not `"edges"`**; `node_count`/`edge_count` read `["nodes"]`/`["links"]` at 340/344). **Add the manifest emit here** after all nodes/edges are finalized, in the same `if gui_wiring` block, passing the already-built graph object (which uses `"links"`) to `emit_content_manifest`.
- **`crates/vox-graphify-reader/src/registry.rs`** — `pub fn surface_nodes(src: &str) -> Vec<RegistryNode>` (line 193): parses `viewKey:` from the generated TS registry, emits `surface:{view_key}` nodes. The manifest walker uses the same node list.
- **`crates/vox-graphify-reader/src/ast.rs`** — `ExtractedNode { id, label, kind }` (line ~5). The `kind` field is the string `"surface"` for surface nodes.
- **`crates/vox-cli/src/commands/graphify/mod.rs`** — the file where `.vox/cache/graphify/{corpus_id}` path strings live (lines ~453, ~455, ~595, ~616, tests at ~706, ~710, ~726). This is the main site for the cache path migration. **Module file stays named `graphify/mod.rs`** — only the path strings inside change (vs1's decision to leave module names unchanged is honored). **The inner enum stays named `GraphifyCmd`** — vs1 verified the real enum is `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }` (seven variants) and `pub async fn run(cmd: GraphifyCmd, repo_root)`; vs1 only renames the *outer* `Cli::Graphify → Cli::Search` clap group, **not** the inner enum. There is **no `SearchCmd` enum**. The `vox search graph <verb>` subgroup wiring (G6) adds a `Graph(GraphCmd)` variant to **`GraphifyCmd`** and forwards each `GraphCmd` verb to `run(GraphifyCmd::<Verb>, repo_root)`.
- **`crates/vox-config/src/graphify.rs`** — `pub const CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml"` (line 13). Update to `"contracts/retrieval/vox-graph-corpora.v1.yaml"`. `pub fn load_graphify_corpora(repo_root)` (line 153) reads `repo_root.join(CORPORA_REL_PATH)` via `fs::read_to_string` (the file imports `use std::fs;` at line 6 — match that style, do **not** introduce `std::fs::read_to_string`). Add a fallback read of the old path (only if new path not found). Tests at lines ~575, ~687 supply the YAML inline — update the fixture file name in the include_str!.
- **`crates/vox-config/src/graphify.rs:16`** — `pub const REGISTERED_REL_PATH: &str = ".vox/cache/graphify/registered.v1.json"` — a **second** hardcoded legacy `graphify/` cache path (the registry overlay). `load_registered_corpora` / `upsert_registered_corpus` read/write this. **Migrate in G4** to `.vox/cache/vox-graph/registered.v1.json` with the same one-release fallback (read old if new absent), or the registry overlay split-cache survives the migration. G4 Step 4b covers this.
- **`contracts/retrieval/graphify-corpora.v1.yaml`** — rename to `vox-graph-corpora.v1.yaml`. No content change to the YAML body required (the corpus IDs inside are separate from the file name).
- **`crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts`** — **15 lines**; `GRAPHIFY_STATUS_QUERY_KEY = ['graphify', 'status']` (line 5); calls `getGraphifyStatus` from `../transport` (line 2); the `useQuery` carries **four** options: `queryKey`, `queryFn`, `staleTime: 30_000`, `refetchInterval: 60_000`. Rename file → `useVoxGraphStatus.ts`; update query key to `['vox-graph', 'status']`; **copy all four `useQuery` options verbatim** (G9 must NOT drop `staleTime`/`refetchInterval` — that is a polling regression); update import site in the panel.
- **`crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx`** — **113 lines** of real corpus-health UI (per-corpus cards, fresh/stale badges, node/edge counts, stale-reason chips, rebuild-command block). Imports `useGraphifyStatus` (line 2). **`git mv` the file → `VoxGraph/VoxGraphStatusPanel.tsx`, preserve the 113-line body byte-for-byte**, rename only the export `GraphifyStatusPanel → VoxGraphStatusPanel` and the hook import → `useVoxGraphStatus`. **Do NOT author a replacement component** (a stub deletes working UI — data loss + gui-honesty regression). Update import in `surfaceComponents.tsx` (line ~24: `import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel'` → `import { VoxGraphStatusPanel } from '../surfaces/VoxGraph/VoxGraphStatusPanel'`). Directory renamed `Graphify/ → VoxGraph/`.
- **`crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx`** — **exists** (52 lines). Line 7: `vi.mock('../../../hooks/useGraphifyStatus', …)` mocking both `useGraphifyStatus` and `GRAPHIFY_STATUS_QUERY_KEY`; line 12 imports `{ GraphifyStatusPanel } from './GraphifyStatusPanel'`. Renaming the hook **breaks this mock path** → test failure. G10 must `git mv` it → `VoxGraph/VoxGraphStatusPanel.test.tsx`, update the `vi.mock` path to `../../../hooks/useVoxGraphStatus` and its exports (`useVoxGraphStatus`, `VOX_GRAPH_STATUS_QUERY_KEY`), and update the render import to `./VoxGraphStatusPanel` / `VoxGraphStatusPanel`. The asserted on-screen string `vox graphify rebuild --corpus …` (line 43) is part of the preserved body — leave it unless vs1 re-keys the CLI verb (out of VG-1 scope).
- **`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`** — `case 'graphify': return <GraphifyStatusPanel />;` (~line 115) and import (~line 24). Update to `<VoxGraphStatusPanel />` and the new import path. The `case 'graphify'` surface key is **not changed here** — the vs1 plan re-keys the surface from `graphify` → `vox-search`; VG-1 only updates the component name.
- **`assets/skills/`** — the auto-hydrated root for pinned skills. Existing examples: `assets/skills/brainstorming/`, each has a `SKILL.md` with YAML front-matter (`name`, `description`) and markdown body. New skill lives at `assets/skills/vox-graph/SKILL.md`.
- **`crates/vox-gui/src/commands/graphify.rs`** — Tauri command `vox_graphify_status` (line ~35). **VG-1 does NOT rename or modify this file** — its rename/retirement is vs1 scope (vs1 retires the direct Tauri call in favor of `invokeMcpTool('vox_search_status')`). VG-1 only renames the hook/panel on the TS side.
- **`crates/vox-orchestrator-mcp/src/lib.rs:54`** — `pub mod graphify_tools;` (the module declaration to rename → `pub mod graph_tools;`). **Not** in `dispatch.rs` — the `mod` decl is in `lib.rs`.
- **`crates/vox-orchestrator-mcp/src/dispatch.rs:628-640`** — **five** `crate::graphify_tools::` call sites: `graphify_status` (628), `graphify_search` (631), `graphify_query` (634), `graphify_path` (637), `graphify_compare` (640). All five `crate::graphify_tools::` prefixes must become `crate::graph_tools::` (the inner fn names `graphify_status`/etc. are unchanged — they are vs1's external rename, not VG-1's). These five lines + `lib.rs:54` are explicit G1 commit-`add` targets; do not dismiss as "references in the same scope".
- **Reverse-dependency crates using `vox-graphify-reader`**: `crates/vox-cli/` (uses `vox_graphify_reader::rebuild::`, `vox_graphify_reader::coverage::`, `vox_graphify_reader::graph_digest`, etc.); `crates/vox-orchestrator-mcp/` (uses the reader for dispatch — see lib.rs:54 + dispatch.rs:628-640 above). **Non-`crates/` CI-gate touchpoints (X5/#5):** `contracts/ci/crate-graph.v1.json` (3 refs) + `contracts/ci/crate-build-map.v1.json` (1 ref) feed the crate-graph parity gate in `.github/workflows/ci.yml`; `docs/src/architecture/layers.toml` lists `vox-graphify-reader` **twice** (lines 170, 262 — layering gate). These are regenerated (contracts) / hand-updated (layers.toml + `where-things-live.md`) in G1, **not** patched ad-hoc. Identify all reverse deps with a **repo-root** grep (not `crates/`-scoped) in Task G0.

## Dependencies (cross-plan)

- **Requires vs1's `Cli::Graphify → Cli::Search` rename** before G6 can verify the `vox search <verb>` verb path. If vs1 has not landed, VG-1 Phases A–C (crate rename, cache path, contracts, skill), E (manifest), and F (GUI rename) are fully independent; only Phase D (G6 CLI verb-path reconcile — verify + cli.md regen, no code) needs vs1's `Cli::Search` variant to exist.
- **Cache-path cross-plan race (X3):** the `.vox/cache/graphify → .vox/cache/vox-graph` rename (G3) is read-only back-compat, but **vs3/vs4 hardcode the legacy write path** — VG-1 must run after vs3/vs4 or co-update their path strings (see G3 warning + Architecture). Flag to the human gate.
- **Blocks VG-2** (Omnibar): the `gui-content-manifest.json` emit (Phase E) is the manifest VG-2's `useContentManifest.ts` hook loads. VG-2 degrades honestly (returns `[]`) until VG-1 lands — the dependency is soft for execution order.
- **VG-3 independent** — Task-Monitor Dashboard uses only `SURFACE_REGISTRY`, not the manifest.

---

## File Structure

**Renamed (crate)**
- `crates/vox-graphify-reader/` → `crates/vox-graph-reader/` (directory rename; all internal module paths unchanged except Cargo.toml `name`).
- `crates/vox-orchestrator-mcp/src/graphify_tools.rs` → `graph_tools.rs` (the legacy internal file; update `mod` declaration in the parent `mod.rs` or `lib.rs`).

**Modified (Rust)**
- `crates/vox-graph-reader/Cargo.toml` — rename `name`; update description.
- `crates/vox-graph-reader/src/manifest.rs` — **new file**: `pub fn emit_content_manifest(graph_json, surface_registry_yaml, surface_dir, out_path)`.
- `crates/vox-graph-reader/src/lib.rs` — add `pub mod manifest;`.
- `crates/vox-graph-reader/src/rebuild.rs` — call `manifest::emit_content_manifest(…)` in gui-wiring mode after graph write; add `cli_catalog_json: Option<String>` to `RebuildMeta` (this field is also used by vs1's T5/T6 CLI-ingest — coordinate, do not clobber).
- `crates/vox-graph-reader/tests/manifest_tests.rs` — **new file**: golden test for the manifest emitter.
- `crates/vox-config/src/graphify.rs` — update `CORPORA_REL_PATH`; add one-release fallback in `load_graphify_corpora`.
- `crates/vox-cli/src/commands/graphify/mod.rs` — update cache path strings (`.vox/cache/graphify/ → .vox/cache/vox-graph/`); add migration fallback; update `vox_graphify_reader::` → `vox_graph_reader::` call sites; update `include_str!` of corpora YAML in inline tests; pass `gui_source_dir`/`surface_registry_yaml` to `RebuildMeta` in gui-wiring mode (G8). **No CLI enum change** — G6 is verify-only (no `graph` infix, no `SearchCmd`).
- `crates/vox-cli/Cargo.toml` — update dep name `vox-graphify-reader` → `vox-graph-reader`.
- `crates/vox-orchestrator-mcp/Cargo.toml` — update dep name if present.
- `crates/vox-orchestrator-mcp/src/lib.rs` — update `pub mod graphify_tools;` (line 54) → `pub mod graph_tools;`.
- `crates/vox-orchestrator-mcp/src/dispatch.rs` — update the **five** `crate::graphify_tools::` call sites (lines 628-640) → `crate::graph_tools::`.
- `crates/vox-orchestrator-mcp/src/graph_tools.rs` (renamed from `graphify_tools.rs`) — update any `vox_graphify_reader::` refs to `vox_graph_reader::`.
- `contracts/ci/crate-graph.v1.json` + `contracts/ci/crate-build-map.v1.json` — **regenerated via their generator** (crate-graph parity gate), not hand-edited.
- `docs/src/architecture/layers.toml` — update the two `vox-graphify-reader` entries (lines 170, 262) → `vox-graph-reader`; update `where-things-live.md` reference.
- `crates/vox-config/src/graphify.rs` — also migrate `REGISTERED_REL_PATH` (line 16) with one-release fallback (G4 Step 4b).
- All other reverse-dep crates identified in Task G0 (repo-root grep).

**Renamed (contracts)**
- `contracts/retrieval/graphify-corpora.v1.yaml` → `contracts/retrieval/vox-graph-corpora.v1.yaml`.

**Modified (GUI TypeScript)**
- `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` → renamed to `useVoxGraphStatus.ts`; query key updated; **all four `useQuery` options preserved**.
- `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts` → renamed to `useVoxGraphStatus.test.ts`; descriptions updated.
- `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx` → `git mv`d to `components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx`; **113-line body preserved**; export + hook import renamed.
- `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx` → `git mv`d to `components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx`; `vi.mock` path + render import updated.
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — import + case arm updated (component name only; **surface key untouched** — vs1 owns the `graphify → vox-search` re-key).

**Created**
- `assets/skills/vox-graph/SKILL.md` — pinned skill steering agents to graph-first discovery.
- `crates/vox-graph-reader/src/manifest.rs` — content-manifest emitter.
- `crates/vox-graph-reader/tests/manifest_tests.rs` — golden test.

---

## Workflow Batch Plan (fan-out structure)

| Batch | Tasks | Class | Depends on | Conflict surface | Dispatch |
|---|---|---|---|---|---|
| **G0 — preflight** | G0 | [SEQUENTIAL] | — | read-only audit | 1 agent |
| **Batch A — crate rename (parallel start)** | G1, G2 — sequential within | [SEQUENTIAL] | G0 green | `Cargo.toml` files, all `vox_graphify_reader::` call sites, `graphify_tools.rs` | 1 agent |
| **Batch B — cache-path migration (parallel with A)** | G3 | [PARALLEL-SAFE] with A | G0 green | `graphify/mod.rs` path strings only | 1 agent |
| **Batch C — contracts + skill (parallel with A+B)** | G4, G5 — parallel | all [PARALLEL-SAFE] | G0 green | disjoint files | 2 agents |
| **Batch D — CLI verb-path reconcile (after A+B)** | G6 | [SEQUENTIAL] | G1+G3 green + vs1 `Cli::Search` | read-only verify + cli.md regen (often no-op) | 1 agent |
| **Batch E — manifest emitter (after A)** | G7, G8 — sequential | [SEQUENTIAL] within E | G1 green (renamed crate) | `manifest.rs`, `rebuild.rs` | 1 agent |
| **Batch F — GUI hook+panel rename (parallel with D+E)** | G9, G10 — parallel | all [PARALLEL-SAFE] | G0 green | `hooks/`, `surfaces/Graphify/`, `surfaceComponents.tsx` | 1 agent |
| **Batch G — close** | G11 | [SEQUENTIAL] (terminal) | all prior green | verify only | 1 agent; no commit |

**Parallelism summary:** Batches A, B, C, and F may all start once G0 (preflight) passes. Batch D waits for A+B. Batch E waits for A. Batch G is the terminal gate. **Max 4 concurrent agents** (A+B+C+F simultaneously).

---

# PHASE A — Preflight

## Task G0: Identify all reverse-dependency crates [SEQUENTIAL]

Before renaming, build the complete list of crates that depend on `vox-graphify-reader`. This avoids mid-rename compile breaks.

**Files:** Read-only.

- [ ] **Step 1: Inventory** — run a **repo-root** grep (NOT `crates/`-scoped — the rename has CI-gate touchpoints under `contracts/` and `docs/`):

```bash
grep -rn "vox-graphify-reader\|vox_graphify_reader" \
  /c/Users/Owner/vox-graphify-gui/ \
  --include="Cargo.toml" --include="*.rs" --include="*.json" --include="*.toml" --include="*.md" \
  | grep -v "/target/" \
  | grep -v "/c/Users/Owner/vox-graphify-gui/crates/vox-graphify-reader/" \
  | grep -v "/docs/superpowers/" # plans/specs cite the old name as history — don't rewrite them
```

Expected at minimum: `crates/vox-cli/Cargo.toml`, `crates/vox-cli/src/commands/graphify/mod.rs` (multiple lines), `crates/vox-orchestrator-mcp/` (lib.rs:54, dispatch.rs:628-640, Cargo.toml), `contracts/ci/crate-graph.v1.json` (3 refs), `contracts/ci/crate-build-map.v1.json` (1 ref), `docs/src/architecture/layers.toml` (lines 170, 262).

- [ ] **Step 1b: Confirm the edge-key gate** — verify the writer emits `"links"` so G7 keys correctly:

```bash
grep -n '"links"\|"edges"' /c/Users/Owner/vox-graphify-gui/crates/vox-graphify-reader/src/rebuild.rs
# expected: "links": links_val (writer) — NOT "edges". G7 must read links w/ edges fallback.
```

- [ ] **Step 1c: Confirm the inner CLI enum name** (hard gate for G6) — pin the real enum before any G6 code:

```bash
grep -n "enum GraphifyCmd\|enum SearchCmd\|fn run(cmd" /c/Users/Owner/vox-graphify-gui/crates/vox-cli/src/commands/graphify/mod.rs | head
# expected: `enum GraphifyCmd { … }` and `run(cmd: GraphifyCmd, …)`. There is NO SearchCmd enum.
# If this prints SearchCmd, vs1 changed its decision — reconcile G6 before writing code.
```

- [ ] **Step 2: Record** — write the list to a scratchpad. This is the checklist for G1.

- [ ] **Step 3: Verify workspace member** — check `Cargo.toml` at repo root for `members = [ … "crates/vox-graphify-reader" … ]`.

```bash
grep "vox-graphify-reader" /c/Users/Owner/vox-graphify-gui/Cargo.toml
```

- [ ] **Step 4: Preflight commit** — no source changes; commit a one-line breadcrumb:

```bash
git -C /c/Users/Owner/vox-graphify-gui commit --allow-empty \
  -m "chore(vg1): G0 preflight — reverse-dep inventory complete (vox-graphify-reader)"
```

---

# PHASE B — Crate rename

## Task G1: Rename crate `vox-graphify-reader` → `vox-graph-reader` [SEQUENTIAL]

The largest mechanical change: rename the directory, update `Cargo.toml`, update all call sites.

**Files:** `crates/vox-graphify-reader/Cargo.toml` (rename + update), workspace `Cargo.toml`, all crates from G0 inventory, `crates/vox-orchestrator-mcp/src/graphify_tools.rs` (rename to `graph_tools.rs`).

- [ ] **Step 1: Rename directory** (git mv preserves history):

```bash
git -C /c/Users/Owner/vox-graphify-gui mv \
  crates/vox-graphify-reader crates/vox-graph-reader
```

- [ ] **Step 2: Update `[package].name`** in `crates/vox-graph-reader/Cargo.toml`:

```toml
[package]
name = "vox-graph-reader"
# description update:
description = "Read-only BFS traversal, path-finding, and cross-manifest comparison for Vox Graph graph.json exports"
```

- [ ] **Step 3: Update workspace `Cargo.toml`** — replace `"crates/vox-graphify-reader"` with `"crates/vox-graph-reader"` in the `members` list.

- [ ] **Step 4: Update reverse-dep `Cargo.toml` files** — for each crate in the G0 inventory, change:

```toml
# Before:
vox-graphify-reader = { workspace = true }
# or:
vox-graphify-reader = { path = "..." }

# After:
vox-graph-reader = { workspace = true }
```

If the workspace `[dependencies]` table has `vox-graphify-reader`, update it too.

- [ ] **Step 5: Update all `vox_graphify_reader::` call sites** — in every `.rs` file identified in G0, replace `vox_graphify_reader::` with `vox_graph_reader::`. Example (from `crates/vox-cli/src/commands/graphify/mod.rs`):

```rust
// Before:
vox_graphify_reader::rebuild::rebuild_graph(…)
vox_graphify_reader::coverage::compute_coverage(…)
let digest = vox_graphify_reader::graph_digest(…);
vox_graphify_reader::snapshot::snapshot_corpus(…)
vox_graphify_reader::snapshot::prune_snapshots(…)

// After:
vox_graph_reader::rebuild::rebuild_graph(…)
vox_graph_reader::coverage::compute_coverage(…)
let digest = vox_graph_reader::graph_digest(…);
vox_graph_reader::snapshot::snapshot_corpus(…)
vox_graph_reader::snapshot::prune_snapshots(…)
```

- [ ] **Step 6: Rename `graphify_tools.rs` → `graph_tools.rs`** in `crates/vox-orchestrator-mcp/src/`:

```bash
git -C /c/Users/Owner/vox-graphify-gui mv \
  crates/vox-orchestrator-mcp/src/graphify_tools.rs \
  crates/vox-orchestrator-mcp/src/graph_tools.rs
```

Update the `mod` declaration at **`crates/vox-orchestrator-mcp/src/lib.rs:54`** (`pub mod graphify_tools;` → `pub mod graph_tools;`). Update the **five** `crate::graphify_tools::` call sites at **`crates/vox-orchestrator-mcp/src/dispatch.rs:628,631,634,637,640`** → `crate::graph_tools::` (the inner fn names `graphify_status`/`graphify_search`/`graphify_query`/`graphify_path`/`graphify_compare` are **unchanged** — those are vs1's external rename, not VG-1's). Confirm exact lines:

```bash
grep -n "crate::graphify_tools::\|graphify_tools;" \
  /c/Users/Owner/vox-graphify-gui/crates/vox-orchestrator-mcp/src/lib.rs \
  /c/Users/Owner/vox-graphify-gui/crates/vox-orchestrator-mcp/src/dispatch.rs
```

- [ ] **Step 6b: Regenerate crate-graph contracts (NEVER hand-edit)** — `contracts/ci/crate-graph.v1.json` (3 refs) + `contracts/ci/crate-build-map.v1.json` (1 ref) feed the crate-graph parity gate in `.github/workflows/ci.yml`. Run their generator (find it via `grep -rn "crate-graph.v1" scripts/ .github/` — typically a `vox ci` / `scripts/*` regen target), not a manual patch. If no generator exists, the JSON is hand-maintained — update the 3+1 refs and note it in the commit.

- [ ] **Step 6c: Update layering contract** — `docs/src/architecture/layers.toml` lists `vox-graphify-reader` at **lines 170 and 262**; update both → `vox-graph-reader`. Update the `vox-graphify-reader` reference in `where-things-live.md` if present.

- [ ] **Step 7: Compile check** — crate-scoped, not `--all`:

```bash
cargo build -p vox-graph-reader 2>&1 | tail -5
cargo build -p vox-cli 2>&1 | tail -5
cargo build -p vox-orchestrator-mcp 2>&1 | tail -5
```

- [ ] **Step 8: Test check**:

```bash
cargo test -p vox-graph-reader 2>&1 | tail -10
```

- [ ] **Step 9: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-graph-reader/ \
  Cargo.toml \
  crates/vox-cli/Cargo.toml crates/vox-cli/src/ \
  crates/vox-orchestrator-mcp/Cargo.toml crates/vox-orchestrator-mcp/src/ \
  contracts/ci/crate-graph.v1.json contracts/ci/crate-build-map.v1.json \
  docs/src/architecture/layers.toml
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "refactor(vg1-G1): rename crate vox-graphify-reader → vox-graph-reader; graphify_tools.rs → graph_tools.rs; regen crate-graph contracts + layers.toml"
```

## Task G2: Add `pub mod manifest;` stub to reader lib [SEQUENTIAL after G1]

Creates the module entry point so later tasks in Batch E can land without compile breaks.

**Files:** `crates/vox-graph-reader/src/lib.rs`, `crates/vox-graph-reader/src/manifest.rs` (stub only).

> **Note (X10):** this G2 smoke test is a **compile/type gate only**, not a red→green TDD test. It asserts the function *signature* exists; it passes the moment the stub lands. The real red→green is G7's golden test (`manifest_golden_surface_approvals`), which fails on the `Err("not yet implemented")` stub and goes green when G7 implements the body. Do not frame G2's smoke as a "failing test first" — it is a link-time guard.

- [ ] **Step 1: Smoke test** — create `crates/vox-graph-reader/tests/manifest_tests.rs` with:

```rust
// Tests for gui-content-manifest emission.
// The real emit function is implemented in Task G7; this file is the test fixture entry point.
// This smoke asserts only the function SIGNATURE (a compile/link gate). Until the G2 stub
// lands, this file fails to COMPILE (the symbol is undefined); once the stub exists, it
// compiles and passes. The real behavioral red→green is G7's golden test.

use vox_graph_reader::manifest::emit_content_manifest;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn manifest_module_exists() {
    // Smoke: the module + signature are reachable (compile gate, not a behavioral assertion).
    let _ = emit_content_manifest as fn(&str, &str, &Path, &Path) -> Result<(), Box<dyn std::error::Error>>;
}
```

Run — expect a **compile error** (symbol `emit_content_manifest` undefined until the stub lands):

```bash
cargo test -p vox-graph-reader --test manifest_tests 2>&1 | tail -10
```

- [ ] **Step 2: Stub `manifest.rs`**. Create `crates/vox-graph-reader/src/manifest.rs`:

```rust
//! GUI content-manifest emitter.
//!
//! Emits `gui-content-manifest.json` from the structural graph + surface-registry YAML.
//! Called by `rebuild::rebuild_graph` in gui-wiring mode after the graph is written.

use std::path::Path;

/// Emit `gui-content-manifest.json` alongside the graph in `out_path`.
///
/// # Parameters
/// - `graph_json` — the already-written `graph.json` string (read it back, or pass in).
/// - `surface_registry_yaml` — contents of `contracts/gui/surface-registry.v1.yaml`.
/// - `surface_dir` — the GUI `ui/src/` directory to scan for headings in TSX files.
/// - `out_path` — destination file path (typically `<cache_dir>/gui-content-manifest.json`).
pub fn emit_content_manifest(
    _graph_json: &str,
    _surface_registry_yaml: &str,
    _surface_dir: &Path,
    _out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Implemented in Task G7.
    Err("emit_content_manifest: not yet implemented (VG-1 Task G7)".into())
}
```

- [ ] **Step 3: Wire into `lib.rs`**. Add to `crates/vox-graph-reader/src/lib.rs`:

```rust
pub mod manifest;
```

- [ ] **Step 4: Run test** — should now **compile and PASS** (the smoke asserts only the signature; it does not call the function):

```bash
cargo test -p vox-graph-reader --test manifest_tests 2>&1 | tail -10
```

Expected: `manifest_module_exists ... ok` (the stub satisfies the signature gate). The behavioral red→green lands in G7 (the golden test calls the function and fails on the `Err` stub until G7 implements the body).

- [ ] **Step 5: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-graph-reader/src/manifest.rs \
  crates/vox-graph-reader/src/lib.rs \
  crates/vox-graph-reader/tests/manifest_tests.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G2): add manifest module stub to vox-graph-reader (VG-1 Phase B)"
```

---

# PHASE C — Cache-path migration

## Task G3: Migrate `.vox/cache/graphify/` → `.vox/cache/vox-graph/` with one-release back-compat [PARALLEL-SAFE with A]

> **⚠ Cross-plan race (X3) — HUMAN-GATE before dispatch.** Sibling plans **vs3**/**vs4** hardcode the legacy `.vox/cache/graphify/repo-code-graph` path for *writes* and *test fixtures*. VG-1's fallback is read-only, so if vs3/vs4 land *after* VG-1 a fresh rebuild lands in the deprecated dir and the deletion window mis-counts. Confirm one of: (a) VG-1 runs after vs3/vs4, or (b) vs3/vs4's path strings are co-updated in the same landing. The reconcile/INDEX phase owns adding cache-path to the cross-plan serialization list — do **not** edit the INDEX here.

**Files:** `crates/vox-cli/src/commands/graphify/mod.rs`.

- [ ] **Step 1: Failing test** — append to the inline tests at the bottom of `crates/vox-cli/src/commands/graphify/mod.rs`:

```rust
#[cfg(test)]
mod vg1_cache_path_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn new_cache_path_is_vox_graph() {
        let tmp = TempDir::new().unwrap();
        let corpus_id = "repo-code-graph";
        // The primary path must be .vox/cache/vox-graph/<corpus_id>
        let expected = tmp.path().join(".vox/cache/vox-graph").join(corpus_id);
        let actual = primary_cache_dir(tmp.path(), corpus_id);
        assert_eq!(actual, expected);
    }

    #[test]
    fn legacy_path_is_detected_as_fallback() {
        let tmp = TempDir::new().unwrap();
        let corpus_id = "repo-code-graph";
        // If the legacy path exists and the new path does not, fallback returns Some(legacy).
        let legacy = tmp.path().join(".vox/cache/graphify").join(corpus_id);
        std::fs::create_dir_all(&legacy).unwrap();
        let result = cache_dir_with_migration(tmp.path(), corpus_id);
        assert_eq!(result, legacy, "should fall back to legacy graphify path");
    }

    #[test]
    fn new_path_preferred_over_legacy() {
        let tmp = TempDir::new().unwrap();
        let corpus_id = "repo-code-graph";
        let new_path = tmp.path().join(".vox/cache/vox-graph").join(corpus_id);
        std::fs::create_dir_all(&new_path).unwrap();
        let legacy = tmp.path().join(".vox/cache/graphify").join(corpus_id);
        std::fs::create_dir_all(&legacy).unwrap();
        let result = cache_dir_with_migration(tmp.path(), corpus_id);
        assert_eq!(result, new_path, "new path must take priority over legacy");
    }
}
```

Run — expect compile error (`primary_cache_dir` and `cache_dir_with_migration` not yet defined):

```bash
cargo test -p vox-cli 2>&1 | grep "error\|FAILED\|ok" | head -10
```

- [ ] **Step 2: Add helper functions**. In `crates/vox-cli/src/commands/graphify/mod.rs`, add two private helpers near the top of the file (before the existing fn impls but after the use declarations):

```rust
/// Returns the primary (post-migration) cache directory for a corpus.
///
/// Primary path: `<repo_root>/.vox/cache/vox-graph/<corpus_id>`.
fn primary_cache_dir(repo_root: &std::path::Path, corpus_id: &str) -> std::path::PathBuf {
    repo_root.join(".vox/cache/vox-graph").join(corpus_id)
}

/// Returns the resolved cache directory for `corpus_id`, with one-release back-compat.
///
/// Resolution order:
/// 1. If `<repo_root>/.vox/cache/vox-graph/<corpus_id>` exists → return it.
/// 2. If `<repo_root>/.vox/cache/graphify/<corpus_id>` exists → print a migration notice
///    and return the legacy path. (The caller is responsible for migrating files if desired;
///    this function only resolves the path, it does not move data.)
/// 3. Otherwise → return the new primary path (callers will create it on first write).
///
/// **One-release contract:** the legacy fallback is removed in the release after VG-1 ships.
fn cache_dir_with_migration(repo_root: &std::path::Path, corpus_id: &str) -> std::path::PathBuf {
    let new_path = primary_cache_dir(repo_root, corpus_id);
    if new_path.exists() {
        return new_path;
    }
    let legacy = repo_root.join(".vox/cache/graphify").join(corpus_id);
    if legacy.exists() {
        eprintln!(
            "[vox-graph] INFO: cache at legacy path '.vox/cache/graphify/{corpus_id}' — \
             run `vox search rebuild --corpus {corpus_id}` to migrate to \
             '.vox/cache/vox-graph/{corpus_id}'. Legacy path support will be removed in the next release."
        );
        return legacy;
    }
    new_path
}
```

- [ ] **Step 3: Replace hardcoded path strings** — find every `.vox/cache/graphify/{corpus_id}` style string in `crates/vox-cli/src/commands/graphify/mod.rs` and replace with calls to `cache_dir_with_migration(repo_root, &corpus_id)` or `primary_cache_dir(repo_root, &corpus_id)` as appropriate. The read/rebuild paths use `cache_dir_with_migration`; new writes use `primary_cache_dir`. Also update the crate-map path:

```rust
// Before (line ~595):
let out_dir = repo_root.join(".vox/cache/graphify/crate-map");

// After:
let out_dir = primary_cache_dir(repo_root, "crate-map");
```

Also update the `Status` subcommand's `graph_path`/`manifest_path` format strings (lines ~453–455):

```rust
// Before:
graph_path: format!(".vox/cache/graphify/{corpus_id}/graph.json"),
// …
".vox/cache/graphify/{corpus_id}/.graphify_manifest.v1.json"

// After:
graph_path: format!("{}", cache_dir_with_migration(repo_root, &corpus_id).join("graph.json").display()),
// …
format!("{}", cache_dir_with_migration(repo_root, &corpus_id).join(".graphify_manifest.v1.json").display()),
```

- [ ] **Step 4: Update inline test fixtures** — the existing tests at lines ~706–751 construct paths to `.vox/cache/graphify/`. Update them to `.vox/cache/vox-graph/`:

```rust
// Before (~line 710):
let graph_dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
// After:
let graph_dir = tmp.path().join(".vox/cache/vox-graph/repo-code-graph");
```

(And similarly for the second fixture at ~line 726.)

- [ ] **Step 5: Run tests**:

```bash
cargo test -p vox-cli 2>&1 | grep "vg1_cache_path_tests\|FAILED\|ok" | head -20
```

All three `vg1_cache_path_tests::*` tests must pass.

- [ ] **Step 6: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-cli/src/commands/graphify/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G3): migrate .vox/cache/graphify/ → .vox/cache/vox-graph/ with one-release fallback (TDD)"
```

---

# PHASE D — Contracts + skill

## Task G4: Rename `graphify-corpora.v1.yaml` → `vox-graph-corpora.v1.yaml` + update CORPORA_REL_PATH [PARALLEL-SAFE with A, B]

**Files:** `contracts/retrieval/graphify-corpora.v1.yaml` (rename), `crates/vox-config/src/graphify.rs`, inline test fixtures in `crates/vox-cli/src/commands/graphify/mod.rs` (the `include_str!` lines).

- [ ] **Step 1: Failing test** — add to `crates/vox-config/src/graphify.rs` inline tests:

```rust
#[cfg(test)]
mod vg1_corpora_path_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn loads_from_vox_graph_corpora_path() {
        let tmp = TempDir::new().unwrap();
        // Write to the NEW path only — must load without fallback.
        let new_path = tmp.path().join("contracts/retrieval/vox-graph-corpora.v1.yaml");
        std::fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        std::fs::write(
            &new_path,
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        ).unwrap();
        let result = load_graphify_corpora(tmp.path());
        assert!(result.is_ok(), "must load from new path: {:?}", result);
    }

    #[test]
    fn falls_back_to_legacy_graphify_corpora() {
        let tmp = TempDir::new().unwrap();
        // Write ONLY to the legacy path — fallback must find it.
        let legacy_path = tmp.path().join("contracts/retrieval/graphify-corpora.v1.yaml");
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy_path,
            include_str!("../../../contracts/retrieval/vox-graph-corpora.v1.yaml"),
        ).unwrap();
        let result = load_graphify_corpora(tmp.path());
        assert!(result.is_ok(), "must fall back to legacy graphify-corpora.v1.yaml: {:?}", result);
    }
}
```

Run — expect the first test to fail (file doesn't exist yet under new name) and the second to pass:

```bash
cargo test -p vox-config 2>&1 | grep "vg1_corpora_path_tests\|FAILED\|ok" | head -10
```

- [ ] **Step 2: Rename the file on disk** (git mv):

```bash
git -C /c/Users/Owner/vox-graphify-gui mv \
  contracts/retrieval/graphify-corpora.v1.yaml \
  contracts/retrieval/vox-graph-corpora.v1.yaml
```

- [ ] **Step 3: Update `CORPORA_REL_PATH`** in `crates/vox-config/src/graphify.rs`:

```rust
// Before:
pub const CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml";

// After:
pub const CORPORA_REL_PATH: &str = "contracts/retrieval/vox-graph-corpora.v1.yaml";

/// One-release legacy path for back-compat migration.
const LEGACY_CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml";
```

- [ ] **Step 4: Add fallback in `load_graphify_corpora`**. In `crates/vox-config/src/graphify.rs`, update `load_graphify_corpora` (line ~153):

```rust
pub fn load_graphify_corpora(repo_root: &Path) -> Result<GraphifyCorporaRegistry, GraphifyError> {
    let path = repo_root.join(CORPORA_REL_PATH);
    // One-release back-compat: if new path is absent, try the legacy name.
    let path = if !path.exists() {
        let legacy = repo_root.join(LEGACY_CORPORA_REL_PATH);
        if legacy.exists() {
            legacy
        } else {
            path // will fail with a clear Io error for the new path
        }
    } else {
        path
    };
    let raw = std::fs::read_to_string(&path).map_err(|source| GraphifyError::Io {
        path: path.clone(),
        source,
    })?;
    let file: CorporaFile = serde_yaml::from_str(&raw).map_err(|e| GraphifyError::Parse {
        path: path.clone(),
        detail: e.to_string(),
    })?;
    Ok(GraphifyCorporaRegistry {
        default_corpus_id: file.default_corpus_id,
        ttl_days_default: file.ttl_days_default,
        corpora: file.corpora,
    })
}
```

- [ ] **Step 4b: Migrate `REGISTERED_REL_PATH`** (the second legacy `graphify/` cache path — #6). In `crates/vox-config/src/graphify.rs:16`:

```rust
// Before:
pub const REGISTERED_REL_PATH: &str = ".vox/cache/graphify/registered.v1.json";

// After:
pub const REGISTERED_REL_PATH: &str = ".vox/cache/vox-graph/registered.v1.json";

/// One-release legacy path for the registry overlay back-compat.
const LEGACY_REGISTERED_REL_PATH: &str = ".vox/cache/graphify/registered.v1.json";
```

Add the same fallback to `load_registered_corpora` (read legacy if new path absent) so the registry overlay does not split across two cache dirs. `upsert_registered_corpus` **writes** the new path. Add a TDD test mirroring the corpora fallback (new-preferred / legacy-fallback). If the registry overlay must intentionally stay at the legacy path, document why here instead — but the default is to migrate it.

- [ ] **Step 5: Update `include_str!` fixtures** in `crates/vox-cli/src/commands/graphify/mod.rs` (lines ~675–688 use `include_str!("../../../../../contracts/retrieval/graphify-corpora.v1.yaml")`) → change to `vox-graph-corpora.v1.yaml`.

- [ ] **Step 6: Run tests**:

```bash
cargo test -p vox-config 2>&1 | grep "vg1_corpora_path_tests\|FAILED\|ok" | head -10
cargo test -p vox-cli 2>&1 | grep "corpora\|FAILED\|ok" | head -10
```

Both new tests must pass; existing corpora tests must still pass.

- [ ] **Step 7: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  contracts/retrieval/vox-graph-corpora.v1.yaml \
  crates/vox-config/src/graphify.rs \
  crates/vox-cli/src/commands/graphify/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G4): rename graphify-corpora.v1.yaml → vox-graph-corpora.v1.yaml + CORPORA_REL_PATH/REGISTERED_REL_PATH + fallbacks (TDD)"
```

## Task G5: Add pinned `vox-graph` skill [PARALLEL-SAFE with A, B, G4]

A concise SKILL.md that tells every harness to call `vox_search`/`vox_discover` before grep.

**Files:** `assets/skills/vox-graph/SKILL.md` (new).

- [ ] **Step 1: Create the skill**. Create `assets/skills/vox-graph/SKILL.md`:

```markdown
---
name: vox-graph
description: "Vox Graph structural discovery: call vox_search / vox_discover BEFORE grep. Use this whenever tracing how code connects — call sites, surface coverage, dead-end commands, or 'what relates to X'."
---

# Vox Graph — Graph-First Discovery

The Vox Graph engine indexes the codebase as a structural graph (fn, struct, surface, command, tool nodes + call/dependency edges). It is faster and more precise than text search for connection questions.

## When to use this skill

- **"Where is X called?"** — `vox_search` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_discover` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_discover` with seed of Z's node id → returns neighbors + their coverage class.
- **"Which commands have no GUI surface?"** — query the `CliOnly` coverage class from the graph's coverage report.

## Rule

**ALWAYS call `vox_search` or `vox_discover` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).

## Key MCP tools

| Tool | Purpose |
|---|---|
| `vox_search_query` | Text-or-id search across corpora |
| `vox_search_path` | Shortest path between two node ids |
| `vox_discover` | Graph-augmented retrieval: seed → expand → composite rank |
| `vox_search_status` | Corpus freshness + coverage summary |

## Graph verbs (CLI)

```
vox search rebuild --corpus <id>   # rebuild the structural graph
vox search status                  # freshness report
vox search index                   # re-index after code change
```

*(There is no `graph` infix — `vox search <verb>` IS the graph subgroup, per vs1. See G6.)*

## Determinism firewall

The structural graph is deterministic and read-only. Semantic overlays (embedding, LLM relation labels) are query-time and provenance-labeled — never written into the graph itself. You can trust graph results as ground truth for the codebase structure at the time of the last rebuild.
```

- [ ] **Step 2: Verify the file is well-formed** — read it back:

```bash
cat /c/Users/Owner/vox-graphify-gui/assets/skills/vox-graph/SKILL.md | head -5
```

- [ ] **Step 3: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add assets/skills/vox-graph/SKILL.md
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G5): add pinned vox-graph skill (graph-first discovery before grep)"
```

---

# PHASE E — CLI verb-path reconciliation

## Task G6: Reconcile `vox search <verb>` as the canonical graph subgroup (no `graph` infix) [SEQUENTIAL after G1, G3, and vs1's Cli::Search]

> **Resolution of the `graph`-infix collision (X3/#3, Critical).** The original VG-1 design proposed `vox search graph <verb>`. But vs1 **already** makes `vox search rebuild`/`status`/`index`/… leaf verbs (the seven `GraphifyCmd` variants ride unchanged under the renamed `Cli::Search` group). Adding a `graph` infix would yield `vox search graph rebuild` **duplicating** `vox search rebuild` with no canonical winner — and spec §1 ("folded into `vox search graph <verb>`") would contradict vs1 ("`vox search <verb>`"). **Decision: drop the `graph` infix entirely.** vs1's `vox search <verb>` **is** the graph subgroup; there is no second parallel surface. This makes G6 a **reconciliation/no-op task** (verify + doc-regen), not a code-adding task. Spec §1 must be reconciled to read `vox search <verb>` — that edit is the reconcile/spec phase's job, flagged here, not done in this plan.

This task depends on vs1 having renamed `Cli::Graphify` → `Cli::Search`. If vs1 has not landed, **hold this task** until vs1's T4 commit is in. **There is no `SearchCmd` enum** — vs1 keeps the inner enum named `GraphifyCmd` (verified G0 Step 1c).

**Files:** `crates/vox-cli/src/commands/graphify/mod.rs` (read-only verify), `docs/src/reference/cli.md` (regenerated, not hand-edited).

- [ ] **Step 1: Verify vs1 landed** — `Cli::Search` must exist and `Cli::Graphify` must be absent (or alias-only):

```bash
grep -n "Search\|Graphify" /c/Users/Owner/vox-graphify-gui/crates/vox-cli/src/lib.rs | head -10
```

Expected: `Cli::Search { … cmd: commands::graphify::GraphifyCmd }` present; `Cli::Graphify` absent or `#[command(alias = "graphify")]` only. If `Cli::Graphify` is still primary, **stop and wait for vs1**.

- [ ] **Step 2: Confirm the canonical verb path** — the seven graph verbs are already reachable as `vox search <verb>` via the unchanged `GraphifyCmd` enum. Confirm no `graph` infix is needed:

```bash
grep -n "enum GraphifyCmd\|fn run(cmd: GraphifyCmd\|fn run(cmd" \
  /c/Users/Owner/vox-graphify-gui/crates/vox-cli/src/commands/graphify/mod.rs | head
# expected: enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }
#           pub async fn run(cmd: GraphifyCmd, repo_root) -> anyhow::Result<()>
```

`vox search status`, `vox search rebuild --corpus <id>`, `vox search index`, `vox search refresh`, `vox search gc`, `vox search crate-map`, `vox search ingest` are the canonical verb paths after vs1. **No new enum, no new variant, no `Graph(GraphCmd)`.** The `vox-graph` skill (G5) already documents `vox search <verb>` — re-check G5's skill body uses `vox search rebuild` (NOT `vox search graph rebuild`).

- [ ] **Step 3: Regenerate CLI docs (NEVER hand-edit)** — vs1's verb rename changed `docs/src/reference/cli.md` (it references `graphify`/`vox search`). Re-run the cli.md generator so the doc reflects `vox search <verb>`:

```bash
grep -rn "cli.md\|gen.*cli\|cli.*gen" /c/Users/Owner/vox-graphify-gui/scripts/ /c/Users/Owner/vox-graphify-gui/.github/ 2>/dev/null | head
# run the discovered generator (e.g. `vox run scripts/<gen>.vox` or `vox ci docs`); do not edit cli.md by hand.
```

*If vs1 already regenerated cli.md as part of its T4, this step is a verify-only no-op — confirm cli.md shows `vox search <verb>` and commit nothing.*

- [ ] **Step 4: Commit (only if cli.md changed)** — if the regen produced a diff vs1 didn't already land:

```bash
git -C /c/Users/Owner/vox-graphify-gui add docs/src/reference/cli.md
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "docs(vg1-G6): reconcile CLI verb path to vox search <verb> (no graph infix); regen cli.md"
```

If there is no diff (vs1 owns the verb path and its docs), **commit nothing** — G6 is a verify-only reconciliation. Record in the close report that G6 was a no-op.

---

# PHASE F — Content-manifest emitter

## Task G7: Implement `emit_content_manifest` [SEQUENTIAL after G2]

Implement the full content-manifest emitter in `manifest.rs`.

**Files:** `crates/vox-graph-reader/src/manifest.rs`, `crates/vox-graph-reader/src/lib.rs`.

The function reads the graph JSON, joins with the surface-registry YAML (a simple line scan — no full serde_yaml dep in the reader; the YAML format is regular enough for a targeted parse), scans TSX files for headings, collects `cmd:` neighbor ids from the graph edges, and writes `gui-content-manifest.json`.

- [ ] **Step 1: Failing golden test** — add to `crates/vox-graph-reader/tests/manifest_tests.rs`:

```rust
use vox_graph_reader::manifest::emit_content_manifest;
use std::path::Path;
use tempfile::TempDir;

/// Fixture A: graph JSON using the PRODUCTION edge key `"links"` (rebuild.rs:333 emits this).
/// Also includes a surface→module edge (surface:approvals → module:ApprovalsView) so the
/// heading join can key off the graph edge, not a filename guess.
const FIXTURE_GRAPH_LINKS: &str = r#"{
  "nodes": [
    { "id": "surface:approvals", "label": "approvals", "kind": "surface" },
    { "id": "cmd:vox_resolve_approval", "label": "vox_resolve_approval", "kind": "command" },
    { "id": "module:components/surfaces/Approvals/ApprovalsView.tsx", "label": "ApprovalsView", "kind": "module" }
  ],
  "links": [
    { "source": "surface:approvals", "target": "cmd:vox_resolve_approval", "confidence": "declared" },
    { "source": "surface:approvals", "target": "module:components/surfaces/Approvals/ApprovalsView.tsx", "confidence": "declared" }
  ]
}"#;

/// Fixture B: the SAME graph but using the legacy `"edges"` key — the emitter must read
/// `links` with an `edges` fallback (mirrors coverage.rs:57-58 / lens.rs:19-20). Both shapes
/// must yield identical manifests.
const FIXTURE_GRAPH_EDGES: &str = r#"{
  "nodes": [
    { "id": "surface:approvals", "label": "approvals", "kind": "surface" },
    { "id": "cmd:vox_resolve_approval", "label": "vox_resolve_approval", "kind": "command" }
  ],
  "edges": [
    { "source": "surface:approvals", "target": "cmd:vox_resolve_approval", "confidence": "declared" }
  ]
}"#;

/// Fixture C: a MULTI-WORD kebab view key (`sub-agents`) whose component is PascalCase
/// (`SubAgentsView.tsx`) — exercises the X5 join (must NOT mis-key via filename heuristic).
const FIXTURE_GRAPH_MULTIWORD: &str = r#"{
  "nodes": [
    { "id": "surface:sub-agents", "label": "sub-agents", "kind": "surface" },
    { "id": "module:components/surfaces/SubAgents/SubAgentsView.tsx", "label": "SubAgentsView", "kind": "module" }
  ],
  "links": [
    { "source": "surface:sub-agents", "target": "module:components/surfaces/SubAgents/SubAgentsView.tsx", "confidence": "declared" }
  ]
}"#;

/// Fixture: a minimal surface-registry YAML with one surface (approvals).
const FIXTURE_REGISTRY_YAML: &str = r#"x_vox_version: 2
schema_version: 1
surfaces:
- view_key: approvals
  cli_group: null
  representation_tier: live_backend
  nav_label: Approvals
  nav_icon: shield
  nav_group: operate
  parent_surface: runs
  notes: Operator approval queue for doubt_task feedback
"#;

/// Helper: emit + parse the manifest, return the `surfaces` array.
fn emit_and_parse(graph: &str, yaml: &str, surface_dir: &Path) -> serde_json::Value {
    let tmp = TempDir::new().unwrap();
    let out_path = tmp.path().join("gui-content-manifest.json");
    emit_content_manifest(graph, yaml, surface_dir, &out_path)
        .expect("emit_content_manifest must not error on valid fixture");
    let raw = std::fs::read_to_string(&out_path).expect("manifest file must be written");
    serde_json::from_str(&raw).expect("manifest must be valid JSON")
}

#[test]
fn manifest_golden_surface_approvals() {
    // surface_dir: empty (no TSX files) — headings will be []; commands come from graph edges.
    let surface_dir = TempDir::new().unwrap();
    let manifest = emit_and_parse(FIXTURE_GRAPH_LINKS, FIXTURE_REGISTRY_YAML, surface_dir.path());

    let surfaces = manifest["surfaces"].as_array().expect("must have a 'surfaces' array");
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("approvals"))
        .expect("approvals must appear in the manifest");

    // nav_label extracted from YAML
    assert_eq!(entry["nav_label"].as_str(), Some("Approvals"));
    // route derived as #view=<view_key>
    assert_eq!(entry["route"].as_str(), Some("#view=approvals"));
    // nav_group extracted from YAML
    assert_eq!(entry["nav_group"].as_str(), Some("operate"));
    // X11: docs field present (empty for VG-1) so VG-2's ContentManifestEntry.docs matches.
    assert!(entry["docs"].is_array(), "docs field must be present (even if empty)");
    // commands: must include vox_resolve_approval (from the cmd: edge under the "links" key)
    let commands = entry["commands"].as_array().expect("must have commands array");
    assert!(
        commands.iter().any(|c| c.as_str() == Some("vox_resolve_approval")),
        "commands must include vox_resolve_approval (from graph 'links' edge); got: {:?}",
        commands
    );
}

/// X1 (Critical): the legacy `"edges"` key must produce the SAME commands as `"links"`.
/// Guards against the production-empty-commands regression.
#[test]
fn manifest_reads_edges_key_as_fallback() {
    let empty_dir = TempDir::new().unwrap();
    let manifest = emit_and_parse(FIXTURE_GRAPH_EDGES, FIXTURE_REGISTRY_YAML, empty_dir.path());
    let surfaces = manifest["surfaces"].as_array().unwrap();
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("approvals"))
        .expect("approvals must appear (edges-key fixture)");
    let commands = entry["commands"].as_array().expect("commands array");
    assert!(
        commands.iter().any(|c| c.as_str() == Some("vox_resolve_approval")),
        "commands must be non-empty when edges live under the legacy 'edges' key; got: {:?}",
        commands
    );
}

/// X5: a multi-word kebab view key (`sub-agents`) with a PascalCase component
/// (`SubAgentsView.tsx`) must join headings via the graph surface→module edge — NOT a
/// filename heuristic (which would mis-key `subagents` ≠ `sub-agents`).
#[test]
fn manifest_headings_multiword_view_key() {
    // Lay out a real-ish component file under a surface_dir, reachable via the graph edge.
    let dir = TempDir::new().unwrap();
    let comp = dir.path().join("components/surfaces/SubAgents/SubAgentsView.tsx");
    std::fs::create_dir_all(comp.parent().unwrap()).unwrap();
    std::fs::write(&comp, "export function SubAgentsView() {\n  return <section><h2>Sub-Agent Roster</h2></section>;\n}\n").unwrap();

    const YAML: &str = "x_vox_version: 2\nschema_version: 1\nsurfaces:\n- view_key: sub-agents\n  nav_label: Sub-Agents\n  nav_group: operate\n";

    let manifest = emit_and_parse(FIXTURE_GRAPH_MULTIWORD, YAML, dir.path());
    let surfaces = manifest["surfaces"].as_array().unwrap();
    let entry = surfaces
        .iter()
        .find(|s| s["view_key"].as_str() == Some("sub-agents"))
        .expect("sub-agents (multi-word kebab) must appear in the manifest");
    let headings = entry["headings"].as_array().expect("headings array");
    assert!(
        headings.iter().any(|h| h.as_str() == Some("Sub-Agent Roster")),
        "heading must join via the graph surface→module edge for a multi-word key; got: {:?}",
        headings
    );
}

#[test]
fn manifest_module_exists() {
    let _ = emit_content_manifest as fn(&str, &str, &Path, &Path) -> Result<(), Box<dyn std::error::Error>>;
}
```

Run — expect failure because the function returns `Err("not yet implemented")`:

```bash
cargo test -p vox-graph-reader --test manifest_tests 2>&1 | tail -15
```

- [ ] **Step 2: Implement `emit_content_manifest`**. Replace the stub in `crates/vox-graph-reader/src/manifest.rs` with the full implementation:

```rust
//! GUI content-manifest emitter.
//!
//! Emits `gui-content-manifest.json` from the structural graph + surface-registry YAML.
//! Called by `rebuild::rebuild_graph` in gui-wiring mode after the graph is written.
//!
//! Output format:
//! ```json
//! {
//!   "schema_version": 1,
//!   "surfaces": [
//!     {
//!       "view_key": "approvals",
//!       "nav_label": "Approvals",
//!       "nav_group": "operate",
//!       "route": "#view=approvals",
//!       "headings": [],
//!       "commands": ["vox_resolve_approval"],
//!       "notes": "Operator approval queue for doubt_task feedback",
//!       "docs": []
//!     }
//!   ]
//! }
//! ```

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

/// Emit `gui-content-manifest.json` to `out_path`.
///
/// # Parameters
/// - `graph_json` — the `graph.json` content string.
/// - `surface_registry_yaml` — contents of `contracts/gui/surface-registry.v1.yaml`.
/// - `surface_dir` — the GUI `ui/src/` directory to scan for TSX heading text.
/// - `out_path` — destination file path.
pub fn emit_content_manifest(
    graph_json: &str,
    surface_registry_yaml: &str,
    surface_dir: &Path,
    out_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let graph: Value = serde_json::from_str(graph_json)?;

    // 1. Collect surface node ids from the graph.
    let surface_ids: HashSet<String> = graph["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["kind"].as_str() == Some("surface"))
        .filter_map(|n| {
            let id = n["id"].as_str()?;
            let view_key = id.strip_prefix("surface:")?;
            Some(view_key.to_string())
        })
        .collect();

    // 2. Build edge maps from the graph.
    //    X1: the PRODUCTION writer emits the edge array under "links" (rebuild.rs:333);
    //    read "links" first, fall back to "edges" (mirrors coverage.rs:57-58 / lens.rs:19-20).
    //    Keying only on "edges" would make every surface's `commands` silently empty in prod.
    let edges = graph["links"]
        .as_array()
        .or_else(|| graph["edges"].as_array());

    // 2a. surface view_key → set of cmd: neighbor names.
    let mut cmd_neighbors: HashMap<String, Vec<String>> = HashMap::new();
    // 2b. surface view_key → component module path (for the X5 heading join — key off the
    //     graph surface→module edge, NOT a filename heuristic).
    let mut module_by_surface: HashMap<String, String> = HashMap::new();
    if let Some(edges) = edges {
        for edge in edges {
            let src = edge["source"].as_str().unwrap_or("");
            let tgt = edge["target"].as_str().unwrap_or("");
            if let Some(view_key) = src.strip_prefix("surface:") {
                if let Some(cmd_name) = tgt.strip_prefix("cmd:") {
                    cmd_neighbors
                        .entry(view_key.to_string())
                        .or_default()
                        .push(cmd_name.to_string());
                } else if let Some(module_path) = tgt.strip_prefix("module:") {
                    // First module edge wins (deterministic: pick the lexically smallest).
                    module_by_surface
                        .entry(view_key.to_string())
                        .and_modify(|existing| {
                            if module_path < existing.as_str() {
                                *existing = module_path.to_string();
                            }
                        })
                        .or_insert_with(|| module_path.to_string());
                }
            }
        }
    }
    // Dedup and sort commands for determinism.
    for v in cmd_neighbors.values_mut() {
        v.sort();
        v.dedup();
    }

    // 3. Parse surface-registry YAML for label/group/notes per surface.
    //    We use a targeted line scan (no serde_yaml dep in this crate).
    let registry_meta = parse_surface_registry_yaml(surface_registry_yaml);

    // 4. Scan headings per surface by resolving the graph's surface→module edge to a file
    //    under `surface_dir` (X5 — no filename guessing).
    let headings_by_surface = scan_surface_headings(surface_dir, &module_by_surface);

    // 5. Build manifest entries for surfaces that appear in both the graph and the YAML.
    let mut surfaces_out: Vec<Value> = surface_ids
        .iter()
        .map(|view_key| {
            let meta = registry_meta.get(view_key);
            let nav_label = meta
                .and_then(|m| m.get("nav_label"))
                .cloned()
                .unwrap_or_else(|| view_key.clone());
            let nav_group = meta
                .and_then(|m| m.get("nav_group"))
                .cloned()
                .unwrap_or_default();
            let notes = meta
                .and_then(|m| m.get("notes"))
                .cloned()
                .unwrap_or_default();
            let route = format!("#view={view_key}");
            let commands = cmd_neighbors
                .get(view_key)
                .cloned()
                .unwrap_or_default();
            let headings = headings_by_surface
                .get(view_key)
                .cloned()
                .unwrap_or_default();
            serde_json::json!({
                "view_key": view_key,
                "nav_label": nav_label,
                "nav_group": nav_group,
                "route": route,
                "headings": headings,
                "commands": commands,
                "notes": notes,
                // X11: empty for VG-1; present so VG-2's ContentManifestEntry.docs type matches.
                "docs": Vec::<String>::new(),
            })
        })
        .collect();

    // Sort by view_key for determinism.
    surfaces_out.sort_by(|a, b| {
        a["view_key"]
            .as_str()
            .cmp(&b["view_key"].as_str())
    });

    let manifest = serde_json::json!({
        "schema_version": 1,
        "surfaces": surfaces_out,
    });

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

/// Targeted YAML scanner for the surface-registry.
/// Returns a map of view_key → { nav_label, nav_group, notes }.
/// Uses line-by-line parsing (no serde_yaml dep) since the YAML format is regular.
fn parse_surface_registry_yaml(yaml: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current_view_key: Option<String> = None;

    for line in yaml.lines() {
        let trimmed = line.trim();

        if let Some(val) = trimmed.strip_prefix("- view_key:") {
            let vk = val.trim().trim_matches('\'').trim_matches('"').to_string();
            if !vk.is_empty() && vk != "null" {
                current_view_key = Some(vk.clone());
                out.entry(vk).or_default();
            } else {
                current_view_key = None;
            }
            continue;
        }

        let Some(ref vk) = current_view_key else {
            continue;
        };

        for field in &["nav_label", "nav_group", "notes"] {
            let prefix = format!("{field}:");
            if let Some(val) = trimmed.strip_prefix(prefix.as_str()) {
                let v = val.trim().trim_matches('\'').trim_matches('"').to_string();
                if v != "null" && !v.is_empty() {
                    out.entry(vk.clone()).or_default().insert(field.to_string(), v);
                }
            }
        }
    }
    out
}

/// Scan surface component files for heading text — keyed by the graph's surface→module edge.
///
/// X5: `module_by_surface` maps `view_key → component module path` (taken from the graph's
/// `surface:<vk> → module:<path>` edge). We resolve each module path to a real file under
/// `surface_dir` and extract headings from it. This avoids the filename heuristic that
/// mis-keys multi-word kebab view keys (`SubAgentsView.tsx → "subagents" ≠ "sub-agents"`).
///
/// A surface with no module edge (or whose module file is missing) gets NO headings entry —
/// the caller emits `headings: []` (honest empty, not a wrong best-effort match).
///
/// Returns a map of view_key → sorted deduplicated heading strings.
fn scan_surface_headings(
    surface_dir: &Path,
    module_by_surface: &HashMap<String, String>,
) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();

    for (view_key, module_path) in module_by_surface {
        // The module id is a repo-relative-ish path (e.g.
        // "components/surfaces/SubAgents/SubAgentsView.tsx"). Strip any "module:" prefix
        // already removed by the caller. Resolve against surface_dir; also try matching by
        // the basename in case the graph stores a different path root than surface_dir.
        let candidate = surface_dir.join(module_path);
        let resolved = if candidate.is_file() {
            Some(candidate)
        } else {
            // Fallback: walk surface_dir for a file whose path ends with the module path's
            // basename. Deterministic: pick the lexically smallest match.
            let base = std::path::Path::new(module_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if base.is_empty() {
                None
            } else {
                walkdir::WalkDir::new(surface_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                    .map(|e| e.path().to_path_buf())
                    .filter(|p| {
                        p.file_name().and_then(|s| s.to_str()) == Some(base)
                    })
                    .min()
            }
        };

        let Some(path) = resolved else { continue };
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let headings = extract_headings(&src);
        if !headings.is_empty() {
            out.entry(view_key.clone()).or_default().extend(headings);
        }
    }

    // Sort + dedup each surface's headings for determinism.
    for v in out.values_mut() {
        v.sort();
        v.dedup();
    }
    out
}

/// Extract heading text from a TSX source string.
/// Matches `<h1>…</h1>` through `<h6>…</h6>` and simple `aria-label="…"` attributes.
fn extract_headings(src: &str) -> Vec<String> {
    let mut out = Vec::new();

    // h1–h6 tags: <h1>text</h1> — only matches single-line, simple text content.
    let heading_re_str = r"<h[1-6][^>]*>([^<]{1,200})</h[1-6]>";
    // aria-label="…"
    let aria_re_str = r#"aria-label=["']([^"']{1,200})["']"#;

    for pattern in &[heading_re_str, aria_re_str] {
        // Simple finite-state scan (no regex crate dep — use a manual scan).
        let tag_open = if pattern.starts_with("<h") { "<h" } else { "aria-label=" };
        let mut rest = src;
        while let Some(start) = rest.find(tag_open) {
            let after = &rest[start..];
            // Find the closing > for the opening tag.
            if let Some(close_bracket) = after.find('>') {
                let after_open = &after[close_bracket + 1..];
                // Find the end of content (< for h-tags, or the closing quote for aria-label).
                let end_marker = if tag_open == "<h" { "</" } else { "\n" };
                if let Some(end) = after_open.find(end_marker) {
                    let text = after_open[..end].trim().to_string();
                    // Filter out JSX expressions and very short/long text.
                    if !text.is_empty()
                        && !text.starts_with('{')
                        && text.len() <= 120
                        && text.is_ascii()
                    {
                        out.push(text);
                    }
                }
            }
            rest = &rest[start + tag_open.len()..];
        }
    }
    out
}
```

- [ ] **Step 3: Run the golden test**:

```bash
cargo test -p vox-graph-reader --test manifest_tests 2>&1 | tail -20
```

Both `manifest_golden_surface_approvals` and `manifest_module_exists` must pass.

- [ ] **Step 4: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-graph-reader/src/manifest.rs \
  crates/vox-graph-reader/tests/manifest_tests.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G7): implement emit_content_manifest — gui-content-manifest.json emitter (golden TDD)"
```

## Task G8: Wire manifest emit into `rebuild_graph` in gui-wiring mode [SEQUENTIAL after G7]

**Files:** `crates/vox-graph-reader/src/rebuild.rs`, `crates/vox-cli/src/commands/graphify/mod.rs` (to pass the registry YAML and surface_dir).

- [ ] **Step 1: Add surface_dir to RebuildMeta**. In `crates/vox-graph-reader/src/rebuild.rs`, add a field to `RebuildMeta`:

```rust
#[derive(Debug, Clone, Default)]
pub struct RebuildMeta {
    pub corpus_id: String,
    pub git_sha: Option<String>,
    pub scope_path: String,
    pub extraction_mode: Option<String>,
    pub built_at_rfc3339: String,
    /// Path to `gui/ui/src/` for heading scans (manifest emit; only used in gui-wiring mode).
    pub gui_source_dir: Option<std::path::PathBuf>,
    /// Contents of `contracts/gui/surface-registry.v1.yaml` (manifest emit).
    pub surface_registry_yaml: Option<String>,
    /// Optional serialised CLI catalog JSON for cli: node ingestion (vs1 T5/T6 field — do not remove).
    pub cli_catalog_json: Option<String>,
}
```

*Note: `cli_catalog_json` is the vs1 T5/T6 field — preserve it; do not clobber vs1's addition.*

- [ ] **Step 2: Call `emit_content_manifest` in `rebuild_graph`**. In the `if gui_wiring { … }` block at the end of `rebuild_graph` (after the final graph object `{"nodes": nodes_val, "links": links_val}` is built at lines 332-333), add:

```rust
// Emit the GUI content manifest alongside the graph (gui-wiring mode only).
if let (Some(ref registry_yaml), Some(ref surface_dir)) = (
    &meta.surface_registry_yaml,
    &meta.gui_source_dir,
) {
    // IMPORTANT: pass the FULL graph object, which keys edges under "links" (NOT just
    // nodes_val). `final_graph` is the `{"nodes": …, "links": …}` value already built for
    // the graph.json write; serialize it so the manifest emitter sees the "links" array.
    let graph_str = serde_json::to_string(&final_graph)?;
    let manifest_out = output_file
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("gui-content-manifest.json");
    if let Err(e) = crate::manifest::emit_content_manifest(
        &graph_str,
        registry_yaml,
        surface_dir.as_path(),
        &manifest_out,
    ) {
        eprintln!("[vox-graph] WARN: content manifest emit failed: {e}");
        // Non-fatal: the graph is still written; manifest is optional for the Omnibar.
    }
}
```

*Note: `final_graph` is the `serde_json::Value` built at lines 332-333 (`json!({"nodes": nodes_val, "links": links_val})`). Pass **that** object (with its `"links"` key) — do **not** pass `nodes_val` alone, or the emitter sees no edges and every `commands` array is empty. If the variable in the real code is named differently, use whatever holds the `{"nodes", "links"}` object that gets written to `graph.json`.*

- [ ] **Step 3: Update callers in CLI** — in `crates/vox-cli/src/commands/graphify/mod.rs`, the three `RebuildMeta { … }` construction sites (the `Rebuild`, `Index`, and `CrateMap` subcommand arms) must pass the new fields when in gui-wiring mode:

```rust
// In the Rebuild arm, after building `meta`:
let meta = vox_graph_reader::rebuild::RebuildMeta {
    corpus_id: corpus_id.clone(),
    git_sha: /* existing */,
    scope_path: /* existing */,
    extraction_mode: /* existing */,
    built_at_rfc3339: /* existing */,
    // VG-1 additions:
    gui_source_dir: if extraction_mode.as_deref() == Some("gui-wiring") {
        Some(source_dir.join("ui/src"))
    } else {
        None
    },
    surface_registry_yaml: if extraction_mode.as_deref() == Some("gui-wiring") {
        std::fs::read_to_string(
            repo_root.join("contracts/gui/surface-registry.v1.yaml")
        ).ok()
    } else {
        None
    },
    cli_catalog_json: /* vs1 field, preserve as-is */,
};
```

- [ ] **Step 4: Compile + test**:

```bash
cargo build -p vox-graph-reader 2>&1 | tail -5
cargo test -p vox-graph-reader 2>&1 | tail -10
cargo build -p vox-cli 2>&1 | tail -5
```

- [ ] **Step 5: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-graph-reader/src/rebuild.rs \
  crates/vox-cli/src/commands/graphify/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G8): wire emit_content_manifest into rebuild_graph (gui-wiring mode); update RebuildMeta"
```

---

# PHASE G — GUI hook + panel rename

## Task G9: Rename `useGraphifyStatus.ts` → `useVoxGraphStatus.ts` [PARALLEL-SAFE with D, E]

**Files:** `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts`, `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts`, and any import sites.

- [ ] **Step 1: Locate all import sites**:

```bash
grep -rn "useGraphifyStatus\|GRAPHIFY_STATUS_QUERY_KEY" \
  /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui/src/ | grep -v ".test.ts"
```

- [ ] **Step 2: Create `useVoxGraphStatus.ts`** at `crates/vox-gui/ui/src/hooks/useVoxGraphStatus.ts`:

```ts
import { useQuery } from '@tanstack/react-query';
import { getGraphifyStatus } from '../transport';
import type { GraphifyStatusDto } from '../types/tauri';

// VG-1: renamed from useGraphifyStatus / GRAPHIFY_STATUS_QUERY_KEY.
// The transport seam (getGraphifyStatus → vox_search_status via invokeMcpTool) is
// retired in vs1; this file only renames the hook and query key.
// #7: copy ALL FOUR useQuery options from the real hook verbatim — dropping
// staleTime/refetchInterval is a polling-behavior regression hidden in a "pure rename".
export const VOX_GRAPH_STATUS_QUERY_KEY = ['vox-graph', 'status'];

export function useVoxGraphStatus() {
  return useQuery<GraphifyStatusDto, Error>({
    queryKey: VOX_GRAPH_STATUS_QUERY_KEY,
    queryFn: getGraphifyStatus,
    staleTime: 30_000,
    refetchInterval: 60_000,
  });
}
```

- [ ] **Step 3: Create `useVoxGraphStatus.test.ts`** at `crates/vox-gui/ui/src/hooks/useVoxGraphStatus.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { VOX_GRAPH_STATUS_QUERY_KEY } from './useVoxGraphStatus';

describe('useVoxGraphStatus', () => {
  it('exports the renamed query key', () => {
    expect(VOX_GRAPH_STATUS_QUERY_KEY).toEqual(['vox-graph', 'status']);
  });
});
```

Run:

```bash
pnpm -C /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui vitest run hooks/useVoxGraphStatus.test.ts 2>&1 | tail -10
```

- [ ] **Step 4: Delete old files** (or keep as re-exports for one release — keep for one release to avoid breaking vs1 if it imports them):

Add to `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` a re-export:

```ts
// VG-1: deprecated — use useVoxGraphStatus instead.
export { useVoxGraphStatus as useGraphifyStatus, VOX_GRAPH_STATUS_QUERY_KEY as GRAPHIFY_STATUS_QUERY_KEY } from './useVoxGraphStatus';
```

And update `useGraphifyStatus.test.ts` to import from `useVoxGraphStatus` instead.

- [ ] **Step 5: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/hooks/useVoxGraphStatus.ts \
  crates/vox-gui/ui/src/hooks/useVoxGraphStatus.test.ts \
  crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts \
  crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G9): rename useGraphifyStatus → useVoxGraphStatus (one-release re-export back-compat)"
```

## Task G10: Rename `GraphifyStatusPanel.tsx` → `VoxGraphStatusPanel.tsx` (BODY-PRESERVING) [PARALLEL-SAFE with D, E]

> **⚠ #4 (Critical) — do NOT author a replacement component.** The real `GraphifyStatusPanel.tsx` is **113 lines** of working corpus-health UI (per-corpus cards, fresh/stale badges, node/edge counts, stale-reason chips, a copyable rebuild-command block). A stub (`<pre>{JSON.stringify(data)}</pre>`) would **delete working UI** — a gui-honesty regression and data loss. G10 is a `git mv` + symbol/import rename only; the 113-line body is preserved byte-for-byte.
>
> **#8 — the panel test exists and must move in lockstep.** `GraphifyStatusPanel.test.tsx` (52 lines) `vi.mock`s `../../../hooks/useGraphifyStatus` (line 7) and imports `{ GraphifyStatusPanel } from './GraphifyStatusPanel'` (line 12). Renaming the hook breaks the mock path → test failure. The test is `git mv`d and its mock path + imports updated.

**Files:** `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx`, `…/GraphifyStatusPanel.test.tsx`, `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`.

- [ ] **Step 1: `git mv` the component (preserve history + body)**:

```bash
git -C /c/Users/Owner/vox-graphify-gui mv \
  crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx \
  crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx
```

- [ ] **Step 2: Edit only the symbol + hook import inside the moved file** — do NOT rewrite the body. Two edits:
  1. Line 2: `import { useGraphifyStatus } from '../../../hooks/useGraphifyStatus';` → `import { useVoxGraphStatus } from '../../../hooks/useVoxGraphStatus';`
  2. Line 4: `export function GraphifyStatusPanel() {` → `export function VoxGraphStatusPanel() {`
  3. Line 5: `const { … } = useGraphifyStatus();` → `= useVoxGraphStatus();`

  Leave the 113-line JSX body (cards, badges, the `vox graphify rebuild --corpus {c.corpus_id}` rebuild-command text, etc.) **unchanged**. The user-facing strings ("Graphify Corpus Health", "Loading graphify status…") are out of VG-1 scope — vs1 owns the user-facing `graphify → vox-search` copy change; do not touch them here.

- [ ] **Step 3: `git mv` the test + update its mock path/imports (#8)**:

```bash
git -C /c/Users/Owner/vox-graphify-gui mv \
  crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx \
  crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx
```

Then edit the moved test:
  1. Line 7: `vi.mock('../../../hooks/useGraphifyStatus', () => ({` → `vi.mock('../../../hooks/useVoxGraphStatus', () => ({`
  2. Lines 8-9 inside the mock factory: `useGraphifyStatus: () => mockUse(),` → `useVoxGraphStatus: () => mockUse(),` and `GRAPHIFY_STATUS_QUERY_KEY: ['graphify', 'status'],` → `VOX_GRAPH_STATUS_QUERY_KEY: ['vox-graph', 'status'],`
  3. Line 12: `import { GraphifyStatusPanel } from './GraphifyStatusPanel';` → `import { VoxGraphStatusPanel } from './VoxGraphStatusPanel';`
  4. `describe('GraphifyStatusPanel', …)` → `describe('VoxGraphStatusPanel', …)`; both `render(<GraphifyStatusPanel />)` → `render(<VoxGraphStatusPanel />)`.

  The body-assertions (`'Repo'`, `'Stale'`, `/graph_missing/`, `/vox graphify rebuild --corpus repo-code-graph/`, `/Loading graphify status/i`) reference the **preserved** panel body — leave them as-is.

- [ ] **Step 4: Update `surfaceComponents.tsx`** — import + component name only (surface key unchanged):

```tsx
// Before (~line 24):
import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel';
// After:
import { VoxGraphStatusPanel } from '../surfaces/VoxGraph/VoxGraphStatusPanel';

// Before (~line 115):
case 'graphify': return <GraphifyStatusPanel />;
// After — surface KEY 'graphify' is NOT changed here (vs1 owns that re-key; see Final surface-key table):
case 'graphify': return <VoxGraphStatusPanel />;
```

- [ ] **Step 5: Run the moved panel test + full vitest + honesty check**:

```bash
pnpm -C /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui vitest run components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx 2>&1 | tail -10
pnpm -C /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui vitest run 2>&1 | tail -15
```

The preserved 113-line component is real (no placeholder text) → no honesty-scanner regression.

- [ ] **Step 6: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx \
  crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx \
  crates/vox-gui/ui/src/components/surfaces/Graphify/ \
  crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "refactor(vg1-G10): git mv GraphifyStatusPanel(+test) → VoxGraphStatusPanel; preserve 113-line body; rename symbol/import/mock"
```

---

# PHASE H — Close

## Task G11: Full verification [SEQUENTIAL — terminal, no commit]

_Depends on G1–G10 all committed._

- [ ] **Step 1: Crate builds**:

```bash
cargo build -p vox-graph-reader 2>&1 | tail -5
cargo build -p vox-cli 2>&1 | tail -5
cargo build -p vox-orchestrator-mcp 2>&1 | tail -5
cargo build -p vox-config 2>&1 | tail -5
```

All must succeed.

- [ ] **Step 2: Crate tests**:

```bash
cargo test -p vox-graph-reader 2>&1 | tail -10
cargo test -p vox-cli 2>&1 | tail -10
cargo test -p vox-config 2>&1 | tail -10
```

All must pass (including `vg1_cache_path_tests::*`, `vg1_corpora_path_tests::*`, and `manifest_golden_surface_approvals`).

- [ ] **Step 3: GUI vitest**:

```bash
pnpm -C /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui vitest run 2>&1 | tail -15
```

804+ tests, 0 failures.

- [ ] **Step 4: Honesty gate**:

```bash
cargo run -p vox-cli -- ci gui-honesty 2>&1 | tail -10
```

Must exit 0. `VoxGraphStatusPanel.tsx` must not trigger honesty flags (real data sources, no placeholder text).

- [ ] **Step 5: Verify no stale `vox_graphify_reader::` references remain** (code + contracts + layers, excluding history docs):

```bash
grep -rn "vox_graphify_reader\|vox-graphify-reader" \
  /c/Users/Owner/vox-graphify-gui/crates/ \
  /c/Users/Owner/vox-graphify-gui/contracts/ \
  /c/Users/Owner/vox-graphify-gui/docs/src/architecture/layers.toml \
  --include="*.rs" --include="Cargo.toml" --include="*.json" --include="*.toml" \
  | grep -v "/target/" | grep -v "^Binary\|^grep"
```

Expected: 0 lines. (Plans/specs under `docs/superpowers/` legitimately cite the old name as history — do not rewrite them; they are excluded here.)

- [ ] **Step 5b: Verify the manifest reads the production `links` key** — guard against the X1 empty-commands regression:

```bash
grep -n '"links"\|"edges"' /c/Users/Owner/vox-graphify-gui/crates/vox-graph-reader/src/manifest.rs
# expected: reads graph["links"] with an or_else fallback to graph["edges"].
```

- [ ] **Step 5c: Verify crate-graph parity gate passes** (the regenerated contracts):

```bash
grep -rn "vox-graph-reader" /c/Users/Owner/vox-graphify-gui/contracts/ci/crate-graph.v1.json | head
# expected: vox-graph-reader present; vox-graphify-reader absent. Run the crate-graph CI check if available.
```

- [ ] **Step 6: Verify no stale `.vox/cache/graphify` hardcodes remain** (outside comments):

```bash
grep -rn "\.vox/cache/graphify" \
  /c/Users/Owner/vox-graphify-gui/crates/ \
  --include="*.rs" \
  | grep -v "//.*vox/cache/graphify\|eprintln\|INFO\|WARN\|legacy"
```

Expected: 0 lines (comments and migration-notice strings are exempt).

- [ ] **Step 7: Done.** Report all commit SHAs to the workflow runner.

---

## Workflow Batch Plan

| Batch | Tasks | Class | Depends on | Conflict surface | Dispatch |
|---|---|---|---|---|---|
| **G0** | G0 | [SEQUENTIAL] | — | read-only | 1 agent |
| **Batch A — crate rename** | G1 → G2 | [SEQUENTIAL] within A | G0 green | `crates/vox-graph-reader/`, `crates/vox-cli/`, `crates/vox-orchestrator-mcp/` Cargo + src | 1 agent in order |
| **Batch B — cache path** | G3 | [PARALLEL-SAFE] with A, C | G0 green | `graphify/mod.rs` path strings | 1 agent |
| **Batch C — contracts + skill** | G4, G5 | both [PARALLEL-SAFE] | G0 green | disjoint: `vox-config/`, `contracts/retrieval/`, `assets/skills/` | 2 agents concurrently |
| **Batch D — CLI verb-path reconcile** | G6 | [SEQUENTIAL] | G1 + G3 + vs1-T4 green | read-only verify + cli.md regen (often no-op) | 1 agent |
| **Batch E — manifest emitter** | G7 → G8 | [SEQUENTIAL] within E | G2 green | `manifest.rs`, `rebuild.rs` | 1 agent in order |
| **Batch F — GUI rename** | G9, G10 | both [PARALLEL-SAFE] | G0 green | `hooks/`, `surfaces/VoxGraph/`, `surfaceComponents.tsx` | 1–2 agents |
| **G close** | G11 | [SEQUENTIAL] terminal | all prior green | verify only — no commit | 1 agent |

**Parallelism summary:** Batches A, B, C, and F all start once G0 passes (4-way concurrent, disjoint file sets). Batch D waits for A+B+vs1-T4. Batch E waits for A (G2 stub). Batch G is the terminal gate.

---

## Self-review checklist

- ✅ **Extends vs1, no duplication:** VG-1 does not re-do the `Cli::Graphify → Cli::Search` or `vox_graphify_* → vox_search_*` renames (those are vs1 scope). VG-1 does the cache-path / crate-name / contract / GUI-hook renames + manifest emit; G6 is a verb-path reconciliation only.
- ✅ **No `graph` infix (X3/#3):** `vox search <verb>` IS the graph subgroup (vs1's unchanged `GraphifyCmd` enum); G6 drops the infix and is verify+regen, not code-adding. There is **no `SearchCmd` enum** (#2). Spec §1 reconcile flagged for the spec phase.
- ✅ **Edge key = `links` w/ `edges` fallback (X1/#1):** the production writer emits `"links"` (rebuild.rs:333); G7 reads `links` then `edges`; golden tests cover both shapes (no fabricated-empty `commands`).
- ✅ **Headings keyed by graph edge (X5/#11):** `scan_surface_headings` resolves the surface→module edge, not a filename guess; multi-word kebab golden case (`sub-agents`) asserts a real heading.
- ✅ **Panel rename is body-preserving (#4):** G10 `git mv`s the 113-line `GraphifyStatusPanel.tsx` and renames only the symbol/import — no stub, no data loss, no honesty regression.
- ✅ **Panel test moves in lockstep (#8):** G10 `git mv`s `GraphifyStatusPanel.test.tsx`, updates the `vi.mock` path → `useVoxGraphStatus` and the render import.
- ✅ **Hook polling preserved (#7):** G9 copies all four `useQuery` options (incl. `staleTime: 30_000`, `refetchInterval: 60_000`).
- ✅ **orchestrator-mcp fully covered (#9):** `lib.rs:54` `pub mod` + the five `dispatch.rs:628-640` `crate::graphify_tools::` call sites are named and in the G1 add-list.
- ✅ **Crate rename reaches CI gates (#5):** `contracts/ci/crate-graph.v1.json` + `crate-build-map.v1.json` regenerated (not hand-edited); `layers.toml` (lines 170, 262) + `where-things-live.md` updated; G0 grep is repo-root.
- ✅ **Second cache path covered (#6):** `REGISTERED_REL_PATH` (graphify.rs:16) migrated in G4 Step 4b with the same one-release fallback.
- ✅ **`docs` field for type parity (X11):** manifest emits `docs: []` so VG-2's `ContentManifestEntry.docs` matches.
- ✅ **Base-branch + rebase precondition (X8):** Base branch note added (branch `claude/graphify-general-gui-ia`, rebase onto `063a3c3235`, consume seam `30a46cc88d`).
- ✅ **Final surface-key table (X7):** single ownership map (`graphify`→vs1, `search`→VG-2, `vox-search`→vs1) prevents double re-key.
- ✅ **Manifest vs omnisearch (#12):** Architecture note reconciles the content manifest against `omnisearch-index.v1.yaml` so VG-2 doesn't build a competing index.
- ✅ **G2 smoke is a compile gate, not red→green (X10):** wording fixed; the real red→green is G7's golden test.
- ✅ **Back-compat on cache path:** `cache_dir_with_migration`; tested in G3 (new preferred, legacy fallback, both→new wins). **Back-compat on corpora YAML:** `load_graphify_corpora` fallback; tested in G4.
- ✅ **`fs::read_to_string` matches file import style (#14):** graphify.rs imports `use std::fs;` — no `std::fs::` qualifier introduced.
- ✅ **No `cargo fmt --all`:** per-crate only. **No `vox ci gui-honesty` regression:** preserved real panel.
- ✅ **Plan format matches VG-2 style:** writing-plans header, TDD steps, [PARALLEL-SAFE]/[SEQUENTIAL] tags, Workflow Batch Plan table.
