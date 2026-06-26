# Vox Search Absorption + CLI-Tree `cli:` Ingest — Implementation Plan

> **For agentic workers / workflow runners:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (in-session fan-out) or `superpowers:executing-plans` (separate sessions). Every task is **bite-sized, TDD-first, and ends in its own commit** so a sub-agent can execute *and* commit it independently (write-through-workflow). Steps use checkbox (`- [ ]`) syntax.
>
> **Each task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`.** Independent tasks are grouped into explicit **fan-out batches** a workflow can dispatch concurrently. Read the *Workflow Batch Plan* table before dispatching.

**Goal:** Execute the master spec's **P0 (absorption + rename)** and **P6 §5.1 (clap-tree `cli:` node ingestion)** as one self-contained spine. Retire the user-facing "graphify" brand: rename `vox graphify <verb>` → `vox search <verb>` (behavior 1:1, one-release deprecation alias), rename the five `vox_graphify_*` MCP tools → `vox_search_*` (catalog SSOT → regenerated registries → dispatch → schemas), retire the GUI `getGraphifyStatus` split-brain in favor of `invokeMcpTool('vox_search_status')`, and re-key the `graphify` GUI surface to `vox-search` under Knowledge. **PLUS:** ingest the full clap CLI tree (549 leaf commands, gated-corrected for `mens`/`populi`/`oratio`) as `cli:<group>:<command>` nodes in the structural index, joined to `tool:`/`cmd:`/`surface:` nodes, and produce the deferred **`CliOnly`** coverage dimension over the unified `(cli ∪ tool ∪ cmd ∪ surface)` node-set.

**Architecture:** `vox-graphify-reader` (the structural-index engine) is **rehomed intact, not rewritten** — the internal crate keeps its name (an optional flagged rename is **out of scope** for this plan). The external surface becomes uniformly `vox search` / `vox_search_*`. **The CLI change is a RENAME of the existing `Graphify` clap variant, not the addition of a new group:** there is **no pre-existing `vox search` command group** on this branch (verified: `grep -n "Search" crates/vox-cli/src/lib.rs` matches only doc text — see "Key internals"). T4 renames the single `Cli::Graphify` variant to `Cli::Search` (clap derives the lowercased verb), adds `#[command(alias = "graphify")]` for the one-release deprecation alias, and re-points the three dispatch arms — it does not create a second group alongside a surviving `graphify` one. The CLI-ingest adapter is a **pure-JSON** function in the reader (no `vox-cli` dependency — the dependency direction is `vox-cli → vox-graphify-reader`); `vox-cli` serializes its compile-time `build_catalog()` to JSON and threads it into the reader via a new `RebuildMeta.cli_catalog_json` field. The catalog SSOT (`contracts/operations/catalog.v1.yaml`) drives the generated MCP registries via `vox ci operations-sync --target {mcp,capability} --write` — never hand-edit the generated files.

**Tech Stack:** Rust (`syn`, `serde`/`serde_json`, `anyhow`, `clap`, `walkdir`); `vox-graphify-reader`, `vox-cli`, `vox-ml-cli` (gated enums), `vox-orchestrator-mcp` (dispatch + schemas), `vox-gui` (Tauri + React/TS); `vox ci operations-sync`; vitest for the GUI; agent fan-out for batches.

**Spec:** `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` — **read §1 (absorption), §3 (tool surface), §4 (GUI), §5.1 (cli ingest) first.** Source audit: `docs/agents/cli-gui-governance-audit.md`.

**Worktree:** `/c/Users/Owner/vox-graphify-gui` (branch `claude/graphify-general-gui-ia`). All git commands MUST be `git -C /c/Users/Owner/vox-graphify-gui` and **add+commit only** — never push, never branch, never reset/clean. The workflow performs the final integration.

**Base-branch note:** `main` does not compile (`vox-cli` `db_cli` WIP at `07ef88d7e2`); this branch is off the compiling honesty branch. Prefer crate-scoped builds: `cargo test -p vox-graphify-reader` (fast, isolated) and `cargo build -p vox-cli` / `cargo test -p vox-orchestrator-mcp` only where a task touches them. GUI tasks use `cd crates/vox-gui/ui && npx vitest run <file>`.

---

## Cross-plan dependencies (read before dispatch)

