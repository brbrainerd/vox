# Vox Search — Data-Flow / Def-Use Layer (P1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is **write-through-workflow**: it ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui add … && git -C … commit …` (add + commit only — **never** `push`, `reset`, `rebase`, `checkout --`, `clean`, or `commit --amend`). The workflow performs the final integration commit; do not branch or merge here.

**Goal:** Add Vox Search's **Layer 4 — data-flow / def-use** index to the structural-index library (`crates/vox-graphify-reader/`). Intra-procedural def-use (`syn` for Rust, tree-sitter walk for TS) emits new deterministic node kinds (`field:<Struct>::<f>`, `binding:<fn>::<n>@<k>`) and edge kinds (`def-write`, `use-read` sub-typed by `read_kind ∈ {control, consume, store}`), plus three local detectors (`ignored_result`, `write_only_field`, `accumulator_never_gates`) feeding `compute_dead_signals(&Value) -> DeadSignalReport`. Surface it via two MCP tools (`vox_search_dataflow`, `vox_search_dead_signals`), two CLI subcommands (`dataflow`, `dead-signals`), and a **non-blocking CI advisory**. **A mandatory end-to-end test reproduces the frontend-emit bug class**: `accumulator_never_gates` fires on a `reactive_view_emit_failures`-like field that is `.push`-accumulated and only `store`-read into an output struct, never read for control flow.

**Architecture:** Pure Rust, additive. A new module `crates/vox-graphify-reader/src/dataflow.rs` runs **per function** (no whole-program fixpoint) and **drops on ambiguity** (a missed defect is acceptable; a false "dead signal" is not). Def-use nodes/edges are merged into the **same** `graph.json` family (same honesty firewall, `confidence`-labeled), routed **around** `resolve_edges` (they are pre-resolved to `field:`/`binding:` ids). The link serializer in `rebuild.rs` is extended to carry `kind` + `read_kind`. `compute_dead_signals` mirrors `compute_coverage`'s shape and honesty firewall: it reports what the def-use edges say, makes no judgment about intent, and labels `confidence` so an agent or CI gate can pick a threshold. MCP tools land in `graphify_tools.rs` (sibling handlers) + one dispatch arm + one inline JSON schema each; CLI subcommands extend the existing command enum; the advisory is a non-blocking `vox ci` print (mirroring the coverage advisory).

**Tech Stack:** Rust — `syn` (`full`/`visit`/`clone-impls`, already a dep) for the Rust def-use visitor; `tree-sitter` + `tree-sitter-typescript` 0.23.2 (already deps, behind `tree-sitter-grammars`, default-on) for the TS walk; `serde`/`serde_json` for `DeadSignal`/`DeadSignalReport` + graph I/O; `tempfile` (dev-dep) for fixtures; `clap`/`anyhow` for the CLI; `chrono`/`vox-config` reused by the MCP handlers.

**Spec:** Source design — `docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md` §1 (read first). Master umbrella — `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` §2.3, §3.1, §6 (P1), §9 (plan index). **Naming authority: the master spec wins on external surface** — MCP tools are `vox_search_dataflow` / `vox_search_dead_signals`; the source design's `vox_graphify_*` names are superseded.

**Dependencies (cross-plan):**
- **Requires P0 (Absorption + structural-core enrichment)** for: (a) `ExtractedEdge.confidence` + `dangling`/`missing` resolution (LANDED on this branch — verified at `ast.rs:18`, `rebuild.rs:67`), (b) the `field:`/`binding:` ids riding alongside `cmd:`/`tool:`/`surface:` registry nodes, (c) `coverage.rs` as the report-shape template (LANDED — `coverage.rs:74`). The structural node/edge schema + `confidence` from P0 are **already present on this worktree**, so P1 is executable now; only the **external tool/CLI rename** (`vox graphify`→`vox search`, `vox_graphify_*`→`vox_search_*`) is owned by P0. This plan therefore (i) names the new MCP tools `vox_search_*` directly (no later rename), and (ii) adds the new CLI subcommands to the **existing** `GraphifyCmd` enum that P0 re-homes under `vox search` — a one-line move P0 carries forward. If P0's CLI rename has not landed when P1 runs, the subcommands are still reachable as `vox graphify dataflow` / `vox graphify dead-signals` (the P0 alias keeps both live for one release).
- **Blocks nothing upstream**; P5 (GUI) consumes `vox_search_dataflow`/`vox_search_dead_signals` panes incrementally — no coupling.
- **Independent of** P2/P3/P4/P6 (parallelizable off P0 per the master DAG §9).

**Base branch note:** Authored/executed on the current worktree branch at `/c/Users/Owner/vox-graphify-gui` (off the compiling honesty branch; `main` does not compile due to the `db_cli` WIP). Prefer `cargo test -p vox-graphify-reader` (fast, isolated, no `vox-cli`/`vox-gui` build) for Phases A–C; build `vox-orchestrator-mcp` only in Phase D and `vox-cli` only in Phase E.

---

## Key internals (verified against the code — exact, with line anchors)

- **`crates/vox-graphify-reader/src/ast.rs`** — `ExtractedNode { id, label, kind: String }` (`:9`), `ExtractedEdge { source, target, #[serde(default = "default_confidence")] confidence: String }` (`:16`), `default_confidence() -> "resolved"` (`:24`). `ExtractedGraph { nodes, edges }` (`:28`). `EXTRACTOR_VERSION: &str = "3"` (`:37`) — **bump to `"4"`** when def-use lands so cached graphs re-extract. `qualify(module_id, sym)` (`:41`). The Rust `RustVisitor` (`:49`) tracks `current_fn: Option<String>` and visits `visit_item_fn`/`visit_item_struct`/`visit_expr_call`. The TS path is a tree-sitter stack-walk under `#[cfg(feature = "tree-sitter-grammars")]` (`:130`) with the A0d-recorded node-kind names in the doc-comment (`:139`).
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `rebuild_graph(_repo_root, source_dir, output_file, cache_dir, meta: &RebuildMeta)` (`:139`). Single walk loop `for path in walk_source_files(source_dir)` (`:172`); `module_id` is the slash-normalized repo-relative path (`:183`). `extract_ast_in_module*` appends into `all_nodes`/`all_edges` (`:213`). `resolve_edges(&all_nodes, &all_edges)` (`:240`) drops edges whose bare target name has no unique def — **so def-use edges must NOT pass through `resolve_edges`** (their targets are `field:`/`binding:` ids, not bare fn names). The **link serializer** (`:320`) hardcodes `{source, target, confidence}` — **must be extended to add `kind` + `read_kind`** when present. `confidence_counts` tally (`:364`). `EXTRACTOR_VERSION` folded into the per-file cache key (`:180`).
- **`crates/vox-graphify-reader/src/coverage.rs`** — `compute_coverage(graph: &Value, kind: &str) -> CoverageReport` (`:74`); `CoverageStatus` enum (`:21`); `nodes()`/`links()`/`str_field()` helpers (`:47`–`:64`). **The exact template** for `dataflow.rs`'s report shape, the `links()` `links`-or-`edges` fallback, and the `#[serde(rename_all = "snake_case")]` enum.
- **`crates/vox-graphify-reader/src/lib.rs`** — `pub mod` list (`:13`); `GraphifyReader::from_value` (`:77`) reads only id/label/community/source/target and **ignores unknown node/edge fields** (`kind`/`read_kind`/`missing` survive serialization round-trip but don't break the reader).
- **`crates/vox-cli/src/commands/graphify/mod.rs`** — `enum GraphifyCmd` (`:13`) with `Coverage { corpus, kind, out }` (`:42`); `pub async fn run(cmd, repo_root) -> anyhow::Result<()>` (`:323`); the `Coverage` arm (`:396`) shows the exact corpus-resolution + read-graph + `compute_coverage` + write/print recipe to copy. Helpers in scope: `load_all_corpora`, `resolve_ingest_corpus_id`, `corpus_by_id`, `use anyhow::Context`.
- **`crates/vox-orchestrator-mcp/src/graphify_tools.rs`** — handler pattern (`graphify_query` `:343`, `graphify_path` `:407`): `load_graphify_corpora` → `resolve_search_corpus` → `load_graph_json` (`:333`) → `GraphifyReader::from_value` / raw `Value` → build `ToolResult::ok(json!({…})).to_json()`. `REM_GRAPHIFY` remediation const (`:15`). `knowledge_id(corpus_id, node_id)` (`:90`). Unit tests use `tempfile` + `write_registry` + `test_state_for_repo` (`:542`).
- **`crates/vox-orchestrator-mcp/src/dispatch.rs`** — the `match name` arm block at `:627`–`:641` (`vox_graphify_*` arms); add the two new arms there.
- **`crates/vox-orchestrator-mcp/src/input_schemas.rs`** — inline JSON-schema arms at `:471`–`:485` (`vox_graphify_*` schemas via `parse_obj(r#"{…}"#)`); add the two new arms there.
- **Frontend-emit bug class (the reproduction target):** `docs/superpowers/plans/2026-06-26-frontend-emit-validation-gate.md` (main repo) — `generate_with_options` `.push`-accumulates `ReactiveViewBridgeStats::reactive_view_emit_failures: Vec<WebIrDiagnostic>` per reactive component, then only moves `reactive_stats` into `Ok(CodegenOutput { … })` (`emitter.rs:624`) — a **`store`** read, **never** a `control` read. That is exactly the `accumulator_never_gates` positive case (source design §1.5).

---

## File Structure

**Created**
- `crates/vox-graphify-reader/src/dataflow.rs` — def-use extraction (`extract_dataflow_in_module`), the `read_kind` classifier, `DeadSignal`/`DeadSignalKind`/`DeadSignalReport`, `compute_dead_signals`.
- `crates/vox-graphify-reader/tests/dataflow_rust.rs` — Rust def-use + detector unit/fixture tests (incl. the **frontend-emit e2e** fixture).
- `crates/vox-graphify-reader/tests/dataflow_ts.rs` — TS def-use tests (gated `#[cfg(feature = "tree-sitter-grammars")]`).
- `crates/vox-graphify-reader/tests/dead_signals.rs` — `compute_dead_signals` whole-graph fixture tests.

**Modified**
- `crates/vox-graphify-reader/src/ast.rs` — add `read_kind: Option<String>` + `kind: Option<String>` to `ExtractedEdge`; bump `EXTRACTOR_VERSION` to `"4"`.
- `crates/vox-graphify-reader/src/lib.rs` — `pub mod dataflow;`.
- `crates/vox-graphify-reader/src/rebuild.rs` — merge def-use nodes/edges (routed around `resolve_edges`); extend the link serializer with `kind`/`read_kind`.
- `crates/vox-orchestrator-mcp/src/graphify_tools.rs` — `vox_search_dataflow` + `vox_search_dead_signals` handlers + params + unit tests.
- `crates/vox-orchestrator-mcp/src/dispatch.rs` — two dispatch arms.
- `crates/vox-orchestrator-mcp/src/input_schemas.rs` — two schema arms.
- `crates/vox-cli/src/commands/graphify/mod.rs` — `Dataflow` + `DeadSignals` subcommands + run arms.
- `crates/vox-cli/src/commands/ci/mod.rs` (+ the dead-signals advisory site) — non-blocking advisory.

---

## Workflow-readiness: dependency DAG + fan-out batches

```
Phase A (schema)  A1 ──► A2 ──► A3            [SEQUENTIAL within A]
                          │
Phase B (Rust)    B1 ─────┴► B2 ─► B3 ─► B4   [SEQUENTIAL within B; whole phase needs A]
Phase C (TS)      C1 ─► C2                     [needs A; PARALLEL with B]
Phase D (dead-    D1 ─► D2                     [needs B (+C optional); the detectors]
  signals+e2e)    D3 (frontend-emit e2e)       [needs D1+D2]
Phase E (rebuild) E1                           [needs A3, B, C, D]
Phase F (MCP)     F1 ─► F2 ─► F3               [needs D; PARALLEL with G]
Phase G (CLI)     G1 ─► G2                     [needs D + E; PARALLEL with F]
Phase H (CI)      H1                           [needs G]
Phase I (close)   I1                           [needs all]
```

**Explicit parallel fan-out batches a workflow can dispatch concurrently:**
- **Batch β (after A3 lands):** Phase **B** (Rust def-use) and Phase **C** (TS def-use) are independent files/tests → dispatch B1 and C1 in parallel; B and C never touch the same lines (`dataflow.rs` Rust-visitor block vs TS-walk block — partitioned by `#[cfg]`/function).
- **Batch δ (after D lands and E1 lands):** Phase **F** (MCP tools) and Phase **G** (CLI) are independent crates (`vox-orchestrator-mcp` vs `vox-cli`) → dispatch F1 and G1 in parallel.
- Everything else is sequential per the DAG.

Each task below is independently committable by a sub-agent. Tags: **[SEQUENTIAL]** = must follow its predecessor in-phase; **[PARALLEL-SAFE]** = no in-flight conflict with its batch siblings.

---

# PHASE A — Edge schema for `read_kind` / `kind` (load-bearing, separately committed)

## Task A1: `ExtractedEdge` carries optional `kind` + `read_kind` (TDD) [SEQUENTIAL]

Def-use edges need an edge `kind` (`def-write` / `use-read`) and, for reads, a `read_kind`. Adding them as `Option<String>` with `skip_serializing_if` keeps existing call edges byte-identical (no `kind`/`read_kind` key emitted), so all current `rebuild`/`ast`/`coverage` golden tests stay green.

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs`. Test: extend the inline `#[cfg(test)]` module at the bottom of `ast.rs` (add one if absent).

- [ ] **Step 1: Failing test** — append to `crates/vox-graphify-reader/src/ast.rs`:

```rust
#[cfg(test)]
mod edge_schema_tests {
    use super::ExtractedEdge;

    #[test]
    fn call_edge_omits_new_fields_in_json() {
        let e = ExtractedEdge {
            source: "m::a".into(),
            target: "m::b".into(),
            confidence: "resolved".into(),
            kind: None,
            read_kind: None,
        };
        let v = serde_json::to_value(&e).unwrap();
        assert!(v.get("kind").is_none(), "kind must be omitted when None");
        assert!(v.get("read_kind").is_none(), "read_kind must be omitted when None");
    }

    #[test]
    fn defuse_edge_carries_kind_and_read_kind() {
        let e = ExtractedEdge {
            source: "m::f".into(),
            target: "field:S::x".into(),
            confidence: "resolved".into(),
            kind: Some("use-read".into()),
            read_kind: Some("control".into()),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "use-read");
        assert_eq!(v["read_kind"], "control");
    }
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader edge_schema_tests`.
  Expected: **compile error** — `ExtractedEdge` has no field `kind`/`read_kind`.

- [ ] **Step 3: Implement** — edit `ExtractedEdge` in `ast.rs` (`:16`):

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ExtractedEdge {
    pub source: String,
    pub target: String,
    #[serde(default = "default_confidence")]
    pub confidence: String,
    /// Edge kind for data-flow edges: `"def-write"` | `"use-read"`. `None` for ordinary
    /// call/composition/boundary edges (omitted from JSON to keep call edges byte-identical).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// For `use-read` edges only: `"control"` | `"consume"` | `"store"`. `None` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_kind: Option<String>,
}
```

- [ ] **Step 4: Fix every `ExtractedEdge { … }` literal** — the new fields have `#[serde(default)]` for deserialize but the **struct literals** need them. Construct-sites are in `ast.rs` (the 6 `ExtractedEdge { … }` literals in the Rust visitor + TS walk) and `rebuild.rs::resolve_edges` (3 literals). For each, add `kind: None, read_kind: None,`. Build to find them all:
  `cargo build -p vox-graphify-reader 2>&1 | grep -E "missing field|ExtractedEdge" | head`.

- [ ] **Step 5: Run, verify pass** — `cargo test -p vox-graphify-reader`.
  Expected: `edge_schema_tests` pass; **all pre-existing tests still pass** (call edges omit the new keys, so `rebuild`/`boundary_edges`/`composition_edges` goldens are unchanged).

- [ ] **Step 6: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs crates/vox-graphify-reader/src/rebuild.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): add optional kind/read_kind to ExtractedEdge"`

