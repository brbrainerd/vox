---
title: "Graph Loader Cache, Omnibar Graph Facet Repair, and TTL Write Path"
description: "Caches the parsed graphify graph in MCP server state, repairs the two silently-dead graph paths in the Omnibar, and adds a write path for the graphify corpus TTL."
category: "architecture"
---

# Graph Loader Cache, Omnibar Graph Facet Repair, and TTL Write Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the already-shipped graphify MCP tools usable from the GUI — cache the 126 MB parse, repair the Omnibar's two dead graph paths, and give the corpus TTL a write path.

**Architecture:** All three changes extend existing seams. The cache mirrors the `CachedCatalog` pattern already in `ServerState`. The Omnibar fix is a constant plus a response-shape correction in an existing parser. The TTL tool is a new MCP tool added through the existing `contracts/operations/catalog.v1.yaml` → generated-registry chain. No new crates, no new crate edges, no Tauri commands.

**Tech Stack:** Rust (`vox-orchestrator-mcp`, `vox-config`), TypeScript/React + vitest (`vox-gui/ui`), YAML SSOT contracts.

**Spec:** None. This plan supersedes the withdrawn `2026-08-24-graphify-map-surface-design.md`, which was withdrawn after seven audit tracks found blocking defects. The measurements that justify these three tasks are recorded in the Background section below, which is the authority this plan argues from.

## Background (the measured facts this plan rests on)

Taken 2026-08-24 against the real `graphify-out/graph.json` (126,034,424 bytes,
100,206 nodes, 178,782 edges, 5,638 communities):

| Fact | Value | Source |
|---|---|---|
| `load_graph_json` + `GraphifyReader::from_value` cost | **10.4–11.8 s** in-process | measured, release binary, two sequential runs |
| Peak commit per parse | **~500 MB** private bytes | measured, 50 ms sampling |
| Cache in `graph_tools.rs` | **none** — every call re-reads and re-parses | `graph_tools.rs:411` |
| `vox_graphify_query` registered? | **No.** Dispatch registers `vox_search_*` only | `dispatch.rs:861-882` |
| `graphify_search` payload key | `hits` | `graph_tools.rs:335` |
| `graphify_query` payload key | `hits` | `graph_tools.rs:494` |
| `parseDiscoverResults` reads | `result.results` | `Omnibar.tsx:67-68` |

Consequences, both currently live in `main`:

1. `Omnibar.tsx:35` points `GRAPH_DISCOVER_TOOL` at `vox_graphify_query`, which
   does not exist. Every keystroke in the palette renders the error string
   `graph facet pending VG-1 — graph-discover tool unavailable`.
2. `Omnibar.tsx:310` calls the **real** `vox_search_neighbors` but parses its
   response with `parseDiscoverResults`, which looks for `result.results` while
   the tool returns `result.hits`. This path fails **silently** — it returns an
   empty array with no error, so neighbor expansion has been inert with no
   visible symptom.

Fixing (1) without (2) would move the discover path from a loud failure to a
silent one. Fixing either without the cache would point a per-keystroke,
debounced call at a 10-second, 500 MB parse. Hence the task order below is
load-bearing, not cosmetic.

## Global Constraints

- **No new workspace crate edges.** `vox-orchestrator-mcp → vox-graph-reader`
  already exists in `contracts/ci/crate-edges.allow.v1.json`. `vox-gui →
  vox-graph-reader` does **not** and must not be added. Adding an `exceptions`
  entry or regenerating a ratchet baseline is USER-AUTHORIZED-ONLY — if a task
  seems to need one, stop and report instead.
- **No new Tauri commands.** GUI reaches graph capability only through
  `voxTransport.invokeMcpTool`.
- **Adding an MCP tool starts at `contracts/operations/catalog.v1.yaml`,** which
  is hand-authored. Every other registry (`tool-registry.canonical.yaml`,
  `command-registry.yaml`, `capability-registry.yaml`,
  `model-manifest.generated.json`, `gui-surface-coverage.v1.json`) is generated
  from it. With no catalog row, `vox ci ssot-drift` reports **nothing** and the
  tool silently never enters `TOOL_REGISTRY`. Follow the precedent playbook at
  `docs/superpowers/plans/2026-06-30-graphify-directed-call-queries.md`.
- **Never run `cargo fmt --all`** on this workspace (Windows `os error 206`).
  Use `cargo fmt -p <crate>` for a single crate.
- **Never run a recursive grep from the repo root** — it times out. Scope every
  search to a crate directory.
- **GUI tests:** `// @vitest-environment jsdom` must be **line 1** of any test
  file that calls `render()` — there is no global environment in
  `vitest.config.ts`. Use `pnpm`, never `npx`. `vitest` does not type-check;
  `pnpm typecheck` is a separate CI step (`vox ci gui-honesty`).
- **Commit by explicit pathspec** — `git commit -m "msg" -- <paths>`, never bare
  `git commit` — the repo may carry unrelated staged work.
- **Fixtures must come from the real artifact.** Before writing any fixture that
  mimics a real file's shape, read that file and copy its observed keys. Do not
  infer a schema from consuming code — in this codebase the consuming code has
  been wrong twice (`GraphifyReader` reads `community` as a string; the real
  graph writes an integer).

---

### Task 1: Cache the parsed graph in `ServerState`

**Files:**
- Modify: `crates/vox-graph-reader/src/lib.rs:89` (add `from_ref`; `from_value` delegates to it)
- Modify: `crates/vox-orchestrator-mcp/src/server_state.rs` (add `CachedGraph` struct + `graph_cache` field; construction sites)
- Modify: `crates/vox-orchestrator-mcp/src/graph_tools.rs:411` (add `get_graph`, rewire 4 call sites)
- Test: `crates/vox-graph-reader/src/lib.rs` (existing `#[cfg(test)] mod` at line 253)
- Test: `crates/vox-orchestrator-mcp/src/graph_tools.rs` (existing `#[cfg(test)] mod` at line 760)

**Interfaces:**
- Consumes: `ServerState` (`server_state.rs:32`), `vox_config::graphify::GraphifyCorpus`
- Produces: `GraphifyReader::from_ref(&serde_json::Value) -> Result<Self, GraphifyReaderError>`
- Produces: `async fn get_graph(state: &ServerState, corpus_id: &str, corpus: &GraphifyCorpus, repo_root: &Path) -> Result<CachedGraph, String>` where `CachedGraph { key: GraphCacheKey, value: Arc<serde_json::Value>, reader: Arc<vox_graph_reader::GraphifyReader> }`

**Why both `value` and `reader` are cached:** `graphify_search` (`vox_search_structural`,
the per-keystroke Omnibar call) passes the raw `serde_json::Value` to
`vox_config::graphify::lexical_search_graph`, while `graphify_query`/`graphify_path`
need a `GraphifyReader`. Caching both, built together on a miss, costs ~650 MB
steady state and removes the ~500 MB *per-call spike* — strictly better than
today, where two concurrent calls transiently commit ~1 GB against under 1 GB
free.

