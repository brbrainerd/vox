---
title: "SCIENTIA Automated Research — agent handoff"
description: "Fresh-agent context for the June 2026 SCIENTIA automated-research implementation: what shipped in working tree, review status, verification gates, and remaining work."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Onboarding context for agents continuing SCIENTIA Waves 0–6 and post-review cleanup without prior session history."
---

# SCIENTIA Automated Research — agent handoff (2026-06-16)

**Purpose:** Give a new coding agent everything needed to continue SCIENTIA automated-research work without reading prior chat transcripts.

**Prior session transcript (optional):** SCIENTIA implementation thread (conversation ID: e7d7e3cc-75f3-48d0-857e-3565aa958bfe) — use only for dispute resolution; this doc is the SSOT for remaining work.

---

## Mission (do not drift)

Upgrade Vox SCIENTIA from human-driven publication tooling to a **serious, human-gated research pipeline**:

| Pillar | Intent |
|--------|--------|
| **Correct novelty** | No false Novel on failed/empty retrieval; real embeddings when online; `contradicted` when conflicts exist; golden harness for regression |
| **Unified research loop** | `vox research run` → finding candidates / mesh → SCIENTIA discovery inbox (not parallel silos) |
| **Retrieval depth** | Tavily extract/research tiers, citation diversity, Graphify P1 lexical leg in prior-art bundle |
| **Archive run** | Zenodo autofill, dual human approval, non-zero exit on blockers, Software Heritage + nanopub test-server path |
| **GUI surfacing** | Novelty evidence panel, discovery inbox, archive status — extend `ScientiaDashboard`, do not fork a parallel surface |

**Human approval is the hard gate everywhere.** Nothing reaches a production network without `ApprovalToken`-backed dual publication approval. Do not bypass `has_dual_publication_approval_for_digest` on archive paths.

**Explicitly out of scope (standing decisions):**

- Production nanopub network publish
- Live arXiv API submission (handoff bundle only)
- New `.ps1` / `.sh` / `.py` glue scripts (VoxScript-first policy)

---

## Authoritative references (read before large changes)

| Doc | Role |
|-----|------|
| [`scientia-automated-research-historical-extension-research-2026.md`](scientia-automated-research-historical-extension-research-2026.md) | Historical context, §4 code-review findings with June 2026 status |
| [`2026-06-12-scientia-research-pipeline-upgrade.md`](../../superpowers/plans/2026-06-12-scientia-research-pipeline-upgrade.md) | Tracks A–E task-level plan (checkboxes may lag code) |
| [`scientia-self-publication-gap-map-2026.md`](scientia-self-publication-gap-map-2026.md) | End-to-end user journey gaps (IMRaD, replay writeback, scout, etc.) |
| [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md) | CRAG, Tavily, Tantivy, retrieval bundle |
| [`graphify-integration-research-2026-06-16.md`](graphify-integration-research-2026-06-16.md) | Graphify P0–P3; MCP query tool still open |

**Do not edit** `.cursor/plans/scientia_research_full_plan*.plan.md` if present — it is a session plan artifact; use this handoff + the superpowers plan instead.

---

## Git / branch state (as of 2026-06-16 handoff)

| Item | Value |
|------|-------|
| **Branch** | `feat/vault-decryption-recovery` |
| **HEAD commit (unrelated vault doc)** | `da9c8afc50` — `docs(secrets): add Clavis vault decryption recovery runbook` |
| **SCIENTIA work** | **Large uncommitted working tree** (~295 files, +20k/−8k lines in full diff; SCIENTIA is a major subset) |
| **Merge-base note** | SCIENTIA implementation was developed against `main`; review base was `908c8c7fbe` in prior session |

**Next human-facing milestone:** Run verification gate (below) → commit SCIENTIA diff (only when human asks) → open focused PR (consider splitting vault vs SCIENTIA if diff is mixed).

---

## What shipped (Waves 0–6 — in working tree)

Treat **code + tests** as source of truth; plan checkboxes were not bulk-updated.

### Wave 0 — Doc hygiene

- Updated §4 status in [`scientia-automated-research-historical-extension-research-2026.md`](scientia-automated-research-historical-extension-research-2026.md)

### Wave 1 — Track A remainder (novelty correctness)