## Task A2: `dataflow` module skeleton + types (TDD) [SEQUENTIAL]

Create the module with the `DeadSignal` shape from the source design §1.3 and register it in `lib.rs`. No logic yet — just compiling types + a unit test on the enum's serde.

**Files:** Create `crates/vox-graphify-reader/src/dataflow.rs`; modify `crates/vox-graphify-reader/src/lib.rs`.

- [ ] **Step 1: Failing test** — create `crates/vox-graphify-reader/src/dataflow.rs`:

```rust
//! Vox Search Layer 4 — intra-procedural data-flow / def-use.
//!
//! Runs PER FUNCTION (no whole-program fixpoint) and DROPS ON AMBIGUITY: a missed defect is
//! acceptable; a false "dead signal" that sends an agent on a wrong hunt is not. New node kinds
//! `field:<Struct>::<field>` and `binding:<fn-id>::<name>@<n>` and edge kinds `def-write` /
//! `use-read` (sub-typed by `read_kind ∈ {control, consume, store}`) join the SAME graph.json
//! family — deterministic, `confidence`-labeled, never an overlay.

use serde::Serialize;
use serde_json::Value;

/// Which detector raised a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadSignalKind {
    /// A `Result`/`Option`-returning call whose value is dropped with no `?`/unwrap/match/assign.
    IgnoredResult,
    /// A `field:` node with ≥1 `def-write` and zero `use-read` edges of any kind.
    WriteOnlyField,
    /// A `field:`/`binding:` node accumulated (`.push`/`.extend`/`+=`) that flows to a `store`/
    /// `consume` read but is NEVER read with `read_kind: "control"`. The frontend-emit class.
    AccumulatorNeverGates,
}

/// One dead-signal finding. Mirrors the source design §1.3 struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeadSignal {
    pub detector: DeadSignalKind,
    pub node_id: String,
    pub label: String,
    pub write_sites: Vec<String>,
    pub read_kinds: Vec<String>,
    pub confidence: String,
    pub rationale: String,
}

/// The lint output, sibling to `coverage::CoverageReport`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeadSignalReport {
    pub findings: Vec<DeadSignal>,
}

#[cfg(test)]
mod tests {
    use super::{DeadSignal, DeadSignalKind, DeadSignalReport};

    #[test]
    fn detector_serializes_snake_case() {
        let r = DeadSignalReport {
            findings: vec![DeadSignal {
                detector: DeadSignalKind::AccumulatorNeverGates,
                node_id: "field:S::errs".into(),
                label: "errs".into(),
                write_sites: vec!["m::f".into()],
                read_kinds: vec!["store".into()],
                confidence: "heuristic".into(),
                rationale: "accumulated, never gates".into(),
            }],
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["findings"][0]["detector"], "accumulator_never_gates");
        assert_eq!(v["findings"][0]["read_kinds"][0], "store");
    }
}
```

- [ ] **Step 2: Register module** — add to `crates/vox-graphify-reader/src/lib.rs` after `pub mod crate_model;` (keep alphabetical-ish ordering near `coverage`):

```rust
pub mod dataflow;
```

- [ ] **Step 3: Run, verify pass** — `cargo test -p vox-graphify-reader detector_serializes_snake_case`.
  Expected: PASS.

- [ ] **Step 4: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/src/lib.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): module skeleton + DeadSignal types"`

## Task A3: Bump `EXTRACTOR_VERSION` to invalidate stale caches [SEQUENTIAL]

Def-use changes the extraction scheme; without a bump, cached per-file graphs from before P1 would be reused and the new edges would silently never appear.

**Files:** Modify `crates/vox-graphify-reader/src/ast.rs`.

- [ ] **Step 1: Implement** — change `ast.rs` (`:37`):

```rust
/// Bump when the extraction scheme changes (node-id format, edge rules). v4 adds the
/// data-flow def-use layer (field:/binding: nodes, def-write/use-read edges).
pub const EXTRACTOR_VERSION: &str = "4";
```

- [ ] **Step 2: Verify** — `cargo test -p vox-graphify-reader` (no test asserts the literal; this is a cache-key change). Expected: all green.

- [ ] **Step 3: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/ast.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(vox-search/dataflow): bump EXTRACTOR_VERSION to 4 for def-use scheme"`

---

# PHASE B — Rust def-use extraction (`syn`) — Batch β, [PARALLEL-SAFE] with Phase C

> All of Phase B writes the **Rust** path in `dataflow.rs` (`extract_dataflow_in_module` for `.rs`) + `tests/dataflow_rust.rs`. It never touches the TS block (Phase C) → safe to run concurrently with C.

## Task B1: `extract_dataflow_in_module` — `field:` nodes + `def-write` on `.push` (TDD) [SEQUENTIAL]

The detector's load-bearing input is a `def-write` whose op is an **accumulation** (`.push`/`.extend`/`.insert`/`+=`) onto a struct field path (`self.x.push(..)`, `local.field.push(..)`). Start there.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; create `crates/vox-graphify-reader/tests/dataflow_rust.rs`.

- [ ] **Step 1: Failing test** — create `crates/vox-graphify-reader/tests/dataflow_rust.rs`:

```rust
use std::path::Path;
use vox_graphify_reader::dataflow::extract_dataflow_in_module;

fn edge<'a>(g: &'a vox_graphify_reader::ast::ExtractedGraph, kind: &str) -> Vec<&'a vox_graphify_reader::ast::ExtractedEdge> {
    g.edges.iter().filter(|e| e.kind.as_deref() == Some(kind)).collect()
}

#[test]
fn push_onto_field_is_accumulating_def_write() {
    let src = r#"
struct Stats { failures: Vec<String> }
fn collect(s: &mut Stats) {
    s.failures.push(format!("boom"));
}
"#;
    let g = extract_dataflow_in_module(Path::new("m.rs"), src, "m");
    // A field node exists for the struct field that was written.
    assert!(g.nodes.iter().any(|n| n.id == "field:Stats::failures" && n.kind == "field"),
        "missing field node: {:?}", g.nodes);
    // A def-write edge from the writing fn to the field, marked accumulation.
    let writes = edge(&g, "def-write");
    let w = writes.iter().find(|e| e.target == "field:Stats::failures")
        .expect("def-write to field:Stats::failures");
    assert_eq!(w.source, "m::collect");
    // accumulation marker rides on read_kind=None but confidence carries the op via a side field;
    // we encode the accumulation op into the edge's `read_kind`-free `confidence` of "resolved"
    // and a dedicated marker on the field node (see Step 3).
    assert!(g.nodes.iter().any(|n| n.id == "field:Stats::failures"
        && n.label == "failures"));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_rust push_onto_field_is_accumulating_def_write`.
  Expected: **compile error / FAIL** — `extract_dataflow_in_module` does not exist.

