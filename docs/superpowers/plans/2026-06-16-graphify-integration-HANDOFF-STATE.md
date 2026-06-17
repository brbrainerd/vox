# Graphify Integration — HANDOFF STATE (start here)

> **You do NOT need any prior conversation.** Read this document first, then the research SSOT at [`docs/src/architecture/graphify-integration-research-2026-06-16.md`](../../src/architecture/graphify-integration-research-2026-06-16.md). For branch-wide context (vault, pipeline, GUI WIP on the same branch), see [`2026-06-16-feat-vault-decryption-recovery-SESSION-HANDOFF.md`](2026-06-16-feat-vault-decryption-recovery-SESSION-HANDOFF.md).

**Branch (current work context):** `feat/vault-decryption-recovery` — graphify work is **interleaved** with unrelated vault/pipeline/GUI changes; prefer a **graphify-only commit or PR** when shipping.  
**Last updated:** 2026-06-16  
**Do NOT commit unless the human operator explicitly asks.**

---

## Human intent

1. **Make graphify maps agent-discoverable** — agents and MCP clients should know which `graph.json` is authoritative, whether it is fresh, and how to search it (lexically today; structurally in P2).
2. **Unify naming collision** — `graphify-out/` currently mixes external Graphify graphs **and** unrelated CI artifacts (`DEP_CYCLES.md`, `build-bench/`, `crate_audit.json`). Long-term: separate namespaces (see blockers C1–C3).
3. **Phased delivery** — P0 freshness → P1 lexical ingest/search → P2 structural MCP → P3 auto-refresh/CI gates. **Do not skip hygiene blockers** before heavy structural work (research doc §5).
4. **Execution style** — TDD for new `pub fn`; use **parallel agents per independent domain** (`dispatching-parallel-agents` skill). Avoid workspace-wide `cargo test` during iteration; use scoped crates and isolated `CARGO_TARGET_DIR`.

**Binding constraints (always):**

- Automation = **VoxScript only** (`vox run scripts/…`); no new `.ps1`/`.sh`/`.py` glue (manifest writer in `scripts/coverage-graph/` is pre-existing Python pipeline).
- **No `cargo fmt --all`** on Windows — use `vox run scripts/fmt.vox`.
- DB writes only through `vox-db`; do not open Turso outside allowed crates.
- Knowledge node IDs: `graphify:{corpus_id}:node:{node_id}`.
- After MCP/CLI surface changes: `cargo run -p vox-cli -- ci operations-sync --target all --write` and `cargo run -p vox-cli -- ci operations-verify`.
- Do not hand-regenerate SSOT after merge — fix generator input; CI `ssot-autoregen` handles drift on PRs.

---

## Two different “graphify” things (do not conflate)

| Concept | What it is |
|--------|------------|
| **External Graphify** (`graphifyy` on PyPI) | Python pipeline: detect → extract → NetworkX → export `graph.json`; optional MCP via `python -m graphify.serve`. |
| **Local `graphify-out/` convention** | Directory used for Graphify exports **and** Vox CI outputs — **namespace collision** is a known blocker. |

Authoritative architecture + upstream MCP tool list: research doc §2.

---

## Authoritative SSOTs

| Artifact | Path |
|----------|------|
| Research + phased plan (P0–P3) | `docs/src/architecture/graphify-integration-research-2026-06-16.md` |
| Corpus registry contract | `contracts/retrieval/graphify-corpora.v1.yaml` |
| Manifest basename | `.graphify_manifest.v1.json` (`MANIFEST_BASENAME` in `vox-config`) |
| Search/retrieval context | `docs/src/architecture/search-retrieval-ssot-2026.md` |
| Data tier placement | `docs/src/architecture/data-storage-ssot-2026.md` (Tier D graphs, Tier A `knowledge_nodes`) |
| Where code lives | `docs/src/architecture/where-things-live.md` (graphify row) |
| MCP tool registry (generated) | `contracts/mcp/tool-registry.canonical.yaml` |
| Operations catalog | `contracts/operations/catalog.v1.yaml` |
| Read-role governance | `contracts/mcp/http-read-role-governance.yaml` |

---

## What is DONE (P0 + P1 core — implemented on branch)

