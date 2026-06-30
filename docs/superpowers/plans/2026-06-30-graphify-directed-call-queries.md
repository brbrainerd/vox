# Graphify Directed Call Queries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose "what calls what" (callers-of / callees-of) over the native code graph by restoring the caller→callee direction the reader currently discards.

**Architecture:** The live graph's links are already stored caller→callee (`{source, target, confidence}`). The reader symmetrizes them (`lib.rs:131-134`), destroying direction. We add two directed adjacency indexes (`forward`=callees, `reverse`=callers) alongside the existing symmetric one, add a `Direction` selector to the reader's traversal methods, thread an optional `direction` param through the two existing MCP tools (default `both` = unchanged), and register two thin pinned wrappers (`vox_search_callers`, `vox_search_callees`) for agent discoverability. No graph rebuild, no edge-schema change, no writer change.

**Tech Stack:** Rust, `serde_json`, RMCP tool registry (build.rs-generated from `contracts/mcp/tool-registry.canonical.yaml`).

Spec: `docs/superpowers/specs/2026-06-30-graphify-directed-call-queries-design.md`

---

## File Structure

- `crates/vox-graph-reader/src/bfs.rs` — add `Direction` enum (traversal free functions unchanged: they already take an `adjacency: &HashMap` ref).
- `crates/vox-graph-reader/src/lib.rs` — add `forward`/`reverse` indexes to `GraphifyReader`; add `direction` arg to `bfs_from_seeds`/`shortest_path` (select which map to pass).
- `crates/vox-orchestrator-mcp/src/graph_tools.rs` — add `direction` field to `GraphifyQueryParams`/`GraphifyPathParams`; refactor handlers to a `_core(.., Direction)` form; add `graphify_callers`/`graphify_callees` wrappers.
- `crates/vox-orchestrator-mcp/src/input_schemas.rs` — add `direction` to neighbors/path schemas; add schemas for the two wrappers.
- `crates/vox-orchestrator-mcp/src/dispatch.rs` — add two dispatch arms.
- `contracts/mcp/tool-registry.canonical.yaml` — add two tool entries (SSOT; build.rs regenerates `TOOL_REGISTRY`).

---

## Task 1: Directed indexes + `Direction` in the reader

**Files:**
- Modify: `crates/vox-graph-reader/src/bfs.rs` (add enum)
- Modify: `crates/vox-graph-reader/src/lib.rs:66-72` (struct), `:117-144` (`from_value`), `:166-175` (methods)
- Test: `crates/vox-graph-reader/src/lib.rs` (new `#[cfg(test)]` test in a module at end of file)

- [ ] **Step 1: Add the `Direction` enum to `bfs.rs`**

At the top of `crates/vox-graph-reader/src/bfs.rs`, after the `use` lines (after line 5):

```rust
/// Which adjacency to traverse. `Both` is the legacy undirected behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Follow caller→callee edges (callees of the seed).
    Out,
    /// Follow callee→caller edges (callers of the seed).
    In,
    /// Undirected: both directions (default; legacy behavior).
    Both,
}
```

- [ ] **Step 2: Re-export `Direction` and add directed fields to the struct**

In `crates/vox-graph-reader/src/lib.rs`, add to the `pub use` / module surface near the top (after the `pub mod bfs;` block, around line 30 use-section) so callers can name it:

```rust
pub use bfs::Direction;
```

Then extend the struct at `lib.rs:66-72`:

```rust
#[derive(Debug)]
pub struct GraphifyReader {
    // node_id → (label, community_id)
    nodes: HashMap<String, (String, Option<String>)>,
    // Undirected adjacency: node_id → Vec<neighbor_ids>
    adjacency: HashMap<String, Vec<String>>,
    // Directed: caller → callees (forward = source→target order on disk).
    forward: HashMap<String, Vec<String>>,
    // Directed: callee → callers (reverse).
    reverse: HashMap<String, Vec<String>>,
}
```

- [ ] **Step 3: Populate `forward`/`reverse` in `from_value`**

In `from_value`, replace the edge-loop block at `lib.rs:117-143`. Keep the symmetric `adjacency` exactly as-is; add the two directed maps in the same pass:

```rust
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());
        let mut forward: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());
        let mut reverse: HashMap<String, Vec<String>> = HashMap::with_capacity(nodes.len());

        if let Some(edges) = edges_arr {
            for edge in edges {
                let src = edge
                    .get("source")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                let dst = edge
                    .get("target")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                if let (Some(s), Some(d)) = (src, dst) {
                    // Symmetric (legacy): both directions.
                    adjacency.entry(s.clone()).or_default().push(d.clone());
                    adjacency.entry(d.clone()).or_default().push(s.clone());
                    // Directed: storage order is caller→callee.
                    forward.entry(s.clone()).or_default().push(d.clone());
                    reverse.entry(d).or_default().push(s);
                }
            }
        }

        for neighbors in adjacency.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }
        for neighbors in forward.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }
        for neighbors in reverse.values_mut() {
            neighbors.sort();
            neighbors.dedup();
        }

        Ok(GraphifyReader {
            nodes,
            adjacency,
            forward,
            reverse,
        })
```

- [ ] **Step 4: Add a `direction` arg to the public traversal methods**

Replace the two methods at `lib.rs:166-175`:

```rust
    /// BFS from one or more seed node IDs up to `max_depth` hops.
    ///
    /// `direction` selects callees ([`Direction::Out`]), callers ([`Direction::In`]),
    /// or the legacy undirected neighborhood ([`Direction::Both`]).
    pub fn bfs_from_seeds(
        &self,
        seeds: &[&str],
        max_depth: u8,
        limit: usize,
        direction: Direction,
    ) -> Vec<TraversalHit> {
        let adj = match direction {
            Direction::Out => &self.forward,
            Direction::In => &self.reverse,
            Direction::Both => &self.adjacency,
        };
        bfs::bfs_from_seeds(&self.nodes, adj, seeds, max_depth, limit)
    }

    /// Shortest path between two node IDs (BFS). Returns `None` if unreachable.
    ///
    /// `direction` selects which adjacency to walk; see [`Self::bfs_from_seeds`].
    pub fn shortest_path(&self, from: &str, to: &str, direction: Direction) -> Option<Vec<String>> {
        let adj = match direction {
            Direction::Out => &self.forward,
            Direction::In => &self.reverse,
            Direction::Both => &self.adjacency,
        };
        bfs::shortest_path(adj, from, to)
    }
```

- [ ] **Step 5: Update the existing `bfs.rs` test call site**

In `crates/vox-graph-reader/src/bfs.rs`, the test at line 135 calls `reader.bfs_from_seeds(&["A", "B"], 5, 100)`. Add the import and the new arg:

```rust
    use crate::{Direction, GraphifyReader};
```
and
```rust
        let hits = reader.bfs_from_seeds(&["A", "B"], 5, 100, Direction::Both);
```

- [ ] **Step 6: Write the failing directed-traversal test**

Append to the end of `crates/vox-graph-reader/src/lib.rs`:

```rust
#[cfg(test)]
mod directed_tests {
    use crate::{Direction, GraphifyReader};

    fn fixture() -> GraphifyReader {
        // A→B→C, plus a stray D→B. Storage order is caller→callee.
        let value = serde_json::json!({
            "nodes": [
                {"id": "A", "label": "A"},
                {"id": "B", "label": "B"},
                {"id": "C", "label": "C"},
                {"id": "D", "label": "D"}
            ],
            "links": [
                {"source": "A", "target": "B"},
                {"source": "B", "target": "C"},
                {"source": "D", "target": "B"}
            ]
        });
        GraphifyReader::from_value(value).expect("reader builds")
    }

    #[test]
    fn callees_of_b_is_c_only() {
        let r = fixture();
        let hits = r.bfs_from_seeds(&["B"], 1, 100, Direction::Out);
        let ids: Vec<_> = hits.iter().map(|h| h.node_id.as_str()).collect();
        assert_eq!(ids, vec!["C"], "callees(B) must be exactly {{C}}");
    }

    #[test]
    fn callers_of_b_are_a_and_d() {
        let r = fixture();
        let mut ids: Vec<_> = r
            .bfs_from_seeds(&["B"], 1, 100, Direction::In)
            .iter()
            .map(|h| h.node_id.clone())
            .collect();
        ids.sort();
        assert_eq!(ids, vec!["A".to_string(), "D".to_string()]);
    }

    #[test]
    fn directed_path_respects_direction() {
        let r = fixture();
        assert!(
            r.shortest_path("A", "C", Direction::Out).is_some(),
            "A→C reachable following callees"
        );
        assert!(
            r.shortest_path("C", "A", Direction::In).is_some(),
            "C→A reachable following callers"
        );
        assert!(
            r.shortest_path("A", "C", Direction::In).is_none(),
            "A→C NOT reachable following callers (regression guard)"
        );
    }

    #[test]
    fn both_preserves_legacy_undirected_neighborhood() {
        let r = fixture();
        let mut ids: Vec<_> = r
            .bfs_from_seeds(&["B"], 1, 100, Direction::Both)
            .iter()
            .map(|h| h.node_id.clone())
            .collect();
        ids.sort();
        assert_eq!(
            ids,
            vec!["A".to_string(), "C".to_string(), "D".to_string()],
            "Both must still return the full undirected 1-hop neighborhood"
        );
    }
}
```

