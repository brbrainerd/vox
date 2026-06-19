# Plan G — Study the Graphify Source (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan A** (`extract_ast_in_module`, tree-sitter branch) **and Plan B** (`vox graphify index`, `source_root`). Both must be landed and green first.

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** First step of each task is an `rg`/read confirming exact symbols/versions. Differs → STOP.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note. No looping.
5. **Parallel dispatch.** Honor tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all`; automation is `.vox` (no `.ps1/.sh/.py`); `docs/src/` `.md` needs YAML frontmatter; no stubs.
7. **Verification ritual** (skill `verification-before-completion`), paste output: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism + no `.unwrap()` on I/O in lib code.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Make Vox able to index and study Graphify's own (Python) source as a first-class corpus, so agents can navigate the upstream implementation and verify the native Rust port against it.

**Architecture:** The native builder is Rust/TS-only, so it would graph nothing from Graphify's Python source. (G1) Add `tree-sitter-python` (declared in the workspace, version-resolved to unify on the workspace `tree-sitter`) and a Python branch in the extractor. (G2) A VoxScript clones the Graphify source and registers+builds it as the `graphify-upstream` corpus via `vox graphify index`. (G3) A port-parity methodology doc mapping each upstream stage to its native Rust equivalent.

**Tech Stack:** Rust; `tree-sitter` + `tree-sitter-python`; VoxScript; Markdown.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `Cargo.toml` (workspace root) | `[workspace.dependencies]` grammar | Modify (G1) |
| `crates/vox-graphify-reader/Cargo.toml` | crate dep + feature | Modify (G1) |
| `crates/vox-graphify-reader/src/ast.rs` | Python branch | Modify (G1) |
| `crates/vox-graphify-reader/src/rebuild.rs` | accept `.py` | Modify (G1) |
| `crates/vox-graphify-reader/tests/python_tests.rs` | Python extraction tests | Create (G1) |
| `scripts/graphify-study-source.vox` | acquire + index upstream | Create (G2) |
| `docs/src/architecture/graphify-upstream-study-methodology-2026-06-18.md` | port-parity methodology | Create (G3) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n 'tree-sitter' Cargo.toml` — note the workspace `tree-sitter` version (expected `0.26.9`) and how grammar deps are declared. The `tree-sitter-python` version chosen in G1 must unify on this `tree-sitter`.
- `rg -n "fn extract_ast_in_module|LANGUAGE_TYPESCRIPT|node.kind\(\)" crates/vox-graphify-reader/src/ast.rs` — confirm Plan A's tree-sitter branch shape (language match + `node.kind()` checks). If absent, STOP — apply Plan A first.
- `rg -n 'ext == "rs" \|\| ext == "ts"' crates/vox-graphify-reader/src/rebuild.rs` — confirm the extension filter to extend.
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task G1 `[SEQUENTIAL]`: Native Python AST extraction

**Files:**
- Modify: `Cargo.toml` (workspace root) + `crates/vox-graphify-reader/Cargo.toml`
- Modify: `crates/vox-graphify-reader/src/ast.rs` + `crates/vox-graphify-reader/src/rebuild.rs`
- Test: `crates/vox-graphify-reader/tests/python_tests.rs` (new)

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight `rg` lines. Confirm workspace `tree-sitter = "0.26.9"`, the `extract_ast_in_module` tree-sitter branch, and the rebuild extension filter. If any differ, STOP.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-graphify-reader/tests/python_tests.rs`:

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::ast::extract_ast_in_module;

#[test]
fn extracts_python_functions_classes_and_calls() {
    let content = "def caller():\n    callee()\n\ndef callee():\n    pass\n\nclass Widget:\n    pass\n";
    let g = extract_ast_in_module(Path::new("mod.py"), content, "pkg/mod.py");
    let ids: Vec<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"pkg/mod.py::caller"), "ids: {ids:?}");
    assert!(ids.contains(&"pkg/mod.py::callee"), "ids: {ids:?}");
    assert!(ids.contains(&"pkg/mod.py::Widget"), "ids: {ids:?}");
    assert_eq!(g.nodes.iter().find(|n| n.label == "Widget").unwrap().kind, "struct");
    assert!(
        g.edges.iter().any(|e| e.source == "pkg/mod.py::caller" && e.target == "callee"),
        "edges: {:?}", g.edges
    );
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-graphify-reader --test python_tests` → FAIL (`.py` yields no nodes).

