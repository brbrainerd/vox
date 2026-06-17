---
title: "Graphify Integration Research (2026-06-16)"
description: "Audit of Vox graphify-out usage, Graphify (graphifyy) architecture, Rust-native feasibility, agent-search integration plan, cache freshness, and multi-map comparison."
category: "architecture"
status: "research"
training_eligible: true
training_rationale: "Agents need a single SSOT for how graphify maps relate to vox-search, MCP, and CI artifacts before wiring retrieval."
---

# Graphify Integration Research (2026-06-16)

## Executive summary

Vox uses **two different things** under the name “graphify”:

1. **External [Graphify](https://graphify.net/)** (`graphifyy` on PyPI, [safishamsi/graphify](https://github.com/safishamsi/graphify)) — a Python pipeline that builds queryable knowledge graphs from code, docs, and media.
2. **Local `graphify-out/` directory convention** — a gitignored sink for **both** Graphify graphs **and** unrelated Vox CI/analysis artifacts (`crate_audit.json`, `DEP_CYCLES.md`, `build-bench/`, semantic-coverage markdown).

**Rust-native port:** Feasible for the **structural** pipeline (detect → AST extract → petgraph build → Leiden cluster → analyze → MCP query). **Not** a full replacement for LLM semantic extraction, multimodal ingest, or skill-level subagent orchestration — those belong in Vox’s orchestrator layer.

**Agent handoff (implementation state):** [`docs/superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md`](../../superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md) — start here for a fresh agent; lists what landed vs remaining P1–P3 work.

---

## 1. How Vox uses Graphify today

### 1.1 Directory collision

`.gitignore` lists `graphify-out/`, but **108 files** under that tree remain **tracked in git** (mostly `COVERAGE_BEHAVIORS_*.md`). New artifacts are ignored; committed behavior summaries are not.

| Family | Producer | Key outputs |
|--------|----------|-------------|
| **Graphify tool** | `graphify` CLI / skill, `rebuild_full_graph.py` | `graph.json`, `GRAPH_REPORT.md`, `graph.full.json`, `graph.coverage.json`, `graph.semantic.json`, `cache/` |
| **Config audit graph** | `_config_audit_graph.py` | `config-audit-graph/graph.json`, `FINDINGS_INDEX.md` |
| **Semantic coverage** | `scripts/coverage-graph/*.py` | `COVERAGE_MAP.md`, `REACHED_VS_PROVEN.md`, `COVERAGE_BEHAVIORS_*.md` |
| **Vox CI (no Graphify)** | `vox ci dep-cycles`, `vox ci build-bench`, `crate-build-audit.vox` | `DEP_CYCLES.md`, `build-bench/`, `crate_audit.json` |
| **Scoped GUI graph** | Graphify on `crates/vox-gui` | `crates/vox-gui/graphify-out/graph.json` (2,176 nodes) |

No subdirectory convention separates CI maps from knowledge graphs.

### 1.2 Measured graph artifacts (local workspace, 2026-06-16)

| Path | Nodes (approx.) | Scope / notes |
|------|-----------------|---------------|
| `graphify-out/graph.json` | **5,541** | Partial; `.graphify_root` = `crates/vox-compiler` |
| `graphify-out/graph.full.json` | large AST graph | Deterministic `rebuild_full_graph.py` |
| `contracts/reports/semantic-coverage-graph.snapshot.json.gz` | **65,981** | **Committed** CI ratchet input |
| `crates/vox-gui/graphify-out/graph.json` | **2,176** | GUI surface map |
| `graphify-out/config-audit-graph/graph.json` | config audit | 2026-06-15 |

**Doc drift:** [`semantic-test-coverage-graph-strategy-2026-06-07.md`](semantic-test-coverage-graph-strategy-2026-06-07.md) cites “15,333 AST nodes” on `graphify-out/graph.json`. That described a **scoped** historical build, not the current canonical file.

### 1.3 CI integration

| Step | Writes to `graphify-out` | Blocking? |
|------|--------------------------|-----------|
| `vox ci dep-cycles` | `DEP_CYCLES.md` | Yes (normal cycles) |
| `vox ci build-bench` | `build-bench/` | No |
| `scripts/crate-build-audit.vox` | `crate_audit.json` | No |
| `ingest_reaches.py` + ratchet | Uses **contract snapshot**, not live `graph.json` | No (advisory) |

CI **never** rebuilds `graphify-out/graph.json`.

### 1.4 Gaps (not wired)

- Graphify **MCP serve** (`python -m graphify.serve`) — documented in external skill only; **zero** `vox-orchestrator-mcp` tools.
- **vox-search** — no graphify corpus; [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md) corpus matrix has no graphify row.
- **Freshness** — no staleness contract vs `git` HEAD or manifest.
- **Python pipeline** — 12 allowlisted `scripts/coverage-graph/*.py` files; VoxScript rewrite deferred per README.

---

## 2. Graphify architecture (upstream)

Source: [ARCHITECTURE.md](https://raw.githubusercontent.com/safishamsi/graphify/main/ARCHITECTURE.md), [graphify.net](https://graphify.net/), PyPI `graphifyy`.

### 2.1 Pipeline

```
detect() → extract() → build_graph() → cluster() → analyze() → report() → export()
```

| Stage | Module | Mechanism |
|-------|--------|-----------|
| Detect | `detect.py` | File walk, type classification, corpus warnings, **mtime manifest** for `--update` |
| Extract (code) | `extract.py` | **tree-sitter** AST + call-graph second pass; `EXTRACTED` / `INFERRED` edges |
| Extract (docs/media) | skill orchestration | Parallel **LLM subagents** (20–25 files/chunk); vision for images |
| Cache | `cache.py` | **SHA256 of file contents** → `graphify-out/cache/{hash}.json` |
| Build | `build.py` | Merge extractions → **NetworkX** undirected graph |
| Cluster | `cluster.py` | **Leiden** (`graspologic`); split oversized communities |
| Analyze | `analyze.py` | God nodes, surprising connections, suggested questions |
| Export | `export.py` | `graph.json`, `graph.html`, Obsidian, Neo4j, GraphML |
| Serve | `serve.py` | **MCP stdio** over `graph.json` |

### 2.2 Extraction honesty model

| Label | Meaning |
|-------|---------|
| `EXTRACTED` | Explicit in source (import, call, doc statement) |
| `INFERRED` | Reasonable deduction (call-graph pass, co-occurrence) |
| `AMBIGUOUS` | Uncertain; surfaced in `GRAPH_REPORT.md` |

### 2.3 MCP tools (upstream)

Transport: **stdio** (`python -m graphify.serve graphify-out/graph.json`).

| Tool | Behavior |
|------|----------|
| `query_graph` | Keyword seeds → BFS/DFS subgraph, token budget |
| `get_node` | Lookup by label/ID |
| `get_neighbors` | Adjacency + relation/confidence |
| `get_community` | Members of community ID |
| `god_nodes` | High-centrality hubs |
| `graph_stats` | Counts + confidence breakdown |
| `shortest_path` | Path between keyword-matched endpoints |

### 2.4 Caching and incremental update

| Mechanism | Path | Invalidation |
|-----------|------|--------------|
| Semantic cache | `cache/{sha256}.json` | File content hash |
| Update manifest | `manifest.json` | `{path: mtime}` diff |
| Cost tracker | `cost.json` | Cumulative LLM tokens per run |
| Watch mode | flag file | Code → AST-only rebuild; docs → `needs_update` |

---

## 3. Rust-native feasibility

### 3.1 Good candidates for Rust (Vox-owned)

| Graphify stage | Rust stack | Vox precedent |
|----------------|------------|---------------|
| Detect + manifest | `walkdir`, `serde_json` | `vox-config::paths` |
| Rust AST extract | **`syn`** (richer than tree-sitter for Rust) | `vox-codegen`, `vox-code-audit` |
| Graph build | **`petgraph`** | New dependency |
| Leiden cluster | **`leiden-rs`** or `graphops` | New dependency |
| Analyze / query | Port `analyze.py` + BFS/path | `vox-orchestrator-mcp` patterns |
| MCP surface | **`rmcp`** | Existing MCP crate |
| Cache | `blake3` / `sha2` + Tier D cache dir | data-storage SSOT §4.4 |

`scripts/coverage-graph/rebuild_full_graph.py` already calls `graphify.extract` deterministically — a credible seed for a Rust port of **structural** extraction only.

### 3.2 Keep Python / orchestrator-owned

- Multi-language tree-sitter extractors (19 languages in upstream `extract.py`)
- PDF / image / video semantic extraction
- Parallel LLM subagent dispatch (skill.md workflow)
- HTML/Obsidian/Neo4j exporters (agent-facing surface is `graph.json` + MCP)

### 3.3 Hybrid recommendation

| Layer | Owner |
|-------|-------|
| Monorepo **code graph** (Rust/Vox crates) | New `vox-graph-*` crates or `vox-search/src/graphify/` |
| Multimodal / doc semantic pass | `graphifyy` subprocess or Vox agent workflow via `vox_actor_runtime::llm` |
| Agent API | `vox-orchestrator-mcp` tools → `vox-search` execution (no forked retrieval) |

Full Rust rewrite of Graphify is **not cost-effective**. Rust-owning **structural + query + freshness** for the Vox workspace is **high ROI**.

---

## 4. Agent-search integration plan

Aligns with [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md): all retrieval through `vox-search` + `RetrievalEvidenceEnvelope`.

### 4.1 Corpus registry (P0)

Add `contracts/retrieval/graphify-corpora.v1.yaml`:

```yaml
corpora:
  - id: repo-code-graph
    scope_path: "."
    graph_path: "graphify-out/graph.json"
    default_for_intents: [code_navigation, repo_structure]
  - id: vox-gui-surface
    scope_path: "crates/vox-gui"
    graph_path: "crates/vox-gui/graphify-out/graph.json"
  - id: config-audit
    graph_path: "graphify-out/config-audit-graph/graph.json"
```

Each map gets `.graphify_manifest.v1.json`: `built_at`, `git_sha`, `node_count`, `graph_json_sha256`, `input_manifest_sha256`, `extraction_mode`, `semantic_pass_crates`.

### 4.2 Vertical search

**Lexical:** Project node labels/behaviors into Turso `knowledge_nodes` with IDs `graphify:{corpus_id}:node:{id}` and metadata filters (`proof_strength`, `honesty`, `corpus_id`). Extend planner / `vox_graphify_search` MCP tool.

**Structural:** `vox-graphify-reader` — mmap `graph.json`, implement BFS/path/explain compatible with upstream MCP semantics. Tools: `vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare`.

### 4.3 Cross-map comparison

Diff two manifest-bound snapshots:

- Community drift (Jaccard on member sets)
- God-node rank delta
- Edge confidence promotions/drops
- Semantic coverage overlay drift (`reached` vs `proven`)

Output: Tier D full diff JSON; optional Tier A summary row for “what changed since last week.”

### 4.4 Cache freshness / recache triggers

| Trigger | Detection | Action |
|---------|-----------|--------|
| Git drift | `git diff manifest.git_sha..HEAD` ∩ scanned files | `stale_reason: git_drift` |
| Input manifest | Re-hash detect file list | Stale |
| TTL | `VOX_GRAPHIFY_TTL_DAYS` (default 30) | Advisory stale |
| Semantic incomplete | Overlay older than graph or missing crates | `stale_reason: semantic_incomplete` |
| Lexical lag | Turso `metadata.graph_sha256` ≠ manifest | Re-ingest |

CLI: `vox graphify status [--strict]`; MCP: `vox_graphify_status`.

Gap detection: files in detect manifest without graph nodes; crates without Phase-2 semantic pass; registry corpus with missing `graph.json`.

### 4.5 Tier placement ([data-storage-ssot-2026.md](data-storage-ssot-2026.md))

| Data | Tier | Location |
|------|------|----------|
| Full `graph.json`, overlays | **D** | `.vox/cache/graphify/<corpus_id>/` via `vox_config::paths` |
| Manifests | **D** | Beside graph |
| Lexical projection | **A** | `knowledge_nodes` / `knowledge_edges` |
| Refresh/diff events | **B** | `vox-spool` channel `graphify.lifecycle` |
| Large diff blobs | **C** | `vox-checksum-manifest` digest; Tier A holds pointer |

**Do not** store 100MB+ graphs in Turso.

### 4.6 Phased delivery

| Phase | Outcome | Key crates |
|-------|---------|------------|
| **P0** | Registry + manifest + `vox graphify status` | `vox-config`, `vox-cli`, `vox-orchestrator-mcp` — **landed on `feat/vault-decryption-recovery`** (see handoff doc) |
| **P1** | Lexical ingest + `vox_graphify_search` in retrieval bundle | `vox-search`, `vox-db` — **core landed** (lexical lib, MCP search, CLI ingest); retrieval bundle + DB FTS **open** |
| **P2** | Structural query + cross-map diff MCP tools | `vox-graphify-reader`, MCP |
| **P3** | Auto-refresh hooks, semantic coverage gate, CI `--strict` freshness | `vox-cli-ci`, VoxScript migration |

---

## 5. Code review findings (integration readiness)

### Critical (fix before P1)

1. **Untrack** `graphify-out/COVERAGE_BEHAVIORS_*` — violates Tier D intent; promote summaries to `contracts/reports/` if needed.
2. **Namespace split** — move CI outputs (`dep-cycles`, `build-bench`, `crate_audit`) out of graphify-named tree; reserve graphify cache under `.vox/cache/graphify/`.
3. **Doc drift** — correct node-count claims in semantic-coverage strategy doc; cite corpus id + sha + path everywhere.

### Important

- Hardcoded `graphify-out` in `dep_cycles.rs`, `build_bench.rs`, `crate-build-audit.vox` → `vox_config::paths`.
- Three graph artifacts with no registry — agents cannot pick authoritative map.
- Pick **one** structural path: embedded Rust reader (preferred) vs subprocess `graphifyy --mcp`.

### Assessment

**Proceed with P0 design and this research doc.** Block MCP/reader implementation until C1–C3 hygiene lands.

---

## 6. Tooling notes

### Tavily CLI

Installed on this machine via `uv tool install tavily-cli` → `C:\Users\Owner\.local\bin\tvly.exe` (v0.1.4). **Not available via winget** (`winget search tavily` returned no package). Add `C:\Users\Owner\.local\bin` to user PATH or run `uv tool update-shell`.

### External references

- [Graphify GitHub](https://github.com/safishamsi/graphify)
- [Graphify ARCHITECTURE.md](https://raw.githubusercontent.com/safishamsi/graphify/main/ARCHITECTURE.md)
- [graphify.net](https://graphify.net/)
- [PyPI graphifyy](https://pypi.org/project/graphifyy/)
- [leiden-rs](https://crates.io/crates/leiden-rs) — Rust Leiden for `petgraph`

### Related Vox docs

- [`semantic-test-coverage-graph-strategy-2026-06-07.md`](semantic-test-coverage-graph-strategy-2026-06-07.md) — overlay `reached`/`proven` on graphify graph
- [`semantic-coverage-status-2026-06-15.md`](semantic-coverage-status-2026-06-15.md) — honest CI ratchet status
- [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md) — retrieval pipeline SSOT
- [`scripts/coverage-graph/README.md`](../../../scripts/coverage-graph/README.md) — operational coverage pipeline