- [ ] **Step 3: Implement** — add to `dataflow.rs` a Rust def-use visitor. Accumulation ops are recorded as a per-field flag (used by the detector in D2) on a side map returned alongside the graph; to keep the public surface an `ExtractedGraph`, encode the accumulation marker by emitting the `def-write` edge with `confidence: "resolved"` **and** marking the field node `kind: "field"` while recording accumulating-write field ids via a node-label convention is NOT used — instead the detector recomputes accumulation from the edge set, so here we simply record def-writes whose write-site is a `.push`/`.extend`/`.insert`/method-chain ending in those, plus `+=`. Emit a companion `def-write` edge with `read_kind: Some("accumulate")` to carry the op losslessly (the detector reads `read_kind == "accumulate"` on `def-write` edges; ordinary assignments emit `def-write` with `read_kind: None`).

```rust
use std::path::Path;
use syn::visit::Visit;
use crate::ast::{ExtractedEdge, ExtractedGraph, ExtractedNode, qualify};

const ACCUMULATING_METHODS: &[&str] = &["push", "extend", "insert"];

struct DfRustVisitor {
    module_id: String,
    nodes: Vec<ExtractedNode>,
    edges: Vec<ExtractedEdge>,
    current_fn: Option<String>,
    // field ids already emitted as nodes (dedup)
    emitted_fields: std::collections::BTreeSet<String>,
    // struct-name lookup for `self.<f>` / `<local>.<f>` is intra-file: we resolve the field's
    // owning struct by the nearest preceding struct only when the receiver is `self`; for a local
    // receiver we cannot know its type intra-procedurally → we use the local's type if it was a
    // `let x: T` / `let x = T { .. }` we saw, else DROP (honesty: no guessed field node).
    self_struct: Option<String>,
    local_types: std::collections::HashMap<String, String>,
}

impl DfRustVisitor {
    fn field_node(&mut self, struct_name: &str, field: &str) -> String {
        let id = format!("field:{struct_name}::{field}");
        if self.emitted_fields.insert(id.clone()) {
            self.nodes.push(ExtractedNode {
                id: id.clone(),
                label: field.to_string(),
                kind: "field".to_string(),
            });
        }
        id
    }

    /// Resolve the owning struct for a field access `recv.field`. `self` → current impl struct;
    /// a known local → its recorded type; otherwise None (drop).
    fn owner_of(&self, recv: &str) -> Option<String> {
        if recv == "self" {
            self.self_struct.clone()
        } else {
            self.local_types.get(recv).cloned()
        }
    }
}

impl<'ast> Visit<'ast> for DfRustVisitor {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Emitting struct nodes is the structural extractor's job (ast.rs); here we only need to
        // know which struct `self` refers to, set per-impl in visit_item_impl.
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let prev = self.self_struct.take();
        if let syn::Type::Path(tp) = &*node.self_ty {
            if let Some(seg) = tp.path.segments.last() {
                self.self_struct = Some(seg.ident.to_string());
            }
        }
        syn::visit::visit_item_impl(self, node);
        self.self_struct = prev;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let id = qualify(&self.module_id, &node.sig.ident.to_string());
        let prev = self.current_fn.replace(id);
        let prev_locals = std::mem::take(&mut self.local_types);
        record_param_types(node, &mut self.local_types);
        syn::visit::visit_item_fn(self, node);
        self.current_fn = prev;
        self.local_types = prev_locals;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let id = qualify(&self.module_id, &node.sig.ident.to_string());
        let prev = self.current_fn.replace(id);
        let prev_locals = std::mem::take(&mut self.local_types);
        record_param_types_impl(node, &mut self.local_types);
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = prev;
        self.local_types = prev_locals;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        // record `let x: T = ..` / `let x = T { .. }` so `x.field` resolves intra-procedurally.
        if let syn::Pat::Type(pt) = &node.pat {
            if let (syn::Pat::Ident(id), syn::Type::Path(tp)) = (&*pt.pat, &*pt.ty) {
                if let Some(seg) = tp.path.segments.last() {
                    self.local_types.insert(id.ident.to_string(), seg.ident.to_string());
                }
            }
        } else if let syn::Pat::Ident(id) = &node.pat {
            if let Some(init) = &node.init {
                if let syn::Expr::Struct(es) = &*init.expr {
                    if let Some(seg) = es.path.segments.last() {
                        self.local_types.insert(id.ident.to_string(), seg.ident.to_string());
                    }
                }
            }
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // Accumulation: `<recv>.<field>.push(..)` where method ∈ ACCUMULATING_METHODS.
        let method = node.method.to_string();
        if ACCUMULATING_METHODS.contains(&method.as_str()) {
            if let syn::Expr::Field(fe) = &*node.receiver {
                if let (Some((recv, owner)), Some(field)) =
                    (field_receiver(&fe.base, self), field_name(&fe.member))
                {
                    let _ = recv;
                    let fid = self.field_node(&owner, &field);
                    if let Some(cur) = &self.current_fn {
                        self.edges.push(ExtractedEdge {
                            source: cur.clone(),
                            target: fid,
                            confidence: "resolved".into(),
                            kind: Some("def-write".into()),
                            read_kind: Some("accumulate".into()), // op marker
                        });
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Return `(receiver-string, owner-struct)` for a `recv.field` base, when the owner resolves.
fn field_receiver(base: &syn::Expr, v: &DfRustVisitor) -> Option<(String, String)> {
    let recv = match base {
        syn::Expr::Path(p) => p.path.segments.last()?.ident.to_string(),
        _ => return None,
    };
    let owner = v.owner_of(&recv)?;
    Some((recv, owner))
}

fn field_name(member: &syn::Member) -> Option<String> {
    match member {
        syn::Member::Named(id) => Some(id.to_string()),
        syn::Member::Unnamed(_) => None, // tuple field: not modeled in the first cut → drop
    }
}

fn record_param_types(f: &syn::ItemFn, out: &mut std::collections::HashMap<String, String>) {
    for inp in &f.sig.inputs {
        if let syn::FnArg::Typed(pt) = inp {
            if let (syn::Pat::Ident(id), ty) = (&*pt.pat, &*pt.ty) {
                if let Some(name) = path_type_name(ty) {
                    out.insert(id.ident.to_string(), name);
                }
            }
        }
    }
}
fn record_param_types_impl(f: &syn::ImplItemFn, out: &mut std::collections::HashMap<String, String>) {
    for inp in &f.sig.inputs {
        if let syn::FnArg::Typed(pt) = inp {
            if let (syn::Pat::Ident(id), ty) = (&*pt.pat, &*pt.ty) {
                if let Some(name) = path_type_name(ty) {
                    out.insert(id.ident.to_string(), name);
                }
            }
        }
    }
}
/// Strip `&`/`&mut`/`Box<>`-ish wrappers down to the last path segment ident.
fn path_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(r) => path_type_name(&r.elem),
        syn::Type::Path(tp) => tp.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// Public entry: intra-procedural def-use for one file. Rust via `syn`; TS via the tree-sitter
/// walk (Phase C). Non-`.rs`/non-TS files yield an empty graph.
pub fn extract_dataflow_in_module(path: &Path, content: &str, module_id: &str) -> ExtractedGraph {
    if path.extension().and_then(|e| e.to_str()) == Some("rs") {
        if let Ok(file) = syn::parse_file(content) {
            let mut v = DfRustVisitor {
                module_id: module_id.to_string(),
                nodes: Vec::new(),
                edges: Vec::new(),
                current_fn: None,
                emitted_fields: std::collections::BTreeSet::new(),
                self_struct: None,
                local_types: std::collections::HashMap::new(),
            };
            v.visit_file(&file);
            return ExtractedGraph { nodes: v.nodes, edges: v.edges };
        }
    }
    // TS path added in Phase C; default empty (honest miss).
    ExtractedGraph { nodes: Vec::new(), edges: Vec::new() }
}
```

  Note the test in Step 1 asserting on the `accumulate` marker via the edge: update the test's last assertion to check the edge marker explicitly. Replace the final `assert!` of Step 1 with:

```rust
    assert_eq!(w.read_kind.as_deref(), Some("accumulate"),
        "accumulating def-write must carry read_kind=accumulate");
```

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_rust push_onto_field_is_accumulating_def_write`.
  Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_rust.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): Rust accumulating def-write extraction (.push/.extend/.insert)"`

## Task B2: `use-read` with `read_kind` classification — `store` (struct-literal move) (TDD) [SEQUENTIAL]

The crux of the frontend-emit case: the field is moved into another aggregate (`Ok(CodegenOutput { … reactive_stats … })`) → `read_kind: "store"`, **not** `control`. Model field/binding reads inside struct-literal construction.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; extend `crates/vox-graphify-reader/tests/dataflow_rust.rs`.

- [ ] **Step 1: Failing test** — append to `tests/dataflow_rust.rs`:

```rust
#[test]
fn struct_literal_field_init_is_store_read() {
    // `Out { stats }` reads the local `stats` (whose type is Stats) into another aggregate.
    let src = r#"
struct Stats { failures: Vec<String> }
struct Out { stats: Stats }
fn build() -> Out {
    let mut stats: Stats = Stats { failures: vec![] };
    stats.failures.push(String::from("x"));
    Out { stats }
}
"#;
    let g = extract_dataflow_in_module(std::path::Path::new("m.rs"), src, "m");
    // A use-read with read_kind="store" targeting the binding `stats` must exist.
    let store = g.edges.iter().find(|e|
        e.kind.as_deref() == Some("use-read")
        && e.read_kind.as_deref() == Some("store")
        && e.target.starts_with("binding:m::build::stats@"));
    assert!(store.is_some(), "expected store read of `stats` binding: {:?}", g.edges);
    // And NO control read of the accumulated field (this is the would-be-bug shape).
    assert!(!g.edges.iter().any(|e|
        e.kind.as_deref() == Some("use-read") && e.read_kind.as_deref() == Some("control")),
        "no control read expected");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_rust struct_literal_field_init_is_store_read`.
  Expected: FAIL (no binding node, no `store` read).

- [ ] **Step 3: Implement** — extend `DfRustVisitor`:
  - Emit **`binding:` nodes** when a `let` binds a typed/struct-init local: `binding:<fn-id>::<name>@<n>` where `@n` is a per-function monotonically increasing counter (intra-function SSA-ish disambiguation). Track `binding_ids: HashMap<String,String>` (name → current binding id) per function, bumped on re-binding.
  - Add `visit_expr_struct` (`ExprStruct`): for each `FieldValue` whose `expr` is a bare path to a known local binding (shorthand `Out { stats }` desugars to `stats: stats`), emit a `use-read` edge `current_fn → binding:<id>` with `read_kind: "store"`. (Field-init by a struct-literal is the "copy into another aggregate" case.)
  - Keep ambiguous receivers dropped (honesty).

  Add to the visitor impl:

```rust
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if let Some(cur) = self.current_fn.clone() {
            for fv in &node.fields {
                if let syn::Expr::Path(p) = &fv.expr {
                    if let Some(seg) = p.path.segments.last() {
                        let name = seg.ident.to_string();
                        if let Some(bid) = self.binding_ids.get(&name).cloned() {
                            self.edges.push(ExtractedEdge {
                                source: cur.clone(),
                                target: bid,
                                confidence: "resolved".into(),
                                kind: Some("use-read".into()),
                                read_kind: Some("store".into()),
                            });
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_struct(self, node);
    }
```

  And in `visit_local`, after recording the type, emit/register the binding node:

```rust
        if let Some(name) = local_bound_name(node) {
            if let Some(cur) = self.current_fn.clone() {
                let n = { let c = self.binding_counter.entry(cur.clone()).or_insert(0); *c += 1; *c };
                let bid = format!("binding:{cur}::{name}@{n}");
                self.nodes.push(ExtractedNode { id: bid.clone(), label: name.clone(), kind: "binding".into() });
                self.binding_ids.insert(name, bid);
            }
        }
```

  Add fields `binding_ids: HashMap<String,String>`, `binding_counter: HashMap<String,u32>` to the struct (and reset `binding_ids`/restore on fn enter/exit alongside `local_types`), plus the helper:

```rust
fn local_bound_name(l: &syn::Local) -> Option<String> {
    match &l.pat {
        syn::Pat::Ident(id) => Some(id.ident.to_string()),
        syn::Pat::Type(pt) => if let syn::Pat::Ident(id) = &*pt.pat { Some(id.ident.to_string()) } else { None },
        _ => None,
    }
}
```

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_rust`.
  Expected: both `dataflow_rust` tests PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_rust.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): binding nodes + store read-kind (struct-literal move)"`

## Task B3: `read_kind: "control"` + `"consume"` classification (TDD) [SEQUENTIAL]

To avoid false positives the detector must recognise a real `control` read (`if`/`match`/`while` scrutinee, `?`, `.is_empty()`-gated return) — its **presence** clears the dead signal. `consume` (passed to a call / `return`ed) is recorded for completeness.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; extend `tests/dataflow_rust.rs`.

- [ ] **Step 1: Failing test** — append to `tests/dataflow_rust.rs`:

```rust
#[test]
fn if_condition_on_field_is_control_read() {
    let src = r#"
struct Stats { failures: Vec<String> }
fn gate(s: &Stats) -> Result<(), String> {
    if !s.failures.is_empty() {
        return Err(String::from("had failures"));
    }
    Ok(())
}
"#;
    let g = extract_dataflow_in_module(std::path::Path::new("m.rs"), src, "m");
    assert!(g.edges.iter().any(|e|
        e.kind.as_deref() == Some("use-read")
        && e.read_kind.as_deref() == Some("control")
        && e.target == "field:Stats::failures"),
        "expected control read of field:Stats::failures: {:?}", g.edges);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_rust if_condition_on_field_is_control_read`. Expected: FAIL.

- [ ] **Step 3: Implement** — add control-context tracking to the visitor:
  - Maintain a `control_depth: u32` bumped while visiting the **condition** sub-expression of `ExprIf` (`node.cond`), the scrutinee of `ExprMatch` (`node.expr`), the condition of `ExprWhile`, and any expression directly under `ExprTry` (`x?`). Implement by overriding `visit_expr_if` / `visit_expr_match` / `visit_expr_while` / `visit_expr_try` to visit the controlling sub-expr with `control_depth += 1` and the body with it restored.
  - Add a generic **field/binding read** sink: override `visit_expr_field` (`recv.field` read, not the write LHS — guard against double-counting the `.push` receiver by skipping when the parent is the accumulating method-call receiver; simplest: in `visit_expr_method_call` for accumulators, do NOT recurse into `node.receiver`'s field as a read — call `syn::visit::visit_expr` on the args only). When a `field:`/`binding:` is read and `control_depth > 0` → `read_kind: "control"`; else if the read is an argument to a call or a `return` operand → `"consume"`; else **drop** (a bare mention we can't classify is not a control read — conservative, never miss-clears a real dead signal).
  - Recognise `recv.field.is_empty()`/`.len()` inside a controlling context as a `control` read of `field:Owner::field` (resolve owner via `owner_of` as in B1).

  Sketch:

```rust
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.control_depth += 1;
        self.visit_expr(&node.cond);
        self.control_depth -= 1;
        self.visit_block(&node.then_branch);
        if let Some((_, else_b)) = &node.else_branch { self.visit_expr(else_b); }
    }
    // analogous visit_expr_match / visit_expr_while / visit_expr_try
```

  In the field-read sink, emit the edge with `read_kind` = `"control"` if `self.control_depth > 0` else skip (B3 only needs `control`; `consume` is recorded when the read is a direct call-arg — optional, may be added if a test needs it). Reuse `owner_of` + `field_name`; emit `field:<owner>::<field>` (dedup via `field_node`).

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_rust`. Expected: all 3 PASS, and re-run B2's `no control read expected` assertion still holds (the store case has no `if`/`match`).

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_rust.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): control read-kind (if/match/while/?/is_empty gate)"`

## Task B4: `ignored_result` detection input — drop-on-unknown (TDD) [SEQUENTIAL]

Record statement-expression calls whose value is dropped (`expr;` / `let _ =`) as candidate `ignored_result` sites. We **cannot** see the callee return type intra-file in general → the detector confidence is `heuristic` and the analysis **drops** when the callee is unknown (no false finding). B4 only records the structural candidate edges; the detector logic is D2.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; extend `tests/dataflow_rust.rs`.

- [ ] **Step 1: Failing test** — append:

```rust
#[test]
fn dropped_call_statement_is_recorded_as_ignored_candidate() {
    let src = r#"
fn risky() -> Result<(), String> { Ok(()) }
fn caller() {
    risky(); // value dropped, no ?/unwrap/match
}
"#;
    let g = extract_dataflow_in_module(std::path::Path::new("m.rs"), src, "m");
    // An ignored-candidate is recorded as a use-read edge with read_kind="ignored" from caller
    // to a synthetic binding node binding:m::caller::<call>@n (the dropped temp).
    assert!(g.edges.iter().any(|e|
        e.source == "m::caller"
        && e.kind.as_deref() == Some("use-read")
        && e.read_kind.as_deref() == Some("ignored")),
        "expected ignored candidate: {:?}", g.edges);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_rust dropped_call_statement_is_recorded_as_ignored_candidate`. Expected: FAIL.

- [ ] **Step 3: Implement** — override `visit_stmt` (or `visit_block`): for a `Stmt::Expr(expr, Some(semi))` where `expr` is an `ExprCall`/`ExprMethodCall` **not** wrapped in `?`/`.unwrap()`/`.expect()`/assignment/`let`, emit a synthetic `binding:<fn>::<call>@<n>` node (label = callee bare name) + a `use-read` edge `read_kind: "ignored"`. Also handle `Stmt::Local` with `Pat::Wild` (`let _ = call();`). Keep it conservative: only fire on a direct call expr statement; anything chained/awaited/etc. is dropped.

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_rust`. Expected: all 4 PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_rust.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): record dropped-call (ignored_result) candidates"`

---

# PHASE C — TS def-use (tree-sitter) — Batch β, [PARALLEL-SAFE] with Phase B

> Phase C writes the **TS** branch of `extract_dataflow_in_module` (under `#[cfg(feature = "tree-sitter-grammars")]`) + `tests/dataflow_ts.rs`. Partitioned from Phase B's `.rs` branch.

## Task C1: TS `.push` accumulation → `def-write` (TDD) [SEQUENTIAL]

Mirror B1 for TS: `this.failures.push(x)` / `failures.push(x)` → `field:`/`binding:` accumulation `def-write`. Use the A0d-recorded node-kind names (`call_expression`, `member_expression`, `arguments`).

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs` (TS branch); create `crates/vox-graphify-reader/tests/dataflow_ts.rs`.

- [ ] **Step 1: Failing test** — create `crates/vox-graphify-reader/tests/dataflow_ts.rs`:

```rust
#![cfg(feature = "tree-sitter-grammars")]
use std::path::Path;
use vox_graphify_reader::dataflow::extract_dataflow_in_module;

