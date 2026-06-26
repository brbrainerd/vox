# Graphify General Enhancement + GUI IA Blueprint — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance the general, Rust-native Graphify engine so it captures the codebase's real structure — string-dispatch boundaries (`invoke`→command), React composition, and declared registries — then use it to produce a ratifiable blueprint for an aggressive Vox GUI reorganization.

**Architecture:** Two plans. **Plan 1 (Phases A–F)** is general-engine Rust work in `vox-graphify-reader` + `vox-cli`: an edge-confidence/node-kind schema bump, composition edges, registry-ingest nodes, string-dispatch boundary edges, and a new `vox graphify coverage` subcommand — proven on the GUI corpus *and* a non-GUI corpus (generality, no fork). **Plan 2 (Phases G–J)** runs the enriched engine on the GUI, joins it with the existing 32-surface honesty audit, and produces an adversarially-verified per-item IA blueprint + new-nav proposal for human ratification (no GUI code changes).

**Tech Stack:** Rust (`syn` for Rust AST, `tree-sitter`/`tree-sitter-typescript` for TS/TSX, `petgraph`/`leiden-rs`, `serde_json`, `anyhow`, `clap`); the existing `vox graphify` CLI + `graphify-corpora.v1.yaml` registry; agent fan-out (dispatching-parallel-agents) for Plan 2 analysis.

**Spec:** `docs/superpowers/specs/2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint-design.md` (read first).

**Base branch note:** `main` does not currently compile (`vox-cli` `db_cli` WIP breakage); this plan is authored on `claude/graphify-general-gui-ia` off the compiling honesty branch. Build/test with `cargo ... -p vox-graphify-reader` (fast, isolated) wherever possible; full `vox-cli` builds only where the CLI subcommand is touched.

---

## Key internals (verified — trust these; file paths are exact)

- **`crates/vox-graphify-reader/src/ast.rs`** — `ExtractedNode { id, label, kind: String }`, `ExtractedEdge { source, target }`, `ExtractedGraph { nodes, edges }`; `qualify(module_id, sym)` → `"module::sym"`; `extract_ast_in_module(path, content, module_id) -> ExtractedGraph`. Rust via `syn` visitor; TS/TSX/JS/PY via a `tree_sitter` walk under `#[cfg(feature = "tree-sitter-grammars")]` (default on). TS node kinds matched today: `function_declaration`, `method_definition`, `class_definition`, `call_expression`; call target via `node.child_by_field_name("function")`.
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `rebuild_graph(repo_root, source_dir, output_file, cache_dir, meta: &RebuildMeta)`; serializes `graph.json` as `{nodes:[{id,label,kind,community}], links:[{source,target}]}` (lines ~92–122); `resolve_edges(nodes, edges)` applies the honesty rule (same-module unique match, else drop; drop self-edges) (lines ~170–213); `module_of(id)` = everything before the final `::`; writes `.graphify_manifest.v1.json`.
- **`crates/vox-graphify-reader/src/lens.rs`** — `collapse_to_modules(graph)` (the `modules` mode).
- **`crates/vox-cli/src/commands/graphify/mod.rs`** — `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap }`; `pub async fn run(cmd, repo_root) -> anyhow::Result<()>`. Rebuild loads the registry via `load_all_corpora`, resolves the corpus, builds `RebuildMeta`, calls `rebuild_graph`.
- **`crates/vox-config/src/graphify.rs`** — `GraphifyCorpus { id, title, scope_path, graph_path, manifest_path, extraction_mode: Option<String>, default_for_intents, is_virtual, source_root }`; `GraphifyManifest { corpus_id, built_at, git_sha, scope_path, node_count, edge_count, graph_json_sha256, extraction_mode, lexical_ingest_sha256 }`; `load_all_corpora(repo_root)`.
- **`contracts/retrieval/graphify-corpora.v1.yaml`** — corpus registry; `vox-gui-surface` corpus scoped to `crates/vox-gui`.
- **Tests:** `crates/vox-graphify-reader/tests/{ast_tests.rs, rebuild_tests.rs, python_tests.rs}` — tempdir fixtures, `extract_ast_in_module`, assert on `graph.nodes/edges`. Python test gated with `#![cfg(feature = "tree-sitter-grammars")]`.
- **Edge digest:** `graph_digest(bytes)` = BLAKE3 (despite the `_sha256` field name).

---

## File Structure

**Plan 1 — modified/created**
- `crates/vox-graphify-reader/src/ast.rs` — add `confidence` to `ExtractedEdge`; extend the TS walk for JSX/import/`invoke`/`callTool`; emit boundary edges with prefixed targets.
- `crates/vox-graphify-reader/src/rebuild.rs` — carry `confidence` through `resolve_edges` + `graph.json`; add prefixed-target resolution (global-unique for `cmd:`/`tool:`); manifest gets `confidence_counts`.
- `crates/vox-graphify-reader/src/registry.rs` *(new)* — registry-ingest adapters (surface registry, Tauri command set, MCP tool set, command catalog) → typed `ExtractedNode`s + join edges.
- `crates/vox-graphify-reader/src/coverage.rs` *(new)* — `compute_coverage(graph, registry_kind) -> CoverageReport`.
- `crates/vox-graphify-reader/src/lib.rs` — `pub mod registry; pub mod coverage;`.
- `crates/vox-graphify-reader/tests/{boundary_edges.rs, composition_edges.rs, registry_ingest.rs, coverage.rs}` *(new)*.
- `crates/vox-cli/src/commands/graphify/mod.rs` — add `Coverage` subcommand + `run_graphify_coverage`.
- `contracts/retrieval/graphify-corpora.v1.yaml` — set `vox-gui-surface` `extraction_mode: gui-wiring`; add a small non-GUI corpus for the generality check.