- [ ] **Step 7: Run tests — expect compile error first, then pass after Steps 1-5 land**

Run: `cargo test -p vox-graph-reader directed_tests`
Expected: PASS (4 tests). Also run `cargo test -p vox-graph-reader` to confirm the legacy `multi_seed_reports_nearest_seed_depth` test still passes.

- [ ] **Step 8: Format and commit**

```bash
cargo fmt -p vox-graph-reader
git add crates/vox-graph-reader/src/bfs.rs crates/vox-graph-reader/src/lib.rs
git commit -m "feat(graph-reader): directed callers/callees traversal via Direction"
```

---

## Task 2: Thread `direction` through the two existing MCP tools

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/graph_tools.rs:345-363` (params), `:382-446` (`graphify_query`), `:449-501` (`graphify_path`)
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs:478-483`
- Test: `crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs`

- [ ] **Step 1: Add a `direction` field + parse helper in `graph_tools.rs`**

Add near the top of the Graphify section in `graph_tools.rs` (after the `use` block):

```rust
use vox_graph_reader::Direction;

/// Parse the optional `direction` param. Unknown/missing → `Both` (legacy).
fn parse_direction(raw: &Option<String>) -> Direction {
    match raw.as_deref() {
        Some("in") => Direction::In,
        Some("out") => Direction::Out,
        _ => Direction::Both,
    }
}

fn direction_label(d: Direction) -> &'static str {
    match d {
        Direction::In => "in",
        Direction::Out => "out",
        Direction::Both => "both",
    }
}
```

Extend `GraphifyQueryParams` (`:346-356`) and `GraphifyPathParams` (`:356-363`) — add to each:

```rust
    /// "in" = callers, "out" = callees, "both" = undirected (default).
    #[serde(default)]
    pub direction: Option<String>,
```

- [ ] **Step 2: Use direction in `graphify_query`**

In `graphify_query`, replace the traversal call at `graph_tools.rs:424`:

```rust
    let direction = parse_direction(&params.direction);
    let hits = reader.bfs_from_seeds(&seeds, max_depth, limit, direction);
```

and add `"direction": direction_label(direction),` to the returned JSON object (after the `"seeds"` line at `:441`).

- [ ] **Step 3: Use direction in `graphify_path`**

In `graphify_path`, replace `graph_tools.rs:488`:

```rust
    let direction = parse_direction(&params.direction);
    let path = reader.shortest_path(&params.from, &params.to, direction);
```

and add `"direction": direction_label(direction),` to the returned JSON (after the `"to"` line at `:495`).

- [ ] **Step 4: Add `direction` to the two schemas**

In `input_schemas.rs`, replace the `vox_search_neighbors` arm (`:478-480`) — add the `direction` property:

```rust
        "vox_search_neighbors" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"seeds":{"type":"array","items":{"type":"string"},"minItems":1,"description":"Seed node IDs to BFS-expand from"},"max_depth":{"type":"integer","minimum":1,"maximum":5,"description":"BFS hop limit (default 2)"},"limit":{"type":"integer","minimum":1,"description":"Max hits returned (default 20)"},"direction":{"type":"string","enum":["in","out","both"],"description":"in=callers, out=callees, both=undirected (default)"}},"required":["seeds"],"additionalProperties":false}"#,
        ),
```

And the `vox_search_path` arm (`:481-483`):

```rust
        "vox_search_path" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"from":{"type":"string","description":"Source node ID"},"to":{"type":"string","description":"Destination node ID"},"direction":{"type":"string","enum":["in","out","both"],"description":"in=callers, out=callees, both=undirected (default)"}},"required":["from","to"],"additionalProperties":false}"#,
        ),
```

- [ ] **Step 5: Write the failing direction test**

In `crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs`, add a test that calls `vox_search_neighbors` with `direction:"out"` against a fixture corpus and asserts the response echoes `"direction":"out"`. Mirror the existing dispatch-test setup in that file (reuse its corpus-fixture/harness helpers — read the top of the file first). The assertion:

```rust
    let resp = dispatch_tool("vox_search_neighbors", serde_json::json!({
        "seeds": ["A"], "direction": "out", "max_depth": 1
    })).await;
    assert_eq!(resp["data"]["direction"], "out");
```

(Adjust `dispatch_tool` / response-unwrap to match the file's existing helpers.)

- [ ] **Step 6: Run the test**

Run: `cargo test -p vox-orchestrator-mcp vox_search_dispatch`
Expected: PASS, including the new direction assertion and all pre-existing dispatch tests.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/graph_tools.rs crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs
git commit -m "feat(mcp): direction param on vox_search_neighbors/path"
```

---

## Task 3: Register `vox_search_callers` / `vox_search_callees` wrappers

**Files:**
- Modify: `contracts/mcp/tool-registry.canonical.yaml:1370` (after the `vox_search_path` entry)
- Modify: `crates/vox-orchestrator-mcp/src/graph_tools.rs` (refactor + two wrappers)
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs` (two schema arms)
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs:654` (two dispatch arms)
- Test: `crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs`

- [ ] **Step 1: Refactor `graphify_query` into a direction-pinnable core + wrappers**

In `graph_tools.rs`, rename the body of `graphify_query` to `graphify_query_core(state, params, forced: Option<Direction>)`. When `forced` is `Some(d)`, use `d`; else `parse_direction(&params.direction)`. Then:

```rust
pub async fn graphify_query(state: &ServerState, params: GraphifyQueryParams) -> String {
    graphify_query_core(state, params, None).await
}

/// `vox_search_callers`: functions that CALL the seed(s) (direction pinned to In).
pub async fn graphify_callers(state: &ServerState, params: GraphifyQueryParams) -> String {
    graphify_query_core(state, params, Some(Direction::In)).await
}

/// `vox_search_callees`: functions the seed(s) CALL (direction pinned to Out).
pub async fn graphify_callees(state: &ServerState, params: GraphifyQueryParams) -> String {
    graphify_query_core(state, params, Some(Direction::Out)).await
}
```

In `graphify_query_core`, the direction line becomes:

```rust
    let direction = forced.unwrap_or_else(|| parse_direction(&params.direction));
```

- [ ] **Step 2: Add the two tool entries to the canonical YAML**

In `contracts/mcp/tool-registry.canonical.yaml`, immediately after the `vox_search_path` entry (after line 1375, before `vox_search_rebuild`):

```yaml
- name: vox_search_callers
  description: Functions that call the given symbol(s) — reverse call edges (read-only).
  product_lane: platform
  http_read_role_eligible: true
  tier: core
- name: vox_search_callees
  description: Functions called by the given symbol(s) — forward call edges (read-only).
  product_lane: platform
  http_read_role_eligible: true
  tier: core
```

- [ ] **Step 3: Add schema arms for the two wrappers**

In `input_schemas.rs`, after the `vox_search_path` arm, add (same shape as neighbors minus `direction`, since it is pinned):

```rust
        "vox_search_callers" | "vox_search_callees" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id; omit for default"},"seeds":{"type":"array","items":{"type":"string"},"minItems":1,"description":"Symbol node IDs to find callers/callees of"},"max_depth":{"type":"integer","minimum":1,"maximum":5,"description":"Transitive hop limit (default 2)"},"limit":{"type":"integer","minimum":1,"description":"Max hits returned (default 20)"}},"required":["seeds"],"additionalProperties":false}"#,
        ),
