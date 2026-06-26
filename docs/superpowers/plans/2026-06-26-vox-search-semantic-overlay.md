# Vox Search — Semantic Overlay (Layer 5) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This plan is **workflow-ready**: every task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`, grouped into explicit fan-out batches, and ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui` add+commit (no push — the workflow pushes/merges at the end).

**Goal:** Build Vox Search's **semantic overlay** (Layer 5 of the unified code-intelligence service): an embedding-backed, LLM-relation-labeled overlay stored in a **physically separate `semantic-overlay.json`** that references structural node ids but **never mutates `graph.json`**. Ship the `vox_search_semantic_related` MCP tool (+ `vox search semantic-related` CLI mirror), the overlay writer/reader, an overlay freshness-sha tied to the structural core, and a **mixed seed-then-structural-expand** query that grounds fuzzy embedding seeds in the deterministic call/composition graph. Every result is provenance-labeled `layer: "structural" | "semantic"`, carries `source`/`similarity`/`confidence`, and is stamped `stale: bool` from the overlay-vs-core sha check.

**Architecture:** One new module `crates/vox-graphify-reader/src/semantic_overlay.rs` (the writer/reader + freshness sha + mixed-expand, all deterministic given a fixed embedder), reusing the existing `GraphifyReader` BFS for the structural expansion. The embedding lane reuses **Vox Search's** `llm_embed` pipeline (no second embedding stack) and the `GraphifyNodes` corpus seam introduced by **P2 (fusion)**. The MCP surface is a thin handler in `crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs` over the reader + an injected embedder, matching the proven `graphify_tools.rs` handler→dispatch-arm→input-schema pattern. The CLI mirror lives in the existing `vox search` (formerly `vox graphify`) command group. **The honesty firewall is the spine:** the overlay is a separate artifact, regenerated-not-incrementally-trusted, dropped-on-sha-mismatch, and queries warn on stale; LLM-guessed relations are **never** promoted into `graph.json`.