| Deliverable | Primary paths |
|-------------|---------------|
| `InsufficientEvidence` verdict | `crates/vox-scientia/src/inspect_bridge/novelty.rs` |
| Embedder guard | `crates/vox-publisher/src/scientia_semantic.rs` — `require_embedder_for_online_novelty`; env `VOX_SCIENTIA_REQUIRE_EMBEDDER` in `contracts/config/env-vars.v1.yaml` |
| Bundle SSOT unification | `NoveltyEvidenceBundleV1` + contract parity |
| `assess_novelty` pipeline | `crates/vox-publisher/src/scientia_novelty_assess.rs` — chrono + conflicts; **conflicts → `contradicted`** |
| Golden harness | `crates/vox-publisher/tests/novelty_golden_harness.rs`, fixture `tests/fixtures/novelty_golden/cases.jsonl` (13 cases incl. `contradicted` conflict_pair) |
| Embedder callsite guard test | `crates/vox-cli/tests/publication_embedder_guard.rs` |

### Wave 2 — Track F (unified research loop)

| Deliverable | Primary paths |
|-------------|---------------|
| Discovery bridge | `crates/vox-research-shim/src/research/discovery_bridge.rs` |
| `planner_degraded` UX | `crates/vox-research-shim/src/research/planner.rs` + session metadata |
| Provider → vox-search | `crates/vox-research-shim/src/research/provider.rs` |
| Pipeline cache fix | `crates/vox-research-shim/src/research/orchestrator/pipeline_cache.rs` — `upsert_knowledge_node` |
| Integration tests | `crates/vox-research-shim/tests/scientia_research_discovery_bridge.rs` |
| WAL on `:memory:` fix | `vox-db` — `apply_pragmas_for_memory` (discovery-bridge DB tests) |

### Wave 3 — Retrieval depth

| Deliverable | Primary paths |
|-------------|---------------|
| Tavily extract / research | `crates/vox-search/src/tavily_extract.rs`, `tavily_research.rs` |
| Graphify P1 lexical leg | `crates/vox-publisher/src/scientia_prior_art.rs` |
| Citation diversity gate | research pipeline stages (see prior-art / research stage modules) |

### Wave 4 — Beyond nanopub (partial)

| Deliverable | Primary paths |
|-------------|---------------|
| Replay writeback to worthiness | worthiness / replay integration |
| MENS training run producer | `crates/vox-scientia/src/producers/mens_training_run.rs` |
| Findings Astro route | `docs-astro/src/pages/findings/[uri].astro` (shells out to `vox-dev.ps1` / `vox-dev.sh` — docs-build coupling) |
| Venue / critic contracts | `contracts/scientia/finding-class-defaults.v1.yaml`, `allows_llm_critic` |

### Wave 5 — Archive + producers

| Deliverable | Primary paths |
|-------------|---------------|
| Zenodo autofill | `crates/vox-publisher/src/scientia_autofill.rs` |
| Archive orchestrator | `crates/vox-cli/src/commands/db/publication/archive_run.rs` |
| Code-uniqueness KNN | `crates/vox-publisher/src/scientia_producers/code_uniqueness.rs` |
| Dual approval + blocker exit | `archive_run.rs` — `has_dual_publication_approval_for_digest`, `archive_run_blocker_result` → `bail!`, unit test `archive_run_blocker_returns_error_not_ok` |
| Autofill before completion report | `compose_autofill_for_archive` ordering in archive run |

### Wave 6 — GUI

| Component | Path |
|-----------|------|
| Novelty evidence panel | `crates/vox-gui/ui/src/components/surfaces/Scientia/NoveltyEvidencePanel.tsx` |
| Discovery inbox | `DiscoveryInbox.tsx` |
| Archive status | `ArchiveStatusSummary.tsx` |
| API types | `noveltyApi.ts` — includes `contradicted` |
| Tests | `*.test.tsx` for above + `ScientiaDashboard.test.tsx` |
| Gamify kudos | wired when gamify enabled (research session complete) |

---

## Post-implementation code review (June 2026)

A `/requesting-code-review` pass flagged **Important** items. **All were already fixed in the working tree** when re-inspected after interrupted parallel-agent runs:

| Finding | Resolution |
|---------|------------|
| Archive dual approval | `has_dual_publication_approval_for_digest` in archive path |
| Blocked archive exits 0 | `archive_run_blocker_result` bails; unit test present |
| Autofill before archive completion | `compose_autofill_for_archive` runs before completion report |
| `contradicted` in assess path | Conflicts override in `scientia_novelty_assess.rs` |
| GUI `contradicted` | Types + panel + vitest |
| `discovery_watch` embedder guard | In `publication_embedder_guard.rs` callsite list + source |

**No net-new implementation was required** for those items; the remaining gate is **fresh verification**, not more feature work.

---

## Remaining work (prioritized)

### P0 — Merge gate (do first)

1. **Run Rust verification sequentially** (see commands below). Do **not** dispatch parallel `cargo test` on the same workspace — Windows + shared `target/` causes lock contention (observed repeatedly in prior session).
2. **Confirm green** on all targeted tests before commit/PR.
3. **Commit** the SCIENTIA diff when the human asks — split from unrelated vault work if the diff is mixed on `feat/vault-decryption-recovery`.

### P1 — Known open items (not blocking Wave 0–6 claim)

| Item | Notes |
|------|-------|
| **Graphify P1 → P2** | Lexical leg shipped; full MCP `vox_graphify_search` + semantic leg still open per [`graphify-integration-research-2026-06-16.md`](graphify-integration-research-2026-06-16.md) |
| **`graphify-out/` collision** | CI artifacts vs Graphify graphs — blocks trustworthy ingest (§4.3 finding 13) |
| **GUI CLI coverage ~10%** | [`vox-gui-scientia-coverage-audit-2026.md`](vox-gui-scientia-coverage-audit-2026.md) — many CLI commands still lack GUI |
| **Prereg doc drift** | Gap map cites `vox-prereg` crate; reality is `vox-orchestrator/src/preregistration/` — dedicated doc-hygiene PR (§8 of historical extension doc) |
| **Findings Astro route** | Depends on dev launcher shell scripts for build-time `vox` invocation |
| **Pre-existing vox-search failures** | `semcov_wave21_tests`, `ingest_markdown_tree` — unrelated to SCIENTIA; do not conflate with this handoff gate |
| **IMRaD scaffold, full collated dashboard, Provider Atlas → registry** | Phase 4 backlog in historical extension §9 |

### P2 — Optional polish

- OpenRouter spend HUD tile (contract exists; GUI incomplete — finding 16)
- Expand golden harness if scorer thresholds change (maintain 1.0 precision/recall on deterministic fixtures)
- `vox ci pre-push --complete` before push (clippy + scoped gates)

---

## Verification commands (binding gate)

Run **one at a time** from repo root. Use full cargo path on Windows if `PATH` is minimal:

```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-scientia inspect_bridge::novelty
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-publisher --test novelty_golden_harness
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-research-shim --test scientia_research_discovery_bridge
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-cli --test publication_embedder_guard --test publication_archive_run_plan
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-search --test tavily_extract_test
cd crates/vox-gui/ui; pnpm test --run NoveltyEvidencePanel DiscoveryInbox ScientiaDashboard ArchiveStatusSummary
& "$env:USERPROFILE\.cargo\bin\cargo.exe" check -p vox-scientia -p vox-publisher -p vox-research-shim -p vox-search -p vox-cli
```

**GUI status at handoff:** vitest **10/10 passed** (NoveltyEvidencePanel 4, DiscoveryInbox 3, ScientiaDashboard 2, ArchiveStatusSummary 1).

**Rust status at handoff:** Prior session reported green; **fresh run blocked** by concurrent `cargo` builds on default `target/` (parallel agent dispatch + other background jobs). Retry when `Get-Process cargo` is empty or use isolated worktree target per `.cargo/config.toml` policy.

**Fast pre-push (after targeted tests):** `vox ci pre-push` or `pwsh -File scripts/windows/vox-dev.ps1 ci pre-push`

---

## Architecture invariants (binding)

### Secrets & LLM

- Resolve secrets via `vox_secrets::resolve_secret(...)` — never raw `env::var` for tokens
- All LLM/embed calls via `vox_actor_runtime::llm` (`llm_embed`, `llm_chat`, …) — no vendor hostnames at callsites

### Data & contracts

- Contract changes: bump `x-vox-version`, regenerate with `vox schema generate`, run `vox ci data-storage-guard`
- New `VOX_*` env vars → `contracts/config/env-vars.v1.yaml`
- Do not hand-edit `@generated-hash` files