**Why `from_ref` is added first:** `GraphifyReader::from_value` takes the `Value`
by value, so retaining both representations would otherwise require cloning
126 MB — pushing peak commit on a cache miss to ~1.15 GB, *worse* than the
500 MB spike this task removes. Its body only ever borrows (`value.get(...)`,
`.as_str()`, `.to_string()`) and never moves out of the `Value`, so a
borrowing variant is behaviour-identical.

- [ ] **Step 1: Read the real cache precedent before writing anything**

Read `crates/vox-orchestrator-mcp/src/repo_catalog_tools.rs:92-115` (`get_catalog`).
It is the read-check-mtime / miss-rebuild-write pattern this task copies. Read
`crates/vox-orchestrator-mcp/src/server_state.rs:17-20` (`CachedCatalog`) and
`:55` (`catalog_cache`). Your new code mirrors both.

- [ ] **Step 2: Write the failing test**

Append to the existing `#[cfg(test)] mod tests` in `graph_tools.rs`. The test
asserts the cache-key logic, which is the part that can silently rot; it does
not require a 126 MB fixture.

```rust
#[test]
fn graph_cache_key_rejects_mtime_change() {
    use std::time::{Duration, SystemTime};
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let key = GraphCacheKey { corpus_id: "repo-code-graph".to_string(), mtime: base, len: 42 };
    assert!(key.matches("repo-code-graph", base, 42));
    // A rebuild rewrites graph.json: mtime moves, so the cache must miss.
    assert!(!key.matches("repo-code-graph", base + Duration::from_secs(1), 42));
    // Same mtime but a different size (possible on coarse filesystems) must miss.
    assert!(!key.matches("repo-code-graph", base, 43));
    // A different corpus must never hit: graphify_compare loads two corpora
    // and a single-slot cache would otherwise thrash between them.
    assert!(!key.matches("docs-graph", base, 42));
}
```

- [ ] **Step 3: Run it to verify it fails**

```bash
cargo test -p vox-orchestrator-mcp graph_cache_key_rejects_mtime_change
```

Expected: FAIL — `cannot find struct GraphCacheKey`.

- [ ] **Step 3b: Add `from_ref` to `vox-graph-reader`**

In `crates/vox-graph-reader/src/lib.rs`, rename the existing `from_value` body to
`from_ref` taking a reference, and make `from_value` delegate. Change **only**
the signature and the delegation — do not alter the body's logic:

```rust
    /// Construct from a parsed `serde_json::Value`.
    ///
    /// Returns [`GraphifyReaderError::MissingNodes`] if the `"nodes"` key is absent or not an array.
    pub fn from_value(value: serde_json::Value) -> Result<Self, GraphifyReaderError> {
        Self::from_ref(&value)
    }

    /// Construct by borrowing a parsed `serde_json::Value`.
    ///
    /// Identical to [`Self::from_value`] but leaves the input intact, so a caller
    /// that must retain the raw `Value` (for example to serve a lexical search)
    /// does not have to clone it. The graph is ~126 MB in this repo, so that
    /// clone is the difference between ~650 MB and ~1.15 GB of peak commit.
    pub fn from_ref(value: &serde_json::Value) -> Result<Self, GraphifyReaderError> {
        // ... existing from_value body verbatim, unchanged ...
    }
```

Add this test to the existing `#[cfg(test)] mod` at line 253:

```rust
    #[test]
    fn from_ref_matches_from_value_and_leaves_input_intact() {
        let value = serde_json::json!({
            "nodes": [
                {"id": "a", "label": "Alpha"},
                {"id": "b", "label": "Beta"}
            ],
            "links": [{"source": "a", "target": "b"}]
        });
        let by_ref = GraphifyReader::from_ref(&value).expect("from_ref builds");
        // The input must still be usable after from_ref — that is the whole point.
        assert!(value.get("nodes").is_some());
        let by_val = GraphifyReader::from_value(value).expect("from_value builds");
        assert_eq!(
            by_ref.neighbors("a", Direction::Both),
            by_val.neighbors("a", Direction::Both)
        );
    }
```

If `neighbors` is not the accessor's real name, use whichever public accessor the
sibling tests in this module already use — read them first rather than guessing.

- [ ] **Step 3c: Run the reader tests**

```bash
cargo test -p vox-graph-reader
```

Expected: the new test PASSES and all existing reader tests still pass —
`from_value`'s behaviour must be unchanged for its four existing consumers.

- [ ] **Step 4: Add the cache types to `server_state.rs`**

Beside `CachedCatalog` (line 17):

```rust
/// Cache key for the parsed graphify graph. `corpus_id` is part of the key
/// because `graphify_compare` loads two corpora in one call and a single-slot
/// cache keyed only on mtime would thrash between them.
#[derive(Debug, Clone)]
pub struct GraphCacheKey {
    pub corpus_id: String,
    pub mtime: std::time::SystemTime,
    pub len: u64,
}

impl GraphCacheKey {
    pub fn matches(&self, corpus_id: &str, mtime: std::time::SystemTime, len: u64) -> bool {
        self.corpus_id == corpus_id && self.mtime == mtime && self.len == len
    }
}

/// Parsed graphify graph, cached to avoid a ~10 s / ~500 MB re-parse per call.
/// Holds both representations: `value` serves `lexical_search_graph`, `reader`
/// serves BFS/path traversal. `GraphifyReader::from_value` consumes its input,
/// so keeping only one would force a full clone to serve the other.
#[derive(Clone)]
pub struct CachedGraph {
    pub key: GraphCacheKey,
    pub value: Arc<serde_json::Value>,
    pub reader: Arc<vox_graph_reader::GraphifyReader>,
}
```

Add the field to `ServerState` beside `catalog_cache` (line 55):

```rust
    /// Cache for the parsed graphify graph. See [`CachedGraph`].
    pub graph_cache: Arc<TokRwLock<Option<CachedGraph>>>,
```

Then find every construction site of `ServerState` and add
`graph_cache: Arc::new(TokRwLock::new(None)),`. Locate them with:

```bash
grep -rn "catalog_cache:" crates/vox-orchestrator-mcp/src/
```

Add the field at each site that line reports — the compiler will also name any
you miss.

- [ ] **Step 5: Add `get_graph` to `graph_tools.rs`**

Replace `load_graph_json` (line 411) with this, keeping the old function only if
another caller still needs it (the compiler will tell you):

