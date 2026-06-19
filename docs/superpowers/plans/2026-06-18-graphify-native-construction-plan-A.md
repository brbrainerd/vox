# Plan A — Graphify Native Construction Hardening (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` (task-by-task) and `crates/vox-skills/skills/superpowers/test-driven-development.skill.md` (each task). Steps use checkbox (`- [ ]`) syntax.

> **🤖 EXECUTION TARGET — READ FIRST.** Run end-to-end by **Gemini 3.5 Flash inside Google Antigravity**, which is unreliable on long tasks (~48% real-world completion; mid-task termination leaves no checkpoint; quota is a hard cutoff) and hallucinates APIs / has weak long-context recall. This plan is engineered against those failure modes. **Obey the Operating Rules on every task.** Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff/skills: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite context: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A task is done only when its tests pass AND you commit. A crash between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits — if a task changes a function signature, it also fixes every caller in the SAME task.
2. **Verify-before-use (anti-hallucination).** Before any code step that references a symbol/type/path, run the `rg`/read step in that task and confirm it exists with the exact signature shown. If reality differs, STOP and report — do not invent.
3. **Self-contained.** Everything needed is in the task. Do not rely on remembering earlier tasks.
4. **Two-strike circuit breaker.** If a step's verification fails twice, STOP, write a one-paragraph handoff note (what failed, last good commit SHA), hand back. Do not loop the same failing action.
5. **Parallel dispatch.** Tasks are tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`. Only dispatch parallel subagents for `[PARALLEL-SAFE]` tasks whose **Files** sets are disjoint. Never let two subagents write the same file (isolated contexts clobber). See handoff §3.
6. **Vox house rules.** Never `cargo fmt --all` (use `cargo fmt -p <crate>`). Automation is `.vox`, not `.ps1/.sh/.py`. `.md` under `docs/src/` needs YAML frontmatter. No stubs/placeholders (`feedback_no_stubs`).
7. **Verification ritual before each commit** (skill: `verification-before-completion.skill.md`), pasting real output: `cargo test -p <crate>` (PASS counts) → `cargo clippy -p <crate> -- -D warnings` (clean) → `vox stub-check` (no stubs) → `cargo fmt -p <crate>`. Self-review with `requesting-code-review.skill.md` before committing.
8. **Rollback on broken tree.** If a task aborts mid-edit leaving a non-compiling tree, `git reset --hard HEAD` to the last green commit, then re-attempt that single task from scratch. Never build forward on a broken tree.
9. **Skill references.** Design choices → `brainstorming` skill; parallel waves → `dispatching-parallel-agents` skill; isolation → `using-git-worktrees` skill (all under `crates/vox-skills/skills/superpowers/`).
10. **Determinism.** Graph output must be deterministic (stable ordering). No `.unwrap()` on I/O in library code — propagate `Result`. `cargo run -p vox-arch-check` must pass before final commit.

**Goal:** Make the already-native `vox graphify rebuild` produce a freshness-correct manifest and a collision-free, honestly-edged code graph, and keep the cache, coverage overlays, and callers consistent with the new node-id contract — so the native builder is trustworthy as the canonical Python-free builder.

**Architecture:** `vox-graphify-reader::rebuild::rebuild_graph` walks a source tree, extracts per-file ASTs (cached by content hash), Leiden-clusters, writes `graph.json` + manifest. Changes: (A1) manifest fields match `vox-config::graphify::assess_corpus_status`; (A2) node ids are module-qualified, the per-file cache is invalidated by an extractor-version bump, and calls resolve only when unambiguous (honesty rule); (A3) coverage overlays match the new qualified ids by bare suffix; (A4) an end-to-end rebuild→freshness test.

**Tech Stack:** Rust; `syn` (Rust AST); `tree-sitter` (TS/JS); `leiden-rs`; `blake3`; `serde_json`; `chrono` (CLI layer).

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-graphify-reader/src/rebuild.rs` | Build orchestration | Modify (A1, A2) |
| `crates/vox-graphify-reader/src/ast.rs` | AST extraction | Modify (A2) |
| `crates/vox-graphify-reader/src/overlay.rs` | Test-target overlay | Modify (A3) |
| `crates/vox-graphify-reader/src/reachability.rs` | LCOV reachability | Modify (A3) |
| `crates/vox-graphify-reader/tests/rebuild_tests.rs` | Builder tests | Create (A1, A2) |
| `crates/vox-graphify-reader/tests/overlay_tests.rs` | Overlay tests | Modify (A3) |
| `crates/vox-graphify-reader/tests/reachability_tests.rs` | Reachability tests | Modify (A3) |
| `crates/vox-cli/src/commands/graphify/mod.rs` | CLI `Rebuild` arm | Modify (A1) |
| `crates/vox-cli/tests/graphify_rebuild.rs` | CLI rebuild test | Modify (A1, A4) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "rebuild_graph" crates/` — enumerate ALL call sites. Expected: definition in `rebuild.rs`; callers in `crates/vox-cli/src/commands/graphify/mod.rs` and `crates/vox-cli/tests/graphify_rebuild.rs`. Every caller is fixed in Task A1.
- `rg -n "pub struct GraphifyManifest" -A14 crates/vox-config/src/graphify.rs` — confirm manifest fields are exactly `corpus_id, built_at, git_sha, scope_path, node_count, edge_count, graph_json_sha256, extraction_mode, lexical_ingest_sha256`. The field is `git_sha` (NOT `git_sha256`).
- `rg -n "fn get_cached_hash|fn write_cache|fn load_cache" crates/vox-graphify-reader/src/cache.rs` — confirm the cache API.
- `rg -n 'node.get\("id"\)' crates/vox-graphify-reader/src/overlay.rs crates/vox-graphify-reader/src/reachability.rs` — confirm both match on the raw `id` (the regression A3 fixes).
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task A1 `[SEQUENTIAL]`: Freshness-correct manifest + signature change (all callers)

The current manifest writes `git_sha256:"dev-sha"` with no `built_at`/`graph_json_sha256`, so `assess_corpus_status` can never detect `git_drift`/`ttl_expired`/`lexical_lag`. Fix the manifest and thread real metadata from the caller. This task changes `rebuild_graph`'s signature, so it fixes BOTH callers in the same commit (atomic rule).

**Files:**
- Modify: `crates/vox-graphify-reader/src/rebuild.rs`
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (`Rebuild` arm)
- Modify: `crates/vox-cli/tests/graphify_rebuild.rs` (existing 4-arg call)
- Test: `crates/vox-graphify-reader/tests/rebuild_tests.rs` (new)

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight `rg -n "rebuild_graph" crates/` and `rg -n "pub struct GraphifyManifest" -A14 crates/vox-config/src/graphify.rs`. Confirm two callers and the `git_sha` field name. If different, STOP.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/rebuild_tests.rs`:

```rust
use std::fs;
use vox_graphify_reader::rebuild::{rebuild_graph, RebuildMeta};

fn read_graph(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn manifest_has_freshness_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn alpha() { beta(); }\nfn beta() {}").unwrap();

    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta {
        corpus_id: "test-corpus".to_string(),
        git_sha: Some("abc123".to_string()),
        scope_path: "src".to_string(),
        extraction_mode: Some("structural".to_string()),
        built_at_rfc3339: "2026-06-18T00:00:00+00:00".to_string(),
    };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();

    let m: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("out/.graphify_manifest.v1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(m["git_sha"], "abc123"); // field is git_sha, NOT git_sha256
    assert_eq!(m["built_at"], "2026-06-18T00:00:00+00:00");
    assert_eq!(m["corpus_id"], "test-corpus");
    assert_eq!(m["scope_path"], "src");
    assert_eq!(m["extraction_mode"], "structural");
    assert!(m["graph_json_sha256"].as_str().unwrap().len() >= 32);
    assert!(m["node_count"].as_u64().unwrap() >= 2);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test rebuild_tests manifest_has_freshness_fields` → FAIL to compile (`RebuildMeta`/5-arg `rebuild_graph` missing).

- [ ] **Step 4: Implement in `rebuild.rs`.** Add the struct after the `use` lines:

```rust
/// Caller-supplied metadata so the manifest is freshness-correct. Field names of the
/// written manifest match `vox_config::graphify::GraphifyManifest`.
#[derive(Debug, Clone, Default)]
pub struct RebuildMeta {
    pub corpus_id: String,
    pub git_sha: Option<String>,
    pub scope_path: String,
    pub extraction_mode: Option<String>,
    pub built_at_rfc3339: String,
}
```

Change the signature to add `meta: &RebuildMeta` as the 5th parameter:

```rust
pub fn rebuild_graph(
    _repo_root: &Path,
    source_dir: &Path,
    output_file: &Path,
    cache_dir: &Path,
    meta: &RebuildMeta,
) -> Result<(), Box<dyn std::error::Error>> {
```

Replace the graph-write + manifest block (everything from `let final_graph = serde_json::json!({` through the final `fs::write(manifest_path, ...)?;`) with:

```rust
    let final_graph = serde_json::json!({
        "nodes": nodes_val,
        "links": links_val
    });
    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let graph_bytes = serde_json::to_string_pretty(&final_graph)?;
    fs::write(output_file, &graph_bytes)?;

    // Content digest of the exact bytes written. Despite the legacy field name
    // `graph_json_sha256`, the digest is BLAKE3 (already a dep); the ingest path MUST
    // use the same algorithm so `lexical_lag` comparisons are valid.
    let graph_digest = blake3::hash(graph_bytes.as_bytes()).to_hex().to_string();

    let manifest_val = serde_json::json!({
        "corpus_id": meta.corpus_id,
        "built_at": meta.built_at_rfc3339,
        "git_sha": meta.git_sha,
        "scope_path": meta.scope_path,
        "node_count": nodes_val.len(),
        "edge_count": links_val.len(),
        "graph_json_sha256": graph_digest,
        "extraction_mode": meta.extraction_mode,
    });
    let manifest_path = output_file
        .parent()
        .ok_or("output_file has no parent directory")?
        .join(".graphify_manifest.v1.json");
    fs::write(manifest_path, serde_json::to_string_pretty(&manifest_val)?)?;

    Ok(())
}
```

- [ ] **Step 5: Fix caller 1 — CLI.** In `crates/vox-cli/src/commands/graphify/mod.rs`, in the `GraphifyCmd::Rebuild` arm, replace the `rebuild_graph(repo_root, &source_dir, &output_file, &cache_dir)` call with a `RebuildMeta` build + 5-arg call:

```rust
            let meta = vox_graphify_reader::rebuild::RebuildMeta {
                corpus_id: corpus_id.clone(),
                git_sha: resolve_head_sha()?,
                scope_path: corpus.scope_path.clone(),
                extraction_mode: corpus.extraction_mode.clone(),
                built_at_rfc3339: Utc::now().to_rfc3339(),
            };
            vox_graphify_reader::rebuild::rebuild_graph(
                repo_root,
                &source_dir,
                &output_file,
                &cache_dir,
                &meta,
            )
            .map_err(|e| anyhow::anyhow!("Rebuild failed: {}", e))?;
```

(`Utc` and `resolve_head_sha` already exist in this file — confirm with `rg -n "use chrono::Utc|fn resolve_head_sha" crates/vox-cli/src/commands/graphify/mod.rs`.)

- [ ] **Step 6: Fix caller 2 — test.** In `crates/vox-cli/tests/graphify_rebuild.rs`, the existing call is 4-arg. Change it to pass a default meta:

```rust
    let res = vox_graphify_reader::rebuild::rebuild_graph(
        tmp.path(),
        &src,
        &output_file,
        &cache_dir,
        &vox_graphify_reader::rebuild::RebuildMeta::default(),
    );
```

