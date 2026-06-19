# Graphify Native-System Plan Suite — Index & Antigravity Handoff

> **🤖 EXECUTION TARGET.** This suite is written to be executed autonomously by **Gemini 3.5 Flash inside Google Antigravity**. Every plan is engineered against that stack's documented failure modes (≈48% real-world completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Read these two first and keep them open:
> - Execution-target profile: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md)
> - Handoff guide + in-repo skill map: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md)

**Why a suite (not one plan):** the user's ask — a robust, internal, native-performance Graphify integrated with Vox's GUI + search, auto-indexing target repos and the Vox codebase along different semantic lines, with data-size-aware caching, and the ability to study the Graphify source — spans multiple independent subsystems. Per `superpowers:writing-plans`, it is decomposed into one shippable, independently-testable plan per subsystem. This index is the SSOT for the decomposition, the cross-plan dependency/wave plan, and the handoff checklist.

**Grounding:** [`../../src/architecture/graphify-capabilities-audit-and-vertical-integration-2026-06-18.md`](../../src/architecture/graphify-capabilities-audit-and-vertical-integration-2026-06-18.md) — the BUILT/PLANNED/GAP audit + best/worst-case framework these plans operationalize.

---

## Verified baseline (evidence — do not rebuild)

Confirmed by reading the source this turn:
- `vox-graphify-reader` is Rust-native: `GraphifyReader` (BFS/path/god-nodes/community), `bfs.rs`, `compare.rs`, plus build-side `ast.rs` (`syn` + tree-sitter), `cluster.rs` (`leiden-rs`), `cache.rs` (BLAKE3), `overlay.rs`/`reachability.rs`, `rebuild.rs`.
- CLI `vox graphify {status,ingest,rebuild}`; 5 read-only MCP tools; freshness model in `vox-config::graphify::assess_corpus_status`.
- **Construction for code corpora is already native** — `scripts/graphify-refresh.vox` only *prints* instructions pointing at native `vox graphify rebuild`; it does **not** shell to Python. The hybrid boundary is firm: native structural; LLM doc/media semantic extraction via Vox egress (not the `graphifyy` pip package).

## Audit critique that reshaped the plans (evidence-based)

These defects were found by reading the code and are fixed in the rewritten plans:

| # | Finding (evidence) | Fix | Where |
|---|---|---|---|
| 1 | **Manifest bug:** `rebuild.rs` writes `git_sha256:"dev-sha"`, no `built_at`/`graph_json_sha256`, but `assess_corpus_status` reads `git_sha`/`built_at`/`graph_json_sha256` → `git_drift`/`ttl_expired`/`lexical_lag` can never fire after a native rebuild. | Write a freshness-correct manifest; thread real metadata from the caller. | Plan A T1 |
| 2 | **Bare-name node-id collisions:** every symbol is a global node by bare name → two `fn new` collapse into a fake god-node; calls link ambiguously. | Module-qualified ids + two-pass ambiguity-safe resolver (drop ambiguous/self/unresolved). | Plan A T2 |
| 3 | **Stale-cache trap:** the per-file cache is keyed only by content hash, so changing the extractor returns pre-change cached graphs for unchanged files. | Fold an `EXTRACTOR_VERSION` into the cache key. | Plan A T2 |
| 4 | **Overlay/reachability regression:** both match the raw `node.id` against bare symbol names; against qualified real graphs they silently match nothing. | Match by the bare suffix of the node id. | Plan A T3 |
| 5 | **Signature-change atomicity:** `rebuild_graph` has a caller in `graphify_rebuild.rs` (a test) besides the CLI; a non-atomic change breaks the tree (fatal under Antigravity's no-checkpoint kills). | Pre-flight enumerates all callers; fix all in one task. | Plan A T1 |
| 6 | **External-repo freshness:** an indexed target repo's manifest `git_sha` ≠ Vox HEAD → always `git_drift`. | Per-corpus freshness head from the corpus's own `source_root` repo. | Plan B T1 |
| 7 | **Wrong grammar version assumed:** workspace pins `tree-sitter 0.26.9`, not 0.23. | Declare `tree-sitter-python` in `[workspace.dependencies]`; resolve via `cargo tree` to unify. | Plan G T1 |

**Design-principle note (deferred, not a task):** the crate is named `vox-graphify-reader` and described "Read-only", but it now builds graphs (`rebuild`/`ast`/`cluster`/`cache`). Honest-naming smell. Recommend updating the Cargo `description` and considering a rename to `vox-graphify` in a later cleanup; out of scope for a Gemini execution run (rename churns every import).

---

## The suite (dependency + status)

```
A ──▶ B ──▶ G
│     │
│     └────────────▶ F ──▶ E
└──▶ C ──▶ D ──────────────▶ E
```

| # | Plan | Status | Tasks | Goal |
|---|---|---|---|---|
| **A** | [Native construction hardening](2026-06-18-graphify-native-construction-plan-A.md) | **WRITTEN — Antigravity edition** | T1–T4 | Freshness-correct manifest + collision-free, honestly-edged graph; cache + overlays consistent. |
| **B** | [Multi-corpus & target-repo indexing](2026-06-18-graphify-multi-corpus-indexing-plan-B.md) | **WRITTEN — Antigravity edition** | T1–T3 | `source_root` + `vox graphify index` + `modules` semantic lens. |
| **G** | [Study the Graphify source](2026-06-18-graphify-source-study-plan-G.md) | **WRITTEN — Antigravity edition** | T1–T3 | Python extraction + index Graphify's own source; port-parity doc. |
| **C** | [Run lifecycle + autonomous rerun](2026-06-18-graphify-run-lifecycle-rerun-plan-C.md) | **WRITTEN — Antigravity edition** | C1–C3 | Close `lexical_lag` loop (manifest writer + ingest stamp); `refresh --auto` gated by a deterministic cost/value policy. |
| **D** | [Storage, data-size & retention](2026-06-18-graphify-storage-retention-plan-D.md) | **WRITTEN — Antigravity edition** | D1–D3 | Snapshot history (copy/list/prune-to-N), value-score + retention policy, data-size lens pick, snapshot-on-rebuild + `gc` cmd. |
| **F** | [Search fusion](2026-06-18-graphify-search-fusion-plan-F.md) | **WRITTEN — Antigravity edition** | F1–F2 | Activate the dead `default_for_intents` field: pure intent→corpus routing wired into `vox_graphify_search`. |
| **E** | [GUI integration](2026-06-18-graphify-gui-integration-plan-E.md) | **WRITTEN — Antigravity edition** | E1–E4 | `vox_graphify_status` Tauri command → React corpus-health panel showing freshness + rebuild command. |

All seven plans follow the same Antigravity edition rules (Operating Rules block, pre-flight `rg`, `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags, atomic green+commit, verify-before-use, two-strike).

### Deferred follow-ons (scoped down per the no-stub rule — recorded, not stubbed)
- **C/D learning loop:** usage-driven value scoring needs a metadata-filtered `knowledge_nodes` query that `vox-db` lacks today (only `query_knowledge_nodes(query, limit)`). Add `count_knowledge_nodes_by_source(corpus_id, source)` (SQL: `json_extract(metadata,'$.corpus_id')` + `'$.source'`), then feed `gc::value_score`.
- **F deep fusion:** add graphify as a source inside `run_retrieval_bundle` (`memory_tools/retrieval.rs:288`) and RRF-merge with knowledge/memory/chunks — recipe: load intent-matched corpora → `lexical_search_graph` → format `[graphify:corpus:node]` lines → `rrf_merge_line_lists`.
- **E visualization + live rebuild + nav:** interactive graph explorer (needs `vox-graphify-reader` as a `vox-gui` dep + a viz library), a click-to-rebuild write command, and a sidebar nav entry via regenerating `surfaceRegistry.generated.ts` from its canonical spec.

---

## Cross-plan execution waves (for the Antigravity orchestrator)

Each task is internally tagged; across plans the safe ordering is:

- **Wave 1 — Plan A, strict order:** A.T1 → A.T2 (shared `rebuild.rs`); then **A.T3 ∥ A.T4** (disjoint files). One agent for T1→T2, then two parallel subagents for T3/T4.
- **Wave 2 — Plan B:** B.T1 → B.T2 (shared `graphify.rs`+CLI). **B.T3** shares `rebuild.rs` with Plan A, so it runs only after Wave 1; it is disjoint from B.T1/B.T2, so a second agent MAY run B.T3 in parallel with B.T1→B.T2.
- **Wave 3 — Plan G:** G.T1 (edits Plan A's `ast.rs`, so after Wave 1) → G.T2; **G.T3** (docs) is PARALLEL-SAFE anytime.

- **Wave 4 — Plan C (after A):** C1→C2→C3 strict (share `mod.rs` + chained helpers).
- **Wave 5 — Plan D (after A, ideally C):** D1→D2 (share `lib.rs`) → D3 (shares `mod.rs`).
- **Wave 6 — Plan F:** F1→F2 (F2 calls F1). Mostly independent of C/D/E (different crates) → can overlap.
- **Wave 7 — Plan E (after A):** **E1 (Rust) ∥ E2 (TS)** → E3 → E4. The only plan with frontend work; its files are disjoint from C/D/F.

**Golden rule (handoff §3):** never dispatch two subagents that write the same file. `crates/vox-cli/src/commands/graphify/mod.rs` is the hottest file — touched by A.T1, B.T1/T2, C.T2/T3, D.T3 — these run strictly sequentially. `rebuild.rs` (A.T1/T2, B.T3) and `ast.rs` (A.T2, G.T1) likewise. `graphify.rs` in vox-config is touched by B, C, F — sequential. Cross-crate plans (E's GUI, F's mcp/config) can overlap with vox-cli work on separate agents.

---

## Plans to write next (goals + interfaces — Antigravity-ready stubs)

### C — Run lifecycle + autonomous rerun
Wire `lexical_ingest_sha256` on the `vox graphify ingest` side (same BLAKE3 digest Plan A writes for `graph_json_sha256`) so `lexical_lag` fires; add `vox graphify refresh [--auto]` that consults a cost/value gate (rebuild on `git_drift` only when the corpus has recent search-log usage; else expire quietly) and rebuilds natively. **Depends on A.** Companion: [`2026-06-18-graphify-run-lifecycle.md`](2026-06-18-graphify-run-lifecycle.md).

### D — Storage, data-size & retention
Snapshot retention (keep last N `graph.json` per corpus); deterministic value-score GC (usage × recency × churn × cost → maintain/expire/discard, audit §7); large-graph handling (auto-`modules` lens above a node threshold; honor `VOX_GRAPHIFY_VIZ_NODE_LIMIT`; partition by `scope_path`). **Depends on A, B, C.**

### F — Search fusion
Route graphify corpora into `vox_memory_search`/the retrieval bundle with a corpus filter. **Depends on B.**

### E — GUI integration
`vox-gui` corpus-health panel (from `vox_graphify_status`), "stale — rebuild?" prompt with accept/decline, embedded BFS-driven graph explorer (node-limited). **Depends on C, D, F.**

---

## Handoff checklist (run once before starting any plan)

- [ ] `AGENTS.md` present and loaded; `GEMINI.md` present (create from `CLAUDE.md` if missing — Antigravity reads it as highest-priority).
- [ ] `.agents/skills/` resolves to / mirrors `crates/vox-skills/skills/superpowers/` so Gemini can load `subagent-driven-development`, `test-driven-development`, `verification-before-completion`, `requesting-code-review`, `dispatching-parallel-agents`, `using-git-worktrees`, `brainstorming` (handoff §4).
- [ ] `cargo run -p vox-arch-check` passes (baseline).
- [ ] Each plan's **Pre-flight `rg` commands** have been run and the real signatures confirmed (anti-hallucination).
- [ ] Confirm the wave plan above; list which task IDs are dispatched in parallel.

## Conventions (all plans in this suite)
- TDD, bite-sized atomic tasks ending green + committed; commit messages end with the repo `Co-Authored-By` trailer.
- VoxScript-only automation (no new `.ps1`/`.sh`/`.py`). Never `cargo fmt --all` (`-p <crate>`). `docs/src/` `.md` needs YAML frontmatter. No stubs.
- Native structural; LLM semantic via Vox egress (hybrid, firm). Graphs under `.vox/cache/graphify/<corpus_id>/` (Tier D); lexical projection in Turso (Tier A).