**Tech Stack:** Rust (`serde`/`serde_json`, `blake3` via the crate's existing `graph_digest`, `syn` already present, `anyhow`, `clap`, `tokio` for the async MCP/embed path); the `vox-graphify-reader` structural-index lib; `vox-actor-runtime::llm::embed::llm_embed` for embeddings; the `vox-orchestrator-mcp` dispatcher + `input_schemas.rs`; the `vox search` CLI group; `contracts/retrieval/graphify-corpora.v1.yaml` corpus registry.

**Spec (read first):** `docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md` §2 (Semantic overlay). **Master umbrella spec:** `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` §2.4 (Layer 5), §2.5 (honesty boundary), §3.1 (tool surface row `vox_search_semantic_related`), §4.1 (Related pane), §6 (sequencing: semantic overlay LAST), §9 (Plan P3).

---

## Cross-plan dependencies (state these at the top — must precede this plan)

This is **P3** in the umbrella plan index (`vox-search-unified-code-intelligence-design.md` §9). Its hard prerequisites:

- **P0 — Absorption + structural-core enrichment** (`2026-06-26-graphify-general-enhancement-and-gui-ia-blueprint.md` Plan 1, Phases A0–F + the rename). Provides: the `confidence`-labeled `graph.json` schema, the `cmd:`/`tool:`/`surface:` node kinds, `coverage.rs`, the `vox search` CLI group (renamed from `vox graphify`), and the final MCP tool names (`vox_search_*`). **This plan reuses node ids and the `graph_json_sha256` manifest field from P0.**
- **P2 — Fusion (`vox_discover`, lexical-seed first)** (`2026-06-26-vox-search-fusion-plan.md`, sibling). Provides: the **`SearchCorpus::GraphifyNodes`** corpus + its embedding seam over `llm_embed`/Qdrant, and `resolve.rs` (hit→node_id). **The semantic overlay's embedding lane rides on this corpus — it does NOT stand up a parallel embedding stack** (umbrella §2.4, §8 non-goal "No second embedding stack"; design §2.2).

> **Critical path:** `P0 → P2 → P3 (this plan)`. The overlay's kNN seeds come from the `GraphifyNodes` corpus that P2 creates; building this before P2 would duplicate embedding infra (the explicit anti-goal). **Within this plan, the structural-expand + freshness-sha + writer/reader work (Phases A, B, D) depends only on P0** and can begin as soon as P0 lands; only the embedding-seed wiring (Phase C) and the live-embed CLI path block on P2. Phases are sequenced accordingly so the workflow can start the P0-only phases in parallel with P2's execution.

**Naming note (from P0 rename):** the external surface is `vox search` / `vox_search_*`. Where this plan touches code that P0 may still be renaming (the `vox graphify` CLI enum, the `graphify_tools.rs` arms), tasks reference **both** the new and legacy symbol and assert against whichever P0 landed; a task's failing test pins the expectation. On-disk artifacts keep stable `.vox/cache/graphify/<corpus>/` paths (umbrella §1.1 "renaming them is no-value churn").

---

## Key internals (verified against the code at HEAD — exact)

- **`crates/vox-graphify-reader/src/lib.rs`** — `GraphifyReader::from_value(value: serde_json::Value) -> Result<Self, GraphifyReaderError>` (line 77); `bfs_from_seeds(&self, seeds: &[&str], max_depth: u8, limit: usize) -> Vec<TraversalHit>` (line 165); `shortest_path(&self, from, to) -> Option<Vec<String>>` (line 172). The reader treats edges as **undirected**, reads `"links"` OR `"edges"`, and **ignores unknown fields/kinds** (`from_value` ~75–141) — so an overlay that adds fields elsewhere never breaks it. `pub mod` list at lines 13–26 (add `semantic_overlay` here).
- **`crates/vox-graphify-reader/src/rebuild.rs`** — `crate::graph_digest(bytes) -> String` is BLAKE3 (used line 358); the manifest writes `"graph_json_sha256": graph_digest` (line 375). **The overlay's freshness sha MUST be computed with the SAME `graph_digest` over the SAME canonical `graph.json` bytes** so a mismatch reliably means "structural core moved."
- **`crates/vox-config/src/graphify.rs`** — `GraphifyManifest` carries `graph_json_sha256: Option<String>` (line 91) and `lexical_ingest_sha256: Option<String>` (line 95) — the existing freshness/`lexical_lag` model the overlay mirrors. `load_graphify_corpora(repo_root)`, `GraphifyCorpus { id, graph_path, manifest_path, … }`, `assess_corpus_status(...)`, `GraphifyError::UnknownCorpus`.
- **`crates/vox-graphify-reader/src/coverage.rs`** — the honesty-firewall report pattern to mirror: `compute_coverage(graph: &Value, kind) -> CoverageReport` with `#[derive(Serialize)]` structs and a `json!`-fixture unit test. The overlay's reader follows this shape exactly.
- **`crates/vox-orchestrator-mcp/src/graphify_tools.rs`** — the handler pattern: `pub async fn graphify_query(state: &ServerState, params: GraphifyQueryParams) -> String`, params via `#[derive(Deserialize)]`, errors via `ToolResult::<Value>::err_with_remediation(msg, REM)`, success via `ToolResult::ok(json!({…})).to_json()`. `load_graph_json(repo_root, corpus)` (line 333), `resolve_search_corpus(&reg, &corpus, &intent)` (line 66), `knowledge_id(corpus_id, node_id)` (line 90). **Copy this file's structure for `semantic_overlay_tools.rs`.**
- **`crates/vox-orchestrator-mcp/src/dispatch.rs`** — the dispatch `match name { … }` (graphify arms lines 627–641). Add the new arm next to them: `"vox_search_semantic_related" => Ok(crate::semantic_overlay_tools::semantic_related(state, serde_json::from_value(args)?).await)`.
- **`crates/vox-orchestrator-mcp/src/input_schemas.rs`** — inline JSON-schema arms (graphify entries lines 471–483), each `parse_obj(r#"{…}"#, name)`. Add the `vox_search_semantic_related` arm here.
- **`crates/vox-actor-runtime/src/llm/embed.rs`** — `pub async fn llm_embed(options: &ActivityOptions, text: &str, config: LlmConfig) -> ActivityResult<Result<Vec<f32>, String>>` (line 14). This is the single embedding primitive; the overlay's CLI writer calls it (behind the `VOX_SEARCH_EMBED` flag), the MCP query path consumes the precomputed vectors P2 stores.
- **`contracts/retrieval/graphify-corpora.v1.yaml`** — corpus registry; `repo-code-graph` (default) graph at `.vox/cache/graphify/repo-code-graph/graph.json`, manifest alongside. The overlay file lands at `.vox/cache/graphify/<corpus>/semantic-overlay.json` (sibling of `graph.json`).
- **`crates/vox-cli/src/commands/graphify/mod.rs`** (renamed to `search/` by P0) — `enum GraphifyCmd { Status, Ingest, Rebuild, Index, Refresh, Gc, CrateMap, … }` + `pub async fn run(cmd, repo_root) -> anyhow::Result<()>`; helpers `load_all_corpora`, `corpus_by_id`. Add the `SemanticRelated` arm here.
- **Tests** — `crates/vox-graphify-reader/tests/*.rs` use tempdir + `json!`/string fixtures; reader unit tests live inline `#[cfg(test)] mod tests`. MCP handler tests use `ServerState::test_stub(...)` (see `graphify_tools.rs` lines 562–596 for the exact stub recipe).

---

## File Structure

**Created**
- `crates/vox-graphify-reader/src/semantic_overlay.rs` — `OverlayNode`, `OverlayRelation`, `SemanticOverlay` (the file model); `OverlayStaleness`; `overlay_freshness(overlay, graph_json_sha256) -> OverlayStaleness`; `read_overlay(path) -> Result<SemanticOverlay,_>`; `write_overlay(path, &SemanticOverlay)`; `cosine(a,b)`, `knn_over_overlay(overlay, query_vec, k, min_similarity) -> Vec<ScoredOverlayNode>`; `expand_seeds_structural(reader, seed_ids, hops, limit) -> Vec<MixedHit>` (mixed structural-expand). Pure, deterministic given inputs; no I/O beyond read/write helpers.
- `crates/vox-graphify-reader/tests/semantic_overlay.rs` — overlay round-trip, freshness-sha mismatch → stale, kNN ordering + min-similarity floor, mixed-expand labels structural vs semantic, overlay-never-touches-graph invariant.
- `crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs` — `SemanticRelatedParams`, `pub async fn semantic_related(state, params) -> String` (the `vox_search_semantic_related` handler).
- `crates/vox-cli/tests/` — (if a CLI integration harness exists) or inline arm test; otherwise the CLI arm is covered by a `--help` smoke + manual e2e step.

**Modified**
- `crates/vox-graphify-reader/src/lib.rs` — `pub mod semantic_overlay;`.
- `crates/vox-orchestrator-mcp/src/dispatch.rs` — one dispatch arm + module wire-in (`mod semantic_overlay_tools;`).
- `crates/vox-orchestrator-mcp/src/input_schemas.rs` — one schema arm.
- `crates/vox-orchestrator-mcp/src/lib.rs` (or `main.rs` mod list) — declare `mod semantic_overlay_tools;` if not auto-declared.
- `crates/vox-cli/src/commands/{graphify,search}/mod.rs` — `SemanticRelated` subcommand arm.
- `crates/vox-gui/ui/src/...` (P5 surface) — the **Related pane** is delivered by **P5**, not here; this plan only ships the tool it calls. (Self-Review §SR-7 notes the boundary.)

---

## Phase / batch map (workflow fan-out)

| Phase | Tasks | Tag | Depends on | Batch |
|---|---|---|---|---|
| **A — Overlay file model + freshness** | A1, A2, A3 | A1 `[SEQUENTIAL]` (anchor), A2/A3 `[PARALLEL-SAFE]` after A1 | P0 only | Batch-1 |
| **B — kNN + mixed structural-expand** | B1, B2 | `[PARALLEL-SAFE]` (after A1) | P0 only | Batch-1 |
| **C — Embedding-seed wiring** | C1 | `[SEQUENTIAL]` | **P2** (`GraphifyNodes` corpus) + A,B | Batch-2 |
| **D — MCP tool** | D1, D2, D3 | D1 `[SEQUENTIAL]`, D2/D3 `[PARALLEL-SAFE]` after D1 | A,B,C | Batch-3 |
| **E — CLI mirror + overlay writer** | E1, E2 | `[SEQUENTIAL]` | C, D | Batch-4 |
| **F — Honesty/e2e gates + docs** | F1, F2 | F1 `[PARALLEL-SAFE]`, F2 `[SEQUENTIAL]` (verification) | E | Batch-5 |

**Fan-out batches a workflow can dispatch in parallel:**
- **Batch-1 (after A1 lands):** `{A2, A3, B1, B2}` — all touch `semantic_overlay.rs` *additively* in disjoint functions + their own tests; safe to fan out across 4 sub-agents, each committing its function + test. (If the workflow serializes file writes, run A2→A3→B1→B2; they have no logic dependency on each other, only on A1's type definitions.)
- **Batch-3 (after D1 lands):** `{D2, D3}` — schema arm and dispatch arm in two different files.
- All other phases are sequential gates.

> **Note on Batch-1 file contention:** A2, A3, B1, B2 all append to the single new file `semantic_overlay.rs`. They are logically independent (different `pub fn`s) but share the file. The workflow MUST either (a) dispatch them serially against the file, or (b) dispatch in parallel and resolve the trivial append-merge. Each task is written to be self-contained (its own function + its own `#[cfg(test)]` test or test-file block) so a 4-way merge is mechanical. Tag remains `[PARALLEL-SAFE]` for logic; the orchestrator picks the IO strategy.

---

# Phase A — Overlay file model + freshness sha

## Task A1: `SemanticOverlay` file model + round-trip (TDD) `[SEQUENTIAL]`

This is the anchor task: it defines the types every later task references. It must land before Batch-1.

**Files:** Create `crates/vox-graphify-reader/src/semantic_overlay.rs`; modify `crates/vox-graphify-reader/src/lib.rs`; create `crates/vox-graphify-reader/tests/semantic_overlay.rs`.

- [ ] **Step 1: Declare the module** — add to `crates/vox-graphify-reader/src/lib.rs` after `pub mod rebuild;` (keep alpha-ish order, place after `reachability`):

```rust
pub mod semantic_overlay;
```

- [ ] **Step 2: Failing test** — create `crates/vox-graphify-reader/tests/semantic_overlay.rs`:

```rust
use vox_graphify_reader::semantic_overlay::{
    OverlayNode, OverlayRelation, SemanticOverlay,
};

#[test]
fn overlay_round_trips_through_json() {
    let overlay = SemanticOverlay {
        graph_json_sha256: "abc123".into(),
        model: "openai:text-embedding-3-small".into(),
        built_at: "2026-06-26T00:00:00+00:00".into(),
        nodes: vec![OverlayNode {
            node_id: "auth::login".into(),
            label: "login".into(),
            embedding: vec![0.1, 0.2, 0.3],
        }],
        relations: vec![OverlayRelation {
            source: "auth::login".into(),
            target: "auth.md".into(),
            relation: "implements".into(),
            model: "anthropic:claude-x".into(),
            confidence: 0.82,
            evidence: "AuthView renders the login flow described in auth.md".into(),
        }],
    };
    let json = serde_json::to_string(&overlay).unwrap();
    let back: SemanticOverlay = serde_json::from_str(&json).unwrap();
    assert_eq!(back.graph_json_sha256, "abc123");
    assert_eq!(back.nodes.len(), 1);
    assert_eq!(back.nodes[0].node_id, "auth::login");
    assert_eq!(back.relations[0].relation, "implements");
    assert!((back.relations[0].confidence - 0.82).abs() < 1e-6);
}
```

- [ ] **Step 3: Run, verify fail** — `git -C /c/Users/Owner/vox-graphify-gui` not needed; run: `cargo test -p vox-graphify-reader semantic_overlay` from the worktree. **Expected:** compile error (`semantic_overlay` module empty / types missing).

- [ ] **Step 4: Implement** — write the file model into `crates/vox-graphify-reader/src/semantic_overlay.rs`:

```rust
//! Layer 5 — Vox Search semantic overlay.
//!
//! A **physically separate** artifact (`semantic-overlay.json`) that references
//! structural node ids but is NEVER merged into `graph.json`. It carries the
//! structural `graph_json_sha256` it was built against; a mismatch marks it
//! stale (queries warn). Embeddings + LLM-labeled relations are non-deterministic
//! and provenance-labeled; the structural core is read-only to this layer.

use serde::{Deserialize, Serialize};

/// One overlay node: a structural node id + its embedding (the retrieval unit).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverlayNode {
    /// Structural node id this overlay node references (lives in `graph.json`).
    pub node_id: String,
    /// Human label copied for display; the overlay never owns structure.
    pub label: String,
    /// Embedding vector from the Vox Search `llm_embed` pipeline.
    pub embedding: Vec<f32>,
}

/// One LLM-asserted typed relation. Written ONLY to the overlay, never to `graph.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverlayRelation {
    pub source: String,
    pub target: String,
    /// e.g. "implements", "alternative_of", "described_by".
    pub relation: String,
    /// Model id that asserted the relation (provenance).
    pub model: String,
    /// LLM confidence in [0, 1].
    pub confidence: f32,
    /// Human-readable evidence string (why the relation was asserted).
    pub evidence: String,
}

/// The on-disk overlay artifact (`semantic-overlay.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticOverlay {
    /// The structural `graph.json` BLAKE3 digest this overlay was built against.
    /// A mismatch vs the live core marks the overlay stale.
    pub graph_json_sha256: String,
    /// Embedding model id used for `nodes[].embedding`.
    pub model: String,
    /// RFC3339 build timestamp.
    pub built_at: String,
    pub nodes: Vec<OverlayNode>,
    pub relations: Vec<OverlayRelation>,
}

/// Read an overlay from disk. Returns `Ok(None)` when the file is absent
/// (an overlay is optional — its absence is not an error).
pub fn read_overlay(path: &std::path::Path) -> Result<Option<SemanticOverlay>, String> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .map(Some)
            .map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Write an overlay to disk (pretty JSON; creates parent dirs).
pub fn write_overlay(path: &std::path::Path, overlay: &SemanticOverlay) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(overlay)
        .map_err(|e| format!("serialize overlay: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write {}: {e}", path.display()))
}
```

- [ ] **Step 5: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** `overlay_round_trips_through_json` PASSES; existing crate tests still green (`cargo test -p vox-graphify-reader`).

- [ ] **Step 6: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): semantic-overlay file model + round-trip (Layer 5)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task A2: Overlay freshness vs structural core sha (TDD) `[PARALLEL-SAFE]` (after A1)

The overlay is stale iff its `graph_json_sha256` differs from the live `graph.json` digest. Mirrors the `vox-config` `lexical_lag` model.

**Files:** Modify `crates/vox-graphify-reader/src/semantic_overlay.rs`; extend `tests/semantic_overlay.rs`.

- [ ] **Step 1: Failing test** — append to `tests/semantic_overlay.rs`:

```rust
use vox_graphify_reader::semantic_overlay::{overlay_freshness, OverlayStaleness};