```rust
/// Load a corpus graph, serving from `state.graph_cache` when `graph.json` is
/// unchanged. Keyed on corpus id + mtime + length, so an out-of-GUI
/// `graphify update .` invalidates it without any explicit signal.
async fn get_graph(
    state: &ServerState,
    corpus_id: &str,
    corpus: &vox_config::graphify::GraphifyCorpus,
    repo_root: &std::path::Path,
) -> Result<crate::server_state::CachedGraph, String> {
    let p = repo_root.join(&corpus.graph_path);
    let meta = fs::metadata(&p).map_err(|e| format!("stat {}: {e}", p.display()))?;
    let mtime = meta.modified().map_err(|e| format!("mtime {}: {e}", p.display()))?;
    let len = meta.len();

    {
        let cache = state.graph_cache.read().await;
        if let Some(cached) = &*cache {
            if cached.key.matches(corpus_id, mtime, len) {
                // Clone is cheap: both payloads are behind Arc.
                return Ok(cached.clone());
            }
        }
    }

    let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))?;
    // from_ref, not from_value: cloning the 126 MB Value here would push peak
    // commit on a miss above the spike this cache exists to remove.
    let reader = vox_graph_reader::GraphifyReader::from_ref(&value).map_err(|e| e.to_string())?;
    let entry = crate::server_state::CachedGraph {
        key: crate::server_state::GraphCacheKey {
            corpus_id: corpus_id.to_string(),
            mtime,
            len,
        },
        value: Arc::new(value),
        reader: Arc::new(reader),
    };

    let mut cache = state.graph_cache.write().await;
    *cache = Some(entry.clone());
    Ok(entry)
}
```

Add `use std::sync::Arc;` to the file's imports if absent.

- [ ] **Step 6: Rewire the four call sites**

Replace each `load_graph_json(...)` + `GraphifyReader::from_value(...)` pair with
a single `get_graph(...)` call, using `entry.reader` where the reader was used
and `entry.value` where the `Value` was used. The sites are:

| Line (pre-edit) | Function | Uses |
|---|---|---|
| ~253/263 | `graphify_search` | inline read+parse → `entry.value` |
| ~448/455 | `graphify_query_core` | → `entry.reader` |
| ~539/546 | `graphify_path` | → `entry.reader` |
| ~737 | `graphify_rebuild` (node/edge count) | → `entry.value` |

`graphify_compare` loads two corpora; call `get_graph` twice with each corpus id.
Note it will always miss on the second call with a single-slot cache — that is
acceptable and is why `corpus_id` is in the key (correctness, not speed).

`lexical_search_graph(&graph, …)` takes `&serde_json::Value`, so pass
`&entry.value` — `Arc<Value>` derefs.

`reader.bfs_from_seeds(...)` and `reader.shortest_path(...)` take `&self`, so
`entry.reader.bfs_from_seeds(...)` works directly.

- [ ] **Step 7: Clear the cache on rebuild**

In `graphify_rebuild` (line 656), after the rebuild subprocess completes
successfully and **before** the node/edge count read, explicitly clear the slot
so the count reflects the new graph even if the filesystem mtime resolution is
too coarse to register the change:

```rust
    {
        let mut cache = state.graph_cache.write().await;
        *cache = None;
    }
```

- [ ] **Step 8: Run the tests**

```bash
cargo test -p vox-orchestrator-mcp graph_cache_key_rejects_mtime_change
cargo test -p vox-orchestrator-mcp graph_
```

Expected: the new test PASSES and no existing `graph_*` test regresses.

- [ ] **Step 9: Format and lint**

```bash
cargo fmt -p vox-orchestrator-mcp
cargo fmt -p vox-graph-reader
cargo clippy -p vox-orchestrator-mcp --all-targets -- -D warnings
cargo clippy -p vox-graph-reader --all-targets -- -D warnings
```

- [ ] **Step 10: Commit**

```bash
git commit -m "perf(mcp): cache parsed graphify graph in ServerState

Every graph tool re-read and re-parsed graph.json on each call: measured
10.4-11.8s and ~500MB peak commit against a 126MB graph. Caches both the
raw Value (for lexical_search_graph) and the GraphifyReader (for BFS/path)
behind one mtime+len+corpus_id key, mirroring the existing CachedCatalog
pattern.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -- crates/vox-graph-reader/src/lib.rs crates/vox-orchestrator-mcp/src/server_state.rs crates/vox-orchestrator-mcp/src/graph_tools.rs
```

---

### Task 2: Repair the Omnibar's two dead graph paths

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Omnibar.tsx:29-37` (constant + its comment), `:66-77` (`parseDiscoverResults`)
- Test: `crates/vox-gui/ui/src/components/layout/Omnibar.graphFacet.test.ts` (create)

**Interfaces:**
- Consumes: `get_graph` from Task 1 only indirectly (Task 1 makes these calls fast enough to fire per keystroke). No code dependency.
- Produces: nothing consumed by later tasks.

**Both graph paths in the Omnibar are currently dead, in two different ways.**
`GRAPH_DISCOVER_TOOL` names a tool that was renamed away, so it errors loudly.
`GRAPH_NEIGHBORS_TOOL` names a real tool but `parseDiscoverResults` reads
`result.results` while both graph tools return `result.hits` — so it fails
silently. Fixing only the constant converts the loud failure into a second
silent one.

- [ ] **Step 1: Verify the real payload shape before writing the test**

Do not infer it. Read `crates/vox-orchestrator-mcp/src/graph_tools.rs:319-338`
(`graphify_search` payload) and `:477-497` (`graphify_query_core` payload).
Confirm with your own eyes that both build `"hits"` and that each element has
`node_id`, `label`, and `knowledge_id`. Your test fixture must use those keys.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/ui/src/components/layout/Omnibar.graphFacet.test.ts`.
No `render()` is involved, so no jsdom docblock is needed here.

```ts
import { describe, expect, it } from 'vitest';
import { parseDiscoverResults } from './Omnibar';

describe('parseDiscoverResults', () => {
  it('reads the hits[] key that vox_search_structural actually returns', () => {
    // Shape copied verbatim from graph_tools.rs graphify_search payload.
    const res = {
      result: {
        corpus_id: 'repo-code-graph',
        searched_at: '2026-08-24T00:00:00Z',
        hits: [
          { node_id: 'crates_vox_gui_src_lib', label: 'vox_gui::lib', score: 0.9, knowledge_id: 'k1' },
        ],
      },
    };
    const rows = parseDiscoverResults(res);
    expect(rows).toHaveLength(1);
    expect(rows[0].id).toBe('crates_vox_gui_src_lib');
    // The tool supplies a human label; using the raw id would be a regression.
    expect(rows[0].label).toBe('vox_gui::lib');
  });

  it('still reads the legacy results[] key', () => {
    const res = { result: { results: [{ node_id: 'n1' }] } };
    expect(parseDiscoverResults(res)).toHaveLength(1);
  });

  it('maps surface: ids to a viewKey', () => {
    const res = { result: { hits: [{ node_id: 'surface:voxgraph', label: 'VoxGraph' }] } };
    expect(parseDiscoverResults(res)[0].viewKey).toBe('voxgraph');
  });

  it('returns [] on an error envelope', () => {
    expect(parseDiscoverResults({ is_error: true })).toEqual([]);
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

```bash
cd crates/vox-gui/ui && pnpm vitest run src/components/layout/Omnibar.graphFacet.test.ts
```

Expected: FAIL — `parseDiscoverResults` is not exported, and the `hits` case
returns `[]`.

- [ ] **Step 4: Fix the constant and its stale comment**

Replace lines 29-37 of `Omnibar.tsx`:

```ts
/**
 * Graph-discover MCP tool. `vox_graphify_query` was renamed to the `vox_search_*`
 * family (dispatch.rs registers vox_search_{status,structural,neighbors,path,
 * callers,callees,compare,rebuild}); this constant was left pointing at the old
 * name, so the facet errored on every keystroke. `vox_search_structural` is the
 * only graph tool that takes a lexical query string.
 */