### P0 — Registry, freshness, status surfaces

| Component | Path / symbol |
|-----------|----------------|
| Core library | `crates/vox-config/src/graphify.rs` — registry load, `assess_corpus_status`, `graph_stats_from_json`, `repo_graphify_cache_dir` |
| Integration tests | `crates/vox-config/tests/graphify_status.rs` (5 tests) |
| CLI | `vox graphify status [--corpus] [--strict] [--json]` → `crates/vox-cli/src/commands/graphify/mod.rs` |
| MCP | `vox_graphify_status` → `crates/vox-orchestrator-mcp/src/graphify_tools.rs` |
| Dispatch + schemas | `crates/vox-orchestrator-mcp/src/dispatch.rs`, `input_schemas.rs` |
| Operations | `graphify`, `graphify.status` in `contracts/operations/catalog.v1.yaml` |

**Freshness model (implemented):**

- **Stale reasons:** `graph_missing`, `git_drift`, `ttl_expired`
- **Warnings:** `manifest_missing`, `node_count_drift`, `edge_count_drift`
- Default TTL: 30 days from registry `ttl_days_default`

### P0.5 — Manifest writer (Python pipeline hook)

| File | Role |
|------|------|
| `scripts/coverage-graph/manifest_writer.py` | Writes `.graphify_manifest.v1.json` beside graphs |
| `scripts/coverage-graph/test_manifest_writer.py` | 4 pytest tests |
| `scripts/coverage-graph/rebuild_full_graph.py` | Calls manifest hook after export |
| `scripts/coverage-graph/ingest_reaches.py` | Same hook on default graph path |

Manifest fields align with `GraphifyManifest` in Rust: `corpus_id`, `built_at`, `git_sha`, `scope_path`, `node_count`, `edge_count`, `graph_json_sha256`, `extraction_mode`.

### P1 — Lexical library, MCP search, CLI ingest

| Component | Path / symbol |
|-----------|----------------|
| Lexical search | `lexical_search_graph()` — token overlap on node labels (min token len 3) |
| Ingest projection | `project_graph_nodes_for_ingest()` → `GraphifyKnowledgeNode` records |
| Lexical tests | `crates/vox-config/tests/graphify_lexical.rs` |
| MCP search | `vox_graphify_search` — reads `graph.json` on disk, returns JSON hits with `knowledge_id` |
| MCP tests | `graphify_tools.rs` — `graphify_search_returns_matching_hit` |
| CLI ingest | `vox graphify ingest [--corpus] [--dry-run]` — upserts via `VoxDb::upsert_knowledge_node` |
| CLI ingest test | `ingest_graph_corpus_projects_minimal_graph_nodes` in `graphify/mod.rs` tests |
| SSOT | `vox_graphify_search` in capability registry, operations catalog, http-read-role-governance |

**Related (pre-existing, not replaced):** `crates/vox-publisher/src/scientia_prior_art.rs` — `graphify_lexical_prior_art()` does on-the-fly lexical hits from local `graph.json` for prior-art traces (`graphify_hits` source). P1 MCP search is the **general-purpose** agent tool; scientia path remains for publication novelty.

---

## What is NOT done (remaining work)

### P1 gaps (finish lexical vertical)

| Task | Intent | Key crates / files |
|------|--------|-------------------|
| **Retrieval bundle integration** | Wire `vox_graphify_search` / ingested nodes into planner and `vox_knowledge_query` with `corpus_id` metadata filter | `vox-orchestrator`, `vox-search`, MCP memory tools |
| **DB-backed search option** | After ingest, `vox_graphify_search` could query Turso FTS on `knowledge_nodes` where `id` prefix `graphify:{corpus}:` (today: disk-only lexical) | `vox-db`, `graphify_tools.rs` |
| **Lexical lag detection** | Compare Turso `metadata` / ingest fingerprint vs manifest `graph_json_sha256`; surface in status | `graphify.rs`, ingest path |
| **Operations entry for ingest** | Optional `graphify.ingest` row in `catalog.v1.yaml` (parent `graphify` CLI op exists; ingest subcommand not separately cataloged) | contracts + `operations-sync` |
| **`VOX_GRAPHIFY_TTL_DAYS` env** | Contract in `contracts/config/env-vars.v1.yaml` (research §4.4); today TTL only from YAML registry | `vox-config`, contracts |