- [ ] **Step 7: Run → PASS + full build.** `cargo test -p vox-graphify-reader --test rebuild_tests manifest_has_freshness_fields` → PASS. `cargo test -p vox-cli --test graphify_rebuild` → PASS. `cargo build -p vox-cli` → clean (no other callers should remain; if Pre-flight found more, fix them here).

- [ ] **Step 8: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/rebuild_tests.rs crates/vox-cli/src/commands/graphify/mod.rs crates/vox-cli/tests/graphify_rebuild.rs
git commit -m "fix(graphify): native rebuild writes freshness-correct manifest (git_sha/built_at/graph_json_sha256)"
```

---

## Task A2 `[SEQUENTIAL]` (ast.rs + rebuild.rs): Qualified ids + cache invalidation + sound edges

Today every symbol is a global node keyed by its bare name, so two files defining `fn new` collapse into one fake god-node. This task qualifies definition ids by module, **bumps an extractor version folded into the cache key** (so unchanged files re-extract under the new id scheme instead of returning stale cached graphs), and resolves each call only when unambiguous (drops ambiguous/self/unresolved — honesty rule).

**Files:**
- Modify: `crates/vox-graphify-reader/src/ast.rs`
- Modify: `crates/vox-graphify-reader/src/rebuild.rs`
- Test: `crates/vox-graphify-reader/tests/rebuild_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "fn extract_ast|struct RustVisitor|fn get_cached_hash" crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/src/cache.rs`. Confirm `extract_ast(path, content)`, `RustVisitor { nodes, edges, current_fn }`, and that `rebuild.rs` computes `let hash = blake3::hash(content.as_bytes())...`. If different, STOP.

- [ ] **Step 2: Write the failing tests.** Append to `crates/vox-graphify-reader/tests/rebuild_tests.rs`:

```rust
#[test]
fn same_named_fns_in_different_files_do_not_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn make() {}").unwrap();
    fs::write(src.join("b.rs"), "fn make() {}").unwrap();
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(tmp.path(), &src, &out, &tmp.path().join("out/fc"), &RebuildMeta::default()).unwrap();
    let g = read_graph(&out);
    let makes: Vec<String> = g["nodes"].as_array().unwrap().iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .filter(|id| id.ends_with("::make")).collect();
    assert_eq!(makes.len(), 2, "expected 2 qualified make() nodes, got {makes:?}");
    assert_ne!(makes[0], makes[1]);
}

#[test]
fn intra_file_call_resolves_within_module() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn caller() { callee(); }\nfn callee() {}").unwrap();
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(tmp.path(), &src, &out, &tmp.path().join("out/fc"), &RebuildMeta::default()).unwrap();
    let links = read_graph(&out)["links"].as_array().unwrap().clone();
    assert_eq!(links.len(), 1, "links: {links:?}");
    assert!(links[0]["source"].as_str().unwrap().ends_with("::caller"));
    assert!(links[0]["target"].as_str().unwrap().ends_with("::callee"));
}

#[test]
fn ambiguous_and_self_calls_are_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.rs"), "fn shared() {}").unwrap();
    fs::write(src.join("b.rs"), "fn shared() {}").unwrap();
    fs::write(src.join("c.rs"), "fn user() { shared(); }").unwrap();        // ambiguous → drop
    fs::write(src.join("d.rs"), "fn recur() { recur(); }").unwrap();        // self → drop
    let out = tmp.path().join("out/graph.json");
    rebuild_graph(tmp.path(), &src, &out, &tmp.path().join("out/fc"), &RebuildMeta::default()).unwrap();
    assert_eq!(read_graph(&out)["links"].as_array().unwrap().len(), 0);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test rebuild_tests` → the three new tests FAIL.

- [ ] **Step 4: Implement in `ast.rs`.** Add near the top (after the struct defs):

```rust
/// Bump when the extraction scheme changes (node-id format, edge rules). Folded into the
/// per-file cache key in `rebuild` so unchanged files re-extract instead of returning a
/// graph built under the old scheme.
pub const EXTRACTOR_VERSION: &str = "2";

