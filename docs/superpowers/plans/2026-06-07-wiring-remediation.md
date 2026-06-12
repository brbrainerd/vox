# Vox Wiring Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire together (or honestly retire) every confirmed unconnected feature found by the 2026-06-07 `/graphify` wiring audit — Scientia decision/social/review, the mesh/distributed forks, desktop speech, and the GUI surface gates — so that built capabilities are reachable and no dead duplicates remain.

**Architecture:** Four independent workstreams (Mesh/Distributed, Scientia, GUI-Gate, Speech) run in isolated git worktrees in parallel; within each, tasks land in a strict dependency order. Each finding is classified WIRE / DELETE / FIX-GATE / DEFER. Genuinely large platform efforts (mobile FFI, cross-daemon hopper replication) are explicitly deferred with unblock criteria rather than stubbed.

**Tech Stack:** Rust (cargo workspace, 107 crates, layered per `docs/src/architecture/layers.toml`), Tauri 2 + React/TypeScript (`crates/vox-gui`), `vox-arch-check` (orphan/layer enforcement), `vox ci gui-surface-*` self-surfacing gates, Playwright + vitest (GUI), `cargo test` (Rust). Windows host — format with `vox run scripts/fmt.vox`, never `cargo fmt --all`.

---

## Decision Summary (all 18 findings)

| ID | Finding | Decision | Effort | Workstream | Blocks on |
|----|---------|----------|--------|-----------|-----------|
| B2 | `vox-distributed-training` byte-identical fork in populi | DELETE fork + depend on crate | S | WS-1 | — |
| B1 | `vox-inference` real backends stranded; populi `inference/` stub fork | DELETE fork | S | WS-1 | — |
| B11 | `vox-mesh-policy` `.vox` parser has 0 consumers | WIRE (load `donations.vox`, JSON fallback) | S | WS-1 | — |
| B3 | Task Hopper built, no production intake/drain | WIRE (daemon construct + HTTP intake) | M | WS-1 | — |
| B4 | `OpFragmentKind::HopperSync` unrouted | **DEFER** (needs B3 + Hp-T5 persistence) | L | WS-1 | B3 |
| B6 | `NoveltyVerdict` missing `InsufficientEvidence`/`Contradicted` | WIRE (enum + retrieval-aware score) | S | WS-2 | — |
| B5 | Scientia decision layer orphaned (Scorer/Conflict/Chrono) | WIRE into worthiness path | M | WS-2 | B6 |
| B7 | Short-form/Twitter leg + `syndicate()` + UTF-8 panic | WIRE + fix panic | M | WS-2 | — |
| A1 | No GUI DiscoveryReview (claim-review/nanopub-build) | WIRE (GUI surface via `execute_command`) | M | WS-2 | — |
| A6 | No GUI Scientia cost panel | WIRE (fold into ScientiaDashboard) | S | WS-2 | — |
| B10 | `vox-nanopub` leaf crate orphaned | DELETE **after hand-grep** (else migrate-then-delete) | S–M | WS-2 | grep gate |
| A7 | Dead IPC registration `get_routing_summary` | DELETE registration | XS | WS-3 | — |
| A5 | `gui-surface-coverage` misses 9 decorator surfaces | FIX-GATE (parser) | S | WS-3 | — |
| A3 | Visus/Safety/Attention invisible to gate + no panel | FIX-GATE (feature-aware) + DEFER panels w/ waiver | M | WS-3 | — |
| A2 | GUI Research runs CLI inline, not async daemon | WIRE (persistent daemon + status poller) | M | WS-3 | — |
| A4 | Desktop GUI has no speech-to-text | WIRE (`oratio_transcribe` Tauri cmd) | S–M | WS-4 | — |
| B8 | RN runtime `spawnActor/startWorkflow/infer` throw | **DEFER** (bindings absent; runtime blockers) | L | WS-4 | runtime §13/§15 |
| B9 | `vox-tauri-stt` native mobile STT stub | **DEFER** (native FFI; A4 is the desktop substitute) | L | WS-4 | mobile plugin |

**Net for this remediation:** 13 actionable findings (8 WIRE, 3 DELETE, 2 FIX-GATE) across 4 workstreams; 4 explicit DEFERs with unblock criteria; 1 conditional DELETE gated on a grep.

---

## Orchestration Strategy (workflows · parallel agents · subagents)

### Why four worktrees in parallel
The four workstreams touch disjoint crate sets, so they can mutate files simultaneously without conflict:
- **WS-1 Mesh:** `vox-populi`, `vox-orchestrator`, `vox-orchestrator-d`, `vox-mesh-policy`, deletes `vox-distributed-training`/inference-fork.
- **WS-2 Scientia:** `vox-scientia`, `vox-publisher`, `vox-research-events`, `vox-gui/ui/.../Scientia`.
- **WS-3 GUI-Gate:** `vox-cli/src/commands/ci`, `vox-gui/src/commands`, `vox-gui/ui` (App/registry).
- **WS-4 Speech:** `vox-gui` (new `oratio.rs`), `vox-gui/ui/.../Loquela`.