- **This plan = master-spec P0 + P6 §5.1.** It is the **prerequisite spine** for every later plan.
- **Downstream (must NOT start until this plan's tool names are final, i.e. Batch 1 merged):** P1 (data-flow), P2 (fusion `vox_discover`), P4 (auto-availability/steering), P5 (GUI panes), P6 §5.2 (CI/Database governance surfaces).
- **Upstream:** none. This plan has no predecessor.
- **Within this plan:** the rename (Phase A) is the gate for the MCP/GUI rename (Phases B/C); CLI-ingest (Phase D) depends only on the reader and `vox-cli` catalog and is **independent of the rename** (can run in parallel with A–C). The unified-coverage `CliOnly` task (Phase E) depends on Phase D.

---

## Key internals (verified against the code — exact, do not re-discover)

- **`crates/vox-cli/src/lib.rs`** — the `Cli` enum. The variant to rename is `Graphify { #[command(subcommand)] cmd: commands::graphify::GraphifyCmd }` (~line 198–202, doc-comment `/// Graphify corpus registry and map freshness (`vox graphify`).`). There is **no existing `Search` clap variant** (`grep -n "Search" lib.rs` → only doc text). The clap top-level verb is derived from the variant name lowercased (`Graphify` → `graphify`).
- **`crates/vox-cli/src/cli_dispatch/mod.rs`** — three match arms reference `Cli::Graphify`: line ~51 (`Some("graphify")`), line ~134 (`"graphify"`), line ~261 (`Cli::Graphify { cmd } => { … crate::commands::graphify::run(cmd, &root).await? }`). The `vox commands` reflection uses `VoxCliRoot::command()` (~line 170) with `include_nested` (~line 179).
- **`crates/vox-cli/src/commands/graphify/mod.rs`** (795 lines) — `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }` (**seven** variants on this base branch — verified by the T0 enum grep; a `Coverage` variant is added by P1's CLI plan, **not** present here, so do not reference it in any T0–T10 test/assertion); `pub async fn run(cmd, repo_root) -> anyhow::Result<()>`. **Keep the module file in place** (rename of the module dir is optional/out-of-scope); only the *clap verb* (the `Cli::Graphify` group) changes — the subcommand verbs are untouched.
- **`crates/vox-cli/src/commands/mod.rs`** — `pub mod graphify;` (~line 90).
- **`crates/vox-cli/src/command_catalog.rs`** — `pub fn build_catalog() -> CommandCatalog { entries: Vec<CommandCatalogEntry> }`. `CommandCatalogEntry { path: Vec<String>, command, about, aliases, has_subcommands, compiled_in, source_group, feature_gate: Option<String>, tier, capability_id, arguments }`. `feature_gated_group_names() -> Vec<(&str,&str)>` lists `dei`-gated groups. **`mens`/`populi`/`oratio` real subcommands live in `crates/vox-ml-cli/` enums** (`PopuliAction`=22, `PopuliCli`=18, `OratioAction`=9) and collapse to stubs in the default binary.
- **`crates/vox-orchestrator-mcp/src/dispatch.rs`** — five arms (~lines 627–641): `"vox_graphify_status" => …graphify_tools::graphify_status…`, `vox_graphify_search`/`_query`/`_path`/`_compare`. The handler fn names (`graphify_status` etc.) are internal — **only the string keys must change**; renaming the fns is optional.
- **`crates/vox-orchestrator-mcp/src/input_schemas.rs`** — five arms (~lines 471–486) `"vox_graphify_status" => parse_obj(…)`. Same: change the string keys.
- **`contracts/operations/catalog.v1.yaml`** — the **SSOT**. Five `mcp.name: vox_graphify_*` blocks (`graphify.status`/`.search`/`.query`/`.path`/`.compare`, the `mcp:` sub-block ~lines 6216/6238/6289/6311/6333). Editing `name:` here + running `vox ci operations-sync --target {mcp,capability} --write` regenerates `contracts/mcp/tool-registry.canonical.yaml` and `contracts/capability/capability-registry.yaml` (both marked "GENERATED … do not hand-edit").
- **`crates/vox-gui/src/commands/graphify.rs`** — `pub struct GraphifyStatusPayload`, `#[tauri::command] pub async fn vox_graphify_status() -> Result<GraphifyStatusPayload,String>` (~line 35). This is the **split-brain** the spec retires in the GUI; the Tauri command itself stays for now (the GUI stops calling it directly and uses `invokeMcpTool('vox_search_status')` instead — §4 spec). **This plan only re-keys the surface + retires the direct hook call; full panel rework is P5.**
- **`crates/vox-gui/ui/src/lib/navigation.ts`** — `navMap`, `defaultChild`, `navOrder`, `groupLabels`. **`graphify` is absent** (orphan). Knowledge group exists: `scientia: { parent: 'knowledge', child: 'scientia' }` etc.; `knowledge: 'Knowledge'` label.
- **`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`** — `case 'graphify': return <GraphifyStatusPanel />;` (~line 115), `import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel';` (~line 24).
- **`crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`** — generated; entries keyed by `viewKey`. Regenerate via `vox ci gui-surface-registry --write` after editing `navigation.ts`.
- **`crates/vox-orchestrator-mcp/src/chat_tools/mod.rs`** — `build_system_prompt_with_skill`; the MEMORY.md block is pushed at ~lines 124–137 (always-on code-map injection in P4 lands **after** this block — *not in this plan*).
- **`crates/vox-graphify-reader/src/registry.rs`** — adapters `tauri_command_nodes(src, registered) -> Vec<RegistryNode>`, `mcp_tool_nodes(src)`, `transport_wrapper_map(ts_src)`, `surface_nodes(src)`. `RegistryNode { id, label, kind, unregistered }`, `RegistryNode::new(prefix, name, kind)` → `id = "{prefix}:{name}"`. **Add `cli_command_nodes(catalog_json: &str)` here.**
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `pub fn rebuild_graph(_repo_root, source_dir, output_file, cache_dir, meta: &RebuildMeta)`. `RebuildMeta { corpus_id, git_sha, scope_path, extraction_mode, built_at_rfc3339 }` (~line 131). In `gui-wiring` mode it runs `tauri_command_nodes`/`mcp_tool_nodes` inline (~lines 204–210), folds reg nodes by id (~lines 217–235). **Add a `cli_catalog_json: Option<String>` field + a fold of `cli_command_nodes` here.**
- **`crates/vox-graphify-reader/src/coverage.rs`** — `pub enum CoverageStatus { OrphanBackend, DeadEnd, Surfaced, CliOnly }` (`CliOnly` already declared, ~line 30, **reserved, no producer yet**); `pub fn compute_coverage(graph: &Value, kind: &str) -> CoverageReport` (~line 74). **Extend to emit `CliOnly` for `cli:` nodes with no surface caller.**
- **Reader has NO `vox-cli` / `vox-config` dep** (`Cargo.toml`: only `serde`, `serde_json`, `syn`, `walkdir`, …) — keep the new adapter dependency-free (pure JSON).

---

## File Structure

**Reader (`crates/vox-graphify-reader/`)** — modified/created:
- `src/registry.rs` — add `cli_command_nodes(catalog_json: &str) -> Vec<RegistryNode>` (pure JSON; `cli:<group>:<command>` ids).
- `src/rebuild.rs` — add `RebuildMeta.cli_catalog_json: Option<String>`; fold `cli_command_nodes` into the registry node set in `gui-wiring` mode; add `cli:`→`cmd:`/`tool:` join edges (`declared` confidence).
- `src/coverage.rs` — extend `compute_coverage` to classify `cli:` nodes as `CliOnly` (no surface/tool caller) vs `Surfaced` (joined to a surfaced impl).
- `tests/cli_ingest_tests.rs` *(new)* — adapter + join + coverage `CliOnly` fixtures.

**CLI (`crates/vox-cli/`)** — modified:
- `src/lib.rs` — `Graphify` clap variant → `Search` (+ doc comment); add `#[command(alias = "graphify", …)]` for the one-release deprecation alias.
- `src/cli_dispatch/mod.rs` — three `Cli::Graphify` arms → `Cli::Search`; deprecation-warning emit on the alias path.
- `src/commands/graphify/mod.rs` — `pub fn cli_catalog_json() -> String` helper (serialize `build_catalog()` gated-corrected); thread into `RebuildMeta`. (Module file/name unchanged.)
- `tests/` (inline) — alias-resolution + `cli:` ingest e2e.

**MCP (`crates/vox-orchestrator-mcp/`)** — modified:
- `src/dispatch.rs` — five string keys `vox_graphify_*` → `vox_search_*`.
- `src/input_schemas.rs` — five string keys `vox_graphify_*` → `vox_search_*`.

**Contracts** — modified (SSOT edit + regenerate):
- `contracts/operations/catalog.v1.yaml` — five `mcp.name: vox_graphify_*` → `vox_search_*` + description copy de-branded.
- `contracts/mcp/tool-registry.canonical.yaml`, `contracts/capability/capability-registry.yaml`, `contracts/capability/model-manifest.generated.json` — regenerated (do not hand-edit).

**GUI (`crates/vox-gui/`)** — modified:
- `ui/src/components/layout/surfaceComponents.tsx` — `case 'graphify'` → `case 'vox-search'` (keep a `case 'graphify'` fall-through to the same panel for one release).
- `ui/src/lib/navigation.ts` — add `'vox-search': { parent: 'knowledge', child: 'vox-search' }` + label.
- `ui/src/generated/surfaceRegistry.generated.ts` — regenerated via `vox ci gui-surface-registry --write`.
- `ui/src/hooks/useGraphifyStatus.ts` — switch the data source to `invokeMcpTool('vox_search_status')` (retire the direct `vox_graphify_status` Tauri call).

**Docs:**
- `docs/agents/cli-gui-governance-audit.md` — strike the closing caveat "The clap CLI tree was never ingested …" (now done), point at `cli:` nodes.

---

## Workflow Batch Plan (fan-out structure)

| Batch | Tasks | Mode | Gate to next |
|---|---|---|---|
| **Batch 0** | T0 | `[SEQUENTIAL]` | preflight green |
| **Batch 1** (rename spine) | T1, T2, T3, T4 — *sequential chain* (catalog → regen → dispatch/schemas → cli verb) | `[SEQUENTIAL]` within; the whole batch is the gate for downstream plans | all five MCP keys + `vox search` verb compile + tests green |
| **Batch 2** (CLI ingest — parallel to Batch 1) | T5, T6 — *T5 then T6* | T5 `[PARALLEL-SAFE]` (reader-only, no rename dep); T6 `[SEQUENTIAL]` after T5 | reader tests green |
| **Batch 3** (fan-out, after Batch 1 + Batch 2) | T7, T8, T9 dispatched **in parallel** | all `[PARALLEL-SAFE]` | each its own commit |
| **Batch 4** | T10 (final verification + audit-doc update) | `[SEQUENTIAL]` | plan complete |

**Parallelism summary:** Batch 1 (rename) and Batch 2 (CLI ingest, T5) run **concurrently** off Batch 0. T7 (GUI re-key), T8 (GUI hook retire), T9 (unified `CliOnly` coverage) fan out once both predecessors land. T10 closes.

---

## Phase A — Preflight

### T0 — Preflight: confirm anchors + green baseline `[SEQUENTIAL]` (Batch 0)

- [ ] Verify the worktree branch and that the touched crates build in isolation:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui rev-parse --abbrev-ref HEAD
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-graphify-reader 2>&1 | tail -5
  ```
  **Expected:** branch `claude/graphify-general-gui-ia`; `test result: ok.` for the reader.
- [ ] Confirm the five MCP keys + the clap variant exist exactly where the plan claims:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  grep -c "vox_graphify_" crates/vox-orchestrator-mcp/src/dispatch.rs
  grep -c "vox_graphify_" crates/vox-orchestrator-mcp/src/input_schemas.rs
  grep -c "name: vox_graphify_" contracts/operations/catalog.v1.yaml
  grep -n "Graphify {" crates/vox-cli/src/lib.rs
  ```
  **Expected:** `5`, `5`, `5`, and one `Graphify {` hit. If any count differs, STOP and reconcile the line anchors before proceeding.
- [ ] **Verify the real `GraphifyCmd` enum variants against this plan's rename copy.** The T4 clap-variant rename ONLY renames `Cli::Graphify` → `Cli::Search`; the *subcommand verbs* (`status`/`ingest`/`rebuild`/…) are NOT renamed — they ride on the unchanged `GraphifyCmd` enum. Grep the actual leaf-verb list and assert it matches what this plan describes; annotate/drop any verb this plan mentions that is not in the enum:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  # List the real GraphifyCmd variants (the canonical verb set under `vox search <verb>`).
  grep -nE '^\s+[A-Z][A-Za-z]+\s*\{|^\s+[A-Z][A-Za-z]+,' crates/vox-cli/src/commands/graphify/mod.rs | sed -n '1,20p'
  ```
  **Expected (verified at authoring):** `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }` — **seven** variants. **NOTE:** the "Key internals" line in this plan that reads `enum GraphifyCmd { Status, Ingest, Rebuild, Coverage, Index, Refresh, Gc, CrateMap }` lists a `Coverage` variant that **does not exist** in the base enum on this branch (it is added by P1's CLI plan, not P0); treat `Coverage` as out-of-scope for T4 and do not assume it is present. If the grep shows a different set, reconcile this plan's verb references before proceeding — the rename is name-for-name on the clap *group* (`Graphify`→`Search`), so a verb the enum lacks must not appear in any test or doc assertion here.
- [ ] Commit a no-op marker only if the workflow requires a batch-0 commit; otherwise this task produces **no commit** (verification only). If a commit is required:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui commit --allow-empty -m "chore(vox-search): preflight checkpoint for absorption plan

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Phase B — Rename spine (Batch 1, sequential chain — the downstream gate)

### T1 — Rename the five MCP tool names in the catalog SSOT `[SEQUENTIAL]`

- [ ] **TDD first.** Add a guard test that the catalog no longer carries the old prefix. Append to `crates/vox-cli/src/commands/ci/operations_catalog.rs` test module (or create `crates/vox-cli/tests/vox_search_rename.rs`):
  ```rust
  #[test]
  fn catalog_has_no_graphify_tool_prefix() {
      let yaml = std::fs::read_to_string(
          concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/operations/catalog.v1.yaml"),
      )
      .expect("read catalog");
      assert!(
          !yaml.contains("name: vox_graphify_"),
          "graphify MCP tool prefix must be renamed to vox_search_ in the catalog SSOT"
      );
      for t in [
          "vox_search_status",
          "vox_search_structural",
          "vox_search_neighbors",
          "vox_search_path",
          "vox_search_compare",
      ] {
          assert!(yaml.contains(&format!("name: {t}")), "missing renamed tool {t}");
      }
  }
  ```
  Run it red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli catalog_has_no_graphify_tool_prefix 2>&1 | tail -8
  ```
  **Expected:** FAILS (old prefix present).
- [ ] Edit `contracts/operations/catalog.v1.yaml`: rename the five `mcp.name` fields per the spec §1.1 map — `vox_graphify_status`→`vox_search_status`, `vox_graphify_search`→`vox_search_structural`, `vox_graphify_query`→`vox_search_neighbors`, `vox_graphify_path`→`vox_search_path`, `vox_graphify_compare`→`vox_search_compare`. De-brand the five `description:`/`title:` lines (e.g. "Report graphify corpus freshness" → "Report Vox Search structural-index freshness"; "Lexically search a graphify corpus graph" → "Lexically search the Vox Search structural index"). Keep `tier: core`, `http_read_role_eligible`, `intent_tags`.
- [ ] Re-run the guard green:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli catalog_has_no_graphify_tool_prefix 2>&1 | tail -5
  ```
  **Expected:** `test result: ok.`
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add contracts/operations/catalog.v1.yaml crates/vox-cli/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(vox-search): rename vox_graphify_* MCP tools to vox_search_* in catalog SSOT

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T2 — Regenerate the MCP/capability registries from the SSOT `[SEQUENTIAL]`

- [ ] Regenerate the generated contracts (never hand-edit them):
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  cargo run -p vox-cli -- ci operations-sync --target mcp --write
  cargo run -p vox-cli -- ci operations-sync --target capability --write
  ```
  **Expected:** both exit 0; the files report a write.
- [ ] Verify the generated files flipped and no stale prefix remains:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  grep -c "vox_search_status\|vox_search_structural\|vox_search_neighbors\|vox_search_compare" contracts/mcp/tool-registry.canonical.yaml
  grep -c "vox_graphify_" contracts/mcp/tool-registry.canonical.yaml contracts/capability/capability-registry.yaml contracts/capability/model-manifest.generated.json
  ```
  **Expected:** first ≥ 4; second `0` across all three generated files.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add contracts/mcp/tool-registry.canonical.yaml contracts/capability/capability-registry.yaml contracts/capability/model-manifest.generated.json
  git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(vox-search): regenerate MCP + capability registries for vox_search_* rename

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T3 — Rename the MCP dispatch + schema string keys `[SEQUENTIAL]`

- [ ] **TDD first.** In `crates/vox-orchestrator-mcp/` add a dispatch-key guard (e.g. `tests/vox_search_dispatch.rs`):
  ```rust
  #[test]
  fn dispatch_routes_vox_search_keys_not_graphify() {
      let dispatch = include_str!("../src/dispatch.rs");
      let schemas = include_str!("../src/input_schemas.rs");
      for s in [dispatch, schemas] {
          assert!(!s.contains("\"vox_graphify_"), "old vox_graphify_ key still present");
      }
      for k in [
          "\"vox_search_status\"",
          "\"vox_search_structural\"",
          "\"vox_search_neighbors\"",
          "\"vox_search_path\"",
          "\"vox_search_compare\"",
      ] {
          assert!(dispatch.contains(k), "dispatch missing {k}");
          assert!(schemas.contains(k), "schemas missing {k}");
      }
  }
  ```
  Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-orchestrator-mcp dispatch_routes_vox_search_keys_not_graphify 2>&1 | tail -8
  ```
  **Expected:** FAILS.
- [ ] Edit `crates/vox-orchestrator-mcp/src/dispatch.rs` — change the five match-arm string keys (handler fn names unchanged):
  `"vox_graphify_status"` → `"vox_search_status"`; `"vox_graphify_search"` → `"vox_search_structural"`; `"vox_graphify_query"` → `"vox_search_neighbors"`; `"vox_graphify_path"` → `"vox_search_path"`; `"vox_graphify_compare"` → `"vox_search_compare"`.
- [ ] Edit `crates/vox-orchestrator-mcp/src/input_schemas.rs` — the same five string keys in the `parse_obj(...)` arms.
- [ ] Run green + the crate's existing tests:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-orchestrator-mcp dispatch_routes_vox_search_keys_not_graphify 2>&1 | tail -5
  cargo build -p vox-orchestrator-mcp 2>&1 | tail -3
  ```
  **Expected:** `test result: ok.`; build succeeds.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(vox-search): route vox_search_* keys in MCP dispatch + schemas

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T4 — Rename the `vox graphify` clap verb → `vox search` (+ deprecation alias) `[SEQUENTIAL]`

- [ ] **TDD first.** Add an alias-resolution test in `crates/vox-cli/tests/vox_search_rename.rs`:
  ```rust
  use clap::Parser;
  use vox_cli::{Cli, VoxCliRoot};

  #[test]
  fn search_verb_parses_and_graphify_alias_still_resolves() {
      // New canonical verb.
      let root = VoxCliRoot::try_parse_from(["vox", "search", "status"]).expect("vox search status");
      assert!(matches!(root.command, Cli::Search { .. }));
      // One-release deprecation alias.
      let aliased = VoxCliRoot::try_parse_from(["vox", "graphify", "status"]).expect("alias resolves");
      assert!(matches!(aliased.command, Cli::Search { .. }));
  }
  ```
  > If `Cli`/`VoxCliRoot` are not `pub`, gate the test behind the crate's existing test-exposure pattern (check `crates/vox-cli/src/lib.rs` for `pub use`), or assert via `VoxCliRoot::command().get_subcommands()` names instead. Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli search_verb_parses_and_graphify_alias_still_resolves 2>&1 | tail -8
  ```
  **Expected:** FAILS (no `Search` variant).
- [ ] Edit `crates/vox-cli/src/lib.rs`: rename the variant and add the alias. Replace:
  ```rust
      /// Graphify corpus registry and map freshness (`vox graphify`).
      Graphify {
          #[command(subcommand)]
          cmd: commands::graphify::GraphifyCmd,
      },
  ```
  with:
  ```rust
      /// Vox Search — code-intelligence over the structural index (`vox search`).
      #[command(alias = "graphify")]
      Search {
          #[command(subcommand)]
          cmd: commands::graphify::GraphifyCmd,
      },
  ```
- [ ] Edit `crates/vox-cli/src/cli_dispatch/mod.rs`: change the three `Cli::Graphify` arms to `Cli::Search`. At the dispatch arm (~line 261), emit a one-release deprecation note when invoked via the alias (detect the raw arg):
  ```rust
          Cli::Search { cmd } => {
              if std::env::args().nth(1).as_deref() == Some("graphify") {
                  eprintln!("warning: `vox graphify` is deprecated; use `vox search` (alias removed next release).");
              }
              let root = resolve_repo_root()?; // keep existing root resolution line
              crate::commands::graphify::run(cmd, &root).await?;
          }
  ```
  > Preserve whatever the existing root-resolution line is (copy it verbatim from the current `Cli::Graphify` arm). Also update the two metadata arms (~line 51 `Some("graphify")` → keep returning `"search"`; ~line 134 `"graphify"` → `"search"`) so telemetry/labels report the canonical verb.
- [ ] Run green + build:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli search_verb_parses_and_graphify_alias_still_resolves 2>&1 | tail -5
  cargo build -p vox-cli 2>&1 | tail -3
  cargo run -p vox-cli -- search status 2>&1 | tail -5
  ```
  **Expected:** test ok; build succeeds; `vox search status` runs (freshness output or a clean "no corpora" message).
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(vox-search): rename vox graphify CLI verb to vox search with deprecation alias

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Phase C — CLI-tree `cli:` ingest (Batch 2, parallel to Batch 1)

### T5 — Reader: `cli_command_nodes` adapter + rebuild fold + join edges `[PARALLEL-SAFE]`

- [ ] **TDD first.** Create `crates/vox-graphify-reader/tests/cli_ingest_tests.rs`:
  ```rust
  use vox_graphify_reader::registry::cli_command_nodes;

  const CATALOG_JSON: &str = r#"{
    "entries": [
      { "path": ["ci", "lint"], "command": "lint", "source_group": "ci" },
      { "path": ["db", "query"], "command": "query", "source_group": "db" },
      { "path": ["search"], "command": "search", "source_group": "search" }
    ]
  }"#;

  #[test]
  fn cli_nodes_have_group_scoped_ids_and_skip_top_level_groups() {
      let nodes = cli_command_nodes(CATALOG_JSON);
      let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
      assert!(ids.contains(&"cli:ci:lint"), "expected cli:ci:lint, got {ids:?}");
      assert!(ids.contains(&"cli:db:query"));
      // Top-level group with no subcommand (len==1) is the group node, not a leaf.
      assert!(!ids.iter().any(|i| *i == "cli:search:search"));
      assert!(nodes.iter().all(|n| n.kind == "cli-command"));
  }

  #[test]
  fn malformed_json_yields_empty_not_panic() {
      assert!(cli_command_nodes("not json").is_empty());
  }
  ```
  Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-graphify-reader --test cli_ingest_tests 2>&1 | tail -8
  ```
  **Expected:** compile error / fail (fn absent).