**Plan 2 — created (artifacts, no GUI code)**
- `graphify-out/gui-coverage/` — `wiring-map.json`, `command-coverage.json`, `orphans.json`, `redundancy.json`.
- `docs/agents/gui-ia-blueprint.md` — the ratification table + new-nav before/after.

---

# PLAN 1 — General Graphify engine

## Phase A — Edge confidence + node-kind schema

### Task A1: Add `confidence` to edges; carry it through resolution and output (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs`, `crates/vox-graphify-reader/src/rebuild.rs`. Test: `crates/vox-graphify-reader/tests/ast_tests.rs` (extend).

- [ ] **Step 1: Failing test** — append to `ast_tests.rs`:

```rust
#[test]
fn edges_default_to_resolved_confidence() {
    use vox_graphify_reader::ast::extract_ast_in_module;
    let g = extract_ast_in_module(
        std::path::Path::new("m.rs"),
        "fn a() { b(); }\nfn b() {}",
        "m.rs",
    );
    assert!(g.edges.iter().all(|e| e.confidence == "resolved"），
        "edges: {:?}", g.edges);
}
```

(Correct the stray full-width parens if your editor inserts them; the assertion is `g.edges.iter().all(|e| e.confidence == "resolved")`.)

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader edges_default_to_resolved_confidence`. Expected: FAIL — no field `confidence`.

- [ ] **Step 3: Implement** — in `ast.rs`, change the struct and every `ExtractedEdge { source, target }` construction:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    #[serde(default = "default_confidence")]
    pub confidence: String,
}
fn default_confidence() -> String { "resolved".to_string() }
```

Update the Rust visitor's call-edge push and the tree-sitter call push to include `confidence: "resolved".to_string()`.

In `rebuild.rs` `resolve_edges`, carry confidence through: the returned `ExtractedEdge` must set `confidence: e.confidence.clone()`. In the `graph.json` links serialization (lines ~92–122) add the field:

```rust
let links_val: Vec<serde_json::Value> = all_edges
    .iter()
    .map(|e| serde_json::json!({
        "source": e.source,
        "target": e.target,
        "confidence": e.confidence,
    }))
    .collect();
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS (existing tests still green; new test passes). Fix any construction sites the compiler flags.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/ast_tests.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): edge confidence field (resolved by default), carried to graph.json"
```

## Phase B — Composition / usage edges (collapse the islands)

### Task B1: JSX element-usage edges in the TS/TSX walk (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs` (the tree-sitter walk). Test: `crates/vox-graphify-reader/tests/composition_edges.rs` (new).

- [ ] **Step 1: Failing test** — new file `composition_edges.rs`:

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;