```

- [ ] **Step 4: Add dispatch arms**

In `dispatch.rs`, after the `vox_search_path` arm (line ~657-658), add:

```rust
        "vox_search_callers" => {
            Ok(crate::graph_tools::graphify_callers(state, serde_json::from_value(args)?).await)
        }
        "vox_search_callees" => {
            Ok(crate::graph_tools::graphify_callees(state, serde_json::from_value(args)?).await)
        }
```

- [ ] **Step 5: Write the failing wrapper test**

In `tests/vox_search_dispatch.rs`, add a test asserting `vox_search_callers` and `vox_search_callees` dispatch and echo the pinned direction:

```rust
    let callers = dispatch_tool("vox_search_callers", serde_json::json!({"seeds": ["B"]})).await;
    assert_eq!(callers["data"]["direction"], "in");
    let callees = dispatch_tool("vox_search_callees", serde_json::json!({"seeds": ["B"]})).await;
    assert_eq!(callees["data"]["direction"], "out");
```

- [ ] **Step 6: Run the registry-parity tests + the new test**

Run: `cargo test -p vox-orchestrator-mcp`
Expected: PASS. Specifically the parity tests in `dispatch.rs` (~`:1765-1790`) that assert every `TOOL_REGISTRY` entry has a dispatch arm and unique name must stay green — they now cover the two new tools. The build.rs regenerates `TOOL_REGISTRY` from the YAML automatically on build.

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-orchestrator-mcp
git add contracts/mcp/tool-registry.canonical.yaml crates/vox-orchestrator-mcp/src/graph_tools.rs crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs
git commit -m "feat(mcp): register vox_search_callers/callees wrappers"
```

---

## Task 4: Verify end-to-end against the live corpus

**Files:** none (verification only)

- [ ] **Step 1: Build the binary**

Run: `cargo build -p vox-cli`
Expected: clean build (build.rs picks up the two new YAML tools).

- [ ] **Step 2: Sanity-check clippy on the touched crates**

Run: `cargo clippy -p vox-graph-reader -p vox-orchestrator-mcp -- -D warnings`
Expected: no warnings. (Per repo policy, run real clippy before any admin-merge — `feedback_admin_merge_clippy_gap`.)

- [ ] **Step 3: Confirm the tools answer a real call query**

Pick a known function in the live graph (e.g. a node id from `.vox/cache/graphify/repo-code-graph/graph.json`) and call `vox_search_callers` / `vox_search_callees` via the MCP surface (or a small integration test that loads the real corpus). Confirm callers ≠ callees and both are non-empty for a mid-degree symbol.

Note: existing on-disk corpora already carry direction (storage order), so **no `vox graphify rebuild` is required** for this to work.

---

## Self-Review

- **Spec coverage:** §1 reader indexes → Task 1; §2 Direction param → Task 1+2; §3 MCP surface (param + two wrappers) → Task 2+3; §4 YAGNI (no relation/no directed-flag/no dataflow) → respected, nothing added; §correctness test (`A→B→C` + `D→B`, both-regression guard) → Task 1 Step 6; scope boundary (only the 6 listed files) → matches File Structure.
- **Placeholder scan:** none — every code step shows full code. Task 2 Step 5 / Task 3 Step 5 reference the existing test harness helpers in `vox_search_dispatch.rs` (read its top before writing) rather than inventing a fixture, which is correct DRY, not a placeholder.
- **Type consistency:** `Direction` enum (Out/In/Both) named identically across bfs.rs, lib.rs, graph_tools.rs. `parse_direction`/`direction_label` defined once (Task 2) and reused (Task 3). `graphify_query_core(.., Option<Direction>)` signature consistent between definition (Task 3 Step 1) and wrappers. Method signatures `bfs_from_seeds(.., Direction)` / `shortest_path(.., Direction)` match every call site updated in Tasks 1-3.
