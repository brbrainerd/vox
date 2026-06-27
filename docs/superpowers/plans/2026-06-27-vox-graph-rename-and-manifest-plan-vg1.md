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
2. Reconcile CLI verb nesting with vs1: graph-specific verbs (`rebuild`, `ingest`, `index`, `refresh`, `gc`, `crate-map`) live under `vox search graph <verb>` (the graph subgroup of the `vox search` group that vs1 creates). **Do not re-derive the vs1 rename here** — vs1 owns `Cli::Graphify → Cli::Search` and `#[command(alias = "graphify")]`; VG-1 only adds a `graph` subcommand group inside `Cli::Search` that forwards the existing seven verbs with a further `--` `#[command(alias)]` for the `vox search <verb>` short-form (one-release back-compat).
3. Pinned Vox skill `vox-graph` under `assets/skills/vox-graph/SKILL.md` steering agents to graph-first discovery via `vox_search`/`vox_discover` before grep.
4. New capability: `gui-content-manifest.json` emitted by the Vox Graph walk. Per surface: `view_key`, `nav_label`, `nav_group`, `route` (derived as `#view=<view_key>`), `headings` (extracted from the surface TSX component via a regex scan for `<h[1-6]` and `aria-label`), `commands` (all `cmd:` neighbor ids for the surface node from the graph), and optionally `notes` from the YAML registry. Golden test: add a fixture surface entry, assert its text appears in the manifest after a walk.

**VG-1 extends vs1** — it does NOT duplicate the external MCP tool rename (`vox_graphify_* → vox_search_*`) or the CLI top-level variant rename (`Cli::Graphify → Cli::Search`). Those are vs1's scope. VG-1's only CLI touch is the `graph` subgroup wire-up.

## Architecture

**No new engine.** The Vox Graph walk already produces the structural graph in `crates/vox-graphify-reader/src/rebuild.rs::rebuild_graph`. The manifest is a **post-walk emit**: after `rebuild_graph` writes `graph.json`, a new `emit_content_manifest` function reads the finished graph (nodes + edges), joins it with the surface-registry YAML, and writes `gui-content-manifest.json` alongside `graph.json` in the cache dir. The surface components are walked once for headings (a lightweight regex scan, not tree-sitter — the heading text is static, not code-structure).

**Back-compat cache path** (`caches/graphify/ → .vox/cache/vox-graph/`): the CLI commands that construct `cache_dir` (in `crates/vox-cli/src/commands/graphify/mod.rs`) currently hardcode `.vox/cache/graphify/{corpus_id}`. VG-1 changes the primary path to `.vox/cache/vox-graph/{corpus_id}`. On first startup after upgrade, the old path is checked as a fallback; if it exists, the cache is migrated (moved, not copied) and a one-line notice printed. After one release, the fallback is deleted. The fallback is a pure runtime check — no config change. The `vox-config` `CORPORA_REL_PATH` constant is updated to point at `contracts/retrieval/vox-graph-corpora.v1.yaml` after the file rename; load_all_corpora also gains a fallback read of the old path for one release.

**GUI hooks** (`useGraphifyStatus.ts` → `useVoxGraphStatus.ts`): the hook already calls `voxTransport.getGraphifyStatus()` → Tauri command `vox_graphify_status`. vs1 has already (or will have) renamed the MCP side to `vox_search_status`; the Tauri command in `crates/vox-gui/src/commands/graphify.rs` (`vox_graphify_status`) is an internal split-brain that vs1's §4 retires by switching the GUI to `invokeMcpTool('vox_search_status')`. VG-1 only renames the **hook file** and **panel component** — the behavior change (Tauri → MCP transport) is vs1 scope. VG-1's rename moves `useGraphifyStatus.ts` → `useVoxGraphStatus.ts` and `GraphifyStatusPanel.tsx` → `VoxGraphStatusPanel.tsx` and updates all import sites.

**Crate rename** (`vox-graphify-reader` → `vox-graph-reader`): rename `[package].name` in `Cargo.toml`, update the crate's `Cargo.toml` description, update all reverse-dependency crates that list `vox-graphify-reader` in their `[dependencies]`, and update the workspace `Cargo.toml` member list if present. Because Rust crate-name identifiers use underscores, library code that imports `vox_graphify_reader::` must be updated to `vox_graph_reader::`.