Overlap risk: WS-2 and WS-3 and WS-4 all touch `crates/vox-gui/ui`. WS-2 adds Scientia decorator surfaces; WS-3 edits `App.tsx`/`decoratorRegistry.ts`/the registry SSOT; WS-4 edits `Loquela.tsx`. To avoid merge churn in the regenerated `surfaceRegistry.generated.ts` and `App.tsx`, **serialize the GUI-registry edits**: WS-3 lands its gate/parser fixes first (it owns the registry generator), then WS-2's A1 surface and WS-4's Loquela change rebase on top and run `vox ci gui-surface-registry --write` once each.

### Execution shape — one Workflow per workstream
Run each workstream as its own `Workflow` (or subagent-driven session) inside `isolation: "worktree"`:

```
phase('WS-1 implement') → pipeline(tasks, implement, verify)
```

- **Implement stage:** a fresh subagent per task (TDD: write failing test → implement → green → commit). `isolation: 'worktree'` only at the workstream level — tasks within a workstream share the worktree and run sequentially in dependency order (they touch shared crate state).
- **Verify stage (adversarial gate before merge):** after each workstream's tasks complete, dispatch 2–3 independent reviewer subagents that try to REFUTE "this is correctly wired and tested" — they run the workstream's tests, `cargo build -p <crate>`, `vox-arch-check`, and the relevant `vox ci` gate, and confirm no new stub/orphan was introduced. Only a clean adversarial pass authorizes the merge to the integration branch.
- **Cross-workstream parallelism:** WS-1 and WS-4 are fully independent of everything → launch immediately. WS-2 and WS-3 share GUI-registry files → WS-3 starts immediately; WS-2's Rust tasks (B5/B6/B7/B10) start immediately, its GUI tasks (A1/A6) gate on WS-3's registry-generator landing.

### Global critical path (shortest wall-clock)
```
t0 ── WS-1 (B2∥B1∥B11 → B3)                        ── merge
t0 ── WS-4 (A4)                                     ── merge
t0 ── WS-3 (A7 → A5 → A3-gate → A2)                 ── merge ┐
t0 ── WS-2 Rust (B6 → B5 ; B7 ; B10-grep)           ──────────┤→ WS-2 GUI (A1 ∥ A6) rebase on WS-3 ── merge
```
Recommended fleet: 4 concurrent workstream worktrees, ≤4 implement subagents live at once (matches the per-workflow concurrency cap), each followed by a 2-voter adversarial verify. Estimated: the long pole is WS-2 (B6→B5→A1 chain) and WS-1 (→B3).

### Standing rules for every subagent
- TDD, one logical change per commit, frequent commits.
- **Never hand-edit generated files** (`surfaceRegistry.generated.ts`, `contracts/reports/*.json`, `SUMMARY.md`, `where-things-live.md`): always re-run the generator (`vox ci … --write`). Authored `notes:` in source YAML are the only allowed hand-edit, followed by a re-`--write`.
- **No new stubs** (repo policy): if a target turns out larger than its slice, scope DOWN to a smaller real artifact, do not ship a hollow function.
- Respect `#[cfg(feature = …)]` gates (`dei`, `populi-transport`, `oratio`, `scholarly-external-jobs`) in both code and tests.
- Format with `vox run scripts/fmt.vox` or `cargo fmt -p <crate>` (never `cargo fmt --all` on Windows).

---

## WS-1 — Mesh / Distributed consolidation

**Files:**
- Delete: `crates/vox-populi/src/distributed_training/` (whole subtree), `crates/vox-populi/src/inference/` (whole subtree)
- Modify: `crates/vox-populi/src/lib.rs:423,425`, `crates/vox-populi/src/mens/tensor/mod.rs:17`, `crates/vox-populi/Cargo.toml`, `crates/vox-orchestrator/src/models/scoring.rs:392-400`, `crates/vox-orchestrator/Cargo.toml`
- New (B3): `crates/vox-orchestrator-d` daemon state + HTTP intake handlers
- Test: `crates/vox-orchestrator/src/models/scoring.rs` (unit), `crates/vox-orchestrator-d/tests/`

Landing order: **B2 → B1 → B11 (parallel-safe) → B3**. B4 is DEFERRED (see register).

### Task 1 (B2): Delete the distributed-training fork; depend on the crate

- [ ] **Step 1 — Failing test:** in `crates/vox-distributed-training/src/mesh_env.rs` tests (or add one) assert `is_mesh_mode()`/`get_mesh_rank()` resolve from secrets — confirms the crate is the live source. Run `cargo test -p vox-distributed-training mesh_env` → PASS already (baseline).
- [ ] **Step 2 — Repoint the re-export:** edit `crates/vox-populi/src/mens/tensor/mod.rs:17`:
  ```rust
  // was: pub use crate::distributed_training::mesh_env::{MeshTrainConfig, get_mesh_rank, is_mesh_mode};
  pub use vox_distributed_training::mesh_env::{MeshTrainConfig, get_mesh_rank, is_mesh_mode};
  ```