- [ ] **Step 4: Add + resolve the grammar dependency.** In the workspace root `Cargo.toml` `[workspace.dependencies]`, beside the other grammars (`tree-sitter-typescript = "0.23.2"`), add:

```toml
tree-sitter-python = "0.23.6"
```

In `crates/vox-graphify-reader/Cargo.toml`, add under the other grammar deps:

```toml
tree-sitter-python = { workspace = true, optional = true }
```

and extend the feature:

```toml
tree-sitter-grammars = ["dep:tree-sitter", "dep:tree-sitter-rust", "dep:tree-sitter-typescript", "dep:tree-sitter-python"]
```

**Resolve the version (do this now, two-strike-bounded):** run `cargo build -p vox-graphify-reader` then `cargo tree -p vox-graphify-reader -i tree-sitter --depth 0`. It MUST show a single `tree-sitter v0.26.x`. If `cargo` reports a second `tree-sitter` version or a build error from `tree-sitter-python`, change the `tree-sitter-python` version in the workspace `Cargo.toml` (try `0.25.0`, then `0.23.4`) until `cargo tree` shows one unified `tree-sitter 0.26.x` and the crate builds. If no version unifies after the second try, STOP and write a handoff note.

- [ ] **Step 5: Add the Python branch in `ast.rs`.** In the `#[cfg(feature = "tree-sitter-grammars")]` block of `extract_ast_in_module`, extend the language match to include Python:

```rust
                let language = match ext {
                    "ts" | "js" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
                    "tsx" | "jsx" => Some(tree_sitter_typescript::LANGUAGE_TSX),
                    "py" => Some(tree_sitter_python::LANGUAGE),
                    _ => None,
                };
```

Replace the two `if node.kind() == ...` blocks inside the `while let Some(node) = stack.pop()` loop with language-agnostic checks (TS and Python use different node-kind names; class nodes reuse the `"struct"` kind):

```rust
                                let is_fn_def = matches!(
                                    node.kind(),
                                    "function_declaration" | "method_definition" | "function_definition"
                                );
                                let is_class = node.kind() == "class_definition";
                                let is_call = matches!(node.kind(), "call_expression" | "call");

                                if is_fn_def || is_class {
                                    if let Some(name_node) = node.child_by_field_name("name") {
                                        if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                                            let id = qualify(module_id, name);
                                            nodes.push(ExtractedNode {
                                                id: id.clone(),
                                                label: name.to_string(),
                                                kind: if is_class { "struct" } else { "fn" }.to_string(),
                                            });
                                            if is_fn_def {
                                                current_fn = Some(id);
                                            }
                                        }
                                    }
                                }
                                if is_call {
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
```

> Bump the cache version (Plan A added `EXTRACTOR_VERSION`): change it from `"2"` to `"3"` in `ast.rs`, since extraction now covers Python and must re-extract cached corpora.

- [ ] **Step 6: Accept `.py` in `rebuild.rs`.** Change the extension filter to:

```rust
                if ext == "rs" || ext == "ts" || ext == "js" || ext == "py" {
```

- [ ] **Step 7: Run → PASS.** `cargo test -p vox-graphify-reader --test python_tests` → PASS; `cargo test -p vox-graphify-reader` → PASS (rs/ts unaffected).

- [ ] **Step 8: Verify (Rule 7) + arch-check + commit.**

```bash
git add Cargo.toml crates/vox-graphify-reader/Cargo.toml crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/python_tests.rs
git commit -m "feat(graphify): native Python AST extraction (tree-sitter-python)"
```