const GRAPH_DISCOVER_TOOL = 'vox_search_structural';
```

Leave `GRAPH_NEIGHBORS_TOOL = 'vox_search_neighbors'` unchanged — that name is
correct; only its parser was wrong.

- [ ] **Step 5: Fix and export the parser**

Replace `parseDiscoverResults` (lines 66-77):

```ts
/**
 * Parse a graph-tool response into rows. Both `vox_search_structural` and
 * `vox_search_neighbors` return `result.hits[]`; `results[]` is accepted as a
 * legacy fallback. Reading only `results` made the neighbor path silently
 * return zero rows.
 */
export function parseDiscoverResults(res: unknown): GraphNeighbor[] {
  const r = res as {
    is_error?: boolean;
    result?: { hits?: unknown[]; results?: unknown[] };
  };
  if (r?.is_error) return [];
  const raw = Array.isArray(r?.result?.hits)
    ? r.result!.hits!
    : Array.isArray(r?.result?.results)
      ? r.result!.results!
      : [];
  return raw
    .map((n) => n as { node_id?: string; id?: string; label?: string })
    .map((n) => ({ id: n.node_id ?? n.id, label: n.label }))
    .filter((n): n is { id: string; label?: string } => typeof n.id === 'string' && n.id.length > 0)
    .map(({ id, label }) => {
      const vk = id.startsWith('surface:') ? id.slice('surface:'.length) : undefined;
      return { id, label: label ?? vk ?? id, viewKey: vk };
    });
}
```

- [ ] **Step 6: Run the tests and the type-check**

```bash
cd crates/vox-gui/ui && pnpm vitest run src/components/layout/Omnibar.graphFacet.test.ts
cd crates/vox-gui/ui && pnpm vitest run src/components/layout/
cd crates/vox-gui/ui && pnpm typecheck
```

Expected: new tests PASS, no existing Omnibar test regresses, typecheck clean.
`pnpm typecheck` is a distinct gate from vitest and is run by `vox ci gui-honesty`.

- [ ] **Step 7: Commit**

```bash
git commit -m "fix(gui): repair both dead graph paths in the Omnibar

GRAPH_DISCOVER_TOOL pointed at vox_graphify_query, which no longer exists
after the vox_search_* rename, so the discover path errored on every
keystroke. Separately, parseDiscoverResults read result.results while both
graph tools return result.hits, so the neighbor path silently parsed to an
empty array. Fixing only the first would have made the second the sole
symptom.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -- crates/vox-gui/ui/src/components/layout/Omnibar.tsx crates/vox-gui/ui/src/components/layout/Omnibar.graphFacet.test.ts
```

---

### Task 3: `vox_search_set_ttl` — write the TTL SSOT

**Files:**
- Modify: `crates/vox-config/src/graphify.rs` (add `validate_ttl_days` + `set_ttl_days`)
- Modify: `crates/vox-orchestrator-mcp/src/graph_tools.rs` (add `GraphifySetTtlParams` + `graphify_set_ttl`; expose TTL in `graphify_status`)
- Modify: `contracts/operations/catalog.v1.yaml` (one hand-authored row — the SSOT)
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs:490` (arm after `vox_search_rebuild`)
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs:882` (arm after `vox_search_rebuild`)
- Modify: `crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs:22` (extend the name guard)
- Regenerated (never hand-edited): `contracts/mcp/tool-registry.canonical.yaml`, `contracts/cli/command-registry.yaml`, `contracts/capability/capability-registry.yaml`, `contracts/capability/model-manifest.generated.json`, `contracts/reports/gui-surface-coverage.v1.json`
- Test: `crates/vox-config/src/graphify.rs` (existing `#[cfg(test)] mod`)

**Interfaces:**
- Consumes: nothing from Tasks 1–2.
- Produces: `vox_config::graphify::validate_ttl_days(days: u64) -> Result<u64, String>`, `vox_config::graphify::set_ttl_days(repo_root: &Path, days: u64) -> std::io::Result<()>`, and MCP tool `vox_search_set_ttl { ttl_days: u64 }`. Task 4 calls the MCP tool; Task 5 depends on the SSOT being the single source.

**The value written is the SSOT, not a local override.** TTL today is global
(`CorporaFile.ttl_days_default` in `contracts/retrieval/vox-graph-corpora.v1.yaml`,
`CORPORA_REL_PATH`) and read-only: `resolve_ttl_days` consults
`VOX_GRAPHIFY_TTL_DAYS` then falls back to that value, and nothing writes it.

An earlier draft of this task persisted the edit to the gitignored
`.vox/cache` overlay instead. **That was wrong**, because CI reads staleness from
the same registry (`vox graphify status --strict`, `.github/workflows/ci.yml`),
and a gitignored override is invisible to CI — the GUI badge and the CI gate
would disagree about what "stale" means. Writing the contract keeps one value
for the GUI, the CLI, and CI, and surfaces the change as a reviewable git diff.

**Edit the one line, do not round-trip the YAML.** The contract is
hand-authored and carries comments and a deliberate key order. `serde_yaml`
round-tripping would strip the comments and reformat the whole file, producing
an unreadable diff for a one-number change. Replace only the
`ttl_days_default:` line.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod` in `crates/vox-config/src/graphify.rs`:

```rust
    #[test]
    fn validate_ttl_days_rejects_zero_and_absurd() {
        assert_eq!(validate_ttl_days(30), Ok(30));
        assert_eq!(validate_ttl_days(1), Ok(1));
        assert_eq!(validate_ttl_days(3650), Ok(3650));
        assert!(validate_ttl_days(0).is_err());
        assert!(validate_ttl_days(3651).is_err());
    }

    #[test]
    fn set_ttl_days_rewrites_one_line_and_preserves_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(CORPORA_REL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Shape copied from the real contract, comments included.
        std::fs::write(
            &path,
            "x-vox-version: 1\nschema_version: 1\n\n# Named Graphify knowledge-graph corpora.\n# See docs/...\n\ndefault_corpus_id: repo-code-graph\nttl_days_default: 30\n\ncorpora:\n  - id: repo-code-graph\n    title: Repository code graph\n",
        )
        .unwrap();

        set_ttl_days(tmp.path(), 7).unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("ttl_days_default: 7"), "value not updated");
        assert!(!after.contains("ttl_days_default: 30"), "old value still present");
        // Everything else must survive byte-for-byte.
        assert!(after.contains("# Named Graphify knowledge-graph corpora."));
        assert!(after.contains("# See docs/..."));
        assert!(after.contains("default_corpus_id: repo-code-graph"));
        assert!(after.contains("    title: Repository code graph"));
        // And the file must still parse.
        let reg = load_graphify_corpora(tmp.path()).unwrap();
        assert_eq!(reg.ttl_days_default, 7);
    }

    #[test]
    fn set_ttl_days_errors_when_key_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(CORPORA_REL_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // No ttl_days_default line: the key is serde-defaulted, so it can be missing.
        std::fs::write(&path, "x-vox-version: 1\ndefault_corpus_id: a\ncorpora: []\n").unwrap();
        assert!(
            set_ttl_days(tmp.path(), 7).is_err(),
            "must not silently no-op when the key is absent"
        );
    }