#[test]
fn freshness_detects_sha_mismatch() {
    let mut overlay = SemanticOverlay {
        graph_json_sha256: "core-sha-OLD".into(),
        model: "m".into(),
        built_at: "t".into(),
        nodes: vec![],
        relations: vec![],
    };
    // Built against the same core sha → fresh.
    let fresh = overlay_freshness(&overlay, "core-sha-OLD");
    assert_eq!(fresh, OverlayStaleness::Fresh);
    // Core moved → stale with both shas reported.
    overlay.graph_json_sha256 = "core-sha-OLD".into();
    let stale = overlay_freshness(&overlay, "core-sha-NEW");
    match stale {
        OverlayStaleness::Stale { overlay_sha, core_sha } => {
            assert_eq!(overlay_sha, "core-sha-OLD");
            assert_eq!(core_sha, "core-sha-NEW");
        }
        OverlayStaleness::Fresh => panic!("expected stale"),
    }
    assert!(stale.is_stale());
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader freshness_detects_sha_mismatch`. **Expected:** compile error (`overlay_freshness`/`OverlayStaleness` missing).

- [ ] **Step 3: Implement** — append to `semantic_overlay.rs`:

```rust
/// Overlay staleness relative to the live structural-core digest.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OverlayStaleness {
    /// Overlay was built against the current `graph.json`.
    Fresh,
    /// The structural core moved since the overlay was built.
    Stale { overlay_sha: String, core_sha: String },
}

impl OverlayStaleness {
    pub fn is_stale(&self) -> bool {
        matches!(self, OverlayStaleness::Stale { .. })
    }
}

/// Compare the overlay's recorded core digest against the live `graph.json` digest.
/// `core_graph_json_sha256` is the BLAKE3 digest of the current `graph.json`
/// (compute with [`crate::graph_digest`] over the same canonical bytes the
/// rebuild manifest used — see `rebuild.rs` line ~358).
pub fn overlay_freshness(overlay: &SemanticOverlay, core_graph_json_sha256: &str) -> OverlayStaleness {
    if overlay.graph_json_sha256 == core_graph_json_sha256 {
        OverlayStaleness::Fresh
    } else {
        OverlayStaleness::Stale {
            overlay_sha: overlay.graph_json_sha256.clone(),
            core_sha: core_graph_json_sha256.to_string(),
        }
    }
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** all overlay tests PASS.

- [ ] **Step 5: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): overlay freshness vs structural-core sha

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task A3: Overlay-never-mutates-graph invariant test (TDD) `[PARALLEL-SAFE]` (after A1)

Locks the non-negotiable honesty invariant (umbrella §2.5.1) as an executable regression: building/reading/writing an overlay leaves `graph.json` byte-identical.

**Files:** Extend `crates/vox-graphify-reader/tests/semantic_overlay.rs`.

- [ ] **Step 1: Failing-then-passing test** — append:

```rust
#[test]
fn overlay_io_never_touches_graph_json() {
    let tmp = tempfile::tempdir().unwrap();
    let graph_path = tmp.path().join("graph.json");
    let graph_bytes =
        br#"{"nodes":[{"id":"auth::login","label":"login","kind":"fn"}],"links":[]}"#;
    std::fs::write(&graph_path, graph_bytes).unwrap();
    let before = std::fs::read(&graph_path).unwrap();

    let overlay = SemanticOverlay {
        graph_json_sha256: "x".into(),
        model: "m".into(),
        built_at: "t".into(),
        nodes: vec![OverlayNode {
            node_id: "auth::login".into(),
            label: "login".into(),
            embedding: vec![0.5],
        }],
        relations: vec![],
    };
    let overlay_path = tmp.path().join("semantic-overlay.json");
    vox_graphify_reader::semantic_overlay::write_overlay(&overlay_path, &overlay).unwrap();
    let _read = vox_graphify_reader::semantic_overlay::read_overlay(&overlay_path)
        .unwrap()
        .unwrap();

    // The structural artifact is byte-identical; the overlay is a sibling file.
    let after = std::fs::read(&graph_path).unwrap();
    assert_eq!(before, after, "overlay I/O must never mutate graph.json");
    assert!(overlay_path.exists());
    assert_ne!(overlay_path, graph_path);
}
```

(`tempfile` is already a dev-dependency of the crate — see `Cargo.toml`.)

- [ ] **Step 2: Run** — `cargo test -p vox-graphify-reader overlay_io_never_touches_graph_json`. **Expected:** PASS.

- [ ] **Step 3: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search): overlay I/O never mutates graph.json (honesty invariant)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

# Phase B — kNN + mixed structural-expand

## Task B1: cosine kNN over the overlay node corpus (TDD) `[PARALLEL-SAFE]` (after A1)

The "find things related to X" capability: embedding kNN over `overlay.nodes`, with a `min_similarity` floor. Deterministic given a fixed query vector.

**Files:** Modify `crates/vox-graphify-reader/src/semantic_overlay.rs`; extend `tests/semantic_overlay.rs`.

- [ ] **Step 1: Failing test** — append to `tests/semantic_overlay.rs`:

```rust
use vox_graphify_reader::semantic_overlay::{knn_over_overlay, ScoredOverlayNode};

#[test]
fn knn_ranks_by_cosine_and_applies_floor() {
    let overlay = SemanticOverlay {
        graph_json_sha256: "x".into(),
        model: "m".into(),
        built_at: "t".into(),
        nodes: vec![
            OverlayNode { node_id: "near".into(),  label: "near".into(),  embedding: vec![1.0, 0.0] },
            OverlayNode { node_id: "mid".into(),   label: "mid".into(),   embedding: vec![0.7071, 0.7071] },
            OverlayNode { node_id: "far".into(),   label: "far".into(),   embedding: vec![0.0, 1.0] },
        ],
        relations: vec![],
    };
    let query = vec![1.0_f32, 0.0];
    let hits: Vec<ScoredOverlayNode> = knn_over_overlay(&overlay, &query, 3, 0.5);
    // floor 0.5 drops "far" (cos = 0.0); order is near (1.0) then mid (~0.707).
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].node_id, "near");
    assert!((hits[0].similarity - 1.0).abs() < 1e-4);
    assert_eq!(hits[1].node_id, "mid");
    assert!(hits[1].similarity > 0.70 && hits[1].similarity < 0.71);
    // k truncation: k=1 keeps only the top.
    let top1 = knn_over_overlay(&overlay, &query, 1, 0.0);
    assert_eq!(top1.len(), 1);
    assert_eq!(top1[0].node_id, "near");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader knn_ranks_by_cosine_and_applies_floor`. **Expected:** compile error (`knn_over_overlay`/`ScoredOverlayNode` missing).

- [ ] **Step 3: Implement** — append to `semantic_overlay.rs`:

```rust
/// One scored kNN hit. Every hit is `layer: "semantic"` provenance at the tool boundary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScoredOverlayNode {
    pub node_id: String,
    pub label: String,
    pub similarity: f32,
}

/// Cosine similarity. Returns 0.0 on a zero-norm vector (never NaN) so ranking is total.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// kNN over the overlay node corpus: cosine vs `query_vec`, drop below
/// `min_similarity`, sort desc, truncate to `k`. Deterministic given inputs.
pub fn knn_over_overlay(
    overlay: &SemanticOverlay,
    query_vec: &[f32],
    k: usize,
    min_similarity: f32,
) -> Vec<ScoredOverlayNode> {
    let mut scored: Vec<ScoredOverlayNode> = overlay
        .nodes
        .iter()
        .map(|n| ScoredOverlayNode {
            node_id: n.node_id.clone(),
            label: n.label.clone(),
            similarity: cosine(query_vec, &n.embedding),
        })
        .filter(|s| s.similarity >= min_similarity)
        .collect();
    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            // tie-break on node_id for byte-stable ordering
            .then_with(|| a.node_id.cmp(&b.node_id))
    });
    scored.truncate(k);
    scored
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** PASS.

- [ ] **Step 5: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): cosine kNN over overlay node corpus (semantic related)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task B2: mixed seed-then-structural-expand (TDD) `[PARALLEL-SAFE]` (after A1)

"What's the auth flow?" — semantic seeds (Phase C/D inject them), then **1–2-hop structural expansion over `graph.json`** to ground them. Seeds are labeled `semantic`; expanded nodes are labeled `structural` (umbrella §2.4: "the seeds are guesses, the connective tissue is ground truth").

**Files:** Modify `crates/vox-graphify-reader/src/semantic_overlay.rs`; extend `tests/semantic_overlay.rs`.

- [ ] **Step 1: Failing test** — append:

```rust
use vox_graphify_reader::semantic_overlay::{expand_seeds_structural, MixedHit};
use vox_graphify_reader::GraphifyReader;

#[test]
fn mixed_expand_labels_seeds_semantic_and_neighbors_structural() {
    // Structural graph: auth::login -> auth::session -> auth::cookie
    let graph = serde_json::json!({
        "nodes": [
            {"id":"auth::login","label":"login","kind":"fn"},
            {"id":"auth::session","label":"session","kind":"fn"},
            {"id":"auth::cookie","label":"cookie","kind":"fn"}
        ],
        "links": [
            {"source":"auth::login","target":"auth::session"},
            {"source":"auth::session","target":"auth::cookie"}
        ]
    });
    let reader = GraphifyReader::from_value(graph).unwrap();
    let seeds = vec!["auth::login".to_string()];
    let hits: Vec<MixedHit> = expand_seeds_structural(&reader, &seeds, 2, 20);

    let seed = hits.iter().find(|h| h.node_id == "auth::login").unwrap();
    assert_eq!(seed.layer, "semantic", "seed must be labeled semantic (a guess)");
    let neighbor = hits.iter().find(|h| h.node_id == "auth::session").unwrap();
    assert_eq!(neighbor.layer, "structural", "expanded node is ground truth");
    assert!(hits.iter().any(|h| h.node_id == "auth::cookie"), "2-hop reached");
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader mixed_expand_labels_seeds_semantic`. **Expected:** compile error (`expand_seeds_structural`/`MixedHit` missing).

- [ ] **Step 3: Implement** — append to `semantic_overlay.rs`:

```rust
use crate::GraphifyReader;

/// One node in a mixed semantic-seed + structural-expand result.
/// `layer` is the provenance: `"semantic"` for a seed (an embedding guess),
/// `"structural"` for a node reached by deterministic BFS over `graph.json`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MixedHit {
    pub node_id: String,
    pub label: String,
    /// "semantic" | "structural"
    pub layer: String,
    /// Hop distance from the nearest seed (0 for seeds).
    pub hops: u8,
}