**Contracts rename** (`graphify-corpora.v1.yaml` → `vox-graph-corpora.v1.yaml`): update `CORPORA_REL_PATH` in `crates/vox-config/src/graphify.rs`, add a one-release fallback read of the old path, rename the file on disk.

**Manifest emission** (`gui-content-manifest.json`): a new module `crates/vox-graph-reader/src/manifest.rs` (post-rename path) with `pub fn emit_content_manifest(graph_json: &str, surface_registry_yaml: &str, surface_dir: &Path, out_path: &Path)`. It: (1) parses the graph JSON for all `surface:` nodes, (2) for each surface node reads the YAML registry entry (label, nav_group, notes), (3) scans the surface's TSX component file for headings (`<h[1-6]`, `aria-label`, section `heading` props), (4) collects `cmd:` neighbor ids from the graph edges, (5) writes one manifest entry per surface. Called from `rebuild_graph` in gui-wiring mode (same condition gate as `surface_nodes`).

## Tech Stack

Rust (`syn`, `serde`/`serde_json`, `walkdir`, `anyhow`); `vox-graph-reader` (renamed crate), `vox-cli`, `vox-config`; `vox-gui` Tauri backend + React/TS frontend; pnpm + vitest (GUI tests). Windows fmt rule: **never `cargo fmt --all`**; use `cargo fmt -p <crate>`. Per-crate builds only: `cargo test -p vox-graph-reader`, `cargo build -p vox-cli`, `cargo test -p vox-config`. GUI tests: `pnpm -C crates/vox-gui/ui vitest run <file>`.

## Spec

