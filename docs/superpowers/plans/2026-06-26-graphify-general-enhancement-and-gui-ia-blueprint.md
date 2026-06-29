# Graphify General Enhancement + GUI IA Blueprint — Implementation Plan (rev2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **rev2 — hardened against a codebase audit + a system-design critique + a design-critique (2026-06-26).** Key corrections folded in: `.tsx`/`.jsx` were never walked (now Phase A0); `resolve_edges`/the file-walk are inline closures, not functions (now extracted in A0); dead-ends were dropped and made invisible (now kept as `dangling` + `missing` nodes); command source-of-truth reconciles `#[tauri::command]`-defined with the `generate_handler!`-registered set; `voxTransport.*` wrappers hide the real command (now resolved via a transport map); the surface-registry key is `viewKey`, not `id`; import-anchor (`::*`) hack dropped; tree-sitter node-kind names are discovered by a print-step, not assumed.

**Goal:** Enhance the general, Rust-native Graphify engine so it captures the codebase's real structure — string-dispatch boundaries (`invoke`→command), React composition, and declared registries — then use it to produce a ratifiable blueprint for an aggressive Vox GUI reorganization.

**Architecture:** Two plans. **Plan 1 (Phases A0–F)** is general-engine Rust work in `vox-graphify-reader` + `vox-cli`. **Plan 2 (Phases G–J)** runs the enriched engine on the GUI, joins it with the existing 32-surface honesty audit, and produces an adversarially-verified per-item IA blueprint + new-nav proposal for human ratification (no GUI code changes).

**Tech Stack:** Rust (`syn` for Rust AST, `tree-sitter`/`tree-sitter-typescript` 0.23.2 for TS/TSX, `petgraph`/`leiden-rs`, `serde_json`, `anyhow`, `clap`, `walkdir`); the `vox graphify` CLI + `graphify-corpora.v1.yaml`; agent fan-out for Plan 2.

**Spec:** `docs/superpowers/specs/2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` (read first).

**Base branch note:** `main` does not compile (`vox-cli` `db_cli` WIP breakage at `07ef88d7e2`); authored on `claude/graphify-general-gui-ia` off the compiling honesty branch. Prefer `cargo test -p vox-graphify-reader` (fast, isolated); build `vox-cli` only for Phase E.

---

## Key internals (verified against the code — exact)