#[test]
fn ts_push_is_accumulating_def_write() {
    let src = r#"
function collect(errors: string[]) {
  errors.push("boom");
}
"#;
    let g = extract_dataflow_in_module(Path::new("m.ts"), src, "m");
    assert!(g.edges.iter().any(|e|
        e.kind.as_deref() == Some("def-write")
        && e.read_kind.as_deref() == Some("accumulate")
        && e.source == "m::collect"),
        "expected accumulating def-write in TS: {:?}", g.edges);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_ts ts_push_is_accumulating_def_write`. Expected: FAIL (TS branch empty).

- [ ] **Step 3: Implement** — in `extract_dataflow_in_module`, add the TS branch (mirroring `ast.rs`'s tree-sitter stack-walk under `#[cfg(feature = "tree-sitter-grammars")]`): parse with the same `language` match (`ts`/`tsx`/`js`/`jsx`/`py`), track `current_fn` on `function_declaration`/`method_definition`/`function_definition`. For a `call_expression` whose `function` child is a `member_expression` ending in `.push`/`.extend`/`.add`/`.set` (TS accumulators; keep a `TS_ACCUMULATING` const), and whose member object is an `identifier` or `member_expression` rooted at `this`, emit a `binding:`/`field:` `def-write` with `read_kind: "accumulate"`. TS field-owner resolution is weaker than Rust (no struct types): for `this.<f>.push` emit `field:<EnclosingClass>::<f>`; for `<local>.push` emit `binding:<fn>::<local>@1`. Drop when neither shape matches.

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_ts`. Expected: PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_ts.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): TS accumulating def-write (tree-sitter)"`

## Task C2: TS `store` vs `control` read classification (TDD) [SEQUENTIAL]

Mirror B2/B3 for TS: object-literal property shorthand (`return { errors }`) → `store`; `if (errors.length)` / `if (!errors.isEmpty())` / `match`-less `switch` scrutinee → `control`.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs` (TS branch); extend `tests/dataflow_ts.rs`.

- [ ] **Step 1: Failing test** — append:

```rust
#[test]
fn ts_object_literal_is_store_if_length_is_control() {
    let store_src = r#"
function build(errors: string[]) {
  errors.push("x");
  return { errors };
}
"#;
    let g = extract_dataflow_in_module(Path::new("b.ts"), store_src, "b");
    assert!(g.edges.iter().any(|e| e.kind.as_deref()==Some("use-read") && e.read_kind.as_deref()==Some("store")),
        "store read: {:?}", g.edges);
    assert!(!g.edges.iter().any(|e| e.read_kind.as_deref()==Some("control")), "no control read");

    let ctrl_src = r#"
function gate(errors: string[]) {
  if (errors.length > 0) { throw new Error("bad"); }
}
"#;
    let g2 = extract_dataflow_in_module(Path::new("g.ts"), ctrl_src, "g");
    assert!(g2.edges.iter().any(|e| e.read_kind.as_deref()==Some("control")), "control read: {:?}", g2.edges);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dataflow_ts ts_object_literal_is_store_if_length_is_control`. Expected: FAIL.

- [ ] **Step 3: Implement** — TS branch: track a `control_depth` bumped while inside an `if_statement` `condition` / `while_statement` `condition` / `switch_statement` value / `ternary_expression` condition. On an `object` node's `pair`/shorthand `shorthand_property_identifier` whose value resolves to a tracked binding → `store` read. On a `member_expression` `.length` / `.size` (or an identifier read) inside `control_depth > 0` → `control` read of the corresponding `binding:`/`field:`. Drop unresolved.

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dataflow_ts`. Expected: all PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_ts.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): TS store/control read classification"`

---

# PHASE D — Detectors + `compute_dead_signals` + frontend-emit e2e

> Needs Phase B (and benefits from C). The detector consumes the **whole graph** (`&Value`), mirroring `compute_coverage`.

## Task D1: `compute_dead_signals` skeleton + `write_only_field` (TDD) [SEQUENTIAL]

`write_only_field` is the whole-graph, purely structural detector (count edges) — implement it first to lock the report wiring.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; create `crates/vox-graphify-reader/tests/dead_signals.rs`.

- [ ] **Step 1: Failing test** — create `crates/vox-graphify-reader/tests/dead_signals.rs`:

```rust
use serde_json::json;
use vox_graphify_reader::dataflow::{compute_dead_signals, DeadSignalKind};

#[test]
fn write_only_field_fires_when_no_reads() {
    let g = json!({
        "nodes": [
            {"id":"field:S::x","label":"x","kind":"field"},
            {"id":"m::w","label":"w","kind":"fn"}
        ],
        "links": [
            {"source":"m::w","target":"field:S::x","confidence":"resolved","kind":"def-write"}
        ]
    });
    let r = compute_dead_signals(&g);
    let f = r.findings.iter().find(|f| f.node_id == "field:S::x").expect("finding");
    assert_eq!(f.detector, DeadSignalKind::WriteOnlyField);
    assert_eq!(f.confidence, "resolved");
}

#[test]
fn field_with_any_read_is_not_write_only() {
    let g = json!({
        "nodes": [{"id":"field:S::x","label":"x","kind":"field"}],
        "links": [
            {"source":"m::w","target":"field:S::x","kind":"def-write"},
            {"source":"m::r","target":"field:S::x","kind":"use-read","read_kind":"store"}
        ]
    });
    let r = compute_dead_signals(&g);
    assert!(!r.findings.iter().any(|f| f.node_id=="field:S::x" && f.detector==DeadSignalKind::WriteOnlyField));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dead_signals`. Expected: FAIL (`compute_dead_signals` absent).

- [ ] **Step 3: Implement** — add to `dataflow.rs` (reuse the `coverage.rs` `nodes()`/`links()`/`str_field()` helper pattern — copy them in, or `pub(crate)` them from `coverage.rs`; copying keeps modules decoupled):

```rust
fn nodes(graph: &Value) -> &[Value] {
    graph.get("nodes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn links(graph: &Value) -> &[Value] {
    graph.get("links").or_else(|| graph.get("edges"))
        .and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn sf<'a>(v: &'a Value, k: &str) -> Option<&'a str> { v.get(k).and_then(Value::as_str) }

/// Compute the dead-signal lint report from the data-flow edges in `graph`. Honesty firewall:
/// reports what the def-use edges say; makes no judgment about whether a swallow is intentional;
/// confidence-labels each finding so an agent or CI gate can pick a threshold.
pub fn compute_dead_signals(graph: &Value) -> DeadSignalReport {
    let links = links(graph);
    let mut findings = Vec::new();
    for node in nodes(graph) {
        let Some(id) = sf(node, "id") else { continue };
        if !(id.starts_with("field:") || id.starts_with("binding:")) { continue; }
        let label = sf(node, "label").unwrap_or(id).to_string();

        let mut writes: Vec<String> = Vec::new();
        let mut read_kinds: Vec<String> = Vec::new();
        let mut accumulates = false;
        for l in links {
            if sf(l, "target") != Some(id) { continue; }
            match sf(l, "kind") {
                Some("def-write") => {
                    if let Some(s) = sf(l, "source") { writes.push(s.to_string()); }
                    if sf(l, "read_kind") == Some("accumulate") { accumulates = true; }
                }
                Some("use-read") => {
                    if let Some(rk) = sf(l, "read_kind") {
                        if rk != "ignored" { read_kinds.push(rk.to_string()); }
                    }
                }
                _ => {}
            }
        }
        read_kinds.sort(); read_kinds.dedup();

        // write_only_field: ≥1 def-write, zero use-read of any kind.
        if !writes.is_empty() && read_kinds.is_empty() {
            findings.push(DeadSignal {
                detector: DeadSignalKind::WriteOnlyField,
                node_id: id.to_string(), label: label.clone(),
                write_sites: writes.clone(), read_kinds: read_kinds.clone(),
                confidence: "resolved".into(),
                rationale: format!("{label} is written but never read."),
            });
        }
        let _ = accumulates; // used by D2
    }
    DeadSignalReport { findings }
}
```

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dead_signals`. Expected: both PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dead_signals.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): compute_dead_signals + write_only_field detector"`

## Task D2: `accumulator_never_gates` + `ignored_result` detectors (TDD) [SEQUENTIAL]

The headline detector: accumulating `def-write` + a `store`/`consume` read + **zero `control` reads** → fire `heuristic`. Plus `ignored_result` from the B4 `read_kind:"ignored"` candidates.

**Files:** Modify `crates/vox-graphify-reader/src/dataflow.rs`; extend `tests/dead_signals.rs`.

- [ ] **Step 1: Failing test** — append to `tests/dead_signals.rs`:

```rust
#[test]
fn accumulator_never_gates_fires() {
    let g = json!({
        "nodes": [{"id":"field:S::errs","label":"errs","kind":"field"}],
        "links": [
            {"source":"m::f","target":"field:S::errs","kind":"def-write","read_kind":"accumulate"},
            {"source":"m::f","target":"field:S::errs","kind":"use-read","read_kind":"store"}
        ]
    });
    let r = compute_dead_signals(&g);
    let f = r.findings.iter().find(|f| f.detector==DeadSignalKind::AccumulatorNeverGates)
        .expect("accumulator_never_gates finding");
    assert_eq!(f.node_id, "field:S::errs");
    assert_eq!(f.confidence, "heuristic");
}

#[test]
fn accumulator_with_control_read_does_not_fire() {
    let g = json!({
        "nodes": [{"id":"field:S::errs","label":"errs","kind":"field"}],
        "links": [
            {"source":"m::f","target":"field:S::errs","kind":"def-write","read_kind":"accumulate"},
            {"source":"m::g","target":"field:S::errs","kind":"use-read","read_kind":"control"}
        ]
    });
    let r = compute_dead_signals(&g);
    assert!(!r.findings.iter().any(|f| f.detector==DeadSignalKind::AccumulatorNeverGates),
        "control read must clear the signal");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test dead_signals accumulator`. Expected: FAIL.

- [ ] **Step 3: Implement** — in `compute_dead_signals`, after the `write_only_field` block, add (using `accumulates` + `read_kinds`):

```rust
        let has_control = read_kinds.iter().any(|k| k == "control");
        let has_store_or_consume = read_kinds.iter().any(|k| k == "store" || k == "consume");
        if accumulates && !has_control && has_store_or_consume {
            findings.push(DeadSignal {
                detector: DeadSignalKind::AccumulatorNeverGates,
                node_id: id.to_string(), label: label.clone(),
                write_sites: writes.clone(), read_kinds: read_kinds.clone(),
                confidence: "heuristic".into(),
                rationale: format!(
                    "{label} is populated via accumulation and flows only into a store/consume read; \
                     never read with read_kind=control to gate a branch or return."),
            });
        }
```

  For `ignored_result`: iterate `links` once for `use-read` edges with `read_kind == "ignored"`, grouping by `source` fn + target candidate node; emit a `DeadSignal { detector: IgnoredResult, confidence: "heuristic", … }` per candidate (node_id = the synthetic `binding:…::<call>@n`). (Confidence is `heuristic` because the callee return type is unverified intra-file — drop is already handled upstream by only recording direct call statements.)

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader --test dead_signals`. Expected: all PASS.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dead_signals.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): accumulator_never_gates + ignored_result detectors"`

## Task D3: Frontend-emit reproduction — end-to-end fixture (MANDATORY) [SEQUENTIAL]

**The required test that reproduces the frontend-emit case.** A `reactive_view_emit_failures`-like field is `.push`-accumulated per component and only moved into an output struct (`store`), never read for control flow. Extraction → `compute_dead_signals` must yield exactly one `accumulator_never_gates` finding for that field. This proves the full pipeline (source → def-use → detector) on the real bug class.

**Files:** Extend `crates/vox-graphify-reader/tests/dataflow_rust.rs`.

- [ ] **Step 1: Failing test** — append to `tests/dataflow_rust.rs`:

```rust
use vox_graphify_reader::dataflow::{compute_dead_signals, DeadSignalKind};

/// Reproduces the frontend-emit bug class (docs/superpowers/plans/2026-06-26-frontend-emit-
/// validation-gate.md): `generate_with_options` populates
/// `ReactiveViewBridgeStats::reactive_view_emit_failures` via `.push`, then only moves the stats
/// into `Ok(CodegenOutput { reactive_stats })` (a STORE read) and returns Ok regardless — the
/// accumulator is never read for control flow. `accumulator_never_gates` MUST flag it.
#[test]
fn frontend_emit_accumulator_is_flagged_end_to_end() {
    let src = r#"
struct WebIrDiagnostic { msg: String }
struct ReactiveViewBridgeStats { reactive_view_emit_failures: Vec<WebIrDiagnostic> }
struct CodegenOutput { reactive_stats: ReactiveViewBridgeStats }

fn generate_with_options() -> Result<CodegenOutput, String> {
    let mut reactive_stats: ReactiveViewBridgeStats =
        ReactiveViewBridgeStats { reactive_view_emit_failures: vec![] };
    for _ in 0..3 {
        // per-component blocking diagnostic accumulation
        reactive_stats.reactive_view_emit_failures.push(WebIrDiagnostic { msg: String::from("blocked") });
    }
    // BUG: never reads reactive_view_emit_failures to gate the return; just stores + Ok.
    Ok(CodegenOutput { reactive_stats })
}
"#;
    // 1. Extract def-use from the source.
    let g = extract_dataflow_in_module(std::path::Path::new("emitter.rs"), src, "emitter");

    // 2. Convert to the graph.json shape (nodes + links with kind/read_kind) the detector reads.
    let nodes: Vec<serde_json::Value> = g.nodes.iter()
        .map(|n| serde_json::json!({"id":n.id,"label":n.label,"kind":n.kind})).collect();
    let links: Vec<serde_json::Value> = g.edges.iter().map(|e| {
        let mut o = serde_json::json!({"source":e.source,"target":e.target,"confidence":e.confidence});
        if let Some(k) = &e.kind { o["kind"] = serde_json::json!(k); }
        if let Some(rk) = &e.read_kind { o["read_kind"] = serde_json::json!(rk); }
        o
    }).collect();
    let graph = serde_json::json!({"nodes":nodes,"links":links});

    // 3. The detector flags the accumulator.
    let report = compute_dead_signals(&graph);
    let hit = report.findings.iter().find(|f|
        f.detector == DeadSignalKind::AccumulatorNeverGates
        && f.node_id == "field:ReactiveViewBridgeStats::reactive_view_emit_failures");
    assert!(hit.is_some(),
        "frontend-emit accumulator must be flagged; findings={:?}", report.findings);
    let hit = hit.unwrap();
    assert_eq!(hit.confidence, "heuristic");
    assert!(hit.read_kinds.iter().any(|k| k == "store"),
        "the only read is a store into CodegenOutput; got {:?}", hit.read_kinds);
    assert!(!hit.read_kinds.iter().any(|k| k == "control"),
        "must have NO control read (that is the bug)");
}
```

- [ ] **Step 2: Run, verify** — `cargo test -p vox-graphify-reader --test dataflow_rust frontend_emit_accumulator_is_flagged_end_to_end`.
  If it FAILS first, debug the extraction (likely: the `.push` is on `reactive_stats.reactive_view_emit_failures` — a **nested** field access `<local>.<field>.push`; ensure B1's `field_receiver` resolves `reactive_stats`'s type via the `let … : ReactiveViewBridgeStats` / struct-init in B2, and the field owner is the struct name not the local). Adjust B1/B2 logic if needed (re-run their commits' tests to confirm no regression), then re-run.
  Expected final: PASS.

- [ ] **Step 3: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/dataflow.rs crates/vox-graphify-reader/tests/dataflow_rust.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search/dataflow): frontend-emit accumulator_never_gates end-to-end repro"`

---

# PHASE E — Merge def-use into `rebuild_graph` (the on-disk graph)

## Task E1: Wire `extract_dataflow_in_module` into the rebuild walk + serialize `kind`/`read_kind` (TDD) [SEQUENTIAL]

So `graph.json` actually carries the new edges and `vox search dead-signals` / the MCP tools read them off disk. Route def-use nodes/edges **around** `resolve_edges` (they are pre-resolved); extend the link serializer.

**Files:** Modify `crates/vox-graphify-reader/src/rebuild.rs`; extend `crates/vox-graphify-reader/tests/rebuild_tests.rs`.

- [ ] **Step 1: Failing test** — append to `crates/vox-graphify-reader/tests/rebuild_tests.rs`:

```rust
#[test]
fn rebuild_emits_dataflow_edges_in_graph_json() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("e.rs"), r#"
struct Stats { failures: Vec<String> }
fn collect(s: &mut Stats) { s.failures.push(String::from("x")); }
"#).unwrap();
    let out = tmp.path().join("out/graph.json");
    let cache = tmp.path().join("out/file_cache");
    let meta = RebuildMeta { corpus_id: "t".into(), git_sha: None, scope_path: "src".into(),
        extraction_mode: Some("structural".into()), built_at_rfc3339: "2026-06-26T00:00:00+00:00".into() };
    rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();
    let g: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert!(g["nodes"].as_array().unwrap().iter().any(|n| n["id"]=="field:Stats::failures"),
        "field node missing from graph.json");
    assert!(g["links"].as_array().unwrap().iter().any(|l|
        l["kind"]=="def-write" && l["target"]=="field:Stats::failures" && l["read_kind"]=="accumulate"),
        "def-write edge missing from graph.json: {}", g["links"]);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader --test rebuild_tests rebuild_emits_dataflow_edges_in_graph_json`. Expected: FAIL.

- [ ] **Step 3: Implement** —
  (a) In the walk loop (`rebuild.rs:172`), after the structural `graph` is computed for each file, also run def-use and collect into **separate** accumulators so they bypass `resolve_edges`:

```rust
    let mut df_nodes: Vec<crate::ast::ExtractedNode> = Vec::new();
    let mut df_edges: Vec<crate::ast::ExtractedEdge> = Vec::new();
```

  inside the loop, after `all_edges.extend(graph.edges);`:

```rust
        let df = crate::dataflow::extract_dataflow_in_module(path, &content, &module_id);
        df_nodes.extend(df.nodes);
        df_edges.extend(df.edges);
```

  (b) After `let all_edges = resolve_edges(&all_nodes, &all_edges);` (`:240`), splice the def-use graph in:

```rust
    // Data-flow nodes/edges are already resolved to field:/binding: ids — do NOT route them
    // through resolve_edges (which would drop them). Dedup field/binding nodes by id.
    {
        use std::collections::HashSet;
        let existing: HashSet<String> = all_nodes.iter().map(|n| n.id.clone()).collect();
        for n in df_nodes {
            if !existing.contains(&n.id) { all_nodes.push(n); }
        }
    }
    let mut all_edges = all_edges; // rebind mutable
    all_edges.extend(df_edges);
```

  (c) Extend the **link serializer** (`:320`) to carry `kind`/`read_kind` when present:

```rust
    let links_val: Vec<serde_json::Value> = all_edges
        .iter()
        .map(|e| {
            let mut l = serde_json::json!({
                "source": e.source, "target": e.target, "confidence": e.confidence
            });
            if let Some(k) = &e.kind { l["kind"] = serde_json::json!(k); }
            if let Some(rk) = &e.read_kind { l["read_kind"] = serde_json::json!(rk); }
            l
        })
        .collect();
```

  Note: the def-use nodes flow through Leiden clustering (`cluster_nodes`, `:294`) like any node — harmless; they get a `community` like everything else. The `cluster_edges_input` (`:286`) will include def-use edges (extra adjacency) — acceptable and deterministic.

- [ ] **Step 4: Run, verify pass** — `cargo test -p vox-graphify-reader`. Expected: the new test PASSES and **all pre-existing `rebuild_tests` goldens still pass** (call edges still serialize without `kind`/`read_kind`; node/edge counts for structural-only fixtures with no `.push`/struct-field writes are unchanged because such fixtures emit no def-use edges).

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/rebuild.rs crates/vox-graphify-reader/tests/rebuild_tests.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search/dataflow): merge def-use nodes/edges into graph.json (around resolve_edges)"`