```

- [ ] **Step 2: Run them to verify they fail**

```bash
cargo test -p vox-config validate_ttl_days_rejects_zero_and_absurd
cargo test -p vox-config set_ttl_days
```

Expected: FAIL — `validate_ttl_days` and `set_ttl_days` do not exist.

- [ ] **Step 3: Implement in `crates/vox-config/src/graphify.rs`**

```rust
/// Accepted TTL range for the corpora registry, in days.
/// Zero would mark every corpus permanently stale; the upper bound is ten
/// years, past which the value is certainly a typo rather than an intent.
const TTL_DAYS_MIN: u64 = 1;
const TTL_DAYS_MAX: u64 = 3650;

/// Validate a TTL in days. Pure — callable from a command boundary before any
/// write, so an absurd value is rejected with a message rather than persisted
/// and discovered later.
pub fn validate_ttl_days(days: u64) -> Result<u64, String> {
    if (TTL_DAYS_MIN..=TTL_DAYS_MAX).contains(&days) {
        Ok(days)
    } else {
        Err(format!(
            "ttl_days must be between {TTL_DAYS_MIN} and {TTL_DAYS_MAX} (got {days})"
        ))
    }
}

/// Rewrite `ttl_days_default` in the corpora contract, leaving every other byte
/// of the file untouched.
///
/// This is a surgical single-line edit rather than a `serde_yaml` round-trip on
/// purpose: the contract is hand-authored with comments and a deliberate key
/// order, and reserializing it would strip both for a one-number change.
///
/// Errors if the key is absent, because `ttl_days_default` is serde-defaulted
/// and a missing key would otherwise make this a silent no-op.
pub fn set_ttl_days(repo_root: &Path, days: u64) -> std::io::Result<()> {
    let path = repo_root.join(CORPORA_REL_PATH);
    let raw = fs::read_to_string(&path)?;
    let mut found = false;
    let mut out = String::with_capacity(raw.len());
    for line in raw.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let body = body.strip_suffix('\r').unwrap_or(body);
        if !found && body.trim_start().starts_with("ttl_days_default:") && !body.starts_with(' ') {
            // Top-level key only (no leading indent), first occurrence only.
            found = true;
            out.push_str(&format!("ttl_days_default: {days}"));
            // Preserve whatever line ending the file already uses.
            out.push_str(&line[body.len()..]);
        } else {
            out.push_str(line);
        }
    }
    if !found {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "ttl_days_default key not found in {}; refusing to write",
                path.display()
            ),
        ));
    }
    fs::write(&path, out)
}
```

- [ ] **Step 4: Run the config tests**

```bash
cargo test -p vox-config graphify
```

Expected: the three new tests PASS and every existing graphify test still passes.

- [ ] **Step 5: Add the MCP handler in `graph_tools.rs`**

Beside the other params structs:

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GraphifySetTtlParams {
    pub ttl_days: u64,
}
```

And the handler, beside `graphify_rebuild`:

```rust
/// `vox_search_set_ttl`: set the corpora registry's global staleness TTL.
///
/// Writes `contracts/retrieval/vox-graph-corpora.v1.yaml`, which is the same
/// value the CLI and the CI freshness gate read — so the GUI, `vox graphify
/// status`, and CI cannot disagree. The edit is a tracked file change and
/// shows up in `git status`; the response says so.
pub async fn graphify_set_ttl(state: &ServerState, params: GraphifySetTtlParams) -> String {
    let repo_root = &state.repository.root;
    let days = match vox_config::graphify::validate_ttl_days(params.ttl_days) {
        Ok(d) => d,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(e, REM_GRAPHIFY).to_json();
        }
    };
    if let Err(e) = vox_config::graphify::set_ttl_days(repo_root, days) {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("write ttl: {e}"),
            REM_GRAPHIFY,
        )
        .to_json();
    }
    // Re-read through the normal loader so the response reflects the same
    // precedence every other caller sees (env > contract).
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err_with_remediation(
                e.to_string(),
                REM_GRAPHIFY,
            )
            .to_json();
        }
    };
    let effective = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
    ToolResult::ok(serde_json::json!({
        "ttl_days_written": days,
        "ttl_days_effective": effective,
        "env_override_active": effective != days,
        "contract_path": vox_config::graphify::CORPORA_REL_PATH,
        "requires_commit": true,
    }))
    .to_json()
}
```

`env_override_active` matters: if `VOX_GRAPHIFY_TTL_DAYS` is set in the
environment, the write succeeds but has no local effect, and the UI must say so
rather than appear to have silently ignored the user.

- [ ] **Step 6: Expose the effective TTL in `vox_search_status`**

The status payload currently carries no TTL, so a UI has nothing to display or
seed an editor with. In `graphify_status` (line 153), after `reg` is loaded:

```rust
    let ttl_days = vox_config::graphify::resolve_ttl_days(reg.ttl_days_default);
```

and add three keys to its `payload` (line ~194):

```rust
    let payload = serde_json::json!({
        "head_git_sha": head,
        "default_corpus_id": reg.default_corpus_id,
        // Effective TTL after env > contract precedence, the contract value
        // itself, and where it lives — so a UI can distinguish "you set this"
        // from "an env var is forcing this" without hardcoding a path.
        "ttl_days": ttl_days,
        "ttl_days_contract": reg.ttl_days_default,
        "ttl_days_env_forced": ttl_days != reg.ttl_days_default,
        "ttl_contract_path": vox_config::graphify::CORPORA_REL_PATH,
        "corpora": corpora,
    });
```

This is additive — every existing consumer ignores unknown keys.

- [ ] **Step 7: Add the catalog SSOT row**

This is the step that fails **silently** if skipped — with no row, `vox ci
ssot-drift` reports nothing and the tool never enters `TOOL_REGISTRY`. Add to
`contracts/operations/catalog.v1.yaml`, mirroring the `graph.rebuild` row's
shape exactly (it is the nearest write-side template):

```yaml
- id: graph.set.ttl
  title: Vox Search Set TTL
  description: Set the Vox Search corpora staleness TTL in days, writing the shared registry contract that the CLI and CI freshness gate also read (write).
  description_human: null
  product_lane: platform
  intent_tags:
  - retrieval
  - graph
  side_effect_class: writes_files
  scope_kind: repository
  reversible: true
  requires_repo: true
  preferred_for_models: false
  human_takeover_friendly: true
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp:
    name: vox_search_set_ttl
    http_read_role_eligible: false
    tier: core
  cli: null