/// Qualify a symbol with its module path. Empty `module_id` yields the bare symbol so the
/// legacy `extract_ast` wrapper keeps its old output.
pub(crate) fn qualify(module_id: &str, sym: &str) -> String {
    if module_id.is_empty() { sym.to_string() } else { format!("{module_id}::{sym}") }
}
```

Change `RustVisitor` to carry `module_id: String`, qualify def ids + edge sources (targets stay bare for later resolution):

```rust
struct RustVisitor {
    module_id: String,
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
    current_fn: Option<String>,
}

impl<'ast> Visit<'ast> for RustVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let fn_name = node.sig.ident.to_string();
        let id = qualify(&self.module_id, &fn_name);
        self.nodes.push(ExtractedNode { id: id.clone(), label: fn_name, kind: "fn".to_string() });
        let old_fn = self.current_fn.replace(id);
        syn::visit::visit_item_fn(self, node);
        self.current_fn = old_fn;
    }
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let struct_name = node.ident.to_string();
        self.nodes.push(ExtractedNode {
            id: qualify(&self.module_id, &struct_name),
            label: struct_name,
            kind: "struct".to_string(),
        });
        syn::visit::visit_item_struct(self, node);
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(ref expr_path) = *node.func {
            if let Some(ref current_fn) = self.current_fn {
                if let Some(segment) = expr_path.path.segments.last() {
                    self.edges.push(ExtractedEdge {
                        source: current_fn.clone(),
                        target: segment.ident.to_string(), // BARE; resolved in rebuild
                    });
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}
```

Replace `pub fn extract_ast` with a module-aware version + back-compat wrapper. Qualify the tree-sitter branch the same way (def id + `current_fn` qualified; edge target = raw callee text):

```rust
/// Back-compat wrapper: bare ids. Used by `ast_tests` (which assert on `label`, unaffected).
pub fn extract_ast(path: &Path, content: &str) -> ExtractedGraph {
    extract_ast_in_module(path, content, "")
}

/// Per-file AST. Definition ids and edge sources are qualified with `module_id`; edge
/// targets are left bare for global resolution in `rebuild`.
pub fn extract_ast_in_module(path: &Path, content: &str, module_id: &str) -> ExtractedGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    if path.extension().map_or(false, |ext| ext == "rs") {
        if let Ok(file) = syn::parse_file(content) {
            let mut visitor = RustVisitor {
                module_id: module_id.to_string(),
                nodes: Vec::new(),
                edges: Vec::new(),
                current_fn: None,
            };
            visitor.visit_file(&file);
            nodes = visitor.nodes;
            edges = visitor.edges;
        }
    } else {
        #[cfg(feature = "tree-sitter-grammars")]
        {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let language = match ext {
                    "ts" | "js" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
                    "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX),
                    _ => None,
                };
                if let Some(lang) = language {
                    let mut parser = tree_sitter::Parser::new();
                    if parser.set_language(&lang.into()).is_ok() {
                        if let Some(tree) = parser.parse(content, None) {
                            let mut cursor = tree.walk();
                            let mut stack = vec![tree.root_node()];
                            let mut current_fn: Option<String> = None;
                            while let Some(node) = stack.pop() {
                                if node.kind() == "function_declaration"
                                    || node.kind() == "method_definition"
                                {
                                    if let Some(name_node) = node.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                            let id = qualify(module_id, name);
                                            nodes.push(ExtractedNode {
                                                id: id.clone(),
                                                label: name.to_string(),
                                                kind: "fn".to_string(),
                                            });
                                            current_fn = Some(id);
                                        }
                                    }
                                }
                                if node.kind() == "call_expression" {
                                    if let Some(ref source_fn) = current_fn {
                                        if let Some(function_node) =
                                            node.child_by_field_name("function")
                                        {
                                            if let Ok(callee) =
                                                function_node.utf8_text(content.as_bytes())
                                            {
                                                edges.push(ExtractedEdge {
                                                    source: source_fn.clone(),
                                                    target: callee.to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                                for child in node.children(&mut cursor) {
                                    stack.push(child);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ExtractedGraph { nodes, edges }
}
```

- [ ] **Step 5: Implement in `rebuild.rs`.** Change the import line to use the new fn:

```rust
use super::ast::{ExtractedEdge, ExtractedGraph, ExtractedNode, EXTRACTOR_VERSION, extract_ast_in_module};
```

Inside the `WalkDir` loop, fold the extractor version into the cache key and compute a module id; replace the `let hash = ...` line and the `let graph = if manager... extract_ast(...)` block with:

```rust
                    // Cache key includes EXTRACTOR_VERSION so a scheme change invalidates
                    // stale cached graphs even when file content is unchanged.
                    let hash = blake3::hash(
                        format!("{EXTRACTOR_VERSION}\u{0}{content}").as_bytes(),
                    )
                    .to_hex()
                    .to_string();
                    let module_id = path
                        .strip_prefix(source_dir)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let graph = if manager.get_cached_hash(path).as_deref() == Some(&hash) {
                        manager
                            .load_cache(path)
                            .unwrap_or_else(|| extract_ast_in_module(path, &content, &module_id))
                    } else {
                        let g = extract_ast_in_module(path, &content, &module_id);
                        manager.write_cache(path, &hash, &g);
                        g
                    };
```

After the loop (right after `all_nodes`/`all_edges` are fully collected, before the `cluster_nodes_input` mapping), insert the resolver:

```rust
    // Resolve each bare call target to a qualified definition id. Preference: same-module
    // definition; else the unique global definition. Ambiguous, unresolved, and self-edges
    // are dropped (honesty rule: never invent an edge).
    fn module_of(id: &str) -> &str {
        id.rsplit_once("::").map(|(m, _)| m).unwrap_or("")
    }
    let mut defs_by_name: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for n in &all_nodes {
        let bare = n.id.rsplit("::").next().unwrap_or(&n.id).to_string();
        defs_by_name.entry(bare).or_default().push(n.id.clone());
    }
    let all_edges: Vec<ExtractedEdge> = all_edges
        .iter()
        .filter_map(|e| {
            let candidates = defs_by_name.get(&e.target)?;
            let src_mod = module_of(&e.source);
            let same: Vec<&String> =
                candidates.iter().filter(|id| module_of(id) == src_mod).collect();
            let target = if same.len() == 1 {
                same[0].clone()
            } else if candidates.len() == 1 {
                candidates[0].clone()
            } else {
                return None;
            };
            if target == e.source {
                return None; // self-edge
            }
            Some(ExtractedEdge { source: e.source.clone(), target })
        })
        .collect();
```

(The existing `cluster_edges_input` and `links_val` code below already iterates `all_edges`, now the resolved list — no further change.)

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-graphify-reader` → PASS (the 3 new tests + existing `ast_tests` which assert on `label`).

- [ ] **Step 7: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/rebuild_tests.rs
git commit -m "feat(graphify): module-qualified ids + cache-version invalidation + ambiguity-safe edges"
```

---

## Task A3 `[PARALLEL-SAFE]` (overlay.rs + reachability.rs; after A2): coverage overlays match qualified ids

A2 made real graph node ids `module::symbol`, but `overlay_test_targets` and `ingest_lcov_reachability` match on the raw `id` against bare symbol names — so against a real rebuilt graph they would match nothing. Make both match by the bare suffix of the node id. (Disjoint files from A4 → the two may run in parallel after A2.)

**Files:**
- Modify: `crates/vox-graphify-reader/src/overlay.rs`
- Modify: `crates/vox-graphify-reader/src/reachability.rs`
- Test: `crates/vox-graphify-reader/tests/overlay_tests.rs`, `crates/vox-graphify-reader/tests/reachability_tests.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n 'node.get\("id"\)' crates/vox-graphify-reader/src/overlay.rs crates/vox-graphify-reader/src/reachability.rs`. Confirm each looks up by the raw `id`. If already suffix-matching, STOP (work done).

- [ ] **Step 2: Write failing tests** with qualified ids. Append to `crates/vox-graphify-reader/tests/overlay_tests.rs`:

```rust
#[test]
fn overlay_matches_qualified_node_ids() {
    let graph = json!({
        "nodes": [{"id": "src/a.rs::func_a", "label": "func_a", "kind": "fn"}],
        "links": []
    });
    let test_src = "#[test]\nfn test_func_a() { func_a(); }";
    let updated = overlay_test_targets(&graph, "src/test.rs", test_src).unwrap();
    let n = &updated["nodes"].as_array().unwrap()[0];
    assert_eq!(n["targeted_by"].as_array().unwrap()[0].as_str().unwrap(), "test_func_a");
}
```

Append to `crates/vox-graphify-reader/tests/reachability_tests.rs`:

```rust
#[test]
fn reachability_matches_qualified_node_ids() {
    let graph = json!({
        "nodes": [{"id": "src/main.rs::hello", "label": "hello", "kind": "fn"}],
        "links": []
    });
    let lcov = "SF:src/main.rs\nFN:3,hello\nFNDA:5,hello\nend_of_record\n";
    let updated = ingest_lcov_reachability(&graph, lcov).unwrap();
    assert_eq!(updated["nodes"].as_array().unwrap()[0]["execution_count"].as_u64().unwrap(), 5);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test overlay_tests overlay_matches_qualified_node_ids` and `--test reachability_tests reachability_matches_qualified_node_ids` → FAIL (no match → `targeted_by`/`execution_count` absent or 0).

- [ ] **Step 4: Implement.** In `overlay.rs`, change the node loop to match the bare suffix:

```rust
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                let bare = id.rsplit("::").next().unwrap_or(id);
                if let Some(test_names) = targets.get(bare) {
                    node.as_object_mut()
                        .unwrap()
                        .insert("targeted_by".to_string(), json!(test_names));
                }
            }
        }
```

In `reachability.rs`, likewise:

```rust
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                let bare = id.rsplit("::").next().unwrap_or(id);
                let count = execution_counts.get(bare).copied().unwrap_or(0);
                node.as_object_mut()
                    .unwrap()
                    .insert("execution_count".to_string(), json!(count));
            }
        }
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-graphify-reader --test overlay_tests --test reachability_tests` → PASS (new + original bare-id tests both pass, since `rsplit("::").next()` on a bare id returns the bare id).

- [ ] **Step 6: Verify (Rule 7) + commit.**

```bash
git add crates/vox-graphify-reader/src/overlay.rs crates/vox-graphify-reader/src/reachability.rs crates/vox-graphify-reader/tests/overlay_tests.rs crates/vox-graphify-reader/tests/reachability_tests.rs
git commit -m "fix(graphify): coverage overlays match module-qualified node ids by bare suffix"
```

---

## Task A4 `[PARALLEL-SAFE]` (graphify_rebuild.rs only; after A2): end-to-end rebuild → freshness

Proves the manifest fix makes `assess_corpus_status` report `fresh` after a rebuild and `git_drift` when HEAD moves. Lives in `vox-cli` (depends on both crates). Disjoint files from A3 → parallel-safe after A2.

**Files:**
- Modify: `crates/vox-cli/tests/graphify_rebuild.rs`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "pub struct GraphifyCorpus" -A14 crates/vox-config/src/graphify.rs`. Note EVERY field so the struct literal below is complete. If Plan B's `source_root` field is already present, include `source_root: None,`.

- [ ] **Step 2: Write the test.** Append to `crates/vox-cli/tests/graphify_rebuild.rs`:

```rust
#[test]
fn rebuild_then_assess_is_fresh_and_detects_drift() {
    use chrono::Utc;
    use vox_config::graphify::{assess_corpus_status, GraphifyCorpus};
    use vox_graphify_reader::rebuild::{rebuild_graph, RebuildMeta};

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "fn one() { two(); }\nfn two() {}").unwrap();

    let graph_rel = ".vox/cache/graphify/repo-code-graph/graph.json";
    let out = tmp.path().join(graph_rel);
    let meta = RebuildMeta {
        corpus_id: "repo-code-graph".to_string(),
        git_sha: Some("headsha".to_string()),
        scope_path: "src".to_string(),
        extraction_mode: Some("structural".to_string()),
        built_at_rfc3339: Utc::now().to_rfc3339(),
    };
    rebuild_graph(tmp.path(), &src, &out, &out.parent().unwrap().join("file_cache"), &meta).unwrap();

    let corpus = GraphifyCorpus {
        id: "repo-code-graph".to_string(),
        title: "t".to_string(),
        scope_path: "src".to_string(),
        graph_path: graph_rel.to_string(),
        manifest_path: ".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json".to_string(),
        extraction_mode: Some("structural".to_string()),
        default_for_intents: vec![],
        is_virtual: false,
        // If Plan B landed: source_root: None,
    };
    let fresh = assess_corpus_status(tmp.path(), &corpus, Some("headsha"), Utc::now(), 30);
    assert!(fresh.is_fresh, "stale: {:?}", fresh.stale_reasons);
    let drifted = assess_corpus_status(tmp.path(), &corpus, Some("other"), Utc::now(), 30);
    assert!(drifted.stale_reasons.contains(&"git_drift".to_string()));
}
```

- [ ] **Step 3: Run → PASS.** `cargo test -p vox-cli --test graphify_rebuild rebuild_then_assess_is_fresh_and_detects_drift` → PASS.

- [ ] **Step 4: Verify (Rule 7) + `cargo run -p vox-arch-check` + commit.**

```bash
git add crates/vox-cli/tests/graphify_rebuild.rs
git commit -m "test(graphify): end-to-end rebuild produces a fresh, drift-detecting corpus"
```

---

## Parallelization summary (for the Antigravity orchestrator)

- **A1 → A2 are a SEQUENTIAL chain** (A1 changes the signature A2's tests call; both touch `rebuild.rs`). One agent, in order.
- **After A2, A3 and A4 are PARALLEL-SAFE** (A3 = `overlay.rs`/`reachability.rs` + their tests; A4 = `graphify_rebuild.rs`). Disjoint files → dispatch together. See handoff §3.
- Never parallelize A1/A2 (shared `rebuild.rs`).

## Self-Review

- **Spec coverage:** manifest correctness (A1), collision-free + honest edges + cache invalidation (A2), coverage-overlay consistency with the new id contract (A3 — the evidence-based fix this edition adds), freshness proof (A4).
- **Placeholder scan:** none; full code in every code step. Every signature change fixes all callers in-task (atomic rule).
- **Type consistency:** `RebuildMeta` fields identical across A1/A4; `extract_ast_in_module(path, content, module_id)` identical in ast.rs/rebuild.rs; manifest key `git_sha` everywhere; bare-suffix match `id.rsplit("::").next().unwrap_or(id)` identical in A2 resolver, A3 overlay, A3 reachability.
- **Antigravity fit:** each task atomic+green+committed; verify-before-use first step on every task; two-strike + rollback rules stated; parallel tags applied; cache-version bump prevents a silent stale-graph failure a fast model would not catch.
- **Deferred:** coverage-overlay *wiring into rebuild* = `2026-06-18-graphify-native-coverage-overlays.md`; `lexical_ingest_sha256` side of `lexical_lag` = Plan C. Crate is named `vox-graphify-reader` but now builds graphs — note in suite index; not renamed here (out of scope).