---

# PHASE F — MCP tools `vox_search_dataflow` + `vox_search_dead_signals` — Batch δ, [PARALLEL-SAFE] with Phase G

## Task F1: `vox_search_dataflow` handler + schema + dispatch (TDD) [SEQUENTIAL]

Returns the def-use edges (`def-write`/`use-read` with `read_kind`) incident to a `node_id`. `layer: "structural"`. Mirrors `graphify_query`'s load-graph-from-disk pattern.

**Files:** Modify `crates/vox-orchestrator-mcp/src/graphify_tools.rs`, `dispatch.rs`, `input_schemas.rs`.

- [ ] **Step 1: Failing test** — append a `#[tokio::test]` to the `mod tests` in `graphify_tools.rs` (reuse `write_registry` + `test_state_for_repo`; write a `graph.json` with a def-write edge to `field:S::x`):

```rust
    #[tokio::test]
    async fn dataflow_returns_incident_defuse_edges() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("graph.json"),
            r#"{"nodes":[{"id":"field:S::x","label":"x","kind":"field"},{"id":"m::w","kind":"fn"}],
                "links":[{"source":"m::w","target":"field:S::x","kind":"def-write","read_kind":"accumulate","confidence":"resolved"}]}"#).unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = vox_search_dataflow(&state, GraphifyDataflowParams {
            corpus: Some("repo-code-graph".into()), node_id: "field:S::x".into() }).await;
        let p: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(p["success"], serde_json::json!(true), "tool error: {json}");
        assert_eq!(p["data"]["layer"], serde_json::json!("structural"));
        let edges = p["data"]["edges"].as_array().expect("edges");
        assert_eq!(edges[0]["kind"], serde_json::json!("def-write"));
        assert_eq!(edges[0]["read_kind"], serde_json::json!("accumulate"));
    }
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-orchestrator-mcp dataflow_returns_incident_defuse_edges`. Expected: FAIL (handler absent).

- [ ] **Step 3: Implement handler** — add to `graphify_tools.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct GraphifyDataflowParams {
    pub corpus: Option<String>,
    /// A `fn`/`field:`/`binding:` node id to report def-use edges for.
    pub node_id: String,
}

/// `vox_search_dataflow`: def-use edges (def-write / use-read w/ read_kind) incident to a node.
/// Deterministic; `layer: "structural"`.
pub async fn vox_search_dataflow(state: &ServerState, params: GraphifyDataflowParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus, &None) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
    };
    let graph = match load_graph_json(repo_root, corpus) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY).to_json(),
    };
    let links = graph.get("links").or_else(|| graph.get("edges"))
        .and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let incident: Vec<serde_json::Value> = links.into_iter().filter(|l| {
        let k = l.get("kind").and_then(|v| v.as_str());
        let is_df = matches!(k, Some("def-write") | Some("use-read"));
        let touches = l.get("source").and_then(|v| v.as_str()) == Some(params.node_id.as_str())
            || l.get("target").and_then(|v| v.as_str()) == Some(params.node_id.as_str());
        is_df && touches
    }).collect();
    ToolResult::ok(serde_json::json!({
        "layer": "structural",
        "corpus_id": corpus_id,
        "node_id": params.node_id,
        "edges": incident,
    })).to_json()
}
```

- [ ] **Step 4: Dispatch arm** — in `dispatch.rs` after the `vox_graphify_compare` arm (`:641`):

```rust
        "vox_search_dataflow" => {
            Ok(crate::graphify_tools::vox_search_dataflow(state, serde_json::from_value(args)?).await)
        }
```

- [ ] **Step 5: Schema arm** — in `input_schemas.rs` after the `vox_graphify_compare` arm (`:485`):

```rust
        "vox_search_dataflow" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"node_id":{"type":"string","minLength":1,"description":"fn/field:/binding: node id to report def-use edges for"}},"required":["node_id"],"additionalProperties":false}"#,
        ),
```

- [ ] **Step 6: Run, verify pass** — `cargo test -p vox-orchestrator-mcp dataflow_returns_incident_defuse_edges`. Expected: PASS.

- [ ] **Step 7: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/graphify_tools.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): vox_search_dataflow MCP tool (def-use edges, layer=structural)"`

## Task F2: `vox_search_dead_signals` handler + schema + dispatch (TDD) [SEQUENTIAL]

Returns `DeadSignalReport.findings[]`, optionally filtered by `detector` / `min_confidence`. `layer: "structural"`.

**Files:** Modify `crates/vox-orchestrator-mcp/src/graphify_tools.rs`, `dispatch.rs`, `input_schemas.rs`.

- [ ] **Step 1: Failing test** — append to `mod tests`:

```rust
    #[tokio::test]
    async fn dead_signals_reports_accumulator_finding() {
        let tmp = tempfile::tempdir().unwrap();
        write_registry(tmp.path());
        let dir = tmp.path().join(".vox/cache/graphify/repo-code-graph");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("graph.json"),
            r#"{"nodes":[{"id":"field:S::errs","label":"errs","kind":"field"}],
                "links":[{"source":"m::f","target":"field:S::errs","kind":"def-write","read_kind":"accumulate"},
                         {"source":"m::f","target":"field:S::errs","kind":"use-read","read_kind":"store"}]}"#).unwrap();
        let state = test_state_for_repo(tmp.path().to_path_buf());
        let json = vox_search_dead_signals(&state, GraphifyDeadSignalsParams {
            corpus: Some("repo-code-graph".into()), detector: None, min_confidence: None }).await;
        let p: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(p["success"], serde_json::json!(true), "tool error: {json}");
        assert_eq!(p["data"]["layer"], serde_json::json!("structural"));
        let fs_ = p["data"]["findings"].as_array().expect("findings");
        assert!(fs_.iter().any(|f| f["detector"]=="accumulator_never_gates"), "{json}");
    }
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-orchestrator-mcp dead_signals_reports_accumulator_finding`. Expected: FAIL.