#[test]
fn jsx_usage_emits_composition_edge() {
    let content = r#"
function Parent() {
  return <Child />;
}
function Child() { return null; }
"#;
    let g = extract_ast_in_module(Path::new("Parent.tsx"), content, "Parent.tsx");
    // Parent uses <Child/> → edge Parent -> Child (bare target, resolved later)
    assert!(
        g.edges.iter().any(|e| e.source == "Parent.tsx::Parent" && e.target == "Child"),
        "edges: {:?}", g.edges
    );
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader jsx_usage_emits_composition_edge`. Expected: FAIL.

- [ ] **Step 3: Implement** — in the tree-sitter `while let Some(node) = stack.pop()` loop in `ast.rs`, add a JSX branch alongside the `is_call` branch. A TSX `<Child/>` parses as `jsx_self_closing_element` (and `<Child>...</Child>` as `jsx_opening_element`); the component name is the `name` field (an `identifier` for capitalized components):

```rust
let is_jsx = matches!(node.kind(), "jsx_self_closing_element" | "jsx_opening_element");
if is_jsx {
    if let Some(ref source_fn) = current_fn {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                // Only treat Capitalized names as components (lowercase = DOM tags).
                if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    edges.push(ExtractedEdge {
                        source: source_fn.clone(),
                        target: name.to_string(),
                        confidence: "resolved".to_string(),
                    });
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader composition_edges`. Expected: PASS. If the `name` field is a `member_expression` (e.g. `<Foo.Bar/>`), `utf8_text` returns the dotted text; that's acceptable (resolves to nothing → dropped honestly).

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/tests/composition_edges.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): JSX element-usage composition edges (TS/TSX)"
```

### Task B2: ES import edges (module → module) (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs`. Test: extend `composition_edges.rs`.

Import edges connect modules, not symbols. We model an import as an edge from a synthetic module node (`<module_id>::<module>`) — but to stay within the existing node model (defs only), we instead emit an edge from the *importing module's first/zero definition* is brittle. Simpler and honest: emit a module-level import edge as `source = module_id + "::*"`, `target = imported specifier`, kind handled by a dedicated confidence label `import`. Consumers (coverage, lens) treat `*` nodes as module anchors.

- [ ] **Step 1: Failing test** — append:

```rust
#[test]
fn es_import_emits_module_edge() {
    let content = "import { Child } from './Child';\nfunction Parent(){ return null; }\n";
    let g = extract_ast_in_module(Path::new("Parent.tsx"), content, "src/Parent.tsx");
    assert!(
        g.edges.iter().any(|e| e.source == "src/Parent.tsx::*"
            && e.target == "Child" && e.confidence == "import"),
        "edges: {:?}", g.edges
    );
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader es_import_emits_module_edge`. Expected: FAIL.

- [ ] **Step 3: Implement** — add an import branch in the walk. `import { X } from '...'` parses as `import_statement` with an `import_clause` → `named_imports` → `import_specifier` nodes (each an `identifier`), plus a `string` source. Emit one edge per imported identifier:

```rust
if node.kind() == "import_statement" {
    let anchor = format!("{module_id}::*");
    // Walk descendants for identifiers inside import specifiers.
    let mut c2 = node.walk();
    let mut substack = vec![node];
    while let Some(n) = substack.pop() {
        if n.kind() == "import_specifier" || n.kind() == "namespace_import" {
            if let Some(idn) = n.child_by_field_name("name").or_else(|| n.named_child(0)) {
                if let Ok(name) = idn.utf8_text(content.as_bytes()) {
                    edges.push(ExtractedEdge {
                        source: anchor.clone(),
                        target: name.to_string(),
                        confidence: "import".to_string(),
                    });
                }
            }
        }
        for ch in n.children(&mut c2) { substack.push(ch); }
    }
}
```

Also ensure a module-anchor node exists so the edge has a real source: after the file walk, if any `import`-confidence edge was emitted, push `ExtractedNode { id: format!("{module_id}::*"), label: "*".into(), kind: "module-anchor".into() }` once.

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader composition_edges`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/tests/composition_edges.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): ES import edges + module-anchor nodes (TS/TSX)"
```

## Phase C — Registry-ingest nodes (the targets boundary edges resolve to)

### Task C1: Registry adapter framework + Tauri-command + MCP-tool node ingest (TDD)

**Files:** Create `crates/vox-graphify-reader/src/registry.rs`; modify `crates/vox-graphify-reader/src/lib.rs` (`pub mod registry;`). Test: `crates/vox-graphify-reader/tests/registry_ingest.rs` (new).

Command/tool names are *globally unique by design* (Tauri dispatch + MCP dispatch are global). We ingest them as typed nodes with prefixed ids so boundary edges (Phase D) can resolve to them by exact id.

- [ ] **Step 1: Failing test** — new file `registry_ingest.rs`:

```rust
use vox_graphify_reader::registry::{tauri_command_nodes, RegistryNode};

#[test]
fn extracts_tauri_command_nodes() {
    let src = r#"
#[tauri::command]
pub async fn do_it(x: u64) -> Result<(), String> { Ok(()) }
fn helper() {}
"#;
    let nodes = tauri_command_nodes("crates/vox-gui/src/commands/x.rs", src);
    assert!(nodes.iter().any(|n| n.id == "cmd:do_it" && n.label == "do_it" && n.kind == "command"));
    assert!(!nodes.iter().any(|n| n.label == "helper"));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader extracts_tauri_command_nodes`. Expected: FAIL — module not found.

- [ ] **Step 3: Implement** `registry.rs`:

```rust
//! Registry-ingest: turn declared registries (Tauri commands, MCP tools, the GUI
//! surface registry, the command catalog) into typed graph nodes that boundary
//! edges can resolve to. General capability; the GUI is the first consumer.

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryNode {
    pub id: String,    // prefixed, globally unique, e.g. "cmd:do_it" / "tool:vox_x" / "surface:chat"
    pub label: String, // bare name
    pub kind: String,  // "command" | "tool" | "surface" | "registry-entry"
}

/// Find `#[tauri::command]`-attributed fns in a Rust source file → command nodes.
/// Deterministic line scan (no syn dependency needed here): an attribute line
/// containing `tauri::command` immediately preceding (within 3 lines) an `fn NAME`.
pub fn tauri_command_nodes(_path: &str, src: &str) -> Vec<RegistryNode> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.contains("tauri::command") {
            // look ahead a few lines for `fn NAME`
            for look in lines.iter().skip(i + 1).take(4) {
                if let Some(name) = fn_name_after(look) {
                    out.push(RegistryNode {
                        id: format!("cmd:{name}"),
                        label: name.clone(),
                        kind: "command".to_string(),
                    });
                    break;
                }
            }
        }
    }
    out
}

fn fn_name_after(line: &str) -> Option<String> {
    let t = line.trim_start();
    let rest = t.strip_prefix("pub ").unwrap_or(t);
    let rest = rest.strip_prefix("async ").unwrap_or(rest);
    let rest = rest.strip_prefix("fn ")?;
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if name.is_empty() { None } else { Some(name) }
}
```

(Honesty note: a line scan can miss exotic formatting; that is acceptable — it under-reports, never invents. A later refinement could use `syn` attribute parsing.)

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader registry_ingest`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/registry.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/registry_ingest.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): registry-ingest — Tauri command nodes"
```

### Task C2: MCP-tool + surface-registry adapters (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/registry.rs`. Test: extend `registry_ingest.rs`.

- [ ] **Step 1: Failing test** — append:

```rust
use vox_graphify_reader::registry::{mcp_tool_nodes, surface_nodes};

#[test]
fn extracts_mcp_tool_and_surface_nodes() {
    // MCP tools: names appear as string literals registered in a dispatch table.
    let dispatch = r#"  "vox_resolve_feedback" => feedback::resolve(args),
  "vox_skill_info" => skills::info(args),"#;
    let tools = mcp_tool_nodes(dispatch);
    assert!(tools.iter().any(|n| n.id == "tool:vox_resolve_feedback" && n.kind == "tool"));

    // Surface registry: the generated TS registry lists surface ids.
    let reg = r#"export const SURFACE_REGISTRY = [
  { id: 'chat', tier: 'live_backend' },
  { id: 'memory', tier: 'live_backend' },
];"#;
    let surfaces = surface_nodes(reg);
    assert!(surfaces.iter().any(|n| n.id == "surface:chat" && n.kind == "surface"));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader extracts_mcp_tool_and_surface_nodes`. Expected: FAIL.

- [ ] **Step 3: Implement** — add to `registry.rs`:

```rust
/// MCP tool names: string literals on the left of a `=>` dispatch arm beginning with `vox_`.
pub fn mcp_tool_nodes(dispatch_src: &str) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    for line in dispatch_src.lines() {
        if let Some(name) = quoted_before_arrow(line) {
            if name.starts_with("vox_") {
                out.push(RegistryNode { id: format!("tool:{name}"), label: name.clone(), kind: "tool".into() });
            }
        }
    }
    out
}

fn quoted_before_arrow(line: &str) -> Option<String> {
    let arrow = line.find("=>")?;
    let head = &line[..arrow];
    let start = head.find('"')? + 1;
    let end = head[start..].find('"')? + start;
    Some(head[start..end].to_string())
}

/// Surface ids: `id: 'X'` entries in the generated SURFACE_REGISTRY.
pub fn surface_nodes(registry_src: &str) -> Vec<RegistryNode> {
    let mut out = Vec::new();
    for line in registry_src.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("id:") {
            let rest = rest.trim_start();
            if let Some(q) = rest.strip_prefix('\'').or_else(|| rest.strip_prefix('"')) {
                let id: String = q.chars().take_while(|c| *c != '\'' && *c != '"').collect();
                if !id.is_empty() {
                    out.push(RegistryNode { id: format!("surface:{id}"), label: id.clone(), kind: "surface".into() });
                }
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader registry_ingest`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/registry.rs crates/vox-graphify-reader/tests/registry_ingest.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): registry-ingest — MCP tool + GUI surface nodes"
```

## Phase D — String-dispatch boundary edges

### Task D1: Extract `invoke('cmd')` / `callTool('tool')` boundary edges (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs` (the TS walk). Test: `crates/vox-graphify-reader/tests/boundary_edges.rs` (new).

Boundary edges target the prefixed registry ids (`cmd:`/`tool:`) so resolution is exact-global, not same-module.

- [ ] **Step 1: Failing test** — new file `boundary_edges.rs`:

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;

#[test]
fn invoke_emits_boundary_edge_to_command() {
    let content = r#"
function save() {
  invoke('save_settings', { x: 1 });
}
"#;
    let g = extract_ast_in_module(Path::new("S.tsx"), content, "S.tsx");
    assert!(
        g.edges.iter().any(|e| e.source == "S.tsx::save"
            && e.target == "cmd:save_settings" && e.confidence == "declared"),
        "edges: {:?}", g.edges
    );
}

#[test]
fn invoke_mcp_tool_emits_tool_edge() {
    let content = r#"
function go() {
  invoke('invoke_mcp_tool', { tool: 'vox_resolve_feedback', args: {} });
}
"#;
    let g = extract_ast_in_module(Path::new("S.tsx"), content, "S.tsx");
    assert!(
        g.edges.iter().any(|e| e.target == "tool:vox_resolve_feedback" && e.confidence == "declared"),
        "edges: {:?}", g.edges
    );
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader boundary_edges`. Expected: FAIL.

- [ ] **Step 3: Implement** — in the TS walk's `is_call` branch, after capturing the callee text, special-case `invoke`/`callTool`. The first argument is a `string` node; for `invoke('invoke_mcp_tool', { tool: 'X' })` read the `tool:` value from the second (object) argument. tree-sitter `call_expression` has an `arguments` field (an `arguments` node whose named children are the args):

```rust
if is_call {
    if let Some(ref source_fn) = current_fn {
        if let Some(fnode) = node.child_by_field_name("function") {
            if let Ok(callee) = fnode.utf8_text(content.as_bytes()) {
                let bare = callee.rsplit('.').next().unwrap_or(callee); // voxTransport.invoke -> invoke
                if bare == "invoke" || bare == "callTool" {
                    if let Some(args) = node.child_by_field_name("arguments") {
                        let mut ac = args.walk();
                        let arg_nodes: Vec<_> = args.named_children(&mut ac).collect();
                        let first = arg_nodes.get(0)
                            .and_then(|n| string_literal_value(n, content));
                        if let Some(cmd) = first {
                            if cmd == "invoke_mcp_tool" {
                                if let Some(obj) = arg_nodes.get(1) {
                                    if let Some(tool) = object_string_field(obj, content, "tool") {
                                        edges.push(ExtractedEdge { source: source_fn.clone(),
                                            target: format!("tool:{tool}"), confidence: "declared".into() });
                                    }
                                }
                            } else {
                                edges.push(ExtractedEdge { source: source_fn.clone(),
                                    target: format!("cmd:{cmd}"), confidence: "declared".into() });
                            }
                        }
                    }
                    // do not also emit the bare-call edge for invoke/callTool
                    for child in node.children(&mut cursor) { stack.push(child); }
                    continue;
                }
                // ... existing bare-call edge push ...
            }
        }
    }
}
```

Add two helpers to `ast.rs`:

```rust
#[cfg(feature = "tree-sitter-grammars")]
fn string_literal_value(n: &tree_sitter::Node, content: &str) -> Option<String> {
    if n.kind() == "string" {
        let t = n.utf8_text(content.as_bytes()).ok()?;
        return Some(t.trim_matches(|c| c == '\'' || c == '"' || c == '`').to_string());
    }
    None
}

#[cfg(feature = "tree-sitter-grammars")]
fn object_string_field(obj: &tree_sitter::Node, content: &str, field: &str) -> Option<String> {
    // obj is an `object` node; find a `pair` whose key == field, value a string.
    let mut c = obj.walk();
    for pair in obj.named_children(&mut c) {
        if pair.kind() == "pair" {
            let key = pair.child_by_field_name("key")?;
            let kt = key.utf8_text(content.as_bytes()).ok()?
                .trim_matches(|ch| ch == '\'' || ch == '"');
            if kt == field {
                let val = pair.child_by_field_name("value")?;
                return string_literal_value(&val, content);
            }
        }
    }
    None
}
```

(The exact tree-sitter child structure may differ slightly; if `string_literal_value` returns `None` on a `template_string`, that is an honest miss — log nothing, drop. Verify the actual node kinds by running the test and, if needed, `println!` the `node.kind()` of the first argument.)

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader boundary_edges`. Expected: PASS. Iterate on node-kind names if the first test reveals different kinds.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/tests/boundary_edges.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): invoke/callTool boundary edges → cmd:/tool: targets"
```

### Task D2: Global-exact resolution for prefixed targets (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/rebuild.rs` (`resolve_edges`). Test: `crates/vox-graphify-reader/tests/boundary_edges.rs` (extend with a rebuild-level test).