```

`http_read_role_eligible: false` is deliberate and required — this tool
mutates state, and `true` would both be a governance error and force a
hand-edit of `contracts/mcp/http-read-role-governance.yaml`, which two gates
cross-check.

- [ ] **Step 8: Regenerate every downstream registry**

```bash
cargo run -q -p vox-cli -- ci operations-sync --target all --write
cargo run -q -p vox-cli -- ci capability-sync --write
cargo run -q -p vox-cli -- ci gui-surface-coverage --write
```

Do **not** hand-edit any file these produce. In particular do not add anything
to the `capability:` block — `project_capability_registry_doc` derives a
capability row from each operation row automatically.

- [ ] **Step 9: Add the input schema arm**

In `crates/vox-orchestrator-mcp/src/input_schemas.rs`, immediately after the
`"vox_search_rebuild"` arm (line 490):

```rust
        "vox_search_set_ttl" => parse_obj(
            r#"{"type":"object","properties":{"ttl_days":{"type":"integer","minimum":1,"maximum":3650,"description":"Staleness TTL in days for Vox Search corpora. WRITE/mutating: edits the tracked contract contracts/retrieval/vox-graph-corpora.v1.yaml, which the CLI and the CI freshness gate also read. The VOX_GRAPHIFY_TTL_DAYS env var still takes precedence at runtime."}},"required":["ttl_days"],"additionalProperties":false}"#,
        ),
```

- [ ] **Step 10: Add the dispatch arm**

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, immediately after the
`"vox_search_rebuild"` arm (line 882):

```rust
        "vox_search_set_ttl" => {
            Ok(crate::graph_tools::graphify_set_ttl(state, serde_json::from_value(args)?).await)
        }
```

`ttl_days` is required, so probing with `{}` yields a serde parse error rather
than "Unknown tool" — which is what `every_registry_tool_has_static_dispatch`
expects. No `SKIP_DISPATCH_PROBE` entry is needed.

- [ ] **Step 11: Extend the name guard**

In `crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs`, add to the array
that currently ends at `"\"vox_search_rebuild\"",` (line 22):

```rust
        "\"vox_search_set_ttl\"",
```

- [ ] **Step 12: Run the full gate set**

```bash
cargo test -p vox-config graphify
cargo test -p vox-orchestrator-mcp
cargo build -p vox-cli
cargo run -q -p vox-cli -- ci operations-sync --target all
cargo run -q -p vox-cli -- ci ssot-drift
```

Expected: all pass; `operations-sync` in verify mode (no `--write`) reports no
drift; `ssot-drift` exits 0. The four gates that specifically police a new tool
are `registry_tools_have_input_schema_coverage`,
`all_parse_obj_schemas_are_valid_jsonschema`,
`yaml_registry_tools_have_dispatch_match_arms`, and
`every_registry_tool_has_static_dispatch`.

**Before committing, confirm you did not leave the real contract modified.**
The tests write to temp dirs, but a stray manual invocation would edit the real
`contracts/retrieval/vox-graph-corpora.v1.yaml`. Run
`git diff -- contracts/retrieval/vox-graph-corpora.v1.yaml` and expect no
output; if there is a diff, `git checkout --` that file.

- [ ] **Step 13: Format, lint, and commit**

```bash
cargo fmt -p vox-config
cargo fmt -p vox-orchestrator-mcp
cargo clippy -p vox-config -p vox-orchestrator-mcp --all-targets -- -D warnings
```

```bash
git commit -m "feat(mcp): vox_search_set_ttl writes the graphify TTL SSOT

resolve_ttl_days had three read sites and no write path. The tool edits
ttl_days_default in contracts/retrieval/vox-graph-corpora.v1.yaml with a
surgical single-line rewrite that preserves comments and key order, so the
GUI, the CLI, and the CI freshness gate all read one value. vox_search_status
now reports the effective TTL, the contract value, and whether an env var is
forcing it.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -- crates/vox-config/src/graphify.rs crates/vox-orchestrator-mcp/src/graph_tools.rs crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/tests/vox_search_dispatch.rs contracts/operations/catalog.v1.yaml contracts/mcp/tool-registry.canonical.yaml contracts/cli/command-registry.yaml contracts/capability/capability-registry.yaml contracts/capability/model-manifest.generated.json contracts/reports/gui-surface-coverage.v1.json
```

If `git status` shows a generated contract file still dirty after this commit,
it was missing from the pathspec — add it and amend. A clean local tree with
uncommitted regenerated artifacts is exactly how this fails on CI while looking
fine locally.

---

### Task 4: TTL editor in the VoxGraph lifecycle rail

**Files:**
- Modify: `crates/vox-gui/ui/src/types/tauri.ts:246-249` (two optional fields on `GraphifyStatusDto`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx` (add `TtlEditor`, render it in the header row)
- Test: `crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx` (append cases; change no existing case)

**Interfaces:**
- Consumes: `vox_search_set_ttl { ttl_days: number }` and the `ttl_days` / `ttl_days_env_forced` keys on `vox_search_status`, both from Task 3.
- Produces: nothing consumed by later tasks.

**Do not break the existing panel test.** `VoxGraphStatusPanel.test.tsx` supplies
fixture data with no `ttl_days` key. The editor must therefore render only when
the field is actually present, and the DTO fields must be optional. Guarding on
presence — rather than defaulting to some number — is what keeps the three
existing cases passing untouched.

- [ ] **Step 1: Write the failing tests**

Append to the existing `describe('VoxGraphStatusPanel', ...)` block. The file
already carries the jsdom environment docblock on line 1 — do not add a second.

```tsx
  it('shows the effective TTL and saves an edited value', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    const input = screen.getByLabelText('Staleness TTL in days') as HTMLInputElement;
    expect(input.value).toBe('30');

    fireEvent.change(input, { target: { value: '7' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalled();
    });
    const call = mockInvoke.mock.calls.at(-1);
    expect(JSON.stringify(call)).toContain('vox_search_set_ttl');
    expect(JSON.stringify(call)).toContain('"ttl_days":7');
  });

  it('rejects an out-of-range TTL without calling the backend', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    fireEvent.change(screen.getByLabelText('Staleness TTL in days'), { target: { value: '0' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/between 1 and 3650/);
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('tells the user when an env var overrides the stored TTL', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 5,
        ttl_days_env_forced: true,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.getByText(/VOX_GRAPHIFY_TTL_DAYS/)).toBeInTheDocument();
  });

  it('omits the TTL editor entirely when the backend sends no ttl_days', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: { default_corpus_id: 'repo-code-graph', corpora: [STALE_CORPUS] },
    });
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.queryByLabelText('Staleness TTL in days')).toBeNull();
  });
```