- [ ] **Step 3: Implement handler** — add to `graphify_tools.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct GraphifyDeadSignalsParams {
    pub corpus: Option<String>,
    /// Optional filter: "ignored_result" | "write_only_field" | "accumulator_never_gates".
    #[serde(default)]
    pub detector: Option<String>,
    /// Optional minimum confidence: "heuristic" (all) | "resolved" (resolved-only).
    #[serde(default)]
    pub min_confidence: Option<String>,
}

/// `vox_search_dead_signals`: the def-use lint report. `layer: "structural"`.
pub async fn vox_search_dead_signals(state: &ServerState, params: GraphifyDeadSignalsParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
    };
    let (corpus, corpus_id) = match resolve_search_corpus(&reg, &params.corpus, &None) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM_GRAPHIFY).to_json(),
    };
    let graph = match load_graph_json(repo_root, corpus) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY).to_json(),
    };
    let report = vox_graphify_reader::dataflow::compute_dead_signals(&graph);
    // Serialize then filter (DeadSignalKind serializes snake_case == the filter string).
    let mut findings: Vec<serde_json::Value> = report.findings.iter()
        .map(|f| serde_json::to_value(f).unwrap_or(serde_json::Value::Null))
        .collect();
    if let Some(d) = &params.detector {
        findings.retain(|f| f.get("detector").and_then(|v| v.as_str()) == Some(d.as_str()));
    }
    if params.min_confidence.as_deref() == Some("resolved") {
        findings.retain(|f| f.get("confidence").and_then(|v| v.as_str()) == Some("resolved"));
    }
    ToolResult::ok(serde_json::json!({
        "layer": "structural",
        "corpus_id": corpus_id,
        "findings": findings,
    })).to_json()
}
```

- [ ] **Step 4: Dispatch arm** — in `dispatch.rs` after the `vox_search_dataflow` arm:

```rust
        "vox_search_dead_signals" => {
            Ok(crate::graphify_tools::vox_search_dead_signals(state, serde_json::from_value(args)?).await)
        }
```

- [ ] **Step 5: Schema arm** — in `input_schemas.rs` after the `vox_search_dataflow` arm:

```rust
        "vox_search_dead_signals" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"detector":{"type":"string","enum":["ignored_result","write_only_field","accumulator_never_gates"],"description":"Filter to one detector"},"min_confidence":{"type":"string","enum":["heuristic","resolved"],"description":"Minimum finding confidence (resolved = resolved-only)"}},"additionalProperties":false}"#,
        ),
```

- [ ] **Step 6: Run, verify pass** — `cargo test -p vox-orchestrator-mcp dead_signals_reports_accumulator_finding`. Expected: PASS.

- [ ] **Step 7: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/graphify_tools.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): vox_search_dead_signals MCP tool (DeadSignalReport, layer=structural)"`

## Task F3: Tier-presence assertion — both tools in the default tier (TDD) [SEQUENTIAL]

Per master spec §3.2: a CI/unit assertion that the new tools are present so a tiering change can't silently drop them. Lightweight: assert the dispatcher resolves a schema for both names.

**Files:** Modify `crates/vox-orchestrator-mcp/src/input_schemas.rs` (add a `#[cfg(test)]` assertion, or extend an existing schema-coverage test).

- [ ] **Step 1: Failing test** — locate the existing schema-resolution test in `input_schemas.rs` (search for `fn ` in its `#[cfg(test)] mod tests`); if none, add:

```rust
#[cfg(test)]
mod dataflow_schema_presence {
    #[test]
    fn dataflow_tools_have_schemas() {
        for name in ["vox_search_dataflow", "vox_search_dead_signals"] {
            let s = super::schema_for(name); // use the real fn name found in this module
            assert!(s.is_some(), "missing input schema for {name}");
        }
    }
}
```
  Replace `schema_for` with the actual public/`pub(crate)` schema-lookup fn name in `input_schemas.rs` (the one whose body holds the `match name { … }` you edited in F1/F2). If that fn returns a non-`Option`, assert it does not panic / returns a non-empty object instead.

- [ ] **Step 2: Run, verify** — `cargo test -p vox-orchestrator-mcp dataflow_tools_have_schemas`.
  Expected: PASS (schemas were added in F1/F2). If the lookup fn signature differs, adjust the assertion to match (this task is a guard, not new behavior).

- [ ] **Step 3: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/input_schemas.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search): assert dataflow tools have input schemas (tier-presence guard)"`

---

# PHASE G — CLI subcommands `dataflow` + `dead-signals` — Batch δ, [PARALLEL-SAFE] with Phase F

> Adds arms to the existing `GraphifyCmd` enum (`crates/vox-cli/src/commands/graphify/mod.rs`). P0 re-homes this enum under `vox search`; until then they are reachable as `vox graphify dataflow` / `vox graphify dead-signals`.

## Task G1: `Dataflow` + `DeadSignals` subcommands + run arms (TDD) [SEQUENTIAL]

Mirror the existing `Coverage` arm (`mod.rs:396`): resolve corpus → read graph.json → call the reader → write/print JSON.

**Files:** Modify `crates/vox-cli/src/commands/graphify/mod.rs`.

- [ ] **Step 1: Add enum variants** — in `enum GraphifyCmd` (`:13`), after `Coverage { … }` (`:52`):

```rust
    /// Report def-use edges (def-write / use-read w/ read_kind) incident to a node.
    Dataflow {
        /// Corpus id (default: registry `default_corpus_id`).
        #[arg(long)]
        corpus: Option<String>,
        /// fn/field:/binding: node id to report def-use edges for.
        node_id: String,
        /// Write JSON to this path instead of printing to stdout.
        #[arg(long)]
        out: Option<String>,
    },
    /// Run the data-flow dead-signal detectors over a corpus graph.
    DeadSignals {
        /// Corpus id (default: registry `default_corpus_id`).
        #[arg(long)]
        corpus: Option<String>,
        /// Filter to one detector (ignored_result | write_only_field | accumulator_never_gates).
        #[arg(long)]
        detector: Option<String>,
        /// Write JSON to this path instead of printing to stdout.
        #[arg(long)]
        out: Option<String>,
    },
```

- [ ] **Step 2: Add run arms** — in `pub async fn run` after the `Coverage` arm (`:428`):

```rust
        GraphifyCmd::Dataflow { corpus, node_id, out } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus = corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let graph_path = repo_root.join(&corpus.graph_path);
            let raw = std::fs::read_to_string(&graph_path).with_context(|| format!("read graph {}", graph_path.display()))?;
            let graph: serde_json::Value = serde_json::from_str(&raw).with_context(|| format!("parse graph JSON {}", graph_path.display()))?;
            let links = graph.get("links").or_else(|| graph.get("edges")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let incident: Vec<serde_json::Value> = links.into_iter().filter(|l| {
                let k = l.get("kind").and_then(|v| v.as_str());
                let is_df = matches!(k, Some("def-write") | Some("use-read"));
                is_df && (l.get("source").and_then(|v| v.as_str()) == Some(node_id.as_str())
                    || l.get("target").and_then(|v| v.as_str()) == Some(node_id.as_str()))
            }).collect();
            let payload = serde_json::json!({"corpus_id": corpus_id, "node_id": node_id, "edges": incident});
            emit_json(repo_root, &payload, out, &format!("dataflow: corpus={corpus_id}"))?;
        }
        GraphifyCmd::DeadSignals { corpus, detector, out } => {
            let reg = load_all_corpora(repo_root).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus_id = resolve_ingest_corpus_id(&reg, corpus).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let corpus = corpus_by_id(&reg, &corpus_id).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let graph_path = repo_root.join(&corpus.graph_path);
            let raw = std::fs::read_to_string(&graph_path).with_context(|| format!("read graph {}", graph_path.display()))?;
            let graph: serde_json::Value = serde_json::from_str(&raw).with_context(|| format!("parse graph JSON {}", graph_path.display()))?;
            let report = vox_graphify_reader::dataflow::compute_dead_signals(&graph);
            let mut findings = serde_json::to_value(&report.findings)?;
            if let (Some(d), Some(arr)) = (detector.as_deref(), findings.as_array_mut()) {
                arr.retain(|f| f.get("detector").and_then(|v| v.as_str()) == Some(d));
            }
            let payload = serde_json::json!({"corpus_id": corpus_id, "findings": findings});
            emit_json(repo_root, &payload, out, &format!("dead-signals: corpus={corpus_id}"))?;
        }
```

  Add a small `emit_json` helper near the top of the file (or inline both, copying the `Coverage` arm's write/print block). The helper:

```rust
fn emit_json(repo_root: &std::path::Path, payload: &serde_json::Value, out: Option<String>, label: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(payload)?;
    match out {
        Some(path) => {
            let abs = repo_root.join(&path);
            if let Some(parent) = abs.parent() { std::fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?; }
            std::fs::write(&abs, &json).with_context(|| format!("write {}", abs.display()))?;
            println!("{label} -> {path}");
        }
        None => println!("{json}"),
    }
    Ok(())
}
```

- [ ] **Step 3: Build, verify** — `cargo build -p vox-cli`. Expected: compiles. (No `vox-gui` build needed; `vox-cli` builds independently.)

- [ ] **Step 4: Smoke** — `cargo run -p vox-cli -- graphify dead-signals --help 2>&1 | head -20`. Expected: clap help for `dead-signals` lists `--corpus`, `--detector`, `--out`.

- [ ] **Step 5: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/graphify/mod.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): vox graphify dataflow + dead-signals CLI subcommands"`

## Task G2: End-to-end CLI run against a real fixture corpus [SEQUENTIAL]

Prove the full path: rebuild a tiny corpus with a frontend-emit-shaped `.rs`, then `dead-signals` reports the accumulator. Use an integration test under `crates/vox-cli/tests/` if one exists, else a `#[test]` driving the reader + the same JSON as the CLI emits (the CLI arm is thin).

**Files:** Add `crates/vox-cli/tests/graphify_dead_signals.rs` (or extend an existing CLI integration test).

- [ ] **Step 1: Test** — create `crates/vox-cli/tests/graphify_dead_signals.rs`:

```rust
// Drives the same reader call the `dead-signals` CLI arm makes, over a rebuilt corpus graph.
#[test]
fn rebuilt_corpus_surfaces_accumulator_dead_signal() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("emitter.rs"), r#"
struct Diag { msg: String }
struct Stats { failures: Vec<Diag> }
struct Out { stats: Stats }
fn generate() -> Result<Out, String> {
    let mut stats: Stats = Stats { failures: vec![] };
    stats.failures.push(Diag { msg: String::from("x") });
    Ok(Out { stats })
}
"#).unwrap();
    let out = tmp.path().join("graph.json");
    let cache = tmp.path().join("cache");
    let meta = vox_graphify_reader::rebuild::RebuildMeta {
        corpus_id: "t".into(), git_sha: None, scope_path: "src".into(),
        extraction_mode: Some("structural".into()),
        built_at_rfc3339: "2026-06-26T00:00:00+00:00".into() };
    vox_graphify_reader::rebuild::rebuild_graph(tmp.path(), &src, &out, &cache, &meta).unwrap();
    let graph: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let report = vox_graphify_reader::dataflow::compute_dead_signals(&graph);
    assert!(report.findings.iter().any(|f|
        f.node_id == "field:Stats::failures"
        && matches!(f.detector, vox_graphify_reader::dataflow::DeadSignalKind::AccumulatorNeverGates)),
        "CLI dead-signals path must surface the accumulator: {:?}", report.findings);
}
```
  (Confirm `rebuild`/`RebuildMeta`/`rebuild_graph` are `pub` in the reader; `lib.rs:24` exposes `pub mod rebuild`. Add `vox-graphify-reader` + `tempfile` to `crates/vox-cli/Cargo.toml` `[dev-dependencies]` if not already present — verify with `grep vox-graphify-reader crates/vox-cli/Cargo.toml`.)