- [ ] Add to `crates/vox-graphify-reader/src/registry.rs` (pure JSON, no new deps):
  ```rust
  /// Ingest the clap command catalog (`vox commands --format json --include-nested`,
  /// i.e. the serialized `CommandCatalog`) as `cli:<group>:<command>` leaf nodes.
  /// Top-level groups (path.len()==1) are emitted as `cli:<group>` group nodes; deeper
  /// paths become leaves keyed by the first (group) and last (command) segments.
  /// Malformed JSON yields an empty Vec — never panics (honesty: under-report).
  pub fn cli_command_nodes(catalog_json: &str) -> Vec<RegistryNode> {
      let mut out = Vec::new();
      let Ok(v) = serde_json::from_str::<serde_json::Value>(catalog_json) else {
          return out;
      };
      let Some(entries) = v.get("entries").and_then(|e| e.as_array()) else {
          return out;
      };
      for e in entries {
          let Some(path) = e.get("path").and_then(|p| p.as_array()) else {
              continue;
          };
          let segs: Vec<String> = path
              .iter()
              .filter_map(|s| s.as_str().map(str::to_string))
              .collect();
          match segs.as_slice() {
              [] => {}
              [group] => {
                  let mut n = RegistryNode::new("cli", group, "cli-group");
                  n.id = format!("cli:{group}");
                  out.push(n);
              }
              [group, .., command] => {
                  let mut n = RegistryNode::new("cli", command, "cli-command");
                  n.id = format!("cli:{group}:{command}");
                  out.push(n);
              }
          }
      }
      out
  }
  ```