Primary: `docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md` §1 (naming), §2.1 (content manifest), §7 (testing — rename tests + manifest golden test), §8 (scope/decomposition — VG-1). Umbrella: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md`. Read vs1 plan (`2026-06-26-vox-search-absorption-and-cli-ingest.md`) Key-internals section before touching any shared file.

## Key internals (verified against the code — exact)

- **`crates/vox-graphify-reader/Cargo.toml`** — `name = "vox-graphify-reader"` (rename to `vox-graph-reader`). No `vox-cli` or `vox-config` dep (dependency direction is `vox-cli → vox-graphify-reader`); do not add those deps. `dev-dependencies = [tempfile]`.
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `pub struct RebuildMeta { corpus_id, git_sha, scope_path, extraction_mode, built_at_rfc3339 }` (line ~131). `pub fn rebuild_graph(_repo_root, source_dir, output_file, cache_dir, meta)` (line ~139). In gui-wiring mode (`meta.extraction_mode == Some("gui-wiring")`), it already runs `surface_nodes(&content)` when `module_id.ends_with("surfaceRegistry.generated.ts")` (line ~208). **Add the manifest emit here** after all nodes/edges are finalized, in the same `if gui_wiring` block (~after line 394, just before the `serde_json::json!` manifest write at line ~368).
- **`crates/vox-graphify-reader/src/registry.rs`** — `pub fn surface_nodes(src: &str) -> Vec<RegistryNode>` (line 193): parses `viewKey:` from the generated TS registry, emits `surface:{view_key}` nodes. The manifest walker uses the same node list.
- **`crates/vox-graphify-reader/src/ast.rs`** — `ExtractedNode { id, label, kind }` (line ~5). The `kind` field is the string `"surface"` for surface nodes.
- **`crates/vox-cli/src/commands/graphify/mod.rs`** — the file where `.vox/cache/graphify/{corpus_id}` path strings live (lines ~453, ~455, ~595, ~616, tests at ~706, ~710, ~726). This is the main site for the cache path migration. **Module file stays named `graphify/mod.rs`** — only the path strings inside change (vs1's decision to leave module names unchanged is honored). The `vox search graph <verb>` subgroup wiring goes here as a new `GraphCmd` sub-enum nested under vs1's `SearchCmd` (or added as a `graph` subcommand of `SearchCmd`).
- **`crates/vox-config/src/graphify.rs`** — `pub const CORPORA_REL_PATH: &str = "contracts/retrieval/graphify-corpora.v1.yaml"` (line 13). Update to `"contracts/retrieval/vox-graph-corpora.v1.yaml"`. `pub fn load_graphify_corpora(repo_root)` (line 153) reads `repo_root.join(CORPORA_REL_PATH)`. Add a fallback read of the old path (only if new path not found). Tests at lines ~575, ~687 supply the YAML inline — update the fixture file name in the include_str!.
- **`contracts/retrieval/graphify-corpora.v1.yaml`** — rename to `vox-graph-corpora.v1.yaml`. No content change to the YAML body required (the corpus IDs inside are separate from the file name).
- **`crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts`** — `GRAPHIFY_STATUS_QUERY_KEY = ['graphify', 'status']` (line 5); calls `getGraphifyStatus` from transport (line 2). Rename file → `useVoxGraphStatus.ts`; update query key to `['vox-graph', 'status']`; update import site in `GraphifyStatusPanel.tsx`.
- **`crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx`** — imports `useGraphifyStatus` (line 2). Rename file → `VoxGraphStatusPanel.tsx`; rename export `GraphifyStatusPanel → VoxGraphStatusPanel`; update import in `surfaceComponents.tsx` (line ~24 currently: `import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel'`; change to `VoxGraphStatusPanel from '../surfaces/Graphify/VoxGraphStatusPanel'`). **The Graphify directory name** may remain `Graphify/` or be renamed `VoxGraph/` — pick `VoxGraph/` for consistency with the brand; update the surfaceComponents.tsx import path.
- **`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`** — `case 'graphify': return <GraphifyStatusPanel />;` (~line 115) and import (~line 24). Update to `<VoxGraphStatusPanel />` and the new import path. The `case 'graphify'` surface key is **not changed here** — the vs1 plan re-keys the surface from `graphify` → `vox-search`; VG-1 only updates the component name.
- **`assets/skills/`** — the auto-hydrated root for pinned skills. Existing examples: `assets/skills/brainstorming/`, each has a `SKILL.md` with YAML front-matter (`name`, `description`) and markdown body. New skill lives at `assets/skills/vox-graph/SKILL.md`.
- **`crates/vox-gui/src/commands/graphify.rs`** — Tauri command `vox_graphify_status` (line ~35). **VG-1 does NOT rename or modify this file** — its rename/retirement is vs1 scope (vs1 retires the direct Tauri call in favor of `invokeMcpTool('vox_search_status')`). VG-1 only renames the hook/panel on the TS side.
- **Reverse-dependency crates using `vox-graphify-reader`**: `crates/vox-cli/` (uses `vox_graphify_reader::rebuild::`, `vox_graphify_reader::coverage::`, `vox_graphify_reader::graph_digest`, etc.); `crates/vox-orchestrator-mcp/` (likely uses the reader for dispatch). Identify all with: `grep -rn "vox-graphify-reader\|vox_graphify_reader" crates/ --include="Cargo.toml" --include="*.rs"` in Task G0.

## Dependencies (cross-plan)

- **Requires vs1 Batch 0 (preflight)** complete so the `vox search` CLI group exists before the `graph` subgroup is wired. If vs1 has not landed, VG-1 Phases A–C (crate rename, cache path, contracts, skill) are fully independent; only Phase D (CLI `graph` subgroup wire-up) needs vs1's `Cli::Search` variant to exist.
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
- `crates/vox-cli/src/commands/graphify/mod.rs` — update cache path strings (`.vox/cache/graphify/ → .vox/cache/vox-graph/`); add migration fallback; add `graph` subcommand wiring inside `SearchCmd`; update `vox_graphify_reader::` → `vox_graph_reader::` call sites; update `include_str!` of corpora YAML in inline tests.
- `crates/vox-cli/Cargo.toml` — update dep name `vox-graphify-reader` → `vox-graph-reader`.
- `crates/vox-orchestrator-mcp/Cargo.toml` — update dep name if present.
- `crates/vox-orchestrator-mcp/src/lib.rs` or `mod.rs` — update `mod graphify_tools;` → `mod graph_tools;`.
- `crates/vox-orchestrator-mcp/src/graph_tools.rs` (renamed from `graphify_tools.rs`) — update any `vox_graphify_reader::` refs to `vox_graph_reader::`.
- All other reverse-dep crates identified in Task G0.