---

## Task G2 `[SEQUENTIAL]`: Acquire + index the Graphify upstream source

**Files:**
- Create: `scripts/graphify-study-source.vox`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "fn vox_binary_path|process.run" scripts/graphify-refresh.vox`. Confirm the VoxScript shape (the `vox_binary_path()` helper + `process.run` usage) to mirror. If `scripts/graphify-refresh.vox` is absent, read any `scripts/*.vox` for the import/`process.run` idiom.

- [ ] **Step 2: Write the script.** Create `scripts/graphify-study-source.vox`:

```vox
import fs;
import process;

fn vox_binary_path() to str {
    if fs.exists("./target/release/vox.exe") { return "./target/release/vox.exe"; }
    if fs.exists("./target/release/vox") { return "./target/release/vox"; }
    if fs.exists("./target/debug/vox.exe") { return "./target/debug/vox.exe"; }
    return "./target/debug/vox";
}

fn main() {
    let bin = vox_binary_path();
    let src_dir = ".vox/cache/graphify-src";

    if not fs.exists(src_dir) {
        print("Cloning Graphify upstream source into " + src_dir + " ...");
        let clone_opt = process.run("git", ["clone", "--depth", "1",
            "https://github.com/safishamsi/graphify", src_dir]);
        if clone_opt is null {
            log.error("Failed to spawn git clone");
            process.exit(1);
        }
        let clone = clone_opt.unwrap();
        if clone.code isnt 0 {
            log.error("git clone failed with code " + str(clone.code));
            process.exit(clone.code);
        }
    } else {
        print("Graphify source already present at " + src_dir);
    }

    print("Indexing Graphify source as corpus 'graphify-upstream' ...");
    let idx_opt = process.run(bin, ["graphify", "index", src_dir,
        "--id", "graphify-upstream", "--mode", "structural"]);
    if idx_opt is null {
        log.error("Failed to spawn " + bin + " graphify index");
        process.exit(1);
    }
    let idx = idx_opt.unwrap();
    if idx.code isnt 0 {
        log.error("graphify index failed with code " + str(idx.code));
        process.exit(idx.code);
    }

    print("");
    print("Graphify source indexed. Query it via corpus 'graphify-upstream':");
    print("  " + bin + " graphify status --corpus graphify-upstream");
}
```

- [ ] **Step 3: Verify it runs.** `cargo build -p vox-cli` (so a `vox` binary exists). Then `cargo run -p vox-cli -- run scripts/graphify-study-source.vox`. Expected: clones (first run), prints "Graphify source indexed."; `.vox/cache/graphify/graphify-upstream/graph.json` exists with node count > 0 (Python defs). Then `cargo run -p vox-cli -- graphify status --corpus graphify-upstream` lists it with `nodes=` > 0.
  - **Two-strike:** if clone fails (offline), STOP with a handoff note — do not invent an alternate source URL.

- [ ] **Step 4: Commit.**

```bash
git add scripts/graphify-study-source.vox
git commit -m "feat(graphify): study-source script clones + indexes the Graphify upstream corpus"
```

---

## Task G3 `[PARALLEL-SAFE]` (docs only): Port-parity methodology doc

Disjoint from all code/script tasks → may run in parallel with any other doc task.

**Files:**
- Create: `docs/src/architecture/graphify-upstream-study-methodology-2026-06-18.md`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "^category:" docs/src/architecture/graphify-integration-research-2026-06-16.md`. Confirm the required frontmatter key set (`title`, `description`, `category`, `status`) used under `docs/src/`. Match it.

- [ ] **Step 2: Write the doc.** Create `docs/src/architecture/graphify-upstream-study-methodology-2026-06-18.md`:

````markdown
---
title: "Graphify Upstream Source — Study & Port-Parity Methodology (2026-06-18)"
description: "How to use the graphify-upstream corpus to navigate Graphify's Python implementation and verify the native Rust port stage-by-stage."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Agents porting or auditing the native graphify builder need a stage map from upstream Python to native Rust."
---

# Graphify Upstream Source — Study & Port-Parity Methodology (2026-06-18)

## Acquire the corpus
```bash
vox run scripts/graphify-study-source.vox
```
Clones `https://github.com/safishamsi/graphify` into `.vox/cache/graphify-src/` and builds the
`graphify-upstream` corpus (`vox graphify index`, native Python extraction).

## Navigate it
- `vox graphify status --corpus graphify-upstream` — freshness + node/edge counts.
- Agents: `vox_graphify_search { corpus: "graphify-upstream", query: "<topic>" }` to find entry
  nodes, then `vox_graphify_query { corpus: "graphify-upstream", seeds: [...] }` to expand.

## Port-parity stage map
| Upstream stage (Python) | Native Rust equivalent | Status |
|---|---|---|
| `graphify.detect` (file classification) | `rebuild` WalkDir filter (`rs`/`ts`/`js`/`py`) | Native, coarser |
| `graphify.extract` (AST) | `ast::extract_ast_in_module` (`syn` + tree-sitter rust/ts/py) | Native |
| `graphify.llm` (semantic doc/media nodes) | Vox orchestrator LLM egress (hybrid lane) | Not in reader (by design) |
| `graphify.build` (NetworkX) | `rebuild` builds NetworkX-shaped `graph.json` | Native |
| `graphify.cluster` (Leiden) | `cluster::cluster_nodes` (`leiden-rs`) | Native |
| `graphify.analyze` (god-nodes/surprises) | `GraphifyReader::god_nodes`; surprises not ported | Partial |
| `graphify.report` / exporters | not ported (HTML/Obsidian/Neo4j stay upstream) | Out of scope |
| content-hash semantic cache | `cache::CacheManager` (BLAKE3 per file + extractor version) | Native |

## Parity-check procedure
For each native stage, query `graphify-upstream` for the equivalent upstream symbol (e.g.
`"god node degree"`, `"leiden partition"`, `"detect classify"`), read the upstream behavior, and
confirm the native module matches or record the intentional divergence here. Known divergences:
semantic/LLM extraction and HTML/Neo4j exporters are deliberately not ported (hybrid boundary —
see the capabilities-audit SSOT).
````

- [ ] **Step 3: Verify frontmatter + commit.** Run the repo docs-frontmatter gate (e.g. `cargo run -p vox-cli -- ci docs-frontmatter`, or the command named in `docs/src/architecture/where-things-live.md`). Expected: passes. Then:

```bash
git add docs/src/architecture/graphify-upstream-study-methodology-2026-06-18.md
git commit -m "docs(graphify): upstream source study + port-parity methodology"
```

---

## Parallelization summary

- **G1 → G2 SEQUENTIAL** (G2 needs G1's Python extraction to produce a non-empty upstream graph).
- **G3 PARALLEL-SAFE** (docs only) — dispatch any time.

## Self-Review

- **Spec coverage:** "study the actual Graphify code itself" = Python extraction (G1) + indexed `graphify-upstream` corpus (G2) + parity methodology (G3).
- **Placeholder scan:** none. The only version uncertainty (`tree-sitter-python`) is a bounded, verifiable resolution procedure (G1 Step 4) with `cargo tree` evidence + two-strike stop — not a guessed pin.
- **Type consistency:** `extract_ast_in_module(path, content, module_id)` matches Plan A; class → `"struct"` kind; edge targets bare (feed Plan A's resolver); `graphify index --id graphify-upstream --mode structural` matches Plan B's flags; `EXTRACTOR_VERSION` bumped to invalidate caches now that Python is covered.
- **Antigravity fit:** atomic+green+commit; verify-before-use first; the grammar-version unification (a classic dependency-hell trap for a fast model) is an explicit, evidence-gated step with a hard stop.
- **Cross-plan:** apply A → B → G.