- [ ] **Step 2: Run, verify pass** — `cargo test -p vox-cli --test graphify_dead_signals`. Expected: PASS.

- [ ] **Step 3: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/tests/graphify_dead_signals.rs crates/vox-cli/Cargo.toml`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search): CLI dead-signals e2e over rebuilt corpus (accumulator)"`

---

# PHASE H — Non-blocking CI advisory

## Task H1: `dead-signals` advisory in `vox ci` (non-blocking) [SEQUENTIAL]

Mirror the existing coverage advisory: run `compute_dead_signals` over the default corpus and **print** findings, **exit 0** (promotable to blocking later once the false-positive rate is measured — per source design §1.4).

**Files:** Modify `crates/vox-cli/src/commands/ci/mod.rs` (add an advisory fn + call site). First locate the coverage advisory to mirror: `grep -n "coverage" crates/vox-cli/src/commands/ci/mod.rs`.

- [ ] **Step 1: Implement** — add a function adjacent to the coverage advisory:

```rust
/// Non-blocking data-flow dead-signal advisory. Prints findings; always exits 0. Promotable to
/// blocking once the false-positive rate is measured (see dataflow design §1.4).
pub fn dead_signals_advisory(repo_root: &std::path::Path) -> anyhow::Result<()> {
    let reg = match vox_config::graphify::load_all_corpora(repo_root) {
        Ok(r) => r,
        Err(_) => { println!("dead-signals advisory: no corpora registry (skipped)"); return Ok(()); }
    };
    let id = reg.default_corpus_id.clone();
    let Some(corpus) = reg.corpora.iter().find(|c| c.id == id) else {
        println!("dead-signals advisory: default corpus '{id}' absent (skipped)"); return Ok(());
    };
    let graph_path = repo_root.join(&corpus.graph_path);
    let Ok(raw) = std::fs::read_to_string(&graph_path) else {
        println!("dead-signals advisory: graph not built for '{id}' (skipped)"); return Ok(());
    };
    let graph: serde_json::Value = serde_json::from_str(&raw)?;
    let report = vox_graphify_reader::dataflow::compute_dead_signals(&graph);
    let acc = report.findings.iter().filter(|f|
        matches!(f.detector, vox_graphify_reader::dataflow::DeadSignalKind::AccumulatorNeverGates)).count();
    println!("dead-signals advisory (corpus={id}): {} finding(s) [{acc} accumulator_never_gates] — non-blocking",
        report.findings.len());
    for f in &report.findings {
        println!("  [{}] {} ({}): {}", serde_json::to_string(&f.detector).unwrap_or_default().trim_matches('"'),
            f.node_id, f.confidence, f.rationale);
    }
    Ok(()) // never fails the gate
}
```
  Wire the call site into the existing CI subcommand that runs advisories (the same place the coverage advisory is invoked — confirm with the grep above; mirror its exact dispatch). If advisories are individual `Ci*` subcommands, add a `DeadSignals` variant that calls `dead_signals_advisory(repo_root)`.

- [ ] **Step 2: Build, verify** — `cargo build -p vox-cli`. Expected: compiles.

- [ ] **Step 3: Smoke** — `cargo run -p vox-cli -- ci dead-signals 2>&1 | head` (adjust to the actual subcommand path discovered in Step 1). Expected: prints the advisory line, exit 0 (`echo $?` → 0).

- [ ] **Step 4: Commit** —
  `git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/ci/mod.rs`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): non-blocking dead-signals CI advisory"`

---

# PHASE I — Full verification + close

## Task I1: Whole-layer verification + summary commit [SEQUENTIAL]

- [ ] **Step 1: Reader suite** — `cargo test -p vox-graphify-reader 2>&1 | tail -30`. Expected: all pass, including `dataflow_rust` (5 tests incl. the frontend-emit repro), `dataflow_ts` (3), `dead_signals` (4), `rebuild_tests` (incl. the def-use serialization test), and all pre-existing goldens.
- [ ] **Step 2: MCP suite** — `cargo test -p vox-orchestrator-mcp 2>&1 | tail -20`. Expected: all pass, incl. `dataflow_returns_incident_defuse_edges`, `dead_signals_reports_accumulator_finding`, `dataflow_tools_have_schemas`.
- [ ] **Step 3: CLI build + targeted tests** — `cargo build -p vox-cli && cargo test -p vox-cli --test graphify_dead_signals 2>&1 | tail -10`. Expected: builds + the e2e CLI test passes.
- [ ] **Step 4: Clippy (reader only, fast)** — `cargo clippy -p vox-graphify-reader --all-targets -- -D warnings 2>&1 | tail`. Expected: clean. (Do NOT clippy the whole workspace — `vox-gui`'s Tauri build script breaks `--all-targets`; see MEMORY feedback.)
- [ ] **Step 5: fmt the touched crates** — `cargo fmt -p vox-graphify-reader && cargo fmt -p vox-orchestrator-mcp && cargo fmt -p vox-cli`. (Per AGENTS.md: never `cargo fmt --all`; per-crate only.)
- [ ] **Step 6: Commit any fmt deltas** —
  `git -C /c/Users/Owner/vox-graphify-gui add -A`
  `git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(vox-search/dataflow): fmt + verification close for P1 data-flow layer"`
  (If `git diff --cached --quiet` would be empty, skip the commit — nothing to record.)

---

## Self-Review — spec coverage

Mapping every P1 requirement in the source design (`2026-06-26-graphify-dataflow-semantic-overlay-design.md` §1) and the master umbrella (§2.3, §3.1, §6, §9) to the task that satisfies it:

| Spec requirement | Source | Task(s) |
|---|---|---|
| New module `dataflow.rs`, runs per-function, drops on ambiguity | design §1.1 | A2, B1–B4, C1–C2 |
| Reuse `syn` visitor (Rust) | design §1.1 | B1–B4 |
| Reuse tree-sitter walk (TS) | design §1.1 | C1–C2 |
| `field:<Struct>::<field>` node kind (lazy) | design §1.2 | B1 (`field_node`), C1 |
| `binding:<fn-id>::<name>@<n>` node kind (SSA-ish `@n`) | design §1.2 | B2 (`binding_counter`) |
| `def-write` edge (`x=…`, `self.x=…`, `x.push(…)`, `x.0=…`) | design §1.2 | B1 (accumulation), B2 (assign via binding); tuple `x.0` honestly dropped (§1.2 "drop") |
| `use-read` edge sub-typed `read_kind: control` | design §1.2 | B3, C2 |
| `read_kind: consume` | design §1.2 | B3 (call-arg/return), recorded |
| `read_kind: store` (copy into another aggregate — the crux) | design §1.2 | B2, C2 |
| Confidence-labeled; drop unmodellable (interior mut/trait obj) | design §1.2 | B1–B4 (`owner_of` → None ⇒ drop) |
| `ignored_result` detector | design §1.3 #1 | B4 (candidates) + D2 |
| `write_only_field` detector (`resolved`) | design §1.3 #2 | D1 |
| `accumulator_never_gates` detector (`heuristic`) | design §1.3 #3 | D2 |
| `DeadSignal` struct (detector/node_id/label/write_sites/read_kinds/confidence/rationale) | design §1.3 | A2 |
| `DeadSignalReport` + `compute_dead_signals(&Value)` | design §1.4 | D1 |
| Honesty firewall mirrors `compute_coverage` (reports edges, no intent judgment, labels confidence) | design §1.4 | D1 (doc-comment + impl) |
| **Frontend-emit detector fires (the §1.5 walkthrough)** | design §1.5 | **D3 (mandatory e2e), G2 (rebuilt-corpus e2e)** |
| Honest limits: intra-procedural miss never a false positive; callee-return miss dropped | design §1.5 limits | B3 (control-only-on-confident), B1 (`owner_of` drop), D2 (accumulation requires local accumulate marker) |
| `vox_search_dataflow` tool, `layer: "structural"` | master §3.1 / design §3 | F1 |
| `vox_search_dead_signals` tool (`detector?`/`min_confidence?`), `layer: "structural"` | master §3.1 / design §3 | F2 |
| Input schemas next to existing graphify entries | design §3 | F1, F2 (input_schemas.rs) |
| CLI mirrors `dataflow` / `dead-signals` | design §3 | G1 |
| Non-blocking CI advisory (like coverage), promotable to blocking | design §1.4, §3 | H1 |
| Adds to structural graph family, NOT an overlay; never mutates determinism | master §2.3, §2.5 | E1 (in `graph.json`, deterministic), no overlay file |
| Tier-presence assertion (tools can't be silently dropped) | master §3.2 | F3 |
| External names `vox_search_*` (master wins over `vox_graphify_*`) | master §1.1 naming authority | F1, F2 |

**Deliberate scope boundaries (stated, not gaps):** (1) inter-procedural data-flow is explicitly out (design §0/non-goals; master §8) — the callee-return write is dropped, never falsely flagged. (2) Tuple-field writes (`x.0 = …`) and macro/trait-object interior mutability are dropped (honesty), not modeled. (3) TS owner resolution is weaker than Rust (no struct types) — `this.<f>` → enclosing-class field; bare-local → `@1` binding; unresolved dropped. (4) The semantic overlay (design §2) is **P3**, not this plan. (5) The CLI external rename to `vox search dataflow`/`dead-signals` is **P0**'s; here the subcommands extend the enum P0 re-homes (reachable via `vox graphify …` until then).

**Edge cases covered by tests:** call edges keep byte-identical JSON (A1) → no golden churn; accumulator WITH a control read does NOT fire (D2 negative test); field with any read is not write-only (D1 negative test); struct-literal move is `store` not `control` (B2); `if (…length)` is `control` in TS (C2). **Risk flagged for the executor:** D3's nested field access `reactive_stats.reactive_view_emit_failures.push(…)` requires B1's `field_receiver` to resolve the local `reactive_stats`'s type (set by the `let … : ReactiveViewBridgeStats` in B2) — D3's Step 2 explicitly instructs adjusting B1/B2 (re-running their tests) if the nested-receiver resolution misses.

## Workflow dispatch summary

- **Total tasks:** 18 (A1–A3, B1–B4, C1–C2, D1–D3, E1, F1–F3, G1–G2, H1, I1).
- **Sequential backbone:** A1→A2→A3 → (β) → D1→D2→D3 → E1 → (δ) → H1 → I1.
- **Parallel fan-out batch β** (after A3): `{B1→B2→B3→B4}` ∥ `{C1→C2}` — two independent file regions, dispatch concurrently.
- **Parallel fan-out batch δ** (after E1): `{F1→F2→F3}` (vox-orchestrator-mcp) ∥ `{G1→G2}` (vox-cli) — two independent crates, dispatch concurrently.
- Every task ends in an add+commit; the workflow performs the final integration commit. No push/branch/merge inside tasks.
