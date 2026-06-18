---
title: "Graphify Native Coverage Overlays Implementation Plan"
description: "Implementation plan to natively translate static test targeting and dynamic coverage reachability overlays into Rust."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Defines native Rust-based test coverage mapping for Graphify."
---

# Graphify Native Coverage Overlays Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Translate the remaining Python-based coverage-graph overlays (`overlay_tests.py` and `ingest_reaches.py`) to native Rust modules in `crates/vox-graphify-reader` to make semantic test-coverage mapping entirely Python-free.

**Architecture:** Implement a static overlay analyzer that parses test symbol references using tree-sitter TS/JS grammars and Rust syn parsing, map dynamic lcov coverage counts into the codebase graph using a Rust lcov-parser, and output unified semantic coverage stats natively in-process.

**Tech Stack:** Rust, `lcov-parser` (v0.6.0), `serde_json` (v1), `vox-graphify-reader`

---

## File Structure

The following files will be created or modified:
1. `crates/vox-graphify-reader/Cargo.toml` [MODIFY]: Add `lcov-parser` dependency.
2. `crates/vox-graphify-reader/src/lib.rs` [MODIFY]: Export the new `overlay` and `reachability` modules.
3. `crates/vox-graphify-reader/src/overlay.rs` [NEW]: Static test-targeting overlay analyzer.
4. `crates/vox-graphify-reader/src/reachability.rs` [NEW]: Dynamic lcov reachability count parser.
5. `crates/vox-graphify-reader/tests/overlay_tests.rs` [NEW]: Unit tests for static targeting.
6. `crates/vox-graphify-reader/tests/reachability_tests.rs` [NEW]: Unit tests for lcov parsing and ingestion.

---

## Tasks

### Task 1: Add lcov-parser Dependency

**Files:**
- Modify: [Cargo.toml](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/Cargo.toml)

- [ ] **Step 1: Write the updated dependency configuration**

Add `lcov-parser` to [crates/vox-graphify-reader/Cargo.toml](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/Cargo.toml):

```toml
[dependencies]
lcov-parser = "0.6.0"
```

- [ ] **Step 2: Run cargo check to verify build**

Run: `cargo check -p vox-graphify-reader`
Expected: SUCCESS

- [ ] **Step 3: Commit dependency addition**

```bash
git add crates/vox-graphify-reader/Cargo.toml
git commit -m "feat: add lcov-parser dependency to graphify reader"
```

---

### Task 2: Implement Static Test-Targeting Overlay

**Files:**
- Create: [crates/vox-graphify-reader/src/overlay.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/overlay.rs)
- Create: [crates/vox-graphify-reader/tests/overlay_tests.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/tests/overlay_tests.rs)
- Modify: [crates/vox-graphify-reader/src/lib.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/lib.rs)

- [ ] **Step 1: Write the failing static targeting test**

Create [crates/vox-graphify-reader/tests/overlay_tests.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/tests/overlay_tests.rs):

```rust
use vox_graphify_reader::overlay::overlay_test_targets;
use serde_json::json;

#[test]
fn test_static_overlay_targeting() {
    let graph = json!({
        "nodes": [
            {"id": "func_a", "label": "func_a", "kind": "fn"}
        ],
        "links": []
    });
    let test_src = "
        #[test]
        fn test_func_a() {
            func_a();
        }
    ";
    let updated = overlay_test_targets(&graph, "src/test.rs", test_src).unwrap();
    let nodes = updated["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["targeted_by"].as_array().unwrap()[0].as_str().unwrap(), "test_func_a");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-graphify-reader --test overlay_tests`
Expected: FAIL due to missing `overlay` module.

- [ ] **Step 3: Implement the static overlay logic**

Create [crates/vox-graphify-reader/src/overlay.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/overlay.rs):

```rust
use serde_json::{Value, json};

pub fn overlay_test_targets(
    graph: &Value,
    _file_path: &str,
    test_content: &str,
) -> Result<Value, String> {
    let mut updated = graph.clone();
    let mut targets = std::collections::HashMap::new();

    // Parse references of functions in the test content
    if let Ok(file) = syn::parse_file(test_content) {
        use syn::visit::Visit;
        struct TestVisitor {
            current_test: Option<String>,
            calls: std::collections::HashMap<String, Vec<String>>,
        }
        impl<'ast> Visit<'ast> for TestVisitor {
            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                let name = node.sig.ident.to_string();
                if name.starts_with("test_") {
                    let old = self.current_test.replace(name.clone());
                    syn::visit::visit_item_fn(self, node);
                    self.current_test = old;
                } else {
                    syn::visit::visit_item_fn(self, node);
                }
            }
            fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
                if let syn::Expr::Path(ref expr_path) = *node.func {
                    if let Some(ref current_test) = self.current_test {
                        if let Some(segment) = expr_path.path.segments.last() {
                            let callee = segment.ident.to_string();
                            self.calls.entry(callee).or_default().push(current_test.clone());
                        }
                    }
                }
                syn::visit::visit_expr_call(self, node);
            }
        }
        let mut visitor = TestVisitor {
            current_test: None,
            calls: std::collections::HashMap::new(),
        };
        visitor.visit_file(&file);
        targets = visitor.calls;
    }

    if let Some(nodes) = updated.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                if let Some(test_names) = targets.get(id) {
                    node.as_object_mut().unwrap().insert(
                        "targeted_by".to_string(),
                        json!(test_names),
                    );
                }
            }
        }
    }

    Ok(updated)
}
```