`resolve_edges` currently drops cross-module edges. Boundary targets (`cmd:X`/`tool:X`) must instead resolve by *exact node id* (the registry node), preserving `confidence`.

- [ ] **Step 1: Failing test** — append a rebuild-level test that injects a command node + a boundary edge and asserts the edge survives resolution. Because `resolve_edges` is private, test via the public `rebuild_graph` over a tempdir containing a `.tsx` with `invoke('do_it')` and a `.rs` with `#[tauri::command] fn do_it`, plus the registry-ingest wired in Task D3. To keep Task D2 self-contained, expose a thin `pub fn resolve_edges_pub(nodes, edges)` test shim, or make `resolve_edges` `pub(crate)` and add the test in a `#[cfg(test)] mod` inside `rebuild.rs`:

```rust
#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::ast::{ExtractedNode, ExtractedEdge};
    #[test]
    fn prefixed_target_resolves_to_registry_node_globally() {
        let nodes = vec![
            ExtractedNode { id: "a.tsx::save".into(), label: "save".into(), kind: "fn".into() },
            ExtractedNode { id: "cmd:do_it".into(), label: "do_it".into(), kind: "command".into() },
        ];
        let edges = vec![ExtractedEdge {
            source: "a.tsx::save".into(), target: "cmd:do_it".into(), confidence: "declared".into() }];
        let out = resolve_edges(&nodes, &edges);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, "cmd:do_it");
        assert_eq!(out[0].confidence, "declared");
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader prefixed_target_resolves`. Expected: FAIL (current code drops it — the bare-name index has `do_it` → `cmd:do_it`, but `module_of("cmd:do_it")` is `cmd:do_it`-with-no-`::`... and the target is already `cmd:do_it`, not a bare name, so the `defs_by_name.get(&e.target)` lookup misses).

- [ ] **Step 3: Implement** — at the top of `resolve_edges`, short-circuit prefixed targets:

```rust
fn resolve_edges(nodes: &[ExtractedNode], edges: &[ExtractedEdge]) -> Vec<ExtractedEdge> {
    use std::collections::HashSet;
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    // ... existing defs_by_name build ...
    edges.iter().filter_map(|e| {
        // Boundary/registry targets resolve by exact global id.
        if e.target.starts_with("cmd:") || e.target.starts_with("tool:") || e.target.starts_with("surface:") {
            return if node_ids.contains(e.target.as_str()) {
                Some(ExtractedEdge { source: e.source.clone(), target: e.target.clone(), confidence: e.confidence.clone() })
            } else {
                None // declared boundary to a non-existent command → DROP (honest; surfaces a dead-end)
            };
        }
        // ... existing same-module honesty rule, but carry confidence ...
    }).collect()
}
```

Ensure the existing branch sets `confidence: e.confidence.clone()` on its returned edge.

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS (all, including existing).

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/boundary_edges.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): global-exact resolution for cmd:/tool:/surface: boundary targets"
```

### Task D3: Wire registry-ingest into `rebuild_graph` under the `gui-wiring` mode (TDD)

**Files:** Modify `crates/vox-graphify-reader/src/rebuild.rs`. Test: `crates/vox-graphify-reader/tests/boundary_edges.rs` (end-to-end rebuild).

- [ ] **Step 1: Failing test** — append an end-to-end test: a tempdir with `crates/vox-gui/ui/src/S.tsx` (`invoke('do_it')`), `crates/vox-gui/src/commands/x.rs` (`#[tauri::command] fn do_it`), rebuild with `extraction_mode = Some("gui-wiring")`, then load `graph.json` and assert a link `S.tsx::* or S.tsx::fn → cmd:do_it` exists and a node `cmd:do_it` exists.

```rust
#[test]
#[cfg(feature = "tree-sitter-grammars")]
fn gui_wiring_mode_connects_invoke_to_command() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let ui = root.join("crates/vox-gui/ui/src");
    let cmds = root.join("crates/vox-gui/src/commands");
    std::fs::create_dir_all(&ui).unwrap();
    std::fs::create_dir_all(&cmds).unwrap();
    std::fs::write(ui.join("S.tsx"), "function save(){ invoke('do_it'); }").unwrap();
    std::fs::write(cmds.join("x.rs"), "#[tauri::command]\npub async fn do_it()->Result<(),String>{Ok(())}").unwrap();
    let out = root.join("out/graph.json");
    let cache = root.join("out/file_cache");
    let meta = RebuildMeta { corpus_id: "t".into(), git_sha: None, scope_path: "crates/vox-gui".into(),
        extraction_mode: Some("gui-wiring".into()), built_at_rfc3339: "2026-06-26T00:00:00+00:00".into() };
    rebuild_graph(root, &root.join("crates/vox-gui"), &out, &cache, &meta).unwrap();
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let has_cmd = g["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "cmd:do_it");
    let has_edge = g["links"].as_array().unwrap().iter().any(|l| l["target"] == "cmd:do_it");
    assert!(has_cmd, "no cmd node");
    assert!(has_edge, "no boundary edge");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader gui_wiring_mode_connects_invoke`. Expected: FAIL (mode not handled; no registry nodes injected).

- [ ] **Step 3: Implement** — in `rebuild_graph`, when `meta.extraction_mode.as_deref() == Some("gui-wiring")`, after the per-file extraction loop and before clustering, inject registry nodes by scanning the source tree:

```rust
if meta.extraction_mode.as_deref() == Some("gui-wiring") {
    use crate::registry::{tauri_command_nodes, mcp_tool_nodes, surface_nodes, RegistryNode};
    let mut reg: Vec<RegistryNode> = Vec::new();
    for entry in walk_files(source_dir) { // reuse the existing file walk helper
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        let p = entry.to_string_lossy();
        if p.ends_with(".rs") { reg.extend(tauri_command_nodes(&p, &text)); reg.extend(mcp_tool_nodes(&text)); }
        if p.ends_with("surfaceRegistry.generated.ts") { reg.extend(surface_nodes(&text)); }
    }
    for r in reg {
        if !all_nodes.iter().any(|n| n.id == r.id) {
            all_nodes.push(ExtractedNode { id: r.id, label: r.label, kind: r.kind });
        }
    }
}
```

(`walk_files` / the existing recursive scan: reuse whatever `rebuild_graph` already uses to enumerate files; if it is inline, factor a small `fn walk_files(dir)->Vec<PathBuf>` and reuse it.) Resolution (Task D2) then connects the `declared` boundary edges to these nodes; unmatched boundary edges drop (honest dead-end signal).

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/boundary_edges.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): gui-wiring mode injects registry nodes + connects boundary edges"
```

## Phase E — `vox graphify coverage` subcommand

### Task E1: Coverage computation over an enriched graph (TDD)

**Files:** Create `crates/vox-graphify-reader/src/coverage.rs`; modify `lib.rs`. Test: `crates/vox-graphify-reader/tests/coverage.rs` (new).

- [ ] **Step 1: Failing test** — new file `coverage.rs`:

```rust
use vox_graphify_reader::coverage::{compute_coverage, CoverageStatus};
use serde_json::json;

#[test]
fn classifies_surfaced_orphan_and_deadend() {
    let graph = json!({
      "nodes": [
        {"id":"cmd:wired","label":"wired","kind":"command","community":"c_0"},
        {"id":"cmd:orphan","label":"orphan","kind":"command","community":"c_0"},
        {"id":"S.tsx::go","label":"go","kind":"fn","community":"c_0"}
      ],
      "links": [
        {"source":"S.tsx::go","target":"cmd:wired","confidence":"declared"}
      ]
    });
    let rep = compute_coverage(&graph, "command");
    let wired = rep.entries.iter().find(|e| e.id == "cmd:wired").unwrap();
    let orphan = rep.entries.iter().find(|e| e.id == "cmd:orphan").unwrap();
    assert_eq!(wired.status, CoverageStatus::Surfaced);
    assert_eq!(orphan.status, CoverageStatus::OrphanBackend);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader classifies_surfaced`. Expected: FAIL.

- [ ] **Step 3: Implement** `coverage.rs`:

```rust
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, PartialEq, Debug, Clone)]
pub enum CoverageStatus { Surfaced, OrphanBackend }

#[derive(Serialize)]
pub struct CoverageEntry { pub id: String, pub label: String, pub status: CoverageStatus, pub callers: Vec<String> }

#[derive(Serialize)]
pub struct CoverageReport { pub registry_kind: String, pub entries: Vec<CoverageEntry> }