/// Ground fuzzy semantic seeds in the deterministic structural graph: emit each
/// seed (layer `"semantic"`, hops 0) plus its BFS frontier to `hops`
/// (layer `"structural"`). Reuses [`GraphifyReader::bfs_from_seeds`].
///
/// The structural expansion is byte-reproducible; only the seed list is
/// non-deterministic (it comes from the embedding lane upstream).
pub fn expand_seeds_structural(
    reader: &GraphifyReader,
    seed_ids: &[String],
    hops: u8,
    limit: usize,
) -> Vec<MixedHit> {
    let mut out: Vec<MixedHit> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for sid in seed_ids {
        if seen.insert(sid.clone()) {
            // label copied from the reader if available; fall back to the id.
            out.push(MixedHit {
                node_id: sid.clone(),
                label: reader.label_of(sid).unwrap_or_else(|| sid.clone()),
                layer: "semantic".to_string(),
                hops: 0,
            });
        }
    }

    let seed_refs: Vec<&str> = seed_ids.iter().map(String::as_str).collect();
    for hit in reader.bfs_from_seeds(&seed_refs, hops, limit) {
        if seen.insert(hit.node_id.clone()) {
            out.push(MixedHit {
                node_id: hit.node_id,
                label: hit.label,
                layer: "structural".to_string(),
                hops: hit.depth,
            });
        }
    }
    out
}
```

> **Reader API check (Task B2 sub-step):** `bfs_from_seeds` returns `TraversalHit` with `node_id`, `label`, `depth` fields (confirmed in `lib.rs` line 165 + `graphify_tools.rs` line 388 which reads `h.node_id`, `h.label`, `h.depth`). **`label_of` may not exist** — if the reader has no `label_of`, the implementing agent adds a one-line `pub fn label_of(&self, id: &str) -> Option<String>` to `lib.rs` returning the node's label from the existing internal node map (the same map `bfs_from_seeds` uses), with its own unit test; OR falls back to `reader.bfs_from_seeds(&[sid], 0, 1)` to recover the seed's label. The failing test in Step 1 forces this resolution. Verify with: `grep -n "label_of\|fn label\|struct TraversalHit\|pub label" crates/vox-graphify-reader/src/lib.rs` before implementing.

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** PASS.

- [ ] **Step 5: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/src/lib.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): mixed semantic-seed + structural-expand (layer-labeled)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

# Phase C — Embedding-seed wiring (depends on P2 `GraphifyNodes`)

## Task C1: embed the query via `llm_embed`, seed from `GraphifyNodes` (TDD) `[SEQUENTIAL]`

> **BLOCKS ON P2.** This task consumes the `SearchCorpus::GraphifyNodes` embedding seam P2 builds. If P2 has not landed, the workflow holds this task; Phases A/B/D (with a `query_vec`-injected handler) can ship a working tool whose embedding lane is gated. Verify P2 first: `grep -rn "GraphifyNodes" crates/ --include=*.rs` must return the enum variant + the corpus embed path.

**Files:** Modify `crates/vox-graphify-reader/src/semantic_overlay.rs` (add a pure `seed_node_ids_from_scored(&[ScoredOverlayNode], usize) -> Vec<String>` helper) + a thin embed adapter in the MCP crate (Phase D consumes it). Embedding I/O stays out of the reader crate (it has no async/network deps) — the reader only does vector math; the `llm_embed` call lives in the MCP handler (Task D1).

- [ ] **Step 1: Failing test** — append to `tests/semantic_overlay.rs`:

```rust
use vox_graphify_reader::semantic_overlay::seed_node_ids_from_scored;