The first test asserts on the serialized call rather than an exact argument
tuple because the exact shape `voxTransport.invokeMcpTool` passes to Tauri's
`invoke` is a detail of `transport.ts`. If you prefer an exact assertion, read
`crates/vox-gui/ui/src/transport.ts` first and write the real shape — do not
adjust the implementation to satisfy a guessed one.

- [ ] **Step 2: Run them to verify they fail**

```bash
cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx
```

Expected: the four new cases FAIL (no such label / no such button); the three
pre-existing cases still PASS. If a pre-existing case fails, stop and fix that
first — it means the presence guard is wrong.

- [ ] **Step 3: Widen the DTO**

In `crates/vox-gui/ui/src/types/tauri.ts`, replace lines 246-249:

```ts
export interface GraphifyStatusDto {
  default_corpus_id: string;
  /** Effective staleness TTL in days after env > override > default precedence.
   *  Optional: older backends omit it, and the editor hides itself when absent. */
  ttl_days?: number;
  /** True when VOX_GRAPHIFY_TTL_DAYS is forcing `ttl_days`, so a stored
   *  override would have no effect. */
  ttl_days_env_forced?: boolean;
  corpora: CorpusStatusDto[];
}
```

- [ ] **Step 4: Add the `TtlEditor` component**

In `VoxGraphStatusPanel.tsx`, beside `RebuildButton`. It follows that component's
existing shape deliberately — local busy/error state, `invokeMcpTool`, then
invalidate the status query so the cards re-read freshness under the new TTL:

```tsx
/** Range mirrors validate_ttl_days in crates/vox-config/src/graphify.rs. */
const TTL_DAYS_MIN = 1;
const TTL_DAYS_MAX = 3650;

/**
 * Editable staleness TTL. TTL is a global registry setting, so this lives in the
 * panel header rather than on a corpus card. Writes go through
 * `vox_search_set_ttl`, which persists a user-local override in gitignored
 * `.vox/cache` — never the tracked contract YAML.
 */
function TtlEditor({ ttlDays, envForced }: { ttlDays: number; envForced: boolean }) {
  const queryClient = useQueryClient();
  const [value, setValue] = useState(String(ttlDays));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSave = useCallback(async () => {
    const parsed = Number(value);
    if (!Number.isInteger(parsed) || parsed < TTL_DAYS_MIN || parsed > TTL_DAYS_MAX) {
      setError(`TTL must be a whole number between ${TTL_DAYS_MIN} and ${TTL_DAYS_MAX}`);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await voxTransport.invokeMcpTool('vox_search_set_ttl', { ttl_days: parsed });
      await queryClient.invalidateQueries({ queryKey: VOX_GRAPH_STATUS_QUERY_KEY });
    } catch (e) {
      setError(sanitizeErrorForToast((e as Error)?.message ?? e));
    } finally {
      setBusy(false);
    }
  }, [value, queryClient]);

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center gap-2">
        <label htmlFor="vg-ttl-days" className="text-[9px] uppercase tracking-wider text-zinc-500">
          TTL (days)
        </label>
        <input
          id="vg-ttl-days"
          type="number"
          inputMode="numeric"
          min={TTL_DAYS_MIN}
          max={TTL_DAYS_MAX}
          value={value}
          disabled={busy}
          aria-label="Staleness TTL in days"
          onChange={(e) => setValue(e.target.value)}
          className="h-7 w-20 rounded-md border border-white/10 bg-zinc-950/40 px-2 font-mono text-[11px] text-zinc-200 disabled:opacity-50"
        />
        <button
          type="button"
          disabled={busy}
          aria-label="Save TTL"
          onClick={handleSave}
          className="h-7 rounded-md border border-white/10 bg-white/5 px-3 text-[11px] font-medium text-zinc-200 transition hover:bg-white/10 disabled:opacity-50"
        >
          {busy ? 'Saving...' : 'Save'}
        </button>
      </div>
      {envForced && (
        <span className="text-[10px] text-amber-400">
          VOX_GRAPHIFY_TTL_DAYS is set and overrides this value.
        </span>
      )}
      {error && (
        <span role="alert" className="text-[10px] text-red-400">
          {error}
        </span>
      )}
    </div>
  );
}
```

The 28 px (`h-7`) control height clears the WCAG 2.2 SC 2.5.8 24x24 minimum;
do not shrink it to match the 10 px type scale around it.

- [ ] **Step 5: Render it in the header**

Replace the header block in `VoxGraphStatusPanel` (the
`<div className="flex items-center justify-between">` containing the `h2`):

```tsx
      <div className="flex items-center justify-between gap-4">
        <h2 className="ds-section-head">{corpusHealthLabel}</h2>
        <div className="flex items-center gap-4">
          {typeof data.ttl_days === 'number' && (
            <TtlEditor ttlDays={data.ttl_days} envForced={data.ttl_days_env_forced === true} />
          )}
          <span className="font-mono text-[10px] text-zinc-500">
            Default: {data.default_corpus_id}
          </span>
        </div>
      </div>
```

Leave the `condensed` branch alone — TTL editing does not belong in a
sidebar-sized summary.

- [ ] **Step 6: Run tests and type-check**

```bash
cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx
cd crates/vox-gui/ui && pnpm typecheck
```

Expected: all seven cases PASS (three pre-existing, four new); typecheck clean.

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(gui): editable staleness TTL in the VoxGraph lifecycle rail

Adds a TTL editor beside the corpus health cards, writing through
vox_search_set_ttl. Hidden when the backend omits ttl_days, so older
backends and the existing panel tests are unaffected. Surfaces
VOX_GRAPHIFY_TTL_DAYS when it would override the stored value.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -- crates/vox-gui/ui/src/types/tauri.ts crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.tsx crates/vox-gui/ui/src/components/surfaces/VoxGraph/VoxGraphStatusPanel.test.tsx
```

---

---

### Task 5: TTL parity between the GUI, the CLI, and the CI freshness gate

**Files:**
- Modify: `.github/workflows/ci.yml:1034-1040` (drop the hardcoded `VOX_GRAPHIFY_TTL_DAYS` env pin)
- Test: `crates/vox-config/tests/graphify_ttl_parity.rs` (create)

**Interfaces:**
- Consumes: `vox_config::graphify::CORPORA_REL_PATH` and `load_graphify_corpora` (both pre-existing); the SSOT write path from Task 3.
- Produces: nothing consumed by later tasks.

**The divergence this closes.** `.github/workflows/ci.yml` currently pins the
freshness gate's TTL in the workflow itself:

```yaml
      - name: Graphify corpus freshness gate (warning)
        if: needs.setup.outputs.full == 'true'
        run: ./target/debug/vox graphify status --strict
        env:
          VOX_GRAPHIFY_TTL_DAYS: "7"
        continue-on-error: true