Export the module in [crates/vox-graphify-reader/src/lib.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/lib.rs):

```rust
pub mod overlay;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-graphify-reader --test overlay_tests`
Expected: PASS

- [ ] **Step 5: Commit static targeting overlay**

```bash
git add crates/vox-graphify-reader/src/overlay.rs crates/vox-graphify-reader/tests/overlay_tests.rs crates/vox-graphify-reader/src/lib.rs
git commit -m "feat: implement static test-targeting overlay analyzer"
```

---

### Task 3: Implement Dynamic Reachability Ingestion

**Files:**
- Create: [crates/vox-graphify-reader/src/reachability.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/reachability.rs)
- Create: [crates/vox-graphify-reader/tests/reachability_tests.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/tests/reachability_tests.rs)
- Modify: [crates/vox-graphify-reader/src/lib.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/lib.rs)

- [ ] **Step 1: Write the failing reachability test**

Create [crates/vox-graphify-reader/tests/reachability_tests.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/tests/reachability_tests.rs):

```rust
use vox_graphify_reader::reachability::ingest_lcov_reachability;
use serde_json::json;

#[test]
fn test_lcov_reachability_ingest() {
    let graph = json!({
        "nodes": [
            {"id": "hello", "label": "hello", "kind": "fn"}
        ],
        "links": []
    });
    let lcov = "
        SF:src/main.rs
        FN:3,hello
        FNDA:5,hello
        end_of_record
    ";
    let updated = ingest_lcov_reachability(&graph, lcov).unwrap();
    let nodes = updated["nodes"].as_array().unwrap();
    assert_eq!(nodes[0]["execution_count"].as_u64().unwrap(), 5);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-graphify-reader --test reachability_tests`
Expected: FAIL due to missing `reachability` module.

- [ ] **Step 3: Implement dynamic reachability parsing**

Create [crates/vox-graphify-reader/src/reachability.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/reachability.rs):

```rust
use serde_json::{Value, json};

pub fn ingest_lcov_reachability(graph: &Value, lcov_content: &str) -> Result<Value, String> {
    let mut updated = graph.clone();
    let mut execution_counts = std::collections::HashMap::new();

    // Simple line parsing for FNDA (function execution counts)
    for line in lcov_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("FNDA:") {
            let parts: Vec<&str> = trimmed[5..].split(',').collect();
            if parts.len() == 2 {
                if let Ok(count) = parts[0].parse::<u64>() {
                    let fn_name = parts[1].to_string();
                    execution_counts.insert(fn_name, count);
                }
            }
        }
    }

    if let Some(nodes) = updated.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes {
            if let Some(id) = node.get("id").and_then(|i| i.as_str()) {
                let count = execution_counts.get(id).copied().unwrap_or(0);
                node.as_object_mut().unwrap().insert(
                    "execution_count".to_string(),
                    json!(count),
                );
            }
        }
    }

    Ok(updated)
}
```

Export the module in [crates/vox-graphify-reader/src/lib.rs](file:///c:/Users/Owner/vox/crates/vox-graphify-reader/src/lib.rs):

```rust
pub mod reachability;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-graphify-reader --test reachability_tests`
Expected: PASS

- [ ] **Step 5: Commit reachability logic**

```bash
git add crates/vox-graphify-reader/src/reachability.rs crates/vox-graphify-reader/tests/reachability_tests.rs crates/vox-graphify-reader/src/lib.rs
git commit -m "feat: implement dynamic lcov reachability count mapping"
```

---

## Verification Plan

### Automated Tests
- Run `cargo test -p vox-graphify-reader --test overlay_tests` to verify static test targeting.
- Run `cargo test -p vox-graphify-reader --test reachability_tests` to verify dynamic lcov reachability parsing.

### Manual Verification
- Execute overlay mapping using a script and compare the output `graph.json` fields with baseline test targets.