**Renamed (contracts)**
- `contracts/retrieval/graphify-corpora.v1.yaml` → `contracts/retrieval/vox-graph-corpora.v1.yaml`.

**Modified (GUI TypeScript)**
- `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` → renamed to `useVoxGraphStatus.ts`; query key updated.
- `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts` → renamed to `useVoxGraphStatus.test.ts`; descriptions updated.
- `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx` → moved to `components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx`; export renamed.
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — import + case arm updated.

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
| **Batch D — CLI graph subgroup (after A+B)** | G6 | [SEQUENTIAL] | G1+G3 green (needs renamed crate + search group from vs1) | `graphify/mod.rs` `SearchCmd` | 1 agent |
| **Batch E — manifest emitter (after A)** | G7, G8 — sequential | [SEQUENTIAL] within E | G1 green (renamed crate) | `manifest.rs`, `rebuild.rs` | 1 agent |
| **Batch F — GUI hook+panel rename (parallel with D+E)** | G9, G10 — parallel | all [PARALLEL-SAFE] | G0 green | `hooks/`, `surfaces/Graphify/`, `surfaceComponents.tsx` | 1 agent |
| **Batch G — close** | G11 | [SEQUENTIAL] (terminal) | all prior green | verify only | 1 agent; no commit |

**Parallelism summary:** Batches A, B, C, and F may all start once G0 (preflight) passes. Batch D waits for A+B. Batch E waits for A. Batch G is the terminal gate. **Max 4 concurrent agents** (A+B+C+F simultaneously).

---

# PHASE A — Preflight

## Task G0: Identify all reverse-dependency crates [SEQUENTIAL]

Before renaming, build the complete list of crates that depend on `vox-graphify-reader`. This avoids mid-rename compile breaks.

**Files:** Read-only.

- [ ] **Step 1: Inventory** — run:

```bash
grep -rn "vox-graphify-reader\|vox_graphify_reader" \
  /c/Users/Owner/vox-graphify-gui/crates/ \
  --include="Cargo.toml" --include="*.rs" \
  | grep -v "^/c/Users/Owner/vox-graphify-gui/crates/vox-graphify-reader/"
```

Expected: at minimum `crates/vox-cli/Cargo.toml`, `crates/vox-cli/src/commands/graphify/mod.rs` (multiple lines), possibly `crates/vox-orchestrator-mcp/`.

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

Update the `mod` declaration in the parent file (check `crates/vox-orchestrator-mcp/src/lib.rs` or `dispatch.rs` for `mod graphify_tools;` → `mod graph_tools;`). Update any `graphify_tools::` references in the same scope to `graph_tools::`.

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
  crates/vox-orchestrator-mcp/Cargo.toml crates/vox-orchestrator-mcp/src/
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "refactor(vg1-G1): rename crate vox-graphify-reader → vox-graph-reader; graphify_tools.rs → graph_tools.rs"
```

## Task G2: Add `pub mod manifest;` stub to reader lib [SEQUENTIAL after G1]

Creates the module entry point so later tasks in Batch E can land without compile breaks.

**Files:** `crates/vox-graph-reader/src/lib.rs`, `crates/vox-graph-reader/src/manifest.rs` (stub only).

- [ ] **Step 1: Failing test** — create `crates/vox-graph-reader/tests/manifest_tests.rs` with:

```rust
// Tests for gui-content-manifest emission.
// The real emit function is implemented in Task G7; this file is the test fixture entry point.
// Step 1 intentionally references the (not-yet-full) `emit_content_manifest` so the test
// fails at link time until G7 lands.