- [ ] **Step 3 — Add the dep:** in `crates/vox-populi/Cargo.toml` add under `[dependencies]`: `vox-distributed-training = { workspace = true }`.
- [ ] **Step 4 — Remove the fork module:** delete `pub mod distributed_training;` at `crates/vox-populi/src/lib.rs:423`, then delete the `crates/vox-populi/src/distributed_training/` directory.
- [ ] **Step 5 — Build + arch-check:** `cargo build -p vox-populi && cargo run -p vox-arch-check`. Expected: PASS (L3→L3 within-layer edge is legal; `vox-distributed-training` deps already in populi's graph).
- [ ] **Step 6 — Commit:** `refactor(populi): consume vox-distributed-training crate, delete byte-identical fork (B2)`

### Task 2 (B1): Delete the dead inference stub fork

- [ ] **Step 1 — Confirm no consumer:** `git grep -n "crate::inference" crates/vox-populi/src` excluding `#[cfg(test)]` → expect only the `pub mod inference;` line. (If any non-test consumer appears, STOP and re-spec — the audit said none exists.)
- [ ] **Step 2 — Remove module:** delete `pub mod inference;` at `crates/vox-populi/src/lib.rs:425`; delete `crates/vox-populi/src/inference/`.
- [ ] **Step 3 — Build:** `cargo build -p vox-populi` → PASS.
- [ ] **Step 4 — Commit:** `refactor(populi): delete orphaned inference stub fork; real backends live in vox-inference (B1)`

> NOTE: standing up a real `vox-inference::InferenceDispatcher` consumer is **Mn-T2 future work**, not this task. `vox-inference` stays `orphan_exempt` (`layers.toml:164`).

### Task 3 (B11): Wire `donations.vox` ingestion with JSON fallback

- [ ] **Step 1 — Failing test:** add to `crates/vox-orchestrator/src/models/scoring.rs` `#[cfg(all(test, feature = "populi-transport"))]`:
  ```rust
  #[test]
  fn donations_vox_file_drives_reciprocity_bonus() {
      let dir = tempfile::tempdir().unwrap();
      let p = dir.path().join("donations.vox");
      std::fs::write(&p, "let public_mesh_opt_in = true\n").unwrap();
      let policy = vox_mesh_policy::load_policy(&p).expect("parse");
      assert!(policy.public_mesh_opt_in);
      // assert scoring path applies the +0.15 reciprocity bonus when opted in
  }
  ```
  Run `cargo test -p vox-orchestrator --features populi-transport donations_vox` → FAIL (crate not yet a dep).
- [ ] **Step 2 — Add dep:** `crates/vox-orchestrator/Cargo.toml` → `vox-mesh-policy = { workspace = true }` (L3→L3 legal).
- [ ] **Step 3 — Producer swap:** in `scoring.rs:392-400`, before the existing `serde_json::from_str::<WorkerDonationPolicy>(VoxMeshDonationPolicyJson)`, attempt `vox_mesh_policy::load_policy(<resolved donations.vox path>)`; use it if `Ok`, else fall back to the JSON secret. Resolve the path from a new optional secret `VoxMeshDonationPolicyPath` (or config dir helper). The downstream consumer (`policy.public_mesh_opt_in`, `scoring.rs:396`) is unchanged — `load_policy` returns the same `vox_mesh_types::WorkerDonationPolicy`.
- [ ] **Step 4 — Green:** `cargo test -p vox-orchestrator --features populi-transport donations_vox` → PASS.
- [ ] **Step 5 — Commit:** `feat(orchestrator): load donations.vox via vox-mesh-policy with JSON-secret fallback (B11)`

> Follow-up (out of slice, flag in PR): the same JSON-only ingestion exists at `vox-ml-cli/.../populi_cli.rs:565` and `vox-populi/src/lib.rs:194`; pointing them at `load_policy` too lets `orphan_exempt`/`staleness_exempt` be lifted from `layers.toml:165`.

### Task 4 (B3): Wire Task Hopper into the daemon (intake + read endpoints)

**Pre-req read:** inspect `crates/vox-orchestrator-d/src/` for the daemon app-state struct and whether it owns the shared `Arc<EventBus>` (the `HopperIntake` trait at `crates/vox-orchestrator/src/hopper/store.rs:38` is the program-against surface; `InMemoryHopper::new(bus)` at `store.rs:88`).

- [ ] **Step 1 — Failing integration test:** in `crates/vox-orchestrator-d/tests/hopper_intake.rs` (new): start the daemon test harness, `POST /api/v2/hopper/submit` a JSON intent, then `GET /api/v2/hopper/inbox` and assert the returned item id matches. Run → FAIL (route 404).
- [ ] **Step 2 — Construct in app state:** at daemon startup build `let hopper: Arc<dyn HopperIntake> = Arc::new(InMemoryHopper::new(event_bus.clone()));` and store in the daemon state struct.
- [ ] **Step 3 — Add handlers (route convention `/api/v2/<surface>`):**
  - `POST /api/v2/hopper/submit` → `hopper.submit(intent, affinity_hints, priority_hint, source, session_id).await` → return `IntakeItem`.
  - `GET /api/v2/hopper/inbox|assigned|history` → corresponding trait reads.
  - `POST /api/v2/hopper/reprioritize` → guarded by a `DeveloperOverride` capability (`store.rs:62-67`).
- [ ] **Step 4 — Green:** rerun the integration test → PASS. `submit` emits `AgentEventKind::HopperItemAdmitted` (`store.rs:116`) on the bus.
- [ ] **Step 5 — Commit:** `feat(orchestrator-d): wire InMemoryHopper intake + read HTTP surface (B3, Hp-T1)`

> Persistent `hopper_inbox` table (Hp-T5) is intentionally NOT built here; the trait keeps the later swap a one-liner. Do not add a vox-db migration in this slice.

### WS-1 adversarial verify gate
Reviewers must: run `cargo test -p vox-populi -p vox-orchestrator -p vox-distributed-training`, `cargo build -p vox-orchestrator-d`, `cargo run -p vox-arch-check`; confirm the two populi forks are gone, no new orphan, and the hopper endpoints actually round-trip. Refute "B3 is a real intake, not a stub" by exercising the POST→GET path.

---

## WS-2 — Scientia decision / social / review

**Files:**
- Modify: `crates/vox-scientia/src/inspect_bridge/novelty.rs`, `crates/vox-publisher/src/publication_worthiness.rs:328`, `crates/vox-research-events/src/publication_format.rs:69`, `crates/vox-publisher/src/adapters/twitter.rs`
- Create: `crates/vox-publisher/src/syndicate.rs`, `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx`
- Modify (GUI, after WS-3): `decoratorRegistry.ts`, `App.tsx`, `contracts/gui/surface-registry.v1.yaml`, `ScientiaDashboard.tsx`
- Test: `crates/vox-scientia/src/inspect_bridge/novelty.rs` (unit), `crates/vox-publisher/tests/scientia_novelty_acceptance.rs`, `crates/vox-gui/ui/...` (vitest)

Landing order: **B6 → B5 ; B7 (independent) ; A1+A6 (after WS-3 registry) ; B10 (last, gated on grep)**.

### Task 1 (B6): Add `InsufficientEvidence` + `Contradicted`; make scorer retrieval-aware

- [ ] **Step 1 — Failing test** in `crates/vox-scientia/src/inspect_bridge/novelty.rs` tests:
  ```rust
  #[test]
  fn failed_retrieval_is_insufficient_evidence() {
      let bundle = bundle_with_all_failed_traces(); // all query_traces non-2xx
      assert!(matches!(AtomicNoveltyScorer::default().score(&bundle),
          NoveltyVerdict::InsufficientEvidence { .. }));
  }
  ```
  Run `cargo test -p vox-scientia failed_retrieval_is_insufficient_evidence` → FAIL.
- [ ] **Step 2 — Extend enum** (`novelty.rs:10-20`): add `InsufficientEvidence { reason: String }` and `Contradicted { conflicting_uri: String }`.
- [ ] **Step 3 — Retrieval gate in `score()`** (`novelty.rs:57`): if `bundle.query_traces` is non-empty and every trace failed (`!t.http_ok()`), return `InsufficientEvidence` before the similarity ladder. (Confirm the per-trace status field name in `crates/vox-research-events/src/schema_types.rs` first.) Leave `Contradicted` emitted from B5, not here.
- [ ] **Step 4 — Green** → PASS. Keep existing 5 tests; adjust `empty_bundle_is_novel` so a no-trace empty bundle stays `Novel` (genuinely no signal ≠ failed fetch).
- [ ] **Step 5 — Commit:** `feat(scientia): NoveltyVerdict gains InsufficientEvidence/Contradicted; retrieval-failure no longer maps to Novel (B6)`

### Task 2 (B5): Wire the decision layer into the worthiness path

- [ ] **Step 1 — Failing acceptance test** in `crates/vox-publisher/tests/scientia_novelty_acceptance.rs`: an empty/failed bundle must NOT max `inputs.novelty` (regression for the false positive); a future-dated hit is dropped by `ChronoFilter`; opposing-polarity hits drive a `Contradicted` cap. Run → FAIL.
- [ ] **Step 2 — Bundle adapter:** add a `fn to_scientia_bundle(v1: &NoveltyEvidenceBundleV1) -> vox_research_events::schema_types::NoveltyEvidenceBundle` in `vox-publisher` (the two bundle types differ; this is the real wiring cost — minimal adapter, not the §8 consolidation).
- [ ] **Step 3 — Insert the typed scorer** in `apply_prior_art_to_worthiness_inputs` (`publication_worthiness.rs:328`), BEFORE the existing scalar `novelty_inputs_adjustment` blend:
  1. `ChronoFilter` over hits (drop future-dated).
  2. `AtomicNoveltyScorer::default().score(&adapted)`.
  3. `EvidenceConflictDetector` (threshold 0.8) over polarized hits; on conflict override verdict → `Contradicted { conflicting_uri }`.
  4. Translate verdict → novelty cap: `InsufficientEvidence`/`Contradicted` → do NOT treat as novel (cap low + explanatory note); `Novel/PossiblyNovel/NotNovel` → existing scalar behavior. Keep the function signature unchanged (caller `worthiness_extraction.rs:211` untouched).
- [ ] **Step 4 — Green** → PASS. (`vox-publisher` already depends on `vox-scientia` unconditionally — no new crate edge.)
- [ ] **Step 5 — Commit:** `feat(publisher): gate publication novelty on vox-scientia AtomicNoveltyScorer/ChronoFilter/ConflictDetector (B5)`

### Task 3 (B7): Fix the UTF-8 panic; add Twitter variant + `syndicate()` gate

- [ ] **Step 1 — Failing panic-regression test** in `crates/vox-research-events/src/publication_format.rs` tests: `adapt_claim_to_platform("café résumé 日本語…overlong", uri, Twitter)` must not panic and must be char-correct. Run → FAIL (panics today at `:69`).
- [ ] **Step 2 — Fix the truncation** (`publication_format.rs:68-72`): replace the byte slice `&claim_text[..max.saturating_sub(11)]` with grapheme/char-safe truncation (reuse the `unicode_segmentation` logic mirrored in `twitter.rs:106-115 truncate_chars`).
- [ ] **Step 3 — Add `Twitter` platform** to `PublicationPlatform` (`:25-30`, max 280) + its `max_chars()` arm; **update** the `bluesky_prioritized_over_x_in_platform_enum` test (`:140`) which currently asserts X does not exist. Check `contracts/scientia/*.schema.json` for an enum constraint and regenerate if present.
- [ ] **Step 4 — `syndicate()` seam:** create `crates/vox-publisher/src/syndicate.rs`:
  ```rust
  pub fn syndicate(claim: &ApprovedClaim, channel: PublicationPlatform, token: &ApprovalToken)
      -> Result<ShortFormVariant>;  // refuses without a valid P2 ApprovalToken bound to the content digest
  ```
  It calls `adapt_claim_to_platform → validate_short_form`, preserving the Trusty URI. Add `twitter::post_variant(cfg, token, variant, dry_run)` posting `variant.adapted_text` through the existing `/2/tweets` body builder. Reuse the **same `ApprovalToken`** type as P2 review (`vox-cli-core/src/scientia.rs:462`).
- [ ] **Step 5 — Tests:** `syndicate()` refuses without token; Trusty URI byte-identical in variant and posted text. Green.
- [ ] **Step 6 — Commit:** `feat(publisher): char-safe short-form truncation + Twitter variant + approval-gated syndicate() (B7)`

### Task 4 (A6): Scientia cost panel (fold into dashboard — no registry churn)

- [ ] **Step 1 — Failing vitest** under `crates/vox-gui/ui/.../Scientia/`: mock `invoke('execute_command', {path:['scientia','cost']})` → known `CostRollup` JSON → assert per-provider totals render. Run → FAIL.
- [ ] **Step 2 — Implement** a cost section inside `ScientiaDashboard.tsx` (already the `scientia` decorator): `const out = await invoke<ExecuteOutput>('execute_command', { path:['scientia','cost'], args:{} }); const rollup = JSON.parse(out.stdout) as CostRollup;` Render `CostByProvider[]` + `QuarterlyCostSummary`. Use the CLI bridge (GUI has no HTTP gateway dep). Guard the empty/zero state.
- [ ] **Step 3 — Green** → PASS. No `surface-registry` change (folded into existing decorator) → no gate exposure.
- [ ] **Step 4 — Commit:** `feat(gui): Scientia cost rollup panel via execute_command bridge (A6)`

### Task 5 (A1): GUI DiscoveryReview surface  *(rebase on WS-3 registry generator)*

- [ ] **Step 1 — Create** `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx`: load claims (`scientia/claims`), per-claim Approve/Reject/Defer → `execute_command(['scientia','publication-claim-review'], {__argv:[...]})`; on approve enable "Build nanopub" → `execute_command(['scientia','publication-nanopub-build'], {__argv:[...]})`; show returned Trusty URI. (Mirror `PublicationsView.tsx:124-141` — no new Tauri command needed.)
- [ ] **Step 2 — Register decorator:** add `review: DiscoveryReviewView` to `surfaceDecorators` in `decoratorRegistry.ts:37-57`.
- [ ] **Step 3 — Register view key:** add `'review'` to the `ViewKey` union and nav allow-list in `App.tsx` (region `:51-62`, `:241`). No `case` needed (decorators consulted first, `:553`).
- [ ] **Step 4 — SSOT + regen:** add a `view_key: review` entry (`representation_tier: curated_decorator`, `nav_group: knowledge`) to `contracts/gui/surface-registry.v1.yaml` mirroring `claims`; run `vox ci gui-surface-registry --write` to regenerate `surfaceRegistry.generated.ts`.
- [ ] **Step 5 — Tests:** vitest for the argv builder; the Playwright registry-driven sweep auto-enrolls the new `review` key. Run `vox ci gui-surface-registry` (no `--write`) → PASS (decorator↔registry parity).
- [ ] **Step 6 — Commit:** `feat(gui): DiscoveryReview surface for human-gated claim review + nanopub build (A1)`

### Task 6 (B10): Retire `vox-nanopub` — grep-gated DELETE

- [ ] **Step 1 — Hand-grep gate (MANDATORY, per `feedback_verify_audit_retirement_claims`):** `git grep -n "vox_scientia::nanopub::\|NanopubDocument\|build_nanopub\|sign_nanopub\|scientia_scholarly::"` excluding `#[cfg(test)]`. If ANY production consumer resolves a Trusty URI through the `vox_scientia::nanopub` re-export (`crates/vox-scientia/src/nanopub/mod.rs:12-15`), **downgrade to migrate-then-delete** (route them to `spec.rs` first) or DEFER to §8 consolidation. Record the grep output in the PR.
- [ ] **Step 2 — If clean:** remove the `pub use vox_nanopub::{...}` re-exports (`mod.rs:12-15`); drop the dep (`vox-scientia/Cargo.toml:16`); delete `crates/vox-nanopub/`; update root `Cargo.toml:54`, `layers.toml:119`, `.config/hakari.toml:47,79`, `contracts/documentation/link-allowlist.v1.yaml:427`.
- [ ] **Step 3 — Regenerate** (never hand-edit): `where-things-live.md`, `SUMMARY.md`, `docs/agents/doc-inventory.json`, `contracts/reports/test-inventory.v1.json` via their generators.
- [ ] **Step 4 — Build + arch-check + doc-pipeline:** `cargo build -p vox-scientia && cargo run -p vox-arch-check` → PASS.
- [ ] **Step 5 — Commit:** `refactor(scientia): retire orphaned vox-nanopub leaf crate; spec.rs is the Trusty-URI SSOT (B10)`

### WS-2 adversarial verify gate
Reviewers run `cargo test -p vox-scientia -p vox-publisher -p vox-research-events`, the GUI vitest + `vox ci gui-surface-registry`; refute "empty retrieval can no longer publish as Novel" by feeding a failed bundle end-to-end; confirm the UTF-8 panic is gone with a multibyte fuzz input; confirm `syndicate()` refuses without a token.

---

## WS-3 — GUI gate integrity + async research

**Files:**
- Modify: `crates/vox-gui/src/commands/models.rs:193`, `crates/vox-gui/src/main.rs:95`, `crates/vox-cli/src/commands/ci/gui_surface_coverage.rs:54`, `crates/vox-cli/src/commands/ci/gui_surface_registry.rs:149`, `crates/vox-cli/src/command_catalog.rs`
- Create: `crates/vox-gui/src/commands/research.rs`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx`
- Test: gate `#[cfg(test)] mod tests` blocks, `crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs`, Research vitest

Landing order: **A7 → A5 → A3 (gate Track1→Track2) → A2**.

### Task 1 (A7): Remove the dead `get_routing_summary` registration

- [ ] **Step 1 — Remove `#[tauri::command]`** on `get_routing_summary` (`models.rs:193`) so it is a plain `pub async fn` (still called by `get_routing_summary_live`, `:328`).
- [ ] **Step 2 — Drop from handler list:** remove `commands::models::get_routing_summary,` from `generate_handler!` (`main.rs:95`); keep `_live` (`:96`).
- [ ] **Step 3 — Tidy e2e mock:** drop the `case 'get_routing_summary':` mock in `crates/vox-gui/ui/e2e/screenshots.spec.ts:138`.
- [ ] **Step 4 — Build:** `cargo build -p vox-gui` → PASS (function still referenced by `_live`).
- [ ] **Step 5 — Commit:** `fix(gui): remove dead get_routing_summary IPC registration; keep internal helper (A7)`

### Task 2 (A5): Make the coverage parser decorator-aware

- [ ] **Step 1 — Refactor for testability + failing test:** change `parse_gui_routes` (`gui_surface_coverage.rs:51`) to take `(app_src, decorator_src): (&str, &str)` (the `run` fn does the `fs::read`). Add a `#[cfg(test)] mod tests` with a fixture containing `case 'dashboard':` and `surfaceDecorators ... = { scientia: X, research: Y }` and assert the result contains `dashboard`, `scientia`, `research`. Run → FAIL.
- [ ] **Step 2 — Implement:** add `const GUI_DECORATOR_REGISTRY` path constant; after the App.tsx `case` regex, slice `decoratorRegistry.ts` to the `surfaceDecorators` object literal and extract keys with `^\s*([a-z][a-z0-9]*):\s` (multiline), unioning into the same `BTreeSet`. Scope the slice to avoid `SurfaceDecoratorProps`/`pushToast:` false positives.
- [ ] **Step 3 — Green** → PASS.
- [ ] **Step 4 — Regenerate report:** `vox ci gui-surface-coverage --write` → `contracts/reports/gui-surface-coverage.v1.json` `gui_routes` grows by the 9 decorator keys.
- [ ] **Step 5 — Commit:** `fix(ci): gui-surface-coverage counts decorator-driven surfaces (A5)`

### Task 3 (A3): Make the surface gate feature-aware; waive panels

- [ ] **Step 1 — Failing test** in `gui_surface_registry.rs` tests: assert the top-level group set contains `"dei"`, `"visus"`, `"safety"`, `"attention"` even in a default (non-`dei`) build. Run → FAIL (clap reflection omits cfg'd variants).
- [ ] **Step 2 — Track 1 (gate sees them):** add `pub fn feature_gated_group_names() -> Vec<(&'static str,&'static str)>` to `command_catalog.rs` returning gated `(group, feature)` pairs — **derive from `command_contract::merged_feature_gate`** (consulted at `command_catalog.rs:318`) if it already enumerates gated paths; else hand-list `[("dei","dei"),("visus","dei"),("safety","dei"),("attention","dei"),…]` plus a test that each name is a real (possibly cfg'd) command. In `top_level_groups_from_catalog()` (`gui_surface_registry.rs:149`) union these with the compiled groups → `missing_groups` now flags them. Green.
- [ ] **Step 3 — Track 2 (classify SSOT):** `vox ci gui-surface-registry --write` backfills the new groups as `representation_tier: none`; then set authored `notes:` on each (e.g. `"dei-gated engine; CLI-only by design pending GUI panel — see A3"`) in `contracts/gui/surface-registry.v1.yaml` and re-`--write` to regenerate the TS + report. Gate now passes honestly.
- [ ] **Step 4 — Enforce in CI:** `vox ci gui-surface-registry` (no `--write`) → PASS, now covering dei groups.
- [ ] **Step 5 — Commit:** `fix(ci): gui-surface-registry is feature-gate aware; Visus/Safety/Attention classified with waiver (A3)`

> Track 3 — actual Visus/Safety/Attention GUI panels — is DEFERRED to its own spec→plan; flip tiers from `none`→`curated_decorator` via `--write` when built.

### Task 4 (A2): Wire async research through the persistent daemon

- [ ] **Step 1 — Failing Rust test** in `crates/vox-orchestrator-mcp/tests/daemon_extra_tests.rs`: assert `RESEARCH_RUN` returns `{session_id, task_id, status:"running"}` and creates a session row. Run → confirm baseline.
- [ ] **Step 2 — New Tauri command** `crates/vox-gui/src/commands/research.rs`:
  ```rust
  #[tauri::command]
  pub async fn start_research_async(daemon: State<'_, PersistentDaemon>, query: String,
      scope: Option<String>, max_sources: Option<u32>, verify_claims: Option<bool>)
      -> Result<serde_json::Value, String> {
      let addr = daemon.ensure().await.map_err(|e| e.to_string())?;
      OrchDaemonClient::new(addr)
          .call(dei_method::RESEARCH_RUN,
                json!({"query":query,"scope":scope,"max_sources":max_sources,"verify_claims":verify_claims}))
          .await.map_err(|e| e.to_string())
  }
  ```
  **Hazard:** use `PersistentDaemon::ensure()` + `OrchDaemonClient` (long-lived daemon, like `orchestrator.rs:37`), NOT `control_plane.rs::call_daemon` (one-shot stdio daemon — would kill the pipeline mid-flight).
- [ ] **Step 3 — Register:** add `mod research;` to `commands/mod.rs` and `commands::research::start_research_async` to `generate_handler!` in `main.rs`.
- [ ] **Step 4 — UI rewire** `ResearchView.tsx`: replace `run()` (`:29-50`) to `invoke('start_research_async', {...})`, set a "running" session and STOP blocking; replace the inline block (`:72-77`) with a running indicator; add `useEffect(() => listenScientiaQueue(() => loadHistory()))` (the watcher already pings research-session transitions, `scientia.rs:119-189`) with a 10s interval fallback (mirror `ScientiaDashboard.tsx:86-98`). Read the answer from `detail.report_markdown ?? detail.artifact_json` once status reaches `completed`.
- [ ] **Step 5 — Tests:** vitest asserts `run()` calls `start_research_async` (not `execute_command`) and does not block; the subscription triggers `loadHistory`. Green.
- [ ] **Step 6 — Commit:** `feat(gui): Research runs async via persistent daemon executor + status poller (A2)`

### WS-3 adversarial verify gate
Reviewers run the gate test suites + `vox ci gui-surface-registry` + `vox ci gui-surface-coverage` (no `--write`) and confirm they now bite on dei groups / decorator surfaces; refute "A2 no longer blocks the UI" by confirming `start_research_async` returns immediately and the persistent (not one-shot) daemon is used.

---

## WS-4 — Desktop speech-to-text

**Files:**
- Create: `crates/vox-gui/src/commands/oratio.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `crates/vox-gui/src/main.rs:75`, `crates/vox-gui/Cargo.toml`, `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx:264-283`
- Test: `crates/vox-gui/src/commands/oratio.rs` (unit), Loquela vitest

Landing order: **A4 only.** B8, B9 DEFERRED (register).

### Task 1 (A4): `oratio_transcribe` Tauri command + Loquela mic button

- [ ] **Step 1 — Failing unit test** in `crates/vox-gui/src/commands/oratio.rs`: map a `.txt`/`.md` fixture path → plugin STT → `TranscribeResultDto` (no mic, no model) — mirror `vox-speech traits.rs:286 txt_fixture_roundtrip` and `search.rs:366-557`. Run → FAIL.
- [ ] **Step 2 — Implement the command:**
  ```rust
  #[tauri::command]
  pub async fn oratio_transcribe(seconds: f32) -> Result<TranscribeResultDto, String> {
      // temp WAV in std::env::temp_dir(); capture via record_default_input_wav (spawn_blocking);
      // transcribe via cached_code_plugin("oratio").as_speech_to_text().transcribe_path(path, cfg_json);
      // delete temp; return { raw_text, refined_text, text }
  }
  ```
  Mirror `oratio_cmd.rs:358-396`. Wrap sync cpal in `tokio::task::spawn_blocking` (`asr_backend.rs:29`).
- [ ] **Step 3 — Deps behind a default-off feature:** add a `oratio` cargo feature to `crates/vox-gui/Cargo.toml` pulling `vox-ml-cli` (`features=["oratio-mic"]`, exposes `record_default_input_wav`) + `vox-speech` + `vox-plugin-host`. Keep it OUT of default to keep the GUI build lean. Register `pub mod oratio;` (`commands/mod.rs`) and the handler (`main.rs:75`).
- [ ] **Step 4 — Green** (the fixture-path test runs without mic/model) → PASS.
- [ ] **Step 5 — UI:** in `Loquela.tsx:264-283` replace the toast-only handler with `setRecording(true) → await invoke('oratio_transcribe',{seconds:5}) → append result.text to the textarea (value at :288) → setRecording(false)`; remove `aria-disabled`; keep the toast as the error path (no mic / plugin missing / GUI built without `oratio`).
- [ ] **Step 6 — UI vitest:** mock `invoke('oratio_transcribe')` → assert textarea receives text, toast fires on reject. Green.
- [ ] **Step 7 — Commit:** `feat(gui): desktop speech-to-text via oratio_transcribe (cpal capture + Oratio plugin) (A4)`

### WS-4 adversarial verify gate
Reviewers confirm the default GUI build does NOT pull ML deps (feature off), the fixture-path test passes in CI, and the mic button is no longer a hardcoded stub. Native mic + Whisper inference are acknowledged untestable in CI (manual/local).

---

## Deferred Items Register (with unblock criteria)

| ID | Why deferred | Unblock criteria | Owner spec |
|----|--------------|------------------|-----------|
| B4 (HopperSync routing) | Needs a hopper to apply into (B3) AND cross-daemon durability (Hp-T5 persistent `hopper_inbox`). No envelope-receive dispatch loop exists; faking a match arm would be a new stub. | B3 merged + an `OpFragmentEnvelope` receive loop + `HopperIntake::replay_admitted(...)` added to the trait. | new: `2026-…-hopper-mesh-sync.md` |
| B8 (RN runtime methods) | Generated uniffi bindings do not contain `spawnActor/startWorkflow/infer`; blocked on `vox-actor-runtime`/`vox-workflow-runtime` adopting `Suspendable` (spec §13) + Candle mobile cross-compile (spec §15). | Runtime `Suspendable` adoption + ML cross-compile pipeline; then `#[uniffi::export]` the methods and regenerate. | mobile-rn-expo spec §13/§15 |
| B9 (native mobile STT) | `vox-tauri-stt` has no `jni`/`swift-bridge` dep; Android `SpeechRecognizer`/iOS `SFSpeechRecognizer` are async-callback, permission-gated, untestable in CI. A4 delivers the desktop equivalent. | Tauri-mobile plugin packaging + FFI bridge + async-callback→Future adapter. | new mobile-STT spec |
| A3-panels (Visus/Safety/Attention GUI) | Three real engines; building panels is separable from making the gate honest (done in WS-3). | Decorator components + registry entries; flip tier `none`→`curated_decorator` via `--write`. | new dei-surfaces spec |

**Do not ship any of these as stubs.** Each interim state is an explicit typed error or a classified `representation_tier: none` waiver — both sanctioned, both visible.

---

## Decisions Needed From Maintainer

1. **B10 path:** if the grep finds production consumers of the `vox_scientia::nanopub` re-export, prefer (a) migrate-then-delete in the same PR, or (b) DEFER to the §8 Trusty-URI consolidation? (Plan assumes: delete if clean, else migrate.)
2. **B11 path resolution:** canonical `donations.vox` location — config dir, or a new `VoxMeshDonationPolicyPath` secret? Precedence vs the existing JSON secret? (Plan assumes: `.vox` file wins, JSON secret is fallback.)
3. **A3 catalog SSOT:** derive `feature_gated_group_names()` from `command_contract` (single source) vs a hand-maintained list + drift test? (Plan assumes: derive if the map exists, else list+test.)
4. **A4 dep weight:** accept `vox-ml-cli`/`vox-speech` pulled into the GUI behind a default-off `oratio` feature, vs a thinner direct `cpal`+`hound` capture copy? (Plan assumes: reuse `vox-ml-cli` behind the feature — no duplication.)

---

## Self-Review

**Spec coverage:** all 18 audit findings appear in the Decision Summary and map to a task (13) or the Deferred Register (4) or a grep-gate (1: B10). ✔
**Placeholder scan:** every task names exact files + the actual change; code blocks are real signatures, not "TODO". Mechanical GUI-registration tasks give the exact registry/App.tsx/decorator edits. ✔
**Type consistency:** `HopperIntake`/`InMemoryHopper`, `NoveltyVerdict` variants, `ApprovalToken` (shared P2↔P5), `OrchDaemonClient`/`PersistentDaemon`/`dei_method::RESEARCH_RUN`, `TranscribeResultDto`, `record_default_input_wav` are used consistently across tasks. ✔
**Cross-cutting hazards encoded:** generated-file discipline (`--write`, never hand-edit), no-stubs policy, Windows fmt, the one-shot-vs-persistent daemon trap (A2), the GUI-registry serialization between WS-2/3/4. ✔