/// For every node of kind `registry_kind` (e.g. "command"/"tool"/"surface"),
/// classify by whether any edge targets it.
pub fn compute_coverage(graph: &Value, registry_kind: &str) -> CoverageReport {
    let empty = vec![];
    let nodes = graph.get("nodes").and_then(|v| v.as_array()).unwrap_or(&empty);
    let links = graph.get("links").and_then(|v| v.as_array()).unwrap_or(&empty);
    let mut entries = Vec::new();
    for n in nodes {
        if n.get("kind").and_then(|v| v.as_str()) != Some(registry_kind) { continue; }
        let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let label = n.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let callers: Vec<String> = links.iter()
            .filter(|l| l.get("target").and_then(|v| v.as_str()) == Some(id.as_str()))
            .filter_map(|l| l.get("source").and_then(|v| v.as_str()).map(String::from))
            .collect();
        let status = if callers.is_empty() { CoverageStatus::OrphanBackend } else { CoverageStatus::Surfaced };
        entries.push(CoverageEntry { id, label, status, callers });
    }
    CoverageReport { registry_kind: registry_kind.to_string(), entries }
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader coverage`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/coverage.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/coverage.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): coverage computation (surfaced vs orphan-backend)"
```

### Task E2: `vox graphify coverage` CLI subcommand

**Files:** Modify `crates/vox-cli/src/commands/graphify/mod.rs`.

- [ ] **Step 1: Add the enum variant** next to `Rebuild`:

```rust
    /// Compute registry-vs-implementation coverage from an enriched corpus graph.
    Coverage {
        #[arg(long)]
        corpus: Option<String>,
        /// Registry node kind to score: command | tool | surface.
        #[arg(long, default_value = "command")]
        kind: String,
        /// Write the JSON report to this path (default: stdout).
        #[arg(long)]
        out: Option<String>,
    },
```

- [ ] **Step 2: Add the dispatch arm** in `run()`:

```rust
        GraphifyCmd::Coverage { corpus, kind, out } => {
            let reg = load_all_corpora(repo_root)?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus)?;
            let corpus = corpus_by_id(&reg, &corpus_id)?;
            let graph_path = repo_root.join(&corpus.graph_path);
            let bytes = std::fs::read(&graph_path)
                .with_context(|| format!("read graph {}", graph_path.display()))?;
            let graph: serde_json::Value = serde_json::from_slice(&bytes)?;
            let report = vox_graphify_reader::coverage::compute_coverage(&graph, &kind);
            let json = serde_json::to_string_pretty(&report)?;
            match out {
                Some(p) => { std::fs::write(repo_root.join(&p), json)?; println!("coverage: wrote {p}"); }
                None => println!("{json}"),
            }
        }
```

(Match the exact helper names already used by the `Rebuild` arm: `load_all_corpora`, `resolve_ingest_corpus_id`, `corpus_by_id`. Add `use anyhow::Context;` if not present.)

- [ ] **Step 3: Build + manual smoke**

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo build -p vox-cli 2>&1 | tail -5
```
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/graphify/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(cli): vox graphify coverage subcommand"
```

## Phase F — Activate GUI corpus + generality check

### Task F1: Switch the GUI corpus to `gui-wiring`, rebuild, prove the island-drop + run a non-GUI corpus

**Files:** Modify `contracts/retrieval/graphify-corpora.v1.yaml`.

- [ ] **Step 1:** In `graphify-corpora.v1.yaml`, set the `vox-gui-surface` corpus `extraction_mode: gui-wiring`. Add one small non-GUI corpus to prove generality (the engine, not a GUI fork), e.g.:

```yaml
  - id: vox-config-graph
    title: vox-config code graph (generality check)
    scope_path: crates/vox-config
    graph_path: ".vox/cache/graphify/vox-config-graph/graph.json"
    manifest_path: ".vox/cache/graphify/vox-config-graph/.graphify_manifest.v1.json"
    extraction_mode: structural
    default_for_intents: []
```

- [ ] **Step 2: Rebuild + measure**

```bash
cd /c/Users/Owner/vox-graphify-gui
cargo run -p vox-cli -- graphify rebuild --corpus vox-gui-surface
cargo run -p vox-cli -- graphify rebuild --corpus vox-config-graph
node -e 'const fs=require("fs");for(const c of ["vox-gui-surface","vox-config-graph"]){const g=JSON.parse(fs.readFileSync(`.vox/cache/graphify/${c}/graph.json`,"utf8"));const links=g.links||[];const deg={};for(const n of g.nodes)deg[n.id]=0;for(const l of links){deg[l.source]++;deg[l.target]++;}const orphans=g.nodes.filter(n=>deg[n.id]===0).length;console.log(c,"nodes",g.nodes.length,"links",links.length,"zero-edge%",Math.round(orphans/g.nodes.length*100));}'
```

Expected: for `vox-gui-surface`, **zero-edge% drops sharply** from the 51% baseline (boundary + composition edges now connect TS↔Rust), and `cmd:`/`tool:`/`surface:` nodes are present. `vox-config-graph` builds without GUI-specific assumptions (generality proven).

- [ ] **Step 3: Coverage smoke**

```bash
cargo run -p vox-cli -- graphify coverage --corpus vox-gui-surface --kind command --out graphify-out/gui-coverage/command-coverage.json
```
Expected: a JSON report with `surfaced` vs `orphan-backend` commands.

- [ ] **Step 4: Commit**

```bash
git -C /c/Users/Owner/vox-graphify-gui add contracts/retrieval/graphify-corpora.v1.yaml
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(graphify): activate gui-wiring mode + generality-check corpus; island-drop verified"
```

**Plan 1 gate:** all `cargo test -p vox-graphify-reader` green; GUI zero-edge% materially reduced; coverage subcommand produces a report; a non-GUI corpus rebuilds. Then `superpowers:requesting-code-review` on the engine diff before Plan 2.

---

# PLAN 2 — GUI coverage + IA blueprint (no GUI code changes)

This phase is **orchestration + analysis**, mirroring the honesty audit's fan-out. It produces artifacts and a ratification table; it changes no GUI source.

## Phase G — Build the joined evidence base

### Task G1: Produce the coverage artifacts

- [ ] Run, committing outputs under `graphify-out/gui-coverage/`:

```bash
cd /c/Users/Owner/vox-graphify-gui
cargo run -p vox-cli -- graphify coverage --corpus vox-gui-surface --kind command --out graphify-out/gui-coverage/command-coverage.json
cargo run -p vox-cli -- graphify coverage --corpus vox-gui-surface --kind tool    --out graphify-out/gui-coverage/tool-coverage.json
cargo run -p vox-cli -- graphify coverage --corpus vox-gui-surface --kind surface --out graphify-out/gui-coverage/surface-coverage.json
```

- [ ] **Derive the orphan-nav report** deterministically: a node script that loads `crates/vox-gui/ui/src/lib/navigation.ts`'s `PARENT_CHILD_MAP` + `surface-coverage.json` and lists surfaces present in the registry but absent from the nav map. Write `graphify-out/gui-coverage/orphans.json`. Commit all.

```bash
git -C /c/Users/Owner/vox-graphify-gui add graphify-out/gui-coverage
git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(gui-ia): coverage + orphan-nav artifacts"
```

### Task G2: Join with the manual honesty audit

- [ ] A node script that joins each `surface:` coverage entry with the matching `docs/agents/gui-honesty-findings/<Surface>.json` (from the honesty branch) — producing `graphify-out/gui-coverage/joined-evidence.json`, one record per surface: `{surface, tier, nav_parent|null, command_coverage, behavioral_findings, visual_findings, graph_community, neighbors}`. Commit.

## Phase H — Per-dimension IA analysis (parallel sub-agents, cap 8)

Dispatch via `superpowers:dispatching-parallel-agents`. Each analyst reads `joined-evidence.json` (+ the graph) and writes one findings file under `graphify-out/gui-ia/<dimension>.json`. Dimensions (one agent each): `wiring-completeness`, `command-coverage`, `redundancy`, `utility`, `semantic-clarity`, `structural-cohesion`, `reachability`, `new-nav-taxonomy`.

- [ ] **Per-dimension agent prompt (template — substitute `<DIMENSION>`):**

```
You are analyzing ONE dimension of the Vox GUI for an aggressive reorganization blueprint.
Dimension: <DIMENSION>
Inputs: graphify-out/gui-coverage/joined-evidence.json and the enriched graph at
  .vox/cache/graphify/vox-gui-surface/graph.json (nodes carry kind incl. command/tool/surface;
  links carry confidence: resolved|declared|import).
Graphify gives you STRUCTURE (what connects to what, coverage, communities). It does NOT give
  you JUDGMENT (utility, naming, UX). Use structure as evidence; make the judgment explicitly.
For each surface / nav-group / command relevant to <DIMENSION>, output a finding:
  { unit, unit_kind: surface|nav-group|command, observation, evidence: {graph_path?|coverage?|finding?},
    recommendation: ADD|CUT|MERGE|MOVE|RENAME|CONDENSE|EXPAND|KEEP, rationale, confidence: low|med|high }
Be specific and cite evidence. Do not invent surfaces/commands not in the inputs.
Write JSON to graphify-out/gui-ia/<DIMENSION>.json and return a one-line summary.
```

- [ ] **Gate H (adversarial recheck):** after all dimensions report, dispatch one verifier over a random sample spanning both aggressive (CUT/MERGE) and conservative (KEEP) recommendations: "confirm each recommendation against the evidence; flag any that the structure/audit does not support." Correct mislabels. Commit `graphify-out/gui-ia/`.

## Phase I — Synthesize the blueprint + new-nav proposal

### Task I1: Build `docs/agents/gui-ia-blueprint.md`

- [ ] Merge all `graphify-out/gui-ia/*.json` into one decision table, **one row per unit** (surface, nav-group, command), columns: `unit | kind | current location | coverage | top evidence | DECISION (ADD/CUT/MERGE/MOVE/RENAME/CONDENSE/EXPAND/KEEP) | rationale`. Where dimensions disagree on a unit, record the conflict and pick the recommendation with the strongest evidence, noting the dissent.
- [ ] Append the **new nav taxonomy**: a from-first-principles parent→child tree derived from command-group cohesion + graph communities, presented as a **before/after** against the current `navigation.ts` tree, with a one-line rationale per group. Resolve the known smells explicitly: orphan-nav surfaces (`needs-you`, `mission-control`, `sub-agents`, `activity`, `search`, `graphify`), the Search→Memory mislabel, the Latin names, and the lopsided `knowledge`/`compute` groups.
- [ ] Commit:

```bash
git -C /c/Users/Owner/vox-graphify-gui add docs/agents/gui-ia-blueprint.md graphify-out/gui-ia
git -C /c/Users/Owner/vox-graphify-gui commit -m "docs(gui-ia): aggressive reorg blueprint + new-nav proposal (pre-ratification)"
```

## Phase J — Ratification gate (HUMAN)

- [ ] **Gate J (human):** present `docs/agents/gui-ia-blueprint.md` to the user. STOP. Capture per-item ratification (accept / amend / reject) back into the table and re-commit. Do NOT begin any reorg execution. Authoring Plan 3 (the ratified reorg + folded-in caveat completions, scoped to surviving surfaces) happens only after this gate.

---

## Self-Review

- **Spec coverage:** Component 1 → Phases A–D (edge confidence A1; composition B1–B2; registry-ingest C1–C2; boundary edges D1–D3). Component 2 → Phase E (coverage.rs + CLI) + G1. Component 3 → G2 (join with audit). Component 4 → Phase H (dimensions incl. the full ADD/CUT/MERGE/MOVE/RENAME/CONDENSE/EXPAND rubric + new-nav). Component 5 → Phases I–J (blueprint + human gate). Generality / non-fork → F1 (non-GUI corpus) + the general registry/coverage modules. Honesty principle → `declared` confidence labels + drop-on-miss in D2. Testing → fixture tests per phase + the orphan/coverage artifacts. Non-goals (no semantic overlay, no reorg before ratification, no GUI-specific fork) honored: Plan 2 changes no GUI code; all extraction is in the general engine.
- **Placeholder scan:** every Rust step carries real code anchored to the verified structs (`ExtractedEdge`, `ExtractedNode`, `resolve_edges`, the tree-sitter walk, `GraphifyCmd`, `RebuildMeta`); agent steps carry full prompt + output schema. The few genuine unknowns (exact tree-sitter node-kind names for JSX/import/arguments; the existing file-walk helper name in `rebuild.rs`; the exact CLI corpus-resolve helper names) are each flagged with "verify by running the first test / copy the neighbor," not left as TODO.
- **Type consistency:** `ExtractedEdge.confidence` added in A1 is consumed by B/C/D/E and the graph writer. `RegistryNode {id,label,kind}` defined in C1 reused in C2 + D3. `cmd:`/`tool:`/`surface:` prefix convention is consistent across C (node ids), D1 (edge targets), D2 (resolution), E1 (coverage `kind` filter matches node `kind`, while edge targets use the prefix — coverage filters on node `kind` field, not the prefix, so both are needed and consistent). `compute_coverage(graph, kind)` signature matches between E1 and E2. `gui-wiring` extraction_mode is consistent between D3 (handler) and F1 (corpus config).
- **Sequencing:** registry nodes (C) exist before boundary-edge resolution (D2/D3) needs them; coverage (E) runs after the enriched graph exists; Plan 2 runs after the Plan-1 gate; ratification (J) precedes any Plan-3 execution.
- **Known unknowns flagged for the executor (not placeholders):** tree-sitter node-kind exact strings (`jsx_self_closing_element`, `import_specifier`, `arguments`, `pair`/`object`) must be confirmed by running each phase's first test and, if it fails, printing `node.kind()` — every such step says so; the `rebuild.rs` file-walk helper and the CLI corpus-resolution helpers must be copied from the neighboring `Rebuild` code, named explicitly.