use vox_graph_reader::manifest::emit_content_manifest;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn manifest_module_exists() {
    // Smoke: the module is reachable (will fail until G2 stub lands).
    let _ = emit_content_manifest as fn(&str, &str, &Path, &Path) -> Result<(), Box<dyn std::error::Error>>;
}
```

Run — expect link error (function signature not yet defined):

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

- [ ] **Step 4: Run test** — should now compile and **fail at runtime** (returns `Err`):

```bash
cargo test -p vox-graph-reader --test manifest_tests 2>&1 | tail -10
```

Expected: `FAILED` with the "not yet implemented" message, not a link error. (The smoke test asserts on the type signature, not the return value — it should actually pass if the function exists. The golden test in G8 will be the proper failing test.)

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
             run `vox search graph rebuild --corpus {corpus_id}` to migrate to \
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
  -m "feat(vg1-G4): rename graphify-corpora.v1.yaml → vox-graph-corpora.v1.yaml + CORPORA_REL_PATH + fallback (TDD)"
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
vox search graph rebuild --corpus <id>   # rebuild the structural graph
vox search graph status                  # freshness report
vox search graph index                   # re-index after code change
```

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

# PHASE E — CLI graph subgroup wire-up

## Task G6: Wire `vox search graph <verb>` subgroup in SearchCmd [SEQUENTIAL after G1, G3, and vs1's Cli::Search]

This task depends on vs1 having renamed `Cli::Graphify` → `Cli::Search`. If vs1 has not landed, **hold this task** until vs1's T4 commit is in.

**Files:** `crates/vox-cli/src/commands/graphify/mod.rs`, `crates/vox-cli/src/lib.rs` (or wherever `SearchCmd` is defined post-vs1).

- [ ] **Step 1: Verify vs1 landed** — `Cli::Search` must exist:

```bash
grep -n "Search\|Graphify" /c/Users/Owner/vox-graphify-gui/crates/vox-cli/src/lib.rs | head -10
```

Expected: `Cli::Search { … }` present; `Cli::Graphify` absent (or alias only). If `Cli::Graphify` is still the primary, **stop and wait for vs1**.

- [ ] **Step 2: Find `SearchCmd`** — vs1 renames the inner `GraphifyCmd` to some form; inspect:

```bash
grep -n "enum.*Cmd\|SearchCmd\|GraphCmd\|GraphifyCmd" \
  /c/Users/Owner/vox-graphify-gui/crates/vox-cli/src/commands/graphify/mod.rs | head -10
```