```

`resolve_ttl_days` gives the env var precedence over the contract, so CI
enforces 7 days while `contracts/retrieval/vox-graph-corpora.v1.yaml` says 30
and every local caller — including the GUI editor from Task 4 — sees 30. The
number a user edits in the GUI would therefore have no effect on the gate that
actually reports staleness in CI. Removing the pin makes CI read the same SSOT
as everyone else.

**Why loosening CI from 7 to 30 is acceptable:** the step is
`continue-on-error: true` — a warning, not a merge blocker. Trading a
non-blocking warning's sensitivity for a single source of truth is the right
side of that trade, and anyone who wants 7 can now set it in the contract,
where the GUI and CLI will agree with them.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-config/tests/graphify_ttl_parity.rs`. It is a repo-level
guard, not a unit test: it reads the real workflow files and fails if any of
them re-pins the env var, so this divergence cannot silently return.

```rust
//! Guard: the graphify staleness TTL has exactly one source of truth.
//!
//! `resolve_ttl_days` lets `VOX_GRAPHIFY_TTL_DAYS` override the contract value.
//! That is fine for an ad-hoc local run, but a workflow that pins it makes CI
//! enforce a different TTL from the one the GUI edits and the CLI reports.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // crates/vox-config/ -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

#[test]
fn no_workflow_pins_the_graphify_ttl_env_var() {
    let dir = repo_root().join(".github/workflows");
    let mut offenders = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("workflows dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("yml") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("read workflow");
        for (i, line) in raw.lines().enumerate() {
            if line.contains("VOX_GRAPHIFY_TTL_DAYS") {
                offenders.push(format!("{}:{}", path.display(), i + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "workflows must not pin VOX_GRAPHIFY_TTL_DAYS — it overrides \
         ttl_days_default in {}, so CI would enforce a different staleness \
         window than the GUI and CLI report. Set the value in the contract \
         instead. Offenders: {offenders:?}",
        vox_config::graphify::CORPORA_REL_PATH,
    );
}

#[test]
fn contract_ttl_is_the_value_every_caller_resolves() {
    // With no env override, resolve_ttl_days must return the contract value
    // verbatim — this is the invariant the guard above protects.
    let root = repo_root();
    let reg = vox_config::graphify::load_graphify_corpora(&root).expect("load contract");
    // SAFETY-of-intent: assert the pure relationship, not the ambient env.
    // If VOX_GRAPHIFY_TTL_DAYS happens to be set in the developer's shell,
    // resolve_ttl_days would return that instead — so only assert when unset.
    if std::env::var("VOX_GRAPHIFY_TTL_DAYS").is_err() {
        assert_eq!(
            vox_config::graphify::resolve_ttl_days(reg.ttl_days_default),
            reg.ttl_days_default
        );
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cargo test -p vox-config --test graphify_ttl_parity
```

Expected: `no_workflow_pins_the_graphify_ttl_env_var` FAILS, naming
`.github/workflows/ci.yml:1038`. If it passes, someone already removed the pin
— re-read `ci.yml` before continuing rather than assuming.

- [ ] **Step 3: Remove the pin**

In `.github/workflows/ci.yml`, delete the two `env:` lines from the freshness
gate step so it reads:

```yaml
      - name: Graphify corpus freshness gate (warning)
        if: needs.setup.outputs.full == 'true'
        # No VOX_GRAPHIFY_TTL_DAYS pin: the staleness window comes from
        # ttl_days_default in contracts/retrieval/vox-graph-corpora.v1.yaml,
        # the same value the GUI editor writes and `vox graphify status`
        # reports. Guarded by crates/vox-config/tests/graphify_ttl_parity.rs.
        run: ./target/debug/vox graphify status --strict
        continue-on-error: true
```

Leave `if:` and `continue-on-error:` exactly as they are — this task changes
which TTL the gate uses, not whether or when it runs.

- [ ] **Step 4: Run the test again**

```bash
cargo test -p vox-config --test graphify_ttl_parity
```

Expected: both tests PASS.

- [ ] **Step 5: Verify no other gate re-introduces the split**

```bash
grep -rn "VOX_GRAPHIFY_TTL_DAYS" .github/ scripts/ lefthook.yml
```

Expected: no hits under `.github/` or `scripts/`. Hits in
`contracts/config/env-vars.v1.yaml` and
`contracts/config/config-registry-baseline.txt` are the env-var *registry*
declaring the variable exists — those are correct and must stay.

- [ ] **Step 6: Commit**

```bash
git commit -m "fix(ci): read the graphify TTL from the contract, not a workflow pin

The freshness gate pinned VOX_GRAPHIFY_TTL_DAYS=7, which resolve_ttl_days
gives precedence over ttl_days_default (30) in the corpora contract. CI
therefore enforced a different staleness window than the CLI reported and
the GUI editor writes. Drops the pin so all three read one value, and adds
a guard test so no workflow can re-introduce the split.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" -- .github/workflows/ci.yml crates/vox-config/tests/graphify_ttl_parity.rs
```

---

## Self-Review

**Spec coverage.** There is no spec; the Background section is the authority.
Its consequences map to tasks: the 10 s / 500 MB uncached parse -> Task 1; the
dead `GRAPH_DISCOVER_TOOL` constant and the `results`-vs-`hits` mismatch ->
Task 2; `resolve_ttl_days` having read sites and no write path -> Task 3
(backend) and Task 4 (UI); the CI/GUI TTL divergence -> Task 5. The user's
request also named "refresh management" — already shipped as `RebuildButton`,
so no task duplicates it.

**Placeholder scan.** No "TBD" / "handle errors appropriately" / "similar to
Task N". Three steps deliberately instruct the implementer to read real source
before writing (Task 2 Step 1; Task 4 Step 1's note on the invoke shape; Task 5
Step 2's "re-read `ci.yml` rather than assuming") instead of supplying a value
inferred from consuming code — that is the fixture rule in the Global
Constraints, not a placeholder. Task 1 Step 3b says "existing body verbatim,
unchanged" rather than reproducing ~100 lines; see the ledger ruling.

**Type consistency.** `GraphCacheKey` / `CachedGraph` field names match between
Task 1's `server_state.rs` definition, its test, and `get_graph`.
`validate_ttl_days` and `set_ttl_days` are named identically in Task 3's tests,
its implementation, and its handler — and Task 5's guard test calls only
pre-existing `vox_config::graphify` items (`CORPORA_REL_PATH`,
`load_graphify_corpora`, `resolve_ttl_days`). `ttl_days` is the wire key in
Task 3's `graphify_status` payload, Task 3's input schema, Task 4's DTO, and
Task 4's fixtures; `ttl_days_env_forced` likewise. `TTL_DAYS_MIN` /
`TTL_DAYS_MAX` = 1 / 3650 in the Rust validator, the JSON schema
(`minimum` / `maximum`), and the TSX constants.

**One SSOT, checked end to end.** After Task 5 the staleness window has exactly
one source: `ttl_days_default` in `contracts/retrieval/vox-graph-corpora.v1.yaml`.
Task 3 writes it, `resolve_ttl_days` reads it, `vox graphify status` reports it,
the CI gate enforces it, and Task 4 displays it — with `VOX_GRAPHIFY_TTL_DAYS`
surviving only as a deliberate ad-hoc runtime override that the UI labels when
active and that Task 5's guard forbids in any workflow.