#[test]
fn seed_ids_takes_top_n_in_rank_order() {
    let scored = vec![
        ScoredOverlayNode { node_id: "a".into(), label: "a".into(), similarity: 0.9 },
        ScoredOverlayNode { node_id: "b".into(), label: "b".into(), similarity: 0.8 },
        ScoredOverlayNode { node_id: "c".into(), label: "c".into(), similarity: 0.7 },
    ];
    assert_eq!(seed_node_ids_from_scored(&scored, 2), vec!["a", "b"]);
    assert_eq!(seed_node_ids_from_scored(&scored, 10), vec!["a", "b", "c"]);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader seed_ids_takes_top_n`. **Expected:** compile error.

- [ ] **Step 3: Implement** — append to `semantic_overlay.rs`:

```rust
/// Take the top-`n` overlay node ids (already rank-sorted) to use as
/// structural-expansion seeds. Pure; the embedding/kNN that produced
/// `scored` happens upstream (the MCP handler embeds the query via `llm_embed`).
pub fn seed_node_ids_from_scored(scored: &[ScoredOverlayNode], n: usize) -> Vec<String> {
    scored.iter().take(n).map(|s| s.node_id.clone()).collect()
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** PASS.

- [ ] **Step 5: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): seed-from-scored helper for mixed expand (P2 GraphifyNodes seam)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

# Phase D — MCP tool `vox_search_semantic_related`

## Task D1: handler `semantic_related` (TDD) `[SEQUENTIAL]`

The thin adapter (umbrella §3.1 row; design §3): load the corpus graph + sibling overlay; assess overlay freshness vs the live core sha; embed the query (via injected vector in unit tests, `llm_embed` in prod); kNN → seeds → mixed structural-expand; return layer-labeled hits + `stale`.

**Files:** Create `crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs`; modify the MCP crate mod list (`lib.rs`/`main.rs`).

- [ ] **Step 1: Wire module** — add `mod semantic_overlay_tools;` to the MCP crate root (find it: `grep -n "mod graphify_tools" crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/src/main.rs` and add the new `mod` line beside it).

- [ ] **Step 2: Failing test** — create `crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs` with the handler skeleton + a `#[cfg(test)]` test that mirrors `graphify_tools.rs` test recipe (write registry + graph + overlay into a tempdir, call the handler with a query whose embedding is injected via the test param, assert `success:true`, `layer` labels present, `stale:false`):

```rust
//! `vox_search_semantic_related` — Layer 5 semantic overlay query tool.
//!
//! Thin adapter: kNN over the corpus overlay → top-N seeds → structural BFS
//! expand (ground truth). Every hit is provenance-labeled; the response carries
//! `stale` from the overlay-vs-core sha check. The overlay is read-only here and
//! is NEVER written back into `graph.json`.

use serde::Deserialize;
use std::fs;

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_config::graphify::load_graphify_corpora;
use vox_graphify_reader::semantic_overlay::{
    expand_seeds_structural, knn_over_overlay, overlay_freshness, read_overlay,
    seed_node_ids_from_scored,
};
use vox_graphify_reader::GraphifyReader;

const REM: &str =
    "Build the overlay with `vox search semantic-related --rebuild` (writes semantic-overlay.json beside graph.json).";

#[derive(Debug, Deserialize)]
pub struct SemanticRelatedParams {
    /// Corpus id; omit for the default corpus.
    pub corpus: Option<String>,
    /// Free-text query (embedded via llm_embed in prod).
    pub query: String,
    /// kNN k for seed selection (default 8).
    pub k: Option<usize>,
    /// Minimum cosine similarity floor (default 0.25).
    pub min_similarity: Option<f32>,
    /// Structural expansion hops from seeds (default 1, max 2).
    pub hops: Option<u8>,
    /// TEST-ONLY: inject a query embedding to bypass the network. Never set in prod.
    #[serde(default)]
    pub query_embedding: Option<Vec<f32>>,
}

pub async fn semantic_related(state: &ServerState, params: SemanticRelatedParams) -> String {
    let repo_root = &state.repository.root;
    let reg = match load_graphify_corpora(repo_root) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM).to_json(),
    };
    let corpus_id = params
        .corpus
        .clone()
        .unwrap_or_else(|| reg.default_corpus_id.clone());
    let corpus = match reg.corpora.iter().find(|c| c.id == corpus_id) {
        Some(c) => c,
        None => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("unknown corpus: {corpus_id}"), REM).to_json(),
    };

    // Load structural graph (read-only) for the BFS expand + freshness sha.
    let graph_path = repo_root.join(&corpus.graph_path);
    let graph_bytes = match fs::read(&graph_path) {
        Ok(b) => b,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("read {}: {e}", graph_path.display()), REM).to_json(),
    };
    let core_sha = vox_graphify_reader::graph_digest(&graph_bytes);
    let graph: serde_json::Value = match serde_json::from_slice(&graph_bytes) {
        Ok(v) => v,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("parse {}: {e}", graph_path.display()), REM).to_json(),
    };
    let reader = match GraphifyReader::from_value(graph) {
        Ok(r) => r,
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e.to_string(), REM).to_json(),
    };

    // Load the sibling overlay (semantic-overlay.json next to graph.json).
    let overlay_path = graph_path.with_file_name("semantic-overlay.json");
    let overlay = match read_overlay(&overlay_path) {
        Ok(Some(o)) => o,
        Ok(None) => return ToolResult::<serde_json::Value>::err_with_remediation(
            format!("no overlay for corpus '{corpus_id}' (expected {})", overlay_path.display()), REM).to_json(),
        Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM).to_json(),
    };
    let staleness = overlay_freshness(&overlay, &core_sha);

    // Embed the query: test injection, else llm_embed (P2 GraphifyNodes model config).
    let query_vec = match &params.query_embedding {
        Some(v) => v.clone(),
        None => match embed_query(state, &overlay.model, &params.query).await {
            Ok(v) => v,
            Err(e) => return ToolResult::<serde_json::Value>::err_with_remediation(e, REM).to_json(),
        },
    };

    let k = params.k.unwrap_or(8).max(1);
    let min_sim = params.min_similarity.unwrap_or(0.25);
    let hops = params.hops.unwrap_or(1).min(2);
    let scored = knn_over_overlay(&overlay, &query_vec, k, min_sim);
    let seeds = seed_node_ids_from_scored(&scored, k);
    let mixed = expand_seeds_structural(&reader, &seeds, hops, 30);

    let related: Vec<serde_json::Value> = scored
        .iter()
        .map(|s| serde_json::json!({
            "node_id": s.node_id, "label": s.label,
            "similarity": s.similarity, "layer": "semantic", "source": overlay.model,
        }))
        .collect();
    let expanded: Vec<serde_json::Value> = mixed
        .iter()
        .map(|m| serde_json::json!({
            "node_id": m.node_id, "label": m.label, "layer": m.layer, "hops": m.hops,
        }))
        .collect();

    ToolResult::ok(serde_json::json!({
        "corpus_id": corpus_id,
        "layer": "semantic",
        "stale": staleness.is_stale(),
        "staleness": staleness,
        "related": related,
        "expanded": expanded,
    }))
    .to_json()
}

/// Embed the query via the Vox Search `llm_embed` pipeline (P2-owned model config).
async fn embed_query(_state: &ServerState, model: &str, query: &str) -> Result<Vec<f32>, String> {
    // model id format "provider:model" recorded in the overlay.
    let (provider, model_name) = model
        .split_once(':')
        .map(|(p, m)| (p.to_string(), m.to_string()))
        .unwrap_or_else(|| ("openrouter".to_string(), model.to_string()));
    let config = vox_actor_runtime::llm::types::LlmConfig {
        provider,
        model: model_name,
        ..Default::default()
    };
    let opts = vox_actor_runtime::ActivityOptions::default();
    match vox_actor_runtime::llm::embed::llm_embed(&opts, query, config).await {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(format!("embed failed: {e}")),
        Err(e) => Err(format!("embed activity failed: {e}")),
    }
}
```

> **Dependency check (D1 sub-step):** verify `vox-actor-runtime` is (or can be added as) a dependency of `vox-orchestrator-mcp`: `grep -n "vox-actor-runtime" crates/vox-orchestrator-mcp/Cargo.toml`. If absent, add `vox-actor-runtime = { workspace = true }` under `[dependencies]` and include that file in the commit. Verify `LlmConfig`/`ActivityOptions`/`embed::llm_embed` paths with `grep -rn "pub struct LlmConfig\|pub struct ActivityOptions\|pub mod embed" crates/vox-actor-runtime/src/` and adjust the `use` paths to whatever P0/P2 left (the failing unit test, which injects `query_embedding`, does not exercise `embed_query`, so the network path is compile-checked only — keep it behind the test injection).

- [ ] **Step 3: Run, verify fail then pass** — `cargo test -p vox-orchestrator-mcp semantic_related`. Iterate to GREEN (the unit test uses `query_embedding` injection + a tempdir overlay, so no network).

- [ ] **Step 4: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/Cargo.toml
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): vox_search_semantic_related handler (kNN seeds -> structural expand)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task D2: dispatch arm (TDD) `[PARALLEL-SAFE]` (after D1)

**Files:** Modify `crates/vox-orchestrator-mcp/src/dispatch.rs`.

- [ ] **Step 1: Implement** — add next to the graphify arms (after line ~640):

```rust
        "vox_search_semantic_related" => Ok(crate::semantic_overlay_tools::semantic_related(
            state,
            serde_json::from_value(args)?,
        )
        .await),
```

- [ ] **Step 2: Failing-then-passing test** — add a dispatch-level test (mirror the existing dispatch test style in that file, or assert routing): call `handle_tool_call(&state, "vox_search_semantic_related", json!({"query":"auth","query_embedding":[1.0]}))` against a tempdir with overlay → `success:true`. Run `cargo test -p vox-orchestrator-mcp vox_search_semantic_related`. **Expected:** PASS.

- [ ] **Step 3: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/dispatch.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): dispatch arm for vox_search_semantic_related

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task D3: input schema arm (TDD) `[PARALLEL-SAFE]` (after D1)

**Files:** Modify `crates/vox-orchestrator-mcp/src/input_schemas.rs`.

- [ ] **Step 1: Implement** — add beside the graphify schema arms (~line 483):

```rust
        "vox_search_semantic_related" => parse_obj(
            r#"{"type":"object","properties":{"corpus":{"type":"string","description":"Corpus id from contracts/retrieval/graphify-corpora.v1.yaml; omit for default"},"query":{"type":"string","minLength":1,"description":"Free-text query; embedded over the semantic overlay node corpus"},"k":{"type":"integer","minimum":1,"description":"kNN k for seed selection (default 8)"},"min_similarity":{"type":"number","description":"Cosine similarity floor (default 0.25)"},"hops":{"type":"integer","minimum":0,"maximum":2,"description":"Structural-expand hops from seeds (default 1, max 2)"}},"required":["query"],"additionalProperties":false}"#,
            name,
        ),
```

(Note: `query_embedding` is intentionally **omitted** from the public schema — it is test-only and `additionalProperties:false` would reject it from real agents, which is correct.)

- [ ] **Step 2: Test** — if `input_schemas.rs` has a "every dispatch arm has a schema" parity test (common in this repo), run it: `cargo test -p vox-orchestrator-mcp schema`. Otherwise add a focused test asserting `input_schema_for("vox_search_semantic_related")` returns an object with a required `query`. Run, **expect PASS**.