### P2 — Structural query + cross-map diff

| Task | Intent |
|------|--------|
| **`vox-graphify-reader` crate** | mmap/read `graph.json`; BFS, shortest path, neighbors compatible with upstream Graphify MCP semantics |
| **MCP tools** | `vox_graphify_query`, `vox_graphify_path`, `vox_graphify_compare` (see research §2.3, §4.2) |
| **Cross-map comparison** | Community drift, god-node rank delta, edge confidence changes (research §4.3) |

**Preferred approach:** embedded Rust reader (not subprocess `graphifyy --mcp`).

### P3 — Auto-refresh, CI gates, migration

| Task | Intent |
|------|--------|
| Auto-refresh hooks | Rebuild triggers on `git_drift`, input manifest change |
| CI `--strict` freshness | Fail CI when default corpus stale |
| VoxScript migration | Replace allowlisted `scripts/coverage-graph/*.py` per README deferral |
| Spool events | Tier B `graphify.lifecycle` via `vox-spool` |

### Code-review blockers (C1–C3 — fix before heavy P2)

Research doc §5 labels these **critical**:

1. **C1 — Untrack** ~108 committed files under `graphify-out/COVERAGE_BEHAVIORS_*` (Tier D violation); promote summaries to `contracts/reports/` if they must stay versioned.
2. **C2 — Namespace split** — move CI outputs (`dep-cycles`, `build-bench`, `crate_audit`) out of graphify-named tree; reserve `.vox/cache/graphify/<corpus_id>/` for knowledge graphs (`repo_graphify_cache_dir` exists but legacy paths still in registry).
3. **C3 — Doc drift** — semantic-coverage strategy doc was partially fixed; always cite **corpus id + git sha + path** when stating node counts.

**Important follow-ups:**

- Hardcoded `graphify-out` in `crates/vox-cli/src/commands/ci/dep_cycles.rs`, `build_bench.rs`, `scripts/crate-build-audit.vox` → `vox_config::paths`.
- Three graph artifacts without single registry authority — **partially addressed** by `graphify-corpora.v1.yaml` (3 corpora); live workspace may have additional graphs not registered.

---

## Architecture decisions already made

1. **Corpus registry** is YAML contract `contracts/retrieval/graphify-corpora.v1.yaml` (not hard-coded paths in tools).
2. **Manifest** lives beside `graph.json` as `.graphify_manifest.v1.json`.
3. **Lexical search v1** reads disk `graph.json` directly (no Turso required for `vox_graphify_search`).
4. **Ingest IDs** use `graphify:{corpus_id}:node:{node_id}` for Turso `knowledge_nodes`.
5. **MCP tools are read-only** for status/search; ingest is CLI-only (`vox graphify ingest`).
6. **Tier D target** for canonical cache: `.vox/cache/graphify/<corpus_id>/` — registry still points at legacy `graphify-out/` paths until C2 migration.

---

## Verification commands (run before claiming done)

Use isolated target dir on Windows to avoid lock contention with running `vox.exe`:

```powershell
$env:CARGO_TARGET_DIR = "$env:TEMP\vox-graphify-verify"

cargo test -p vox-config --test graphify_status
cargo test -p vox-config --test graphify_lexical
cargo test -p vox-cli --lib graphify::
cargo test -p vox-orchestrator-mcp graphify_tools
cargo test -p vox-orchestrator-mcp registry_dispatch_tests

py -m pytest scripts/coverage-graph/test_manifest_writer.py -q

cargo run -p vox-cli -- ci operations-verify
```

**Smoke (requires graph on disk):**

```powershell
vox graphify status --corpus repo-code-graph
vox graphify ingest --corpus repo-code-graph --dry-run
```

**Note:** Full `cargo test -p vox-cli` integration suite may fail on unrelated branch issues (e.g. `ci_workflow_contract` / `CiCmd::command()` compile errors reported in session). Scope to graphify tests above.