### SCIENTIA-specific

- Online novelty with `VOX_SCIENTIA_REQUIRE_EMBEDDER=1` must call `require_embedder_for_online_novelty` at every online prior-art callsite (enforced by `publication_embedder_guard.rs`)
- Route novelty UX through `assess_novelty`, not raw `AtomicNoveltyScorer::score` alone
- Archive run must enforce dual approval and fail closed on blockers

### Build / Windows

- Format: `vox run scripts/fmt.vox` — **never** `cargo fmt --all`
- Bootstrap without global `vox`: `pwsh -File scripts/windows/vox-dev.ps1 <cmd>`

### Agent workflow

- TDD for new `pub fn` (see `AGENTS.md`)
- **Parallel subagents:** one agent per independent domain; **never** parallel `cargo build/test` on same checkout
- Commits / PRs only when human explicitly asks
- Local CI first — do not push to GitHub for iteration feedback

---

## Key file map (quick lookup)

```text
Novelty scorer (raw)     crates/vox-scientia/src/inspect_bridge/novelty.rs
Novelty assess (full)    crates/vox-publisher/src/scientia_novelty_assess.rs
Prior art fetch          crates/vox-publisher/src/scientia_prior_art.rs
Embedder guard           crates/vox-publisher/src/scientia_semantic.rs
Golden fixture           crates/vox-publisher/tests/fixtures/novelty_golden/cases.jsonl
Research orchestrator    crates/vox-research-shim/src/research/
Discovery bridge         crates/vox-research-shim/src/research/discovery_bridge.rs
Archive CLI              crates/vox-cli/src/commands/db/publication/archive_run.rs
Publication guards       crates/vox-cli/tests/publication_embedder_guard.rs
                         crates/vox-cli/tests/publication_archive_run_plan.rs
Tavily tiers             crates/vox-search/src/tavily_{extract,research}.rs
GUI Scientia surfaces    crates/vox-gui/ui/src/components/surfaces/Scientia/
Review / approval        crates/vox-scientia/src/review_flow.rs
Claim extractor          crates/vox-scientia/src/claim_extractor/
Contracts                contracts/scientia/
```

---

## Parallel-agent dispatch lessons (from prior session)

When using `/dispatching-parallel-agents`:

| ✅ Good parallel domains | ❌ Bad parallel domains |
|--------------------------|-------------------------|
| GUI vitest vs Rust tests **after** build exists | Multiple `cargo test` on same `target/` |
| Archive_run.rs vs novelty_golden vs discovery_bridge **if** agents don't share files | Two agents editing `scientia_novelty_assess.rs` |
| Code review verification (read-only) per crate | Full workspace `cargo build` × N |

Review-fix dispatch was **interrupted twice** (~14–30 min) with **no agent output**; parent inspection showed fixes already landed. Prefer **sequential verification** over re-dispatch for merge gates.

---

## Suggested commit message (when human requests commit)

Use conventional commits; scope to SCIENTIA only if diff is split:

```text
feat(scientia): automated research Waves 0–6 — novelty, unified loop, archive, GUI

Implements embedder guard, assess_novelty contradicted path, discovery bridge,
Tavily extract/research, archive autofill with dual approval, and Scientia GUI
panels. Golden harness + integration tests included.
```

---

## Related handoffs

- [GUI Operator Console v2 handoff](gui-operator-console-v2-handoff-2026-06-16.md) — separate mega-plan; shares `ScientiaDashboard` surface registry
- [Graphify integration handoff](../../superpowers/plans/2026-06-16-graphify-integration-HANDOFF-STATE.md) — P1 lexical leg touches `scientia_prior_art.rs`

---

## Checklist for the next agent

- [ ] Read this doc + §4 of [`scientia-automated-research-historical-extension-research-2026.md`](scientia-automated-research-historical-extension-research-2026.md)
- [ ] Ensure no other `cargo` processes hold `target/` lock
- [ ] Run P0 verification commands sequentially; record pass/fail
- [ ] If Rust fails, fix **only** the failing domain; do not refactor unrelated SCIENTIA code
- [ ] Ask human before commit/PR; consider branch split if vault + SCIENTIA are mixed
- [ ] Update §4 status in historical extension doc if verification reveals new gaps