- [ ] **Step 3: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/input_schemas.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): input schema for vox_search_semantic_related

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

# Phase E — CLI mirror + overlay writer

## Task E1: overlay writer (`--rebuild`) behind `VOX_SEARCH_EMBED` flag (TDD) `[SEQUENTIAL]`

Build `semantic-overlay.json` from a corpus's `graph.json`: select embeddable nodes (`fn`/`struct`/`cmd:`/`tool:`/`surface:` with a label), embed each via `llm_embed`, stamp the core sha, write the sibling file. Embedding lane is **behind a flag** (umbrella §2.4 / §2.6 "embedding lane behind a flag"); without the flag the writer emits an empty-embedding overlay (relations-only / metadata) so the deterministic plumbing is testable offline.

**Files:** Modify `crates/vox-graphify-reader/src/semantic_overlay.rs` (add `build_overlay_skeleton(graph, model, core_sha) -> SemanticOverlay` — pure, no network); the live embedding loop lives in the CLI arm (Task E2) which has the async runtime.

- [ ] **Step 1: Failing test** — append to `tests/semantic_overlay.rs`:

```rust
use vox_graphify_reader::semantic_overlay::build_overlay_skeleton;

#[test]
fn skeleton_selects_labeled_code_nodes_and_stamps_sha() {
    let graph = serde_json::json!({
        "nodes": [
            {"id":"a::f","label":"f","kind":"fn"},
            {"id":"S","label":"S","kind":"struct"},
            {"id":"x","kind":"fn"},                       // no label -> skipped
            {"id":"c_0","label":"community","kind":"community"} // non-code -> skipped
        ],
        "links": []
    });
    let ov = build_overlay_skeleton(&graph, "openai:text-embedding-3-small", "CORE_SHA");
    assert_eq!(ov.graph_json_sha256, "CORE_SHA");
    assert_eq!(ov.model, "openai:text-embedding-3-small");
    let ids: Vec<&str> = ov.nodes.iter().map(|n| n.node_id.as_str()).collect();
    assert!(ids.contains(&"a::f"));
    assert!(ids.contains(&"S"));
    assert!(!ids.contains(&"x"));    // unlabeled dropped
    assert!(!ids.contains(&"c_0"));  // non-code dropped
    // skeleton has empty embeddings (filled by the embed loop in the CLI arm)
    assert!(ov.nodes.iter().all(|n| n.embedding.is_empty()));
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p vox-graphify-reader skeleton_selects_labeled`. **Expected:** compile error.

- [ ] **Step 3: Implement** — append to `semantic_overlay.rs`:

```rust
/// Node kinds whose label is a meaningful retrieval unit for the overlay.
const EMBEDDABLE_KINDS: &[&str] = &["fn", "struct", "command", "tool", "surface", "cli"];

/// Build an overlay skeleton (node ids + labels + core sha, empty embeddings).
/// The CLI `--rebuild` arm fills `embedding` via `llm_embed` when `VOX_SEARCH_EMBED`
/// is set; this pure step keeps the selection logic unit-testable offline.
pub fn build_overlay_skeleton(
    graph: &serde_json::Value,
    model: &str,
    core_graph_json_sha256: &str,
) -> SemanticOverlay {
    let nodes = graph
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|n| {
            let id = n.get("id").and_then(|v| v.as_str())?;
            let label = n.get("label").and_then(|v| v.as_str())?;
            let kind = n.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            // accept "fn"/"struct"/... and prefixed ids like "cmd:"/"tool:"/"surface:"/"cli:"
            let kind_ok = EMBEDDABLE_KINDS.contains(&kind)
                || id.starts_with("cmd:")
                || id.starts_with("tool:")
                || id.starts_with("surface:")
                || id.starts_with("cli:");
            if !kind_ok {
                return None;
            }
            Some(OverlayNode {
                node_id: id.to_string(),
                label: label.to_string(),
                embedding: Vec::new(),
            })
        })
        .collect();
    SemanticOverlay {
        graph_json_sha256: core_graph_json_sha256.to_string(),
        model: model.to_string(),
        built_at: String::new(), // CLI stamps RFC3339 at write time
        nodes,
        relations: Vec::new(),
    }
}
```

- [ ] **Step 4: Run tests** — `cargo test -p vox-graphify-reader semantic_overlay`. **Expected:** PASS.

- [ ] **Step 5: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/semantic_overlay.rs crates/vox-graphify-reader/tests/semantic_overlay.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): overlay skeleton builder (embeddable-node selection)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task E2: `vox search semantic-related` CLI arm + `--rebuild` `[SEQUENTIAL]`

The CLI mirror (umbrella §1.1 verb `vox search semantic-related`; design §3). Two modes: **query** (print kNN + mixed expand JSON) and `--rebuild` (build + write the overlay; live-embed only when `VOX_SEARCH_EMBED` is set).

**Files:** Modify the renamed CLI group `crates/vox-cli/src/commands/search/mod.rs` (or `graphify/mod.rs` if P0 kept the dir name). Verify first: `ls crates/vox-cli/src/commands/ | grep -iE 'search|graphify'` and `grep -n "enum .*Cmd" crates/vox-cli/src/commands/{search,graphify}/mod.rs`.

- [ ] **Step 1: Add the subcommand variant** — extend the command enum:

```rust
    /// Semantic overlay: find nodes related to QUERY, grounded by structural expand.
    SemanticRelated {
        /// Corpus id (omit for default).
        #[arg(long)]
        corpus: Option<String>,
        /// Free-text query (ignored with --rebuild).
        #[arg(default_value = "")]
        query: String,
        /// (Re)build semantic-overlay.json from the corpus graph instead of querying.
        #[arg(long)]
        rebuild: bool,
        #[arg(long, default_value_t = 8)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        hops: u8,
    },
```

- [ ] **Step 2: Add the `run` arm** — handle both modes. Rebuild: read `graph.json`, `core_sha = graph_digest(bytes)`, `build_overlay_skeleton`, then **iff `std::env::var("VOX_SEARCH_EMBED").is_ok()`** loop `llm_embed` per node filling `embedding`, stamp `built_at = Utc::now().to_rfc3339()`, `write_overlay(overlay_path, &ov)`. Query: `read_overlay`, embed query via `llm_embed`, `knn_over_overlay` → `seed_node_ids_from_scored` → `expand_seeds_structural`, print `serde_json::to_string_pretty`. Reuse the corpus-resolution helpers already in the file (`load_all_corpora` / `corpus_by_id`). Match the existing arms' error handling (`anyhow::Result`, `.context(...)`).

- [ ] **Step 3: Build + smoke** — `cargo build -p vox-cli` then `cargo run -p vox-cli -- search semantic-related --help` (or `vox graphify semantic-related --help` if the alias path is what P0 shipped). **Expected:** help text lists `--corpus`, `--rebuild`, `--k`, `--hops`. (Full e2e with live embeddings is the manual gate in F2; offline `--rebuild` without `VOX_SEARCH_EMBED` produces an empty-embedding overlay file and exits 0.)

- [ ] **Step 4: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(vox-search): vox search semantic-related CLI (query + --rebuild)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

# Phase F — Honesty gate + e2e verification + docs

## Task F1: overlay-vs-core honesty regression test `[PARALLEL-SAFE]`

A permanent gate (umbrella §7) proving the overlay query **warns/stamps stale** when the core moves and **never** writes overlay data into `graph.json`. End-to-end through the MCP handler with a deliberately stale overlay.

**Files:** Extend `crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs` `#[cfg(test)]`.