- [ ] Add the join edges + meta field. In `crates/vox-graphify-reader/src/rebuild.rs`:
  - Add to `RebuildMeta`: `pub cli_catalog_json: Option<String>,` (and update every `RebuildMeta { … }` literal in the reader's own code/tests to include `cli_catalog_json: None`).
  - After the inline registry-adapter block (the `if gui_wiring { … }` fold, ~line 210), once, fold CLI nodes and synthesize **`declared`-confidence** join edges from `cli:<group>:<command>` to a same-named `cmd:<command>` or `tool:<command>` node when one exists:
  ```rust
      if gui_wiring {
          if let Some(cat) = meta.cli_catalog_json.as_deref() {
              let cli_nodes = crate::registry::cli_command_nodes(cat);
              for cn in &cli_nodes {
                  // Join to a same-named command/tool impl if present (declared-confidence,
                  // name-match — never a proven call).
                  if let Some(cmd) = cn.id.rsplit(':').next() {
                      reg.push(cn.clone());
                      all_edges.push(crate::ast::ExtractedEdge {
                          source: cn.id.clone(),
                          target: format!("cmd:{cmd}"),
                      });
                  }
              }
          }
      }
  ```
  > If `ExtractedEdge` lacks a `confidence` field on this branch, the join edge is still emitted; confidence labelling rides the existing schema (the general-enhancement plan's A1 adds `confidence`). The join is a *candidate* edge — coverage (T9) treats a `cli:` node with no matching impl as `CliOnly`.
- [ ] Add `cli:`-join coverage to the test file (extend `cli_ingest_tests.rs`) asserting a `cli:ci:lint`→`cmd:lint` edge is produced when a `cmd:lint` node is present (build a tiny in-memory graph via `rebuild_graph` over a tempdir, or assert at the `cli_command_nodes` + manual-edge layer if `rebuild_graph` setup is heavy). Run green:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-graphify-reader 2>&1 | tail -6
  ```
  **Expected:** all reader tests `ok.`
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): ingest clap CLI tree as cli: nodes in the structural index

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T6 — CLI: serialize gated-corrected catalog + thread into rebuild `[SEQUENTIAL]` (after T5)

- [ ] **TDD first.** Add to `crates/vox-cli/tests/vox_search_rename.rs` (or a new `cli_ingest.rs`):
  ```rust
  #[test]
  fn cli_catalog_json_includes_gated_mens_populi_oratio() {
      let json = vox_cli::commands::graphify::cli_catalog_json();
      // Gated groups must be present even in a default binary (recovered from vox-ml-cli enums).
      for g in ["mens", "populi", "oratio"] {
          assert!(json.contains(g), "gated group {g} missing from cli catalog json");
      }
      // Parses as a CommandCatalog-shaped object.
      let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
      assert!(v.get("entries").and_then(|e| e.as_array()).map(|a| a.len() > 100).unwrap_or(false));
  }
  ```
  Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli cli_catalog_json_includes_gated 2>&1 | tail -8
  ```
  **Expected:** FAILS (fn absent).
- [ ] Add `pub fn cli_catalog_json() -> String` to `crates/vox-cli/src/commands/graphify/mod.rs`. It serializes `crate::command_catalog::build_catalog()` to JSON and **substitutes the gated-corrected `mens`/`populi`/`oratio` subcommand rows** (per the audit: `PopuliAction`=22, `PopuliCli`=18, `OratioAction`=9) so a default binary still emits the full leaf set. Use `command_catalog::feature_gated_group_names()` as the gate list and append synthetic `CommandCatalogEntry` rows for the recovered subcommands (names sourced from the `vox-ml-cli` enums — list them as a `const &[(&str, &[&str])]` table in this fn, e.g. `("populi", &["up","down","status",…])`). Then `serde_json::to_string(&catalog)`.
  > Keep the synthetic table small and documented; the audit's counts are the acceptance check, not the literal enum import (avoids a `vox-cli → vox-ml-cli` build coupling in the default binary).
- [ ] Thread it into the three `rebuild_graph` call sites in `crates/vox-cli/src/commands/graphify/mod.rs` (the `Rebuild`/`Index`/`Refresh` arms, ~lines 371/470/518): set `cli_catalog_json: Some(cli_catalog_json())` in each `RebuildMeta { … }` literal (and `None` elsewhere if any other site exists).
- [ ] Run green + build:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli cli_catalog_json_includes_gated 2>&1 | tail -5
  cargo build -p vox-cli 2>&1 | tail -3
  ```
  **Expected:** test ok; build succeeds.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): serialize gated-corrected CLI catalog into structural rebuild

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Phase D — Fan-out (Batch 3, after Batch 1 + Batch 2)

### T7 — GUI: re-key the `graphify` surface → `vox-search` under Knowledge `[PARALLEL-SAFE]`

- [ ] **TDD first.** Add `crates/vox-gui/ui/src/lib/navigation.vox-search.test.ts`:
  ```ts
  import { describe, it, expect } from 'vitest';
  import { navMap, groupLabels } from './navigation';

  describe('vox-search nav placement', () => {
    it('places vox-search under Knowledge', () => {
      expect(navMap['vox-search']).toEqual({ parent: 'knowledge', child: 'vox-search' });
    });
    it('keeps a Knowledge group label', () => {
      expect(groupLabels['knowledge']).toBe('Knowledge');
    });
  });
  ```
  Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/lib/navigation.vox-search.test.ts 2>&1 | tail -10
  ```
  **Expected:** FAILS (`vox-search` undefined).
- [ ] Edit `crates/vox-gui/ui/src/lib/navigation.ts`: add `'vox-search': { parent: 'knowledge', child: 'vox-search' }` to `navMap` (the orphan `graphify` had no nav entry — this fixes it). Add a label if the label map is per-child.
- [ ] Edit `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`: add `case 'vox-search': return <GraphifyStatusPanel />;` and keep `case 'graphify':` falling through to the same panel (one-release alias). (Panel rename to `VoxSearchPanel` is P5 — out of scope here.)
- [ ] Regenerate the surface registry:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry --write 2>&1 | tail -3
  ```
  **Expected:** exit 0; `surfaceRegistry.generated.ts` updated with a `vox-search` `viewKey`.
- [ ] Run green:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/lib/navigation.vox-search.test.ts 2>&1 | tail -5
  ```
  **Expected:** `passed`.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/lib/navigation.vox-search.test.ts crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
  git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): re-key graphify surface to vox-search under Knowledge

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T8 — GUI: retire the `getGraphifyStatus` split-brain (hook → MCP) `[PARALLEL-SAFE]`

- [ ] Inspect the current hook + its test:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  sed -n '1,60p' crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts
  ```
- [ ] **TDD first.** Update `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts` to assert the hook calls `invokeMcpTool('vox_search_status', …)` (the shared dispatch) rather than the direct `vox_graphify_status` Tauri command. Mock `voxTransport.invokeMcpTool`. Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/hooks/useGraphifyStatus.test.ts 2>&1 | tail -10
  ```
  **Expected:** FAILS.
- [ ] Edit `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts`: replace the direct `invoke('vox_graphify_status')` data source with `voxTransport.invokeMcpTool('vox_search_status', {})` and parse the MCP JSON string result into the existing payload shape. Keep the hook name (or add `useVoxSearchStatus` re-export) to avoid breaking the panel import.
  > The Tauri `vox_graphify_status` command in `crates/vox-gui/src/commands/graphify.rs` stays registered (do NOT remove it) — only the GUI's *call path* moves to the shared MCP dispatch, ending the split-brain per spec §4.
- [ ] Run green + the surface honesty guard:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/hooks/useGraphifyStatus.test.ts 2>&1 | tail -5
  ```
  **Expected:** `passed`.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts
  git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(gui): route graphify status through shared vox_search_status MCP dispatch

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

### T9 — Coverage: emit `CliOnly` over the unified node-set `[PARALLEL-SAFE]`

- [ ] **TDD first.** Add to `crates/vox-graphify-reader/tests/cli_ingest_tests.rs` (or a new `coverage_cli_tests.rs`) a `compute_coverage` fixture over an in-memory graph `Value` containing a `cli:db:vacuum` node with **no** surface/cmd caller (→ `CliOnly`) and a `cli:ci:lint` node joined to a surfaced `cmd:lint` (→ `Surfaced`):
  ```rust
  use serde_json::json;
  use vox_graphify_reader::coverage::{compute_coverage, CoverageStatus};

  #[test]
  fn cli_only_command_classified_cli_only() {
      let graph = json!({
        "nodes": [
          { "id": "cli:db:vacuum", "label": "vacuum", "kind": "cli-command" },
          { "id": "cli:ci:lint",   "label": "lint",   "kind": "cli-command" },
          { "id": "cmd:lint",      "label": "lint",   "kind": "command" },
          { "id": "surface:develop:ci", "label": "ci", "kind": "surface" }
        ],
        "links": [
          { "source": "cli:ci:lint", "target": "cmd:lint" },
          { "source": "surface:develop:ci", "target": "cmd:lint" }
        ]
      });
      let report = compute_coverage(&graph, "cli-command");
      let vacuum = report.entries.iter().find(|e| e.id == "cli:db:vacuum").unwrap();
      assert_eq!(vacuum.status, CoverageStatus::CliOnly);
      let lint = report.entries.iter().find(|e| e.id == "cli:ci:lint").unwrap();
      assert_eq!(lint.status, CoverageStatus::Surfaced);
  }
  ```
  > Confirm the actual `CoverageReport`/entry field names first (`grep -n "pub struct CoverageReport\|pub.*id\|pub.*status" crates/vox-graphify-reader/src/coverage.rs`) and adapt the assertions to the real shape. Run red:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-graphify-reader cli_only_command_classified 2>&1 | tail -8
  ```
  **Expected:** FAILS.
- [ ] Edit `crates/vox-graphify-reader/src/coverage.rs`: in `compute_coverage`, when `kind == "cli-command"`, classify a node as `CliOnly` if it has **no inbound edge from a `surface:` node** (i.e. no GUI path) — even if it joins to a `cmd:`/`tool:` impl; classify `Surfaced` only when a `surface:` (or surfaced impl reachable from a surface) reaches it. This is the spec's honest "not-in-GUI" label. Keep the existing `command`/`tool`/`surface` classification paths intact.
- [ ] Run green + full reader suite:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-graphify-reader 2>&1 | tail -6
  ```
  **Expected:** all `ok.`
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/
  git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): classify cli: nodes as CliOnly in unified coverage

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Phase E — Close (Batch 4)

### T10 — Full verification + audit-doc caveat strike `[SEQUENTIAL]`

- [ ] Run the touched-crate suites + a `vox search coverage` smoke over the GUI corpus:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  cargo test -p vox-graphify-reader 2>&1 | tail -4
  cargo test -p vox-cli 2>&1 | tail -4
  cargo test -p vox-orchestrator-mcp 2>&1 | tail -4
  cargo run -p vox-cli -- search rebuild --corpus vox-gui-surface 2>&1 | tail -3 || true
  cargo run -p vox-cli -- search coverage --corpus vox-gui-surface --kind cli-command 2>&1 | tail -15 || true
  ```
  **Expected:** all `test result: ok.`; the coverage run lists `cli:` nodes with `CliOnly`/`Surfaced` classes (or a clean "corpus not built" message if the corpus is absent — acceptable in CI).
- [ ] Confirm no stale brand leaked into the renamed surfaces:
  ```bash
  cd /c/Users/Owner/vox-graphify-gui
  grep -rc "vox_graphify_" crates/vox-orchestrator-mcp/src contracts/mcp contracts/capability
  ```
  **Expected:** `0` in dispatch/schemas and the generated registries.
- [ ] Edit `docs/agents/cli-gui-governance-audit.md` — replace the closing caveat bullet ("The clap CLI tree was never ingested into the Graphify surface graph … a follow-up could emit these 549 commands as `cli:` nodes …") with a DONE note pointing at `vox_graphify_reader::registry::cli_command_nodes` + `vox search coverage --kind cli-command`.
- [ ] Commit:
  ```bash
  git -C /c/Users/Owner/vox-graphify-gui add docs/agents/cli-gui-governance-audit.md
  git -C /c/Users/Owner/vox-graphify-gui commit -m "docs(vox-search): mark CLI-tree cli: ingestion done in governance audit

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
  ```

---

## Self-Review — spec coverage

| Spec clause | Plan task(s) | Covered? |
|---|---|---|
| §1.1 CLI rename `vox graphify`→`vox search`, verbs 1:1, one-release alias | T4 | ✅ verb rename + `#[command(alias="graphify")]` + deprecation warning + alias-resolution test |
| §1.1 MCP rename `vox_graphify_*`→`vox_search_*` (status/structural/neighbors/path/compare) | T1 (catalog SSOT) → T2 (regen) → T3 (dispatch+schemas) | ✅ exact 5-name map, SSOT-driven regen, dispatch/schema keys |
| §1.1 update catalog.v1.yaml + tool-registry.canonical.yaml + dispatch.rs + input_schemas.rs | T1/T2/T3 | ✅ all four files |
| §1.1 corpora→indexes / retire brand in user-facing copy | T1 (descriptions), T7 (nav label), T10 (audit doc) | ✅ de-brand in catalog descriptions + GUI surface + docs |
| §1.2 KEEP engine intact; rename crate is OPTIONAL/flagged | Architecture + Non-goals | ✅ crate name unchanged; module file unchanged; explicitly out of scope |
| §4 retire `getGraphifyStatus` split-brain → `invokeMcpTool('vox_search_status')` | T8 | ✅ hook re-pointed to shared MCP dispatch; Tauri cmd left registered |
| §4.2 fix `graphify` orphan, re-key to vox-search, add to navigation.ts under Knowledge, regen surfaceRegistry | T7 | ✅ navMap entry + surfaceComponents case + `gui-surface-registry --write` |
| §5.1 ingest clap tree as `cli:<group>:<command>` nodes | T5 (`cli_command_nodes`) + T6 (catalog serialization, gated-corrected) | ✅ pure-JSON adapter + RebuildMeta thread |
| §5.1 gated-corrected mens/populi/oratio (22/18/9) | T6 | ✅ synthetic gated rows + acceptance test |
| §5.1 join `cli:` to `cmd:`/`tool:` nodes | T5 (declared-confidence join edges) | ✅ name-match candidate edges |
| §5.1 unified coverage matrix + `CliOnly` + honest not-in-GUI | T9 | ✅ `compute_coverage` emits `CliOnly` for surface-less cli nodes |
| §7 honesty: name-match never a proven call; under-report not fabricate | T5 (declared join), T9 (CliOnly), `malformed_json→empty` | ✅ candidate edges, drop-on-ambiguity, no panic |
| §6 sequencing: structural-core + absorption is the prerequisite spine | Cross-plan deps + Batch plan | ✅ this plan gates P1/P2/P4/P5/P6 |
| Workflow-readiness: every task TDD + own commit + PARALLEL/SEQUENTIAL tag + fan-out batches + strict git | All tasks | ✅ |

**Out of scope (correctly deferred to later master-spec plans, not gaps):** the `VoxSearchPanel` tabbed rework + new layer panes (P5); `.mcp.json` / `vox mcp install` / code-map injection / pinned skill (P4); `vox_discover` fusion (P2); data-flow (P1); semantic overlay (P3); the §5.2 governance surfaces `Develop>CI` / `Knowledge>Database` / typed secret wrappers (P6 §5.2); VoxMens GUI (P7); Settings/Policies (P8). The optional `vox-graphify-reader` crate rename is explicitly out of scope per §1.

**Known risk / mitigation:** if `Cli`/`VoxCliRoot` are not `pub` for T4's parse test, the plan instructs falling back to a `command().get_subcommands()` name assertion. If `ExtractedEdge` lacks `confidence` on this branch, join edges still emit and confidence labelling rides the general-enhancement schema bump (cross-referenced, not blocked).