- **`crates/vox-graphify-reader/src/ast.rs`** — `ExtractedNode { id, label, kind: String }`, `ExtractedEdge { source, target }` (NO `confidence` yet — A1 adds it). `qualify(module_id, sym)` → `"module::sym"`. `extract_ast_in_module(path, content, module_id) -> ExtractedGraph`. Rust via `syn`; TS/TSX/JS/PY via a `tree_sitter` stack-walk under `#[cfg(feature="tree-sitter-grammars")]` (default on). TS kinds matched today: `function_declaration`, `method_definition`, `class_definition`, `call_expression`; callee via `child_by_field_name("function")`. **JSX/import/argument node-kind names are NOT verified for tree-sitter-typescript 0.23.2 — discover them with a print-step (Task A0d), do not assume.**
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `rebuild_graph(repo_root, source_dir, output_file, cache_dir, meta: &RebuildMeta)`. **The file walk is INLINE** (`walkdir::WalkDir`, ~lines 29–69) filtering `ext == "rs"|"ts"|"js"|"py"` — **`.tsx`/`.jsx` are NOT walked**; there is **no `walk_files` helper**. **Edge resolution is an INLINE `.filter_map()` closure** (~lines 71–107) with a nested `module_of` — **there is no `resolve_edges` function**. Serializes `graph.json` as `{nodes:[{id,label,kind,community}], links:[{source,target}]}`. Leiden clustering ~110–126. Manifest digest = BLAKE3 (`graph_digest`, ~line 182) despite the `_sha256` field name.
- **`crates/vox-graphify-reader/src/lib.rs`** — `GraphifyReader::from_value` (~75–141) reads only id/label/community/source/target and **ignores unknown fields/kinds** (so the schema bump won't break the reader). `pub mod` list lives here.
- **`crates/vox-cli/src/commands/graphify/mod.rs`** — `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }`; `pub async fn run(cmd, repo_root) -> anyhow::Result<()>`. Helpers CONFIRMED: `load_all_corpora(repo_root)`, `resolve_ingest_corpus_id(&reg, corpus)`, `corpus_by_id(&reg, &id)`; `use anyhow::Context;` already imported. The `Rebuild` arm shows the corpus-resolution lines to copy.
- **Command source-of-truth:** 126 `#[tauri::command]` fns under `crates/vox-gui/src/commands/`; `tauri::generate_handler![...]` in `crates/vox-gui/src/main.rs` (~line 109+) lists 154 entries — a SUPERSET that includes non-command state managers/streams. So: command NAMES come from `#[tauri::command]`; invokability requires membership in `generate_handler!`. A defined-but-unregistered command is dead.
- **MCP tools:** `crates/vox-orchestrator-mcp/src/dispatch.rs` (~line 417+) is a `match name { "vox_x" => ... }`. No `tool-registry.canonical.yaml` exists; the match IS the SSOT.
- **Surface registry:** `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — entries are `{ viewKey: 'x', cliGroup: null, tier: '...', navLabel: '...', navGroup: '...', parentSurface: ... }`. **Key is `viewKey`, not `id`**; entries with `viewKey: null` are CLI groups (skip them).
- **GUI invoke distribution (coverage ceiling):** direct `invoke('cmd', …)` ~30–50; `invoke<T>('cmd')` template-generic ~10–20 (callee field is still `invoke` → handled); **`voxTransport.<method>()` wrappers ~50+ where the command name lives inside `transport.ts`, NOT at the call site** (must resolve through a transport map — Task C3); `voxTransport.callTool`/`callTool` ~12. Variable command names not observed.
- **Tests:** `crates/vox-graphify-reader/tests/{ast_tests.rs, rebuild_tests.rs, python_tests.rs}` — tempdir fixtures; Python test gated `#![cfg(feature="tree-sitter-grammars")]`.

---

## File Structure

**Plan 1 — modified/created**
- `crates/vox-graphify-reader/src/rebuild.rs` — A0: add `tsx`/`jsx` to the walk, factor `walk_source_files()` + `resolve_edges()`; later: carry `confidence`, prefixed-target + dead-end resolution, single-walk registry ingest, manifest `confidence_counts`.
- `crates/vox-graphify-reader/src/ast.rs` — `confidence` on `ExtractedEdge`; JSX composition edges; `invoke`/`callTool`/`voxTransport.*` boundary edges (callee literals in one `BOUNDARY_CALLEES` table).
- `crates/vox-graphify-reader/src/registry.rs` *(new)* — registry adapters: Tauri commands (syn) + registered-set check, MCP tools, surfaces (`viewKey`), transport-wrapper map, clap/command-catalog.
- `crates/vox-graphify-reader/src/coverage.rs` *(new)* — `compute_coverage(graph, kind) -> CoverageReport` with `CoverageStatus { OrphanBackend, DeadEnd, Surfaced, CliOnly }`.
- `crates/vox-graphify-reader/src/lib.rs` — `pub mod registry; pub mod coverage;`.
- `crates/vox-graphify-reader/tests/{composition_edges.rs, boundary_edges.rs, registry_ingest.rs, coverage.rs}` *(new)*.
- `crates/vox-cli/src/commands/graphify/mod.rs` — `Coverage` subcommand.
- `contracts/retrieval/graphify-corpora.v1.yaml` — `vox-gui-surface` → `extraction_mode: gui-wiring`; add a non-GUI generality corpus.

**Plan 2 — created (artifacts; no GUI code)**
- `graphify-out/gui-coverage/*.json`, `graphify-out/gui-ia/*.json`, `docs/agents/gui-ia-blueprint.md`.

---

# PLAN 1 — General Graphify engine

## Phase A0 — Prerequisite refactors (load-bearing; pure, separately committed)

### Task A0a: Walk `.tsx`/`.jsx` + factor `walk_source_files()` (TDD)

Without this, every JSX/boundary edge silently extracts nothing — the GUI is mostly `.tsx`.

**Files:** Modify `crates/vox-graphify-reader/src/rebuild.rs`. Test: `crates/vox-graphify-reader/tests/rebuild_tests.rs` (extend).

- [ ] **Step 1: Failing test** — append to `rebuild_tests.rs`:

```rust
#[test]
#[cfg(feature = "tree-sitter-grammars")]
fn tsx_files_contribute_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("C.tsx"), "function Widget(){ return null; }").unwrap();
    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta { corpus_id: "t".into(), git_sha: None, scope_path: "src".into(),
        extraction_mode: Some("structural".into()), built_at_rfc3339: "2026-06-26T00:00:00+00:00".into() };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert!(g["nodes"].as_array().unwrap().iter().any(|n| n["label"] == "Widget"), "tsx not walked");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader tsx_files_contribute_nodes`. Expected: FAIL (no `Widget` node — `.tsx` filtered out).

- [ ] **Step 3: Implement** — factor the inline walk into a helper at the top of `rebuild.rs` and add `tsx`/`jsx`:

```rust
pub(crate) fn walk_source_files(source_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != ".git" && n != "target" && n != ".vox" && n != "node_modules"
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| matches!(
            e.path().extension().and_then(|x| x.to_str()),
            Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py")))
        .map(|e| e.path().to_path_buf())
        .collect()
}
```

Replace the inline `for entry in walkdir::WalkDir::new(...)` loop body in `rebuild_graph` with `for path in walk_source_files(source_dir) { ... }` (read each file, call `extract_ast_in_module`). Keep the existing per-file cache logic.

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS (existing + new). 

- [ ] **Step 5: Commit** — `feat(graphify): walk .tsx/.jsx; factor walk_source_files helper`.

### Task A0b: Extract `resolve_edges()` as a named fn (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/rebuild.rs`.

- [ ] **Step 1: Failing test** — add inside `rebuild.rs`:

```rust
#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::ast::{ExtractedNode, ExtractedEdge};
    #[test]
    fn same_module_unique_resolves_and_drops_ambiguous() {
        let nodes = vec![
            ExtractedNode { id: "m.rs::a".into(), label: "a".into(), kind: "fn".into() },
            ExtractedNode { id: "m.rs::b".into(), label: "b".into(), kind: "fn".into() },
        ];
        let edges = vec![ExtractedEdge { source: "m.rs::a".into(), target: "b".into() }];
        let out = resolve_edges(&nodes, &edges);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, "m.rs::b");
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader same_module_unique_resolves`. Expected: FAIL (`resolve_edges` not found).

- [ ] **Step 3: Implement** — lift the inline `.filter_map()` closure (rebuild.rs ~71–107) into:

```rust
fn resolve_edges(nodes: &[crate::ast::ExtractedNode], edges: &[crate::ast::ExtractedEdge]) -> Vec<crate::ast::ExtractedEdge> {
    use std::collections::HashMap;
    fn module_of(id: &str) -> &str { id.rsplit_once("::").map(|(m, _)| m).unwrap_or("") }
    let mut defs_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for n in nodes {
        let bare = n.id.rsplit("::").next().unwrap_or(&n.id).to_string();
        defs_by_name.entry(bare).or_default().push(n.id.clone());
    }
    edges.iter().filter_map(|e| {
        let candidates = defs_by_name.get(&e.target)?;
        let src_mod = module_of(&e.source);
        let same: Vec<&String> = candidates.iter().filter(|id| module_of(id) == src_mod).collect();
        let target = if same.len() == 1 { same[0].clone() } else { return None; };
        if target == e.source { return None; }
        Some(crate::ast::ExtractedEdge { source: e.source.clone(), target })
    }).collect()
}
```

Call it from `rebuild_graph` where the closure was. (Preserve EXACT current behavior — this is a pure refactor; confidence + prefixed-target branches come in A1/D2.)

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS (behavior identical, all existing green).

- [ ] **Step 5: Commit** — `refactor(graphify): extract resolve_edges() from rebuild_graph (no behavior change)`.

### Task A0c: Consumer-impact check (verification only — no code)

The schema bump (A1 onward) changes `graph.json` for EVERY corpus and shifts each `graph_json_sha256` (BLAKE3). Confirm consumers tolerate it before changing the schema.

- [ ] **Step 1:** Grep + read each consumer of `graph.json`: the 5 MCP tools in `crates/vox-orchestrator-mcp/src/graphify_tools.rs`, `crates/vox-gui/src/commands/graphify.rs` (`vox_graphify_status`), the ingest/lexical path, `lens::collapse_to_modules`, and any `tests/` golden asserting exact `graph.json` shape. For each, confirm it reads by key and ignores unknown fields/kinds (note `from_value` already does). Record findings in the commit message.
- [ ] **Step 2:** Decide + note: `module-anchor` is NOT introduced (import edges dropped — see Phase B note); new node kinds `command|tool|surface` and the `confidence` link field are additive. State explicitly that A1 rewrites every corpus's digest (expected, not a regression) and whether `lens::collapse_to_modules` should filter `cmd:`/`tool:`/`surface:` nodes out of `modules`-mode output (recommended: yes — add that filter in Task E-adjacent work if a `modules` corpus exists). No code change in this task.
- [ ] **Step 3: Commit** — `docs(graphify): consumer-impact check for the graph.json schema bump` (a short note file under `docs/agents/` or the commit body).

### Task A0d: Discover tree-sitter-typescript node-kind names (verification — feeds B1/D1)

- [ ] **Step 1:** Write a temporary `#[test]` in `composition_edges.rs` that parses `function P(){ return <Child x={1}/>; }\nimport {Q} from './q';\ninvoke('do_it', { tool: 't' });` with `tree_sitter_typescript::LANGUAGE_TSX` and prints every `node.kind()` (pre-order). Run `cargo test -p vox-graphify-reader -- --nocapture print_node_kinds`. Record the EXACT kind strings for: the JSX self-closing element + its component-name field; the import statement + how named-import identifiers are reached; the `call_expression`'s `arguments` field + the string-literal kind + object/`pair` kinds. **Use these recorded names verbatim in B1 and D1** (replace the assumed `jsx_self_closing_element`/`import_specifier`/`string`/`object`/`pair` if they differ).
- [ ] **Step 2:** Delete the temporary print test. Commit nothing (or commit the recorded names as a comment block at the top of `ast.rs`'s TS walk for the executor's reference) — `docs(graphify): record tree-sitter-typescript 0.23.2 node-kind names`.

## Phase A — Edge confidence

### Task A1: Add `confidence` to edges; carry through resolution + `graph.json` (TDD)

**Files:** Modify `ast.rs`, `rebuild.rs`. Test: `ast_tests.rs`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn edges_default_to_resolved_confidence() {
    use vox_graphify_reader::ast::extract_ast_in_module;
    let g = extract_ast_in_module(std::path::Path::new("m.rs"), "fn a(){ b(); }\nfn b(){}", "m.rs");
    assert!(g.edges.iter().all(|e| e.confidence == "resolved"), "edges: {:?}", g.edges);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader edges_default_to_resolved_confidence`. Expected: FAIL (no field).

- [ ] **Step 3: Implement** — `ExtractedEdge` gains `#[serde(default = "default_confidence")] pub confidence: String` + `fn default_confidence() -> String { "resolved".into() }`. Update every `ExtractedEdge { .. }` construction (syn visitor + tree-sitter walk) to set `confidence: "resolved".into()`. In `resolve_edges` (A0b) carry `confidence: e.confidence.clone()` on the returned edge. In the `graph.json` links serialization add `"confidence": e.confidence`. Add `confidence_counts` (a `{resolved,declared,dangling,...}` tally) to the manifest `json!`.

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS.

- [ ] **Step 5: Commit** — `feat(graphify): edge confidence (resolved default) in resolve_edges + graph.json + manifest`.

## Phase B — Composition edges (collapse the islands)

> **Import edges are intentionally OUT of scope.** The 51%-island problem is driven by missing React composition; JSX usage edges (below) collapse it. Synthetic `module::*` import-anchor nodes would skew Leiden clustering and inflate counts, so they are dropped from v1 (revisit only if a non-React corpus needs import structure, modeled as real node→node edges, not anchors).

### Task B1: JSX element-usage composition edges (TDD)

**Files:** Modify `ast.rs`. Test: `composition_edges.rs`.

- [ ] **Step 1: Failing test** (uses the kind names recorded in A0d):

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;
#[test]
fn jsx_usage_emits_composition_edge() {
    let g = extract_ast_in_module(Path::new("P.tsx"),
        "function Parent(){ return <Child/>; }\nfunction Child(){ return null; }", "P.tsx");
    assert!(g.edges.iter().any(|e| e.source == "P.tsx::Parent" && e.target == "Child"),
        "edges: {:?}", g.edges);
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** — add a JSX branch in the tree-sitter stack-walk using the A0d-recorded kind for a self-closing/opening element and its name field; emit `ExtractedEdge { source: current_fn, target: <ComponentName>, confidence: "resolved" }` only when the name is Capitalized (lowercase = DOM tags). **Step 4:** `cargo test -p vox-graphify-reader composition_edges` → PASS. **Step 5: Commit** — `feat(graphify): JSX composition edges (TS/TSX)`.

## Phase C — Registry-ingest nodes

### Task C1: Tauri command nodes via `syn`, flagged by registration (TDD)

Use `syn` (already a dep) for robust attribute/multiline handling, and cross-check the `generate_handler!` registered set so dead (defined-but-unregistered) commands are flagged, not silently treated as wired.

**Files:** Create `registry.rs`; modify `lib.rs`. Test: `registry_ingest.rs`.

- [ ] **Step 1: Failing test**

```rust
use vox_graphify_reader::registry::{tauri_command_nodes, RegistryNode};
#[test]
fn extracts_tauri_commands_and_flags_unregistered() {
    let src = "#[tauri::command]\npub async fn do_it(x:u64)->Result<(),String>{Ok(())}\n#[tauri::command]\nfn hidden(){}\nfn helper(){}";
    let registered = ["do_it"]; // generate_handler! lists do_it but not hidden
    let nodes = tauri_command_nodes(src, &registered);
    let d = nodes.iter().find(|n| n.id == "cmd:do_it").unwrap();
    assert_eq!(d.kind, "command"); assert!(!d.unregistered);
    let h = nodes.iter().find(|n| n.id == "cmd:hidden").unwrap();
    assert!(h.unregistered, "hidden should be flagged dead");
    assert!(!nodes.iter().any(|n| n.label == "helper"));
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** `registry.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct RegistryNode { pub id: String, pub label: String, pub kind: String, pub unregistered: bool }
impl RegistryNode {
    fn new(prefix: &str, name: &str, kind: &str) -> Self {
        RegistryNode { id: format!("{prefix}:{name}"), label: name.to_string(), kind: kind.to_string(), unregistered: false }
    }
}

/// Parse `#[tauri::command]` fns with syn; flag those absent from `registered`.
pub fn tauri_command_nodes(src: &str, registered: &[&str]) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    let Ok(file) = syn::parse_file(src) else { return out };
    for item in file.items {
        if let syn::Item::Fn(f) = item {
            let is_cmd = f.attrs.iter().any(|a| a.path().segments.iter().any(|s| s.ident == "command"));
            if is_cmd {
                let name = f.sig.ident.to_string();
                let mut n = RegistryNode::new("cmd", &name, "command");
                n.unregistered = !registered.contains(&name.as_str());
                out.push(n);
            }
        }
    }
    out
}
```

(Add `pub mod registry;` to `lib.rs`. The `generate_handler!` registered list is parsed once at ingest time — Task D3 — by reading `main.rs` and collecting the final path segment of each entry; pass that slice here.)

- [ ] **Step 4: Run tests** → PASS. **Step 5: Commit** — `feat(graphify): syn-based Tauri command nodes + unregistered flag`.

### Task C2: MCP-tool + surface adapters (corrected `viewKey` parser) + real-file count guards (TDD)

**Files:** Modify `registry.rs`. Test: `registry_ingest.rs`.

- [ ] **Step 1: Failing test**

```rust
use vox_graphify_reader::registry::{mcp_tool_nodes, surface_nodes};
#[test]
fn extracts_tools_and_surfaces_viewkey() {
    let dispatch = "  \"vox_resolve_feedback\" => f::r(a),\n  \"vox_skill_info\" => s::i(a),";
    assert!(mcp_tool_nodes(dispatch).iter().any(|n| n.id == "tool:vox_resolve_feedback" && n.kind == "tool"));
    let reg = "{ viewKey: 'chat', cliGroup: null, tier: 'live_backend' },\n  { viewKey: null, cliGroup: 'add', tier: 'none' },";
    let s = surface_nodes(reg);
    assert!(s.iter().any(|n| n.id == "surface:chat"));
    assert!(!s.iter().any(|n| n.label == "null"), "null viewKey must be skipped");
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** — `mcp_tool_nodes`: for each line, take the quoted literal left of `=>`; if it starts with `vox_`, emit `tool:` node. `surface_nodes`: match `viewKey:` (NOT `id:`), skip when the value starts with `null`, else take the quoted id → `surface:` node. **Step 4:** add two guard tests that run the adapters over the REAL files (read `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` and `crates/vox-orchestrator-mcp/src/dispatch.rs` at test time via a path relative to `CARGO_MANIFEST_DIR/../..`) and assert a sane lower bound (e.g. `surfaces.len() >= 20`, `tools.len() >= 30`) — this catches silent under-extraction from formatting drift. `cargo test -p vox-graphify-reader registry_ingest` → PASS. **Step 5: Commit** — `feat(graphify): MCP-tool + surface (viewKey) adapters + real-file count guards`.

### Task C3: Transport-wrapper map (`voxTransport.<method>` → command/tool) (TDD)

~50+ GUI calls go through `voxTransport.<method>()`; the command is bound inside `transport.ts`. Without this, those surfaces look like false orphans.

**Files:** Modify `registry.rs`. Test: `registry_ingest.rs`.

- [ ] **Step 1: Failing test**

```rust
use vox_graphify_reader::registry::transport_wrapper_map;
#[test]
fn maps_wrapper_methods_to_commands() {
    let ts = "doubtTask(taskId: number){ return invoke('doubt_orchestrator_task', { taskId }); }\n  getCatalog(){ return invoke('get_command_catalog'); }";
    let m = transport_wrapper_map(ts);
    assert_eq!(m.get("doubtTask").map(String::as_str), Some("cmd:doubt_orchestrator_task"));
    assert_eq!(m.get("getCatalog").map(String::as_str), Some("cmd:get_command_catalog"));
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** — `pub fn transport_wrapper_map(ts_src: &str) -> std::collections::HashMap<String,String>`: scan for a method header `name(...){` followed (within the method body, simple brace-depth or same-line) by `invoke('CMD'` / `invoke('invoke_mcp_tool', { tool: 'T'` — map `name → cmd:CMD` or `tool:T`. A line/region scan is acceptable here (transport.ts wrappers are one-liners); add a guard test over the real `transport.ts` asserting `>= 20` mappings. **Step 4:** PASS. **Step 5: Commit** — `feat(graphify): transport-wrapper → command/tool map`.

## Phase D — Boundary edges

### Task D1: `invoke`/`callTool`/`voxTransport.*` boundary edges (TDD)

**Files:** Modify `ast.rs`. Test: `boundary_edges.rs`. Uses A0d node-kind names + the C3 map passed into extraction.

- [ ] **Step 1: Failing test**

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path; use std::collections::HashMap;
use vox_graphify_reader::ast::extract_ast_in_module_with_wrappers;
#[test]
fn invoke_and_wrapper_boundary_edges() {
    let mut wrappers = HashMap::new();
    wrappers.insert("doubtTask".to_string(), "cmd:doubt_orchestrator_task".to_string());
    let content = "function save(){ invoke('save_settings'); voxTransport.doubtTask(7); \
        invoke('invoke_mcp_tool', { tool: 'vox_resolve_feedback' }); }";
    let g = extract_ast_in_module_with_wrappers(Path::new("S.tsx"), content, "S.tsx", &wrappers);
    let t: Vec<&str> = g.edges.iter().map(|e| e.target.as_str()).collect();
    assert!(t.contains(&"cmd:save_settings"));
    assert!(t.contains(&"cmd:doubt_orchestrator_task"));
    assert!(t.contains(&"tool:vox_resolve_feedback"));
    assert!(g.edges.iter().filter(|e| e.target == "cmd:save_settings").count() == 1, "no double-count");
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** —
  - Add `extract_ast_in_module_with_wrappers(path, content, module_id, wrappers: &HashMap<String,String>)`; keep `extract_ast_in_module` as `..._with_wrappers(.., &HashMap::new())`.
  - Define the boundary literals in ONE place: `const BOUNDARY_CALLEES: &[&str] = &["invoke", "callTool"];` (TODO: a declarative config when a 3rd boundary kind lands — see spec §non-fork).
  - In the `call_expression` branch: read the callee field text; `bare = callee.rsplit('.').next()`. If `bare` ∈ `BOUNDARY_CALLEES`: read arg0 string; if `"invoke_mcp_tool"`, read `tool:` from the arg1 object → `tool:<t>`; else → `cmd:<arg0>`; push `confidence:"declared"`, then `continue` (do NOT also emit the bare-call edge — prevents the double-count). If the callee is `voxTransport.<m>` (i.e. `callee` starts with `voxTransport.` and `bare` is a wrapper key), emit the mapped `wrappers[bare]` target with `confidence:"declared"`. Template-string / computed args → no string value → emit nothing (honest miss).
  - Helpers `string_literal_value` / `object_string_field` use the A0d-recorded node kinds.

- [ ] **Step 4: Run tests** → PASS (iterate kind names if needed). **Step 5: Commit** — `feat(graphify): invoke/callTool/voxTransport boundary edges (declared)`.

### Task D2: Dead-end-preserving resolution for prefixed targets (TDD)

Unresolved boundary targets must NOT vanish — they are the dead-ends coverage exists to find.

**Files:** Modify `rebuild.rs` (`resolve_edges`). Test: in-module `resolve_tests`.

- [ ] **Step 1: Failing test** — append:

```rust
#[test]
fn prefixed_target_resolves_or_dangles() {
    use crate::ast::{ExtractedNode, ExtractedEdge};
    let nodes = vec![ExtractedNode{ id:"cmd:real".into(), label:"real".into(), kind:"command".into() }];
    let edges = vec![
        ExtractedEdge{ source:"S.tsx::a".into(), target:"cmd:real".into(), confidence:"declared".into() },
        ExtractedEdge{ source:"S.tsx::b".into(), target:"cmd:gone".into(), confidence:"declared".into() },
    ];
    let out = resolve_edges(&nodes, &edges);
    assert!(out.iter().any(|e| e.target=="cmd:real" && e.confidence=="declared"));
    let dangling = out.iter().find(|e| e.target=="cmd:gone").expect("dead-end edge must survive");
    assert_eq!(dangling.confidence, "dangling");
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** — at the top of `resolve_edges`, for targets starting `cmd:`/`tool:`/`surface:`: if the node id exists → keep edge with its `confidence`; else → keep edge but set `confidence:"dangling"` (do NOT drop). The existing same-module rule (for bare targets) is unchanged. **Step 4:** PASS. **Step 5: Commit** — `feat(graphify): dead-end boundary edges survive as confidence=dangling`.

### Task D3: `gui-wiring` mode — single-walk registry ingest + missing nodes (TDD)

**Files:** Modify `rebuild.rs`. Test: `boundary_edges.rs` (end-to-end).

- [ ] **Step 1: Failing test** — tempdir with `crates/vox-gui/ui/src/S.tsx` (`invoke('do_it'); invoke('gone');`), `crates/vox-gui/src/commands/x.rs` (`#[tauri::command] fn do_it`), `crates/vox-gui/src/main.rs` (`generate_handler![commands::x::do_it]`), rebuild mode `gui-wiring`; assert nodes `cmd:do_it` (kind command) and a `missing`-flagged `cmd:gone`, and edges to both. (Add `missing: bool` to the serialized command node when the boundary referenced it but no definition exists.)

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** — in `rebuild_graph`, when `meta.extraction_mode == Some("gui-wiring")`:
  - Build the registered set once: read `crates/vox-gui/src/main.rs`, collect `generate_handler!` entries' final path segments.
  - Build the transport map once: read `crates/vox-gui/ui/src/transport.ts` → `transport_wrapper_map`. Pass it into `extract_ast_in_module_with_wrappers` for each `.ts/.tsx` file **in the single existing walk loop** (no second walk).
  - In that SAME loop, when reading a file, also run the registry adapters on the content (Rust → `tauri_command_nodes(content, &registered)` + `mcp_tool_nodes`; `surfaceRegistry.generated.ts` → `surface_nodes`). Accumulate into a `reg: Vec<RegistryNode>` and add as nodes (carry `unregistered`).
  - After resolution, for every `dangling` boundary edge whose target node is absent, synthesize a `missing`-flagged node so coverage can see the dead-end.

- [ ] **Step 4: Run tests** → PASS. **Step 5: Commit** — `feat(graphify): gui-wiring single-walk registry ingest + missing/unregistered nodes`.

## Phase E — Coverage

### Task E1: Coverage with DeadEnd/CliOnly status (TDD)

**Files:** Create `coverage.rs`; modify `lib.rs`. Test: `coverage.rs`.

- [ ] **Step 1: Failing test**

```rust
use vox_graphify_reader::coverage::{compute_coverage, CoverageStatus};
use serde_json::json;
#[test]
fn classifies_surfaced_orphan_deadend() {
    let g = json!({"nodes":[
        {"id":"cmd:wired","label":"wired","kind":"command","community":"c_0"},
        {"id":"cmd:orphan","label":"orphan","kind":"command","community":"c_0"},
        {"id":"cmd:gone","label":"gone","kind":"command","community":"c_0","missing":true},
        {"id":"S::go","label":"go","kind":"fn","community":"c_0"}],
      "links":[
        {"source":"S::go","target":"cmd:wired","confidence":"declared"},
        {"source":"S::go","target":"cmd:gone","confidence":"dangling"}]});
    let r = compute_coverage(&g, "command");
    let f = |id:&str| r.entries.iter().find(|e| e.id==id).unwrap().status.clone();
    assert_eq!(f("cmd:wired"), CoverageStatus::Surfaced);
    assert_eq!(f("cmd:orphan"), CoverageStatus::OrphanBackend);
    assert_eq!(f("cmd:gone"), CoverageStatus::DeadEnd);
}
```

- [ ] **Step 2: Run, verify fail.** **Step 3: Implement** `coverage.rs` — `enum CoverageStatus { OrphanBackend, DeadEnd, Surfaced, CliOnly }`. For each node of `kind`: collect caller edges targeting it; if the node carries `"missing": true` → `DeadEnd`; else if callers non-empty → `Surfaced`; else `OrphanBackend`. (`CliOnly`: when a command node also appears in the ingested clap/command-catalog set but has no GUI caller — include if the command-catalog adapter is ingested in D3; otherwise mark this enum arm reserved and note in the report that CLI-union scoring is deferred. Pick ONE explicitly and, if deferring, amend the spec's Component-2 line to match.) **Step 4:** PASS. **Step 5: Commit** — `feat(graphify): coverage with DeadEnd/OrphanBackend/Surfaced`.

### Task E2: `vox graphify coverage` subcommand

**Files:** Modify `crates/vox-cli/src/commands/graphify/mod.rs`.

- [ ] **Step 1:** Add the `Coverage { corpus: Option<String>, #[arg(long, default_value="command")] kind: String, out: Option<String> }` variant. **Step 2:** Dispatch arm copies the confirmed `load_all_corpora`/`resolve_ingest_corpus_id`/`corpus_by_id` lines from the `Rebuild` arm, reads `corpus.graph_path`, calls `compute_coverage`, writes/prints JSON. **Step 3:** `cargo build -p vox-cli` → compiles. **Step 4: Commit** — `feat(cli): vox graphify coverage subcommand`.

## Phase F — Activate + generality + metric

### Task F1: `gui-wiring` on, rebuild, measure (excluding any synthetic nodes), generality corpus

- [ ] **Step 1:** `graphify-corpora.v1.yaml`: `vox-gui-surface` → `extraction_mode: gui-wiring`; add `vox-config-graph` (scope `crates/vox-config`, `structural`).
- [ ] **Step 2: Rebuild + measure** both corpora; compute zero-edge% **excluding `missing`-flagged nodes** (they are intentionally dangling targets). Expected: GUI zero-edge% materially below the 51% baseline (driven by JSX composition + boundary edges, NOT by synthetic anchors — there are none). `vox-config-graph` builds with no GUI assumptions.
- [ ] **Step 3: Coverage smoke** — `vox graphify coverage --corpus vox-gui-surface --kind command --out graphify-out/gui-coverage/command-coverage.json`; sanity-check that known dead-ends/orphans appear.
- [ ] **Step 4: Re-verify consumers** (A0c list) against the freshly rebuilt graphs; confirm MCP tools + `vox_graphify_status` still work. **Step 5: Commit** — `feat(graphify): activate gui-wiring + generality corpus; island-drop verified`.

**Plan 1 gate:** all `cargo test -p vox-graphify-reader` green; GUI zero-edge% materially reduced (excluding `missing` nodes); coverage emits Surfaced/Orphan/DeadEnd; non-GUI corpus builds; consumers unbroken. Then `superpowers:requesting-code-review` on the engine diff.

---

# PLAN 2 — GUI coverage + IA blueprint (no GUI code changes)

Orchestration + analysis. **Honesty firewall (preserve):** Graphify supplies STRUCTURE; the LLM/audit supplies JUDGMENT. Every blueprint row labels its `evidence_basis` so the user sees fact vs opinion.

## Phase G — Joined evidence base

### Task G1: Coverage + orphan-nav artifacts
- [ ] Run `vox graphify coverage` for `--kind command|tool|surface` → `graphify-out/gui-coverage/*.json`. Derive `orphans.json` (surfaces in the registry absent from `navigation.ts` `PARENT_CHILD_MAP`). Commit.

### Task G2: Join with the manual honesty audit
- [ ] Join each `surface:` coverage entry with the matching `docs/agents/gui-honesty-findings/<Surface>.json` (honesty branch) → `graphify-out/gui-coverage/joined-evidence.json`: `{surface, tier, nav_parent|null, command_set, command_coverage, dead_ends, behavioral_findings, visual_findings, graph_community, neighbors, redundancy_peers}`. Commit.

## Phase H — Per-dimension analysis (parallel sub-agents, cap 8)

**Seven de-correlated dimensions** (one agent each), each writing `graphify-out/gui-ia/<dim>.json`:
1. **structural-coverage** — merges wiring/command-coverage/reachability (one dataset): report dead-ends, orphan-backends, orphan-nav, unregistered commands.
2. **redundancy** — surfaces sharing command/data sets.
3. **utility** — per the rubric below.
4. **semantic-clarity** — label↔content match, **including naming-collisions** (e.g. Search vs Memory).
5. **structural-cohesion** — surface's graph neighborhood vs its nav group.
6. **journey-coherence** — task flows spanning surfaces (catches "MERGE these — the user shuttles between them").
7. **new-nav-taxonomy** — the synthesizer (consumes 1–6), UX-first per below.

- [ ] **Utility rubric (no telemetry exists — verified; utility is structural + audit, NOT measured use; state this in the output).** Utility = a transparent triple, not a score: `richness {dead=0 cmds | thin=1–2 | rich≥3}` × `reachability {reachable | orphan-nav}` × `uniqueness {unique | redundant}` (from dim 2). **KEEP is the default.** CUT requires a NAMED disqualifier (`dead AND orphan`, or `redundant AND thin`); MERGE is preferred to CUT whenever richness > 0.
- [ ] **New-nav method (two-pass).** PRIMARY = UX-first grouping by user job-to-be-done under explicit principles: (a) task coherence, (b) label predicts content (kills Latin names), (c) 7±2 balanced groups (kills lopsided knowledge/compute), (d) information scent. SECONDARY = Leiden communities as a *falsifier only*: where a proposed UX group splits a tight code community, record it as `migration_cost`, never as a reason to regroup. Per group record `members | UX rationale | dominant community | agreement: aligned | crosses-community(cost)`.
- [ ] **Per-dimension agent prompt (template):** as the honesty audit, but each finding carries `evidence_basis ∈ {structural, audit, judgment, structural+judgment}` and a `recommendation ∈ {ADD,CUT,MERGE,MOVE,RENAME,CONDENSE,EXPAND,KEEP}` with `confidence`. "Graphify gives STRUCTURE not JUDGMENT — cite structure as evidence, make the judgment explicit. Do not invent surfaces/commands."
- [ ] **Gate H (adversarial recheck):** stratified sample over aggressive (CUT/MERGE) and conservative (KEEP) recs; confirm each against evidence; correct mislabels. Commit `graphify-out/gui-ia/`.

## Phase I — Synthesize the blueprint

### Task I1: `docs/agents/gui-ia-blueprint.md`
- [ ] **Conflict resolution (apply mechanically):** evidence-type precedence structural > audit > judgment (a structural fact can veto a judgment, not vice-versa); on ties prefer the least-destructive verb (KEEP > RENAME/MOVE > CONDENSE > MERGE > CUT); CUT/MERGE needs ≥2 dimensions OR one structural fact, else downgrade to "flag, default KEEP"; record `dissent: <dim>:<rec>` when unresolved.
- [ ] **Per-row required fields** (so Plan 3 is executable), by verb:
  - **CUT:** view_key(s) removed | nav entries to delete | route/hash redirect target | tests to delete/update | downstream refs (grep handle).
  - **MERGE:** absorber | absorbed | features/commands that move vs drop | absorbed view_key → redirect | nav keys retired | component disposition.
  - **MOVE:** from→to parent | PARENT_CHILD_MAP edit | DEFAULT_CHILD_BY_PARENT impact | breadcrumb effect.
  - **RENAME:** old key (keep) | old→new label | NAV_LABELS diff | does the *key* change? (yes → redirect + call-site grep) | docs refs.
  - **ADD:** proposed view_key | parent | the real `cmd:`/`tool:` node it surfaces (MUST cite one) | nearest component to reuse.
  - **KEEP/CONDENSE/EXPAND:** KEEP none; CONDENSE/EXPAND name the sub-elements.
  - Every row carries `evidence_basis`; any `judgment`-only CUT/MERGE/RENAME is visually flagged and routed to ratification Group C.
- [ ] **Migration ledger** (consolidated section): per key-affecting decision — `old view_key → new|null`, a `parseViewFromLocation` fallback for removed keys (route to nearest surviving parent), deprecation policy (silent alias one release vs hard-remove), and the specific spec/test files that assert the key/label (grep handle). The `#view=` hash deep-links MUST NOT silently break.
- [ ] **New-nav before/after** tree (current `navigation.ts` vs proposed) resolving the named smells: orphan-nav surfaces, Search→Memory, Latin names, lopsided groups.
- [ ] Commit `docs(gui-ia): aggressive reorg blueprint + new-nav + migration ledger (pre-ratification)`.

## Phase J — Ratification gate (HUMAN)

- [ ] **Structure the gate for real review** (don't hand over a flat 60-row table):
  - **Recommended-set:** every row pre-marked with a default decision; the user EDITS exceptions, ratification captured as a diff.
  - **Tier by blast radius:** Group A (RENAME/KEEP) opt-out default-accept; Group B (MOVE/MERGE) opt-in review; Group C (CUT, and any judgment-only destructive) explicit per-item confirm with the disqualifier shown.
  - **Bundles:** present the new-nav as one before/after to accept/edit as a unit, plus themed bundles ("retire Latin labels — N renames", "fold orphan-nav — N moves").
  - **Header:** counts per verb + "N destructive decisions, shown first".
- [ ] **STOP.** Capture ratification into the table; re-commit. No reorg execution. Plan 3 (ratified reorg + folded-in caveat completions, scoped to surviving surfaces) is authored only after this gate.

---

## Self-Review (rev2)

- **Spec coverage:** Component 1 → A0–D (confidence A1; JSX composition B1; registry-ingest C1–C3; boundary edges + dead-ends D1–D3). Component 2 → E (DeadEnd/Orphan/Surfaced; CliOnly explicitly built-or-deferred-with-spec-amendment in E1) + G1. Component 3 → G2. Component 4 → H (7 dimensions, utility rubric, UX-first nav). Component 5 → I–J (executable blueprint + migration ledger + tiered human gate). Generality/non-fork → F1 non-GUI corpus + boundary literals isolated in one table (config deferred, spec §non-fork softened accordingly). Honesty → `declared`/`dangling` labels, dead-ends preserved, `evidence_basis` per row.
- **Audit fixes folded:** tsx walk (A0a); `walk_source_files`/`resolve_edges` extracted (A0a/A0b); consumer-impact + digest-shift (A0c); node-kind discovery (A0d); syn commands + registered-set flag (C1); `viewKey` parser + real-file count guards (C2); transport-wrapper map (C3); double-count `continue` asserted (D1); dead-end `dangling`+`missing` (D2/D3); single-walk (D3); richer coverage (E1); import-anchor hack dropped (Phase B note); metric excludes `missing` nodes (F1).
- **Placeholder scan:** every Rust step carries real code anchored to verified structs; the genuinely undiscoverable bits (tree-sitter 0.23.2 kind strings) are resolved by a concrete print-step (A0d) before use, not assumed.
- **Type consistency:** `RegistryNode{id,label,kind,unregistered}` (C1) reused C2/C3/D3; `confidence` values `resolved|declared|dangling` consistent A1→D2→E1; `cmd:`/`tool:`/`surface:` prefixes consistent across C/D/E; `extract_ast_in_module_with_wrappers` (D1) consumed by D3; `CoverageStatus` (E1) consumed by E2.
- **Sequencing:** A0 refactors precede everything; registry nodes (C) + transport map exist before D resolves to them; coverage (E) after the enriched graph; Plan 2 after the Plan-1 gate; ratification (J) before any Plan-3 execution.
- **Known unknowns (flagged, not placeholders):** tree-sitter kind names (A0d resolves); whether to build `CliOnly` now or defer+amend-spec (E1 forces an explicit choice); exact `generate_handler!` parse shape in main.rs (D3 reads it — copy the real macro invocation).