**Cold compile:** First `cargo test` on this workspace can take several minutes on Windows; prefer `--no-run` only when checking compile, or reuse `CARGO_TARGET_DIR` across commands.

---

## Parallel agent dispatch map (remaining work)

Dispatch **one agent per row** — domains are independent unless noted.

| Agent | Scope | Do NOT touch |
|-------|--------|--------------|
| **A — Hygiene C1** | `git rm --cached` coverage behavior markdown; `.gitignore` audit; optional `contracts/reports/` promotion | Rust graphify APIs |
| **B — Hygiene C2** | Path migration plan + update registry YAML to `.vox/cache/graphify/`; fix `dep_cycles.rs`, `build_bench.rs`, `crate-build-audit.vox` | MCP tool semantics |
| **C — P1 retrieval** | Planner + `vox_knowledge_query` corpus filter; optional DB-backed search in `vox_graphify_search` | P2 reader crate |
| **D — P2 reader** | New `vox-graphify-reader` + `vox_graphify_query` / `path` / `compare` with TDD | Python pipeline |
| **E — P3 CI gate** | `vox ci` freshness `--strict` for default corpus; wire into `ssot-drift` or pre-push tier | Namespace migration (depends on B) |

After agents return: run verification block above; check for conflicting edits to `graphify-corpora.v1.yaml` or `dispatch.rs`.

---

## Key code map (quick grep targets)

```
crates/vox-config/src/graphify.rs          # core: status, lexical, projection
crates/vox-cli/src/commands/graphify/mod.rs # CLI status + ingest
crates/vox-orchestrator-mcp/src/graphify_tools.rs  # MCP status + search
crates/vox-orchestrator-mcp/src/dispatch.rs        # match arms
contracts/retrieval/graphify-corpora.v1.yaml
scripts/coverage-graph/manifest_writer.py
crates/vox-publisher/src/scientia_prior_art.rs     # graphify_lexical_prior_art
crates/vox-db/src/store/ops_memory.rs              # upsert_knowledge_node, query_knowledge_nodes
```

---

## Suggested PR / commit split

Graphify work should **not** ride the full `feat/vault-decryption-recovery` diff to main if vault/pipeline/GUI are not review-ready:

1. **PR 1 (hygiene):** C1 untrack + C2 path migration (docs + contracts + CI output paths).
2. **PR 2 (graphify P0–P1):** `vox-config`, CLI, MCP, manifest writer, contracts sync — already implemented; needs isolated branch/cherry-pick.
3. **PR 3 (P1 retrieval):** orchestrator/search integration.
4. **PR 4 (P2):** `vox-graphify-reader` + structural MCP tools.

---

## Session transcript (optional deep context)

Full agent conversation JSONL (pre/post summary):  
`C:\Users\Owner\.cursor\projects\c-Users-Owner-vox\agent-transcripts\3f485032-a05d-4baf-8091-93e00cb9dd6b\3f485032-a05d-4baf-8091-93e00cb9dd6b.jsonl`

Search keywords: `graphify`, `lexical_search`, `manifest_writer`, `vox_graphify_search`, `operations-sync`.

---

## Verification criteria for handoff receiver

Before closing graphify work, confirm:

- [ ] All scoped tests in [Verification commands](#verification-commands-run-before-claiming-done) pass
- [ ] `vox ci operations-verify` OK (557+ catalog rows; graphify tools in read-role governance)
- [ ] `vox graphify status` reports expected corpus when local `graph.json` + manifest exist
- [ ] `vox graphify ingest --dry-run` prints node count matching `graph_stats_from_json`
- [ ] No new direct `std::env::var` secret reads; ingest uses `VoxDb::connect_default`
- [ ] If MCP/CLI changed: `operations-sync --write` committed or CI bot will regen

---

## Open questions (human decision if blocked)

1. **Ship disk-only search vs DB FTS** for `vox_graphify_search` long-term — both documented in P1 gaps.
2. **When to migrate registry paths** from `graphify-out/` to `.vox/cache/graphify/` — blocked on C2 approval (may break local workflows until rebuild scripts updated).
3. **Whether graphify ingest** should become an MCP tool (`vox_graphify_ingest`) for agent autonomy — currently CLI-only by design (writes Tier A).