- [ ] **Step 3: Add `Graph` subvariant** to `SearchCmd` (whatever name vs1 left for the top-level search command enum). If vs1 kept `GraphifyCmd` internally (vs1's decision: "module file unchanged"), add a `Graph` variant:

```rust
// In the enum that vs1 renamed / left as SearchCmd or similar:
pub enum SearchCmd {
    // … existing vs1 variants (Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap) …

    /// Graph-specific subcommands (`vox search graph <verb>`).
    ///
    /// Graph verbs are also accessible as `vox search <verb>` directly (one-release alias)
    /// via the top-level aliases on each variant.
    #[command(subcommand)]
    Graph(GraphCmd),
}

/// Subcommand group for `vox search graph <verb>`.
///
/// All verbs here are forwarded to the same handler as the top-level `vox search <verb>`
/// (which vs1 created); `graph` is the canonical nesting; the top-level is the compat alias.
#[derive(Debug, clap::Subcommand)]
pub enum GraphCmd {
    /// Rebuild the structural graph for a corpus.
    #[command(alias = "rebuild")]
    Rebuild {
        #[arg(long, default_value = "repo-code-graph")]
        corpus: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Report corpus freshness.
    #[command(alias = "status")]
    Status,
    /// Re-index after a code change (incremental).
    #[command(alias = "index")]
    Index {
        #[arg(long, default_value = "repo-code-graph")]
        corpus: String,
    },
    /// Refresh stale corpora.
    #[command(alias = "refresh")]
    Refresh,
    /// Garbage-collect old snapshots.
    #[command(alias = "gc")]
    Gc {
        #[arg(long, default_value = "5")]
        keep: usize,
    },
    /// Map the crate dependency graph.
    #[command(alias = "crate-map")]
    CrateMap {
        #[arg(long)]
        ingest: bool,
    },
    /// Ingest a corpus into the lexical index.
    #[command(alias = "ingest")]
    Ingest {
        #[arg(long, default_value = "repo-code-graph")]
        corpus: String,
        #[arg(long)]
        dry_run: bool,
    },
}
```

- [ ] **Step 4: Add dispatch arm** for `SearchCmd::Graph(cmd)` in the existing `run(cmd, repo_root)` function. Each `GraphCmd` variant forwards to the same handler logic already written for the corresponding `SearchCmd` variant:

```rust
SearchCmd::Graph(graph_cmd) => match graph_cmd {
    GraphCmd::Rebuild { corpus, dry_run } => {
        // forward to existing Rebuild handler (same body)
        run(SearchCmd::Rebuild { corpus, dry_run }, repo_root).await?;
    }
    GraphCmd::Status => run(SearchCmd::Status, repo_root).await?,
    GraphCmd::Index { corpus } => run(SearchCmd::Index { corpus }, repo_root).await?,
    GraphCmd::Refresh => run(SearchCmd::Refresh, repo_root).await?,
    GraphCmd::Gc { keep } => run(SearchCmd::Gc { keep }, repo_root).await?,
    GraphCmd::CrateMap { ingest } => run(SearchCmd::CrateMap { ingest }, repo_root).await?,
    GraphCmd::Ingest { corpus, dry_run } => {
        run(SearchCmd::Ingest { corpus, dry_run }, repo_root).await?;
    }
},
```

*Note: the exact variant field names must match what vs1 left in `SearchCmd`; adapt as needed after reading the vs1-modified file in Step 2.*

- [ ] **Step 5: Compile + test**:

```bash
cargo build -p vox-cli 2>&1 | tail -5
cargo test -p vox-cli 2>&1 | grep "graph\|FAILED\|ok" | head -10
```

- [ ] **Step 6: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-cli/src/commands/graphify/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G6): wire vox search graph <verb> subgroup inside SearchCmd (forwards to existing handlers)"
```

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

/// Fixture: a minimal graph JSON containing one surface node (approvals) with a cmd: edge.
const FIXTURE_GRAPH: &str = r#"{
  "nodes": [
    { "id": "surface:approvals", "label": "approvals", "kind": "surface" },
    { "id": "cmd:vox_resolve_approval", "label": "vox_resolve_approval", "kind": "command" }
  ],
  "edges": [
    { "source": "surface:approvals", "target": "cmd:vox_resolve_approval", "confidence": "declared" }
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

#[test]
fn manifest_golden_surface_approvals() {
    let tmp = TempDir::new().unwrap();
    let out_path = tmp.path().join("gui-content-manifest.json");

    // surface_dir: empty (no TSX files) — headings will be []; commands come from graph edges.
    let surface_dir = tmp.path();

    emit_content_manifest(FIXTURE_GRAPH, FIXTURE_REGISTRY_YAML, surface_dir, &out_path)
        .expect("emit_content_manifest must not error on valid fixture");

    let raw = std::fs::read_to_string(&out_path).expect("manifest file must be written");
    let manifest: serde_json::Value =
        serde_json::from_str(&raw).expect("manifest must be valid JSON");

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
    // commands: should include vox_resolve_approval (from the cmd: edge in the graph)
    let commands = entry["commands"].as_array().expect("must have commands array");
    assert!(
        commands.iter().any(|c| c.as_str() == Some("vox_resolve_approval")),
        "commands must include vox_resolve_approval (from graph edge); got: {:?}",
        commands
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
//!       "notes": "Operator approval queue for doubt_task feedback"
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

    // 2. Build edge map: surface view_key → set of cmd: neighbor names.
    let mut cmd_neighbors: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(edges) = graph["edges"].as_array() {
        for edge in edges {
            let src = edge["source"].as_str().unwrap_or("");
            let tgt = edge["target"].as_str().unwrap_or("");
            if let Some(view_key) = src.strip_prefix("surface:") {
                if let Some(cmd_name) = tgt.strip_prefix("cmd:") {
                    cmd_neighbors
                        .entry(view_key.to_string())
                        .or_default()
                        .push(cmd_name.to_string());
                }
            }
        }
    }
    // Dedup and sort for determinism.
    for v in cmd_neighbors.values_mut() {
        v.sort();
        v.dedup();
    }

    // 3. Parse surface-registry YAML for label/group/notes per surface.
    //    We use a targeted line scan (no serde_yaml dep in this crate).
    let registry_meta = parse_surface_registry_yaml(surface_registry_yaml);

    // 4. Scan surface dir for heading text per surface.
    let headings_by_surface = scan_surface_headings(surface_dir);

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

/// Scan surface component files for heading text.
///
/// Looks for TSX files matching `<ViewKey|view_key>*.tsx` under `surface_dir`.
/// Extracts text from `<h1>`…`<h6>` tags and `aria-label` attributes.
/// Returns a map of view_key (lowercased) → sorted deduplicated heading strings.
fn scan_surface_headings(surface_dir: &Path) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();

    // Walk all TSX files under surface_dir.
    if let Ok(entries) = walkdir::WalkDir::new(surface_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "tsx")
                    .unwrap_or(false)
        })
        .map(Ok::<_, std::io::Error>)
        .collect::<Result<Vec<_>, _>>()
    {
        for entry in entries {
            let path = entry.path();
            let Ok(src) = std::fs::read_to_string(path) else {
                continue;
            };

            // Derive a likely surface key from the filename (lowercase stem without "View"/"Panel").
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let surface_key_hint = stem
                .to_lowercase()
                .replace("view", "")
                .replace("panel", "")
                .replace("statusview", "")
                .replace("surface", "");

            if surface_key_hint.is_empty() {
                continue;
            }

            let headings = extract_headings(&src);
            if !headings.is_empty() {
                out.entry(surface_key_hint)
                    .or_default()
                    .extend(headings);
            }
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

- [ ] **Step 2: Call `emit_content_manifest` in `rebuild_graph`**. In the `if gui_wiring { … }` block at the end of `rebuild_graph` (after the `serde_json::json!` manifest write at ~line 368, before `fs::write(manifest_path, …)`), add:

```rust
// Emit the GUI content manifest alongside the graph (gui-wiring mode only).
if let (Some(ref registry_yaml), Some(ref surface_dir)) = (
    &meta.surface_registry_yaml,
    &meta.gui_source_dir,
) {
    let graph_str = serde_json::to_string(&nodes_val)?; // use the already-serialized graph
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

*Note: `nodes_val` is the `Vec<serde_json::Value>` built just before the JSON write — wrap it in `{"nodes": nodes_val, "edges": edges_val}` if needed, or serialize the whole `out_json` object.*

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
export const VOX_GRAPH_STATUS_QUERY_KEY = ['vox-graph', 'status'];

export function useVoxGraphStatus() {
  return useQuery<GraphifyStatusDto, Error>({
    queryKey: VOX_GRAPH_STATUS_QUERY_KEY,
    queryFn: getGraphifyStatus,
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

## Task G10: Rename `GraphifyStatusPanel.tsx` → `VoxGraphStatusPanel.tsx` [PARALLEL-SAFE with D, E]

**Files:** `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx`, `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`.

- [ ] **Step 1: Create new component** at `crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx`:

```tsx
import { useVoxGraphStatus } from '../../../hooks/useVoxGraphStatus';

// VG-1: renamed from GraphifyStatusPanel. The transport seam (direct Tauri call →
// invokeMcpTool('vox_search_status')) is retired in vs1; this rename is pure cosmetic.
export function VoxGraphStatusPanel() {
  const { data, isLoading, isError, error } = useVoxGraphStatus();

  if (isLoading) return <div role="status" aria-label="Loading Vox Graph status">Loading…</div>;
  if (isError) return <div role="alert">Error: {String(error)}</div>;
  if (!data) return <div role="status" aria-label="No Vox Graph status data">No data</div>;

  return (
    <div>
      <h2>Vox Graph Status</h2>
      <pre>{JSON.stringify(data, null, 2)}</pre>
    </div>
  );
}
```

- [ ] **Step 2: Update `surfaceComponents.tsx`** — change the import and the component name:

```tsx
// Before (~line 24):
import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel';

// After:
import { VoxGraphStatusPanel } from '../surfaces/VoxGraph/VoxGraphStatusPanel';
```

And the case arm (~line 115):

```tsx
// Before:
case 'graphify': return <GraphifyStatusPanel />;

// After (the surface key 'graphify' is NOT changed here — vs1 owns that re-key):
case 'graphify': return <VoxGraphStatusPanel />;
```

- [ ] **Step 3: Run vitest + honesty check**:

```bash
pnpm -C /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui vitest run 2>&1 | tail -15
```

(No honesty scanner regression expected since the component is real and has no placeholder text.)

- [ ] **Step 4: Commit**:

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx \
  crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
git -C /c/Users/Owner/vox-graphify-gui commit \
  -m "feat(vg1-G10): rename GraphifyStatusPanel → VoxGraphStatusPanel; update surfaceComponents import"
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

- [ ] **Step 5: Verify no stale `vox_graphify_reader::` references remain**:

```bash
grep -rn "vox_graphify_reader\|vox-graphify-reader" \
  /c/Users/Owner/vox-graphify-gui/crates/ \
  --include="*.rs" --include="Cargo.toml" \
  | grep -v "^Binary\|^grep"
```

Expected: 0 lines.

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
| **Batch D — CLI graph subgroup** | G6 | [SEQUENTIAL] | G1 + G3 + vs1-T4 green | `graphify/mod.rs` SearchCmd | 1 agent |
| **Batch E — manifest emitter** | G7 → G8 | [SEQUENTIAL] within E | G2 green | `manifest.rs`, `rebuild.rs` | 1 agent in order |
| **Batch F — GUI rename** | G9, G10 | both [PARALLEL-SAFE] | G0 green | `hooks/`, `surfaces/VoxGraph/`, `surfaceComponents.tsx` | 1–2 agents |
| **G close** | G11 | [SEQUENTIAL] terminal | all prior green | verify only — no commit | 1 agent |

**Parallelism summary:** Batches A, B, C, and F all start once G0 passes (4-way concurrent, disjoint file sets). Batch D waits for A+B+vs1-T4. Batch E waits for A (G2 stub). Batch G is the terminal gate.

---

## Self-review checklist

- ✅ **Extends vs1, no duplication:** VG-1 does not re-do the `Cli::Graphify → Cli::Search` or `vox_graphify_* → vox_search_*` renames (those are vs1 scope). VG-1 only adds the `graph` subgroup and the cache-path / crate-name / contract / GUI-hook renames.
- ✅ **CLI verb nesting stated once:** graph verbs live under `vox search graph <verb>`; stated in the Goal and in G6 — not repeated elsewhere.
- ✅ **Back-compat on cache path:** `cache_dir_with_migration` function; tested in G3 with three TDD assertions (new preferred, legacy fallback, both→new wins).
- ✅ **Back-compat on corpora YAML:** `load_graphify_corpora` fallback; tested in G4 with two TDD assertions.
- ✅ **Every anchor verified against code before citing:** `CORPORA_REL_PATH` line 13 ✓; `RebuildMeta` line ~131 ✓; `surface_nodes` line 193 ✓; `rebuild_graph` gui-wiring mode ~line 208 ✓; `useGraphifyStatus` / `GraphifyStatusPanel` file contents ✓; `voxDocsIndex` pattern in `docs_index.rs` ✓; `assets/skills/brainstorming/` pattern ✓.
- ✅ **No `cargo fmt --all`:** per-crate only (`cargo fmt -p <crate>`).
- ✅ **No `vox ci gui-honesty` regression:** `VoxGraphStatusPanel` uses real data, no placeholder text.
- ✅ **Golden test in G7:** fixture surface `approvals` → asserts `nav_label`, `route`, `nav_group`, `commands` in the emitted manifest.
- ✅ **No stubs:** G2 creates an explicit stub with a clear "implemented in G7" comment; G7 replaces it with the full implementation.
- ✅ **Plan format matches VG-2 style:** writing-plans header, TDD steps, [PARALLEL-SAFE]/[SEQUENTIAL] tags, Workflow Batch Plan table.