- [ ] **Step 1: Test** — write a tempdir corpus where `semantic-overlay.json` carries `graph_json_sha256: "STALE"` but the on-disk `graph.json` digests to something else; call `semantic_related(query_embedding injected)`; assert `data.stale == true`, `data.staleness.state == "stale"`, and assert the on-disk `graph.json` bytes are unchanged after the call. Run `cargo test -p vox-orchestrator-mcp overlay_stale`. **Expected:** PASS.

- [ ] **Step 2: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/semantic_overlay_tools.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(vox-search): overlay staleness stamped + graph.json untouched (honesty gate)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

## Task F2: full verification + design doc cross-link `[SEQUENTIAL]`

**Files:** Modify `docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md` (status note) and `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` (mark P3 in-progress/landed in §9 table).

- [ ] **Step 1: Full crate verification** (evidence before claims, per `superpowers:verification-before-completion`):
  - `cargo test -p vox-graphify-reader` → all green (record count).
  - `cargo test -p vox-orchestrator-mcp semantic` → green.
  - `cargo build -p vox-cli` → success; `cargo run -p vox-cli -- search semantic-related --help` → help shown.
  - `cargo clippy -p vox-graphify-reader -p vox-orchestrator-mcp --lib` → no new warnings (do NOT run `--all-targets` on `vox-gui`; see MEMORY note on the Tauri build-script clippy gotcha).
  - Paste the actual command outputs into the task notes (not "looks good").
- [ ] **Step 2: Update the design-doc status** — in `2026-06-26-graphify-dataflow-semantic-overlay-design.md`, add a top-of-§2 note: `> **Status: IMPLEMENTED** as Vox Search Layer 5 — see plan 2026-06-26-vox-search-semantic-overlay.md.` In the umbrella §9 P3 row, append ` *(landed)*`.
- [ ] **Step 3: Commit** —
```
git -C /c/Users/Owner/vox-graphify-gui add docs/superpowers/specs/2026-06-26-graphify-dataflow-semantic-overlay-design.md docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md
git -C /c/Users/Owner/vox-graphify-gui commit -m "docs(vox-search): mark semantic overlay (Layer 5 / P3) implemented

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review — spec coverage

Mapping every spec requirement to the task that satisfies it. Source: design §2 + umbrella §2.4/§2.5/§3.1/§4.1/§6/§9.

| # | Spec requirement (source) | Covered by | Notes |
|---|---|---|---|
| SR-1 | **`SearchCorpus::GraphifyNodes` embeddings, reuse EmbeddingService/Qdrant/`llm_embed`** (umbrella §2.4; design §2.2) | C1 + D1 `embed_query` | Embedding lane rides on `llm_embed` (no second stack); `GraphifyNodes` corpus is **P2's** deliverable (dependency stated at top). Live-embed gated behind `VOX_SEARCH_EMBED` (E1/E2). |
| SR-2 | **Separate `semantic-overlay.json`, never mutates `graph.json`** (design §2.1/§2.3; umbrella §2.5.1) | A1 (sibling file), A3 + F1 (executable invariant) | Two regression tests assert byte-identical `graph.json` across overlay I/O and live query. |
| SR-3 | **LLM-labeled typed relations `{relation, model, confidence, evidence}`** (design §2.3) | A1 `OverlayRelation` | Exact field set; written only to overlay, never promoted (model field = provenance). Relation *production* (offline batch) is a follow-on; the schema + storage + read path ship here. |
| SR-4 | **`vox_search_semantic_related` tool** (umbrella §3.1 row; design §3) | D1 (handler) + D2 (dispatch) + D3 (schema) | Output: `related[]` (semantic kNN) + `expanded[]` (mixed) + `stale`. |
| SR-5 | **`layer: structural\|semantic` tagging** (umbrella §2.5.3; design §2.1) | B2 `MixedHit.layer` + D1 response | Seeds → `semantic`; BFS frontier → `structural`; tool envelope `layer:"semantic"`. |
| SR-6 | **Freshness sha vs structural core** (design §2.3; umbrella §2.4) | A2 (`overlay_freshness`) + D1 (`core_sha` via `graph_digest`) + F1 | Uses the **same** `graph_digest` BLAKE3 as the rebuild manifest's `graph_json_sha256`; mismatch → `stale:true` + both shas. |
| SR-7 | **GUI overlay staleness warn** (umbrella §4.1 Related pane) | (delivered by **P5**) — this plan ships the `stale`/`staleness` fields the pane renders | Boundary stated in File Structure; the tool carries everything the pane needs. |
| SR-8 | **Mixed seed-then-structural-expand ("what's the auth flow")** (design §2.4; umbrella §2.4) | B2 + D1 | Semantic seeds, deterministic 1–2-hop ground truth. |
| SR-9 | **Honesty: deterministic structural expand, non-det seeds, provenance-labeled, regenerated-not-trusted, drop on sha mismatch** (design §2.1; umbrella §2.5, §7) | A2/A3/B2/F1 | Overlay is regenerated by `--rebuild`; query stamps stale, never silently serves a mismatched overlay (warns + flag). |
| SR-10 | **Depends on vs1 (P0 structural) + vs3 (P2 corpus hook)** (umbrella §9 DAG `P0→P2→P3`) | Cross-plan deps section + C1/E gating | Phases A/B/D-with-injection are P0-only and unblock early; embedding-live path (C1, E live loop) gates on P2. |
| SR-11 | **Reuse `vox-graphify-reader` intact, rehomed not rewritten** (umbrella §1/§8) | All reader work is **additive** (`semantic_overlay.rs` + one `pub mod`) | No existing reader logic changed except an optional 1-line `label_of` helper (B2), itself test-covered. |
| SR-12 | **No line locators; node-id results** (design §2.2 constraint 1) | A1/B1/D1 all key on `node_id` | No file:line in any output. |

**Gaps deliberately deferred (stated, not hidden):**
- **Offline LLM-relation *generation*** (the batched labeler that fills `OverlayRelation`s) is out of this plan's first cut — the design (§2.3) describes it as "produced offline, batched." This plan ships the relation **schema, storage, freshness, and read path**; populating relations is a thin follow-on (a `--label-relations` rebuild mode) gated on the same `VOX_SEARCH_EMBED`-style flag. SR-3 is satisfied for storage/transport; production is a one-task amendment. This keeps the deterministic + kNN core shippable without blocking on an LLM-labeling harness.
- **Qdrant collection wiring** for the overlay corpus is **P2's** (`GraphifyNodes` collection); this plan's kNN runs over the in-file `overlay.nodes` embeddings (sufficient for a per-corpus overlay; P2's Qdrant path is the scale lane and is reused transparently once present).

**Honesty invariant proven executable:** SR-2 + SR-6 + SR-9 are not just asserted in prose — A3, F1, and A2's tests fail if any future change lets the overlay mutate `graph.json` or serve a stale overlay without the `stale` flag.

---

## Done criteria

1. `cargo test -p vox-graphify-reader` and `cargo test -p vox-orchestrator-mcp semantic` both green.
2. `vox search semantic-related --help` works; `--rebuild` writes `semantic-overlay.json` beside `graph.json`; query mode returns layer-labeled `related[]` + `expanded[]` + `stale`.
3. `vox_search_semantic_related` is dispatchable (arm + schema + handler), test-covered with injected embeddings.
4. The overlay never mutates `graph.json` (A3, F1) and stamps `stale:true` on a core-sha mismatch (A2, F1).
5. Design docs marked implemented; umbrella §9 P3 row updated.
