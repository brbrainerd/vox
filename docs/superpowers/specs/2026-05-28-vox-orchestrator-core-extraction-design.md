---
title: "vox-orchestrator-core extraction design (2026-05-28)"
description: "Trigger-gated design for splitting vox-orchestrator into a `-core` crate. Supersedes the 2026-05-15 tier-d plan. Verified D0 measurement (2026-05-28): 61,061 LoC, 12.8% headroom, Rule 13 not firing. D1 manifest committed: 31 Move / 17 Trait-inject / 49 Stay across 97 sibling modules."
last_updated: "2026-05-28"
category: "Architecture SSOTs"
status: "current"
---

# vox-orchestrator-core extraction design

**Supersedes:** [`2026-05-15-orchestrator-tier-d-plan.md`](../../src/architecture/2026-05-15-orchestrator-tier-d-plan.md) (kept as historical context; this doc is the execution surface).

**Spec posture:** trigger-gated. The plan derived from this spec is fully written and shelf-ready, but its first step is a measurement gate that exits the plan if the conditions aren't met. Today the gate is not met (see §1).

## 1. Trigger gate — verified state (2026-05-28)

| Signal | Threshold | Current | Status |
|---|---|---:|---|
| Rule 13 finding for `vox-orchestrator` | absent | absent | ✅ Not tripped |
| `max_loc` headroom | <5% of `max_loc = 70,000` (<3,500 LoC remaining) | 12.8% (~8,940 LoC) at 61,061 LoC (D0 canonical, 2026-05-28) | ✅ Not tripped |
| `vox-arch-check` clean on `main` | clean | clean | ✅ |
| Active MENS / mesh sprint touching `vox-orchestrator/src/` | none | unknown — query before D2 | ⚠️ |

**Today's verdict:** plan exits at D0. Surgery is not warranted. Re-check at every release tag and after any feature lands that touches `vox-orchestrator/src/`.

Rule 13 fires when a budgeted crate's current LoC exceeds the LoC at the last release tag by >15%. The exact LoC threshold therefore depends on what `vox-orchestrator` was at the `v0.5.0` tag — a number we don't pre-compute here. D0 reads the threshold from `vox-arch-check`'s output rather than hard-coding it.

The accuracy of these measurements is itself a recent guarantee — `vox-arch-check` Rule 13 was line-count-buggy until commit `760dae75da` (2026-05-27). Before that, headroom readings could under-report by 1 line per file without a trailing newline. The gate is now reliable.

## 2. What this extracts

`crates/vox-orchestrator/src/orchestrator/` (the densest subdir, 12,933 LoC current) into a new `crates/vox-orchestrator-core/` crate, along with whatever sibling modules the D1 audit determines must co-move.

The `Orchestrator` struct itself (defined at [crates/vox-orchestrator/src/orchestrator.rs:53](../../../crates/vox-orchestrator/src/orchestrator.rs:53)) moves to `vox-orchestrator-core`. The current `vox-orchestrator` crate retains daemon glue, a2a transport, runtime entry points, session management, hopper, routing, legacy shims, and integration glue — becoming a thinner shell that re-exports the new crate's surface for backward compatibility with `vox-cli`, `vox-orchestrator-mcp`, and other consumers.

Estimated post-split: `vox-orchestrator-core` ~30–35K LoC, `vox-orchestrator` ~25–30K LoC. Both well below their post-split `max_loc` budgets.

## 3. Why this is the right wedge

The 2026-05-15 audit considered two paths:
- **Vertical slice on `agentos/`** — rejected; only 358 LoC across 8 stubs.
- **`orchestrator/` subdir** (this design) — densest subdir, contains the Orchestrator inherent impls, primary source of LoC pressure.

The Rust coherence constraint forces the path: inherent `impl Orchestrator { … }` blocks must live in the same crate as the struct definition. To shrink `vox-orchestrator`, we move the struct and its impls together. Without the coherence constraint, there would be cheaper extractions; with it, the extraction is non-decomposable.

Sharper detail from this session's verification (about files **inside** `src/orchestrator/`, not the sibling subdirs of `src/`): only **16 of 50 files** in the subdir actually contain `impl Orchestrator { … }` blocks. The other 34 are pure helpers (agent ops, core utilities, persistence, integration tests). Helpers can technically stay in `vox-orchestrator` IF they don't import the struct directly — but in practice they all consume `Orchestrator` instances through method calls, so they migrate with the struct unless they're refactored behind a trait. The sibling-subdir analysis (which 19 subdirs of `src/` must co-move) is a separate question, handled in §4 D1.

## 4. Phased plan (D0–D7)

### D0 — Gate check (5 min)

Run `cargo run -p vox-arch-check 2>&1 | grep -E "vox-orchestrator"` and confirm:
1. No Rule 13 (LoC-delta) finding for `vox-orchestrator`
2. The reported LoC for `vox-orchestrator` leaves >5% headroom against `max_loc = 70_000` (i.e., reported LoC < 66,500)

If either fails: continue to D1. If both pass: exit; re-check after next major feature lands in `vox-orchestrator/src/`. (Headroom-only failure also warrants extraction — it means the budget cap is about to bite even if Rule 13 hasn't fired against the release-tag baseline.)

### D1 — Co-move manifest audit (1–2h)

Read-only measurement. Output: a per-module table in this spec doc with these columns:

| Module | LoC | `crate::` imports from `src/orchestrator/` | Has `impl Orchestrator`? | Is `Orchestrator` struct field? | Decision |

Classification rules:
- **Move** if: has `impl Orchestrator` OR is an `Orchestrator` struct field type OR is imported by >5 files in `src/orchestrator/`
- **Trait-inject** if: imported by 1–5 files, no `impl Orchestrator`, not a struct field, interface ≤5 methods
- **Stay** if: imported by 0 files in `src/orchestrator/`

Method: `cargo check -p vox-orchestrator` iteratively after speculative move of each candidate (not grep alone — grep misclassified 34/50 files in the 2026-05-15 plan).

Verified inputs from this session (so D1 doesn't have to re-discover):
- Sibling modules currently imported by `src/orchestrator/`: `affinity`, `attention`, `budget`, `bulletin`, `catalog`, `config`, `context`, `groups`, `locks`, `models`, `oplog`, `planning`, `queue`, `scope`, `services`, `snapshot`, `socrates`, `topology`, `types` (19 modules; all still imported as of 2026-05-28)
- External crates that import `Orchestrator` directly: `vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs:4` and `tests.rs:2` (only)

D1 expected outcome (pre-audit estimate; **superseded by the manifest in §D1 manifest below**):
- ~10 move, ~3–4 trait-inject, ~3–4 stay

**Actual D1 outcome (audited 2026-05-28, committed `85899c0237`):**
- **31 Move / 17 Trait-inject / 49 Stay** across 97 candidates
- The pre-audit estimate enumerated only 23 directory modules; the audit added 74 single-`.rs`-file modules at the top level of `src/` (see expansion note in the manifest)
- Module count is ~3× the estimate; LoC volume (Move sum = 18,741) still falls within the spec §2 post-split projection of ~30–35K for `vox-orchestrator-core` (Move + `src/orchestrator/` = 18,741 + 12,933 = 31,674)

## D1 manifest (audited 2026-05-28)

Generated by following the plan's D1 steps on the current main HEAD (commit after D0: `82b909af8a` gate-not-tripped recorded).

**Expansion note (later same day):** the original audit query enumerated only subdirectories of `src/`, missing top-level `.rs` files (single-file modules). Re-run with the union of dirs + `.rs` files brings the candidate count from 23 to 97. Newly-added rows are marked with `(file-mod)` in the Module column.

**Struct-field detection note:** the plan's Step 4 regex (`\b$mod::\w+\b` against the raw struct body) misses struct fields whose module types were imported via `use crate::<mod>::<Type>` at the top of `orchestrator.rs` and then used by short name in the struct body. A corrected check (searching the whole file for `use crate::<mod>::` and the struct-body for direct `crate::<mod>::`) was applied. Modules `config`, `queue`, and `types` were identified as struct-field-sourced this way (OrchestratorConfig, AgentQueue, and {AgentId, TaskId, ...} respectively). The same corrected check was applied to all newly-added file-mod candidates.

**impl Orchestrator note:** all `impl Orchestrator { … }` blocks live inside `src/orchestrator/` itself (accessors.rs, campaigns.rs, comms.rs, lease_watchdog.rs, safety.rs, workflow_bridge.rs) — not in any sibling module directory. Step 3 returns 0 for every candidate; this column is informational.

| Module | LoC | `crate::` imports from `src/orchestrator/` | `impl Orchestrator` blocks | Struct field? | Decision |
|---|---:|---:|---:|:---:|:---:|
| types | 1418 | 89 | 0 | yes (AgentId, TaskId, …) | **Move** |
| a2a | 2909 | 14 | 0 | yes (MessageBus) | **Move** |
| models | 3235 | 14 | 0 | yes (ModelRegistry) | **Move** |
| config | 2564 | 6 | 0 | yes (OrchestratorConfig) | **Move** |
| context | 278 | 6 | 0 | yes (ContextStore) | **Move** |
| budget | 902 | 5 | 0 | yes (BudgetManager) | **Move** |
| queue | 733 | 5 | 0 | yes (AgentQueue) | **Move** |
| agentos | 306 | 2 | 0 | yes (AgentosPolicyLedger) | **Move** |
| bulletin (file-mod) | 116 | 2 | 0 | yes (BulletinBoard) | **Move** |
| budget_gate (file-mod) | 266 | 2 | 0 | yes (OrchestratorBudgetGate) | **Move** |
| conflicts (file-mod) | 296 | 4 | 0 | yes (ConflictManager) | **Move** |
| context_lifecycle (file-mod) | 602 | 9 | 0 | no | **Move** |
| contract (file-mod) | 76 | 8 | 0 | no | **Move** |
| events (file-mod) | 859 | 42 | 0 | yes (EventBus) | **Move** |
| groups (file-mod) | 480 | 2 | 0 | yes (AffinityGroupRegistry) | **Move** |
| heartbeat (file-mod) | 404 | 2 | 0 | yes (HeartbeatMonitor) | **Move** |
| judge_model (file-mod) | 62 | 6 | 0 | yes (JudgeModel) | **Move** |
| lineage (file-mod) | 33 | 33 | 0 | no | **Move** |
| monitor (file-mod) | 122 | 2 | 0 | yes (AiMonitor) | **Move** |
| populi_federation (file-mod) | 155 | 3 | 0 | yes (RemotePopuliRoutingHint) | **Move** |
| populi_remote (file-mod) | 127 | 12 | 0 | no | **Move** |
| privacy_router (file-mod) | 177 | 6 | 0 | yes (PrivacyRouter) | **Move** |
| qa (file-mod) | 74 | 3 | 0 | yes (QARouter) | **Move** |
| reconstruction (file-mod) | 326 | 39 | 0 | no | **Move** |
| scope (file-mod) | 260 | 3 | 0 | yes (ScopeGuard) | **Move** |
| snapshot (file-mod) | 370 | 5 | 0 | yes (SnapshotStore) | **Move** |
| socrates (file-mod) | 546 | 30 | 0 | no | **Move** |
| summary (file-mod) | 169 | 3 | 0 | yes (SummaryManager) | **Move** |
| tool_receipt (file-mod) | 192 | 3 | 0 | yes (ToolReceiptLedger) | **Move** |
| topology (file-mod) | 234 | 4 | 0 | yes (AgentDelegationBinding, DynamicSpawnContext) | **Move** |
| workspace (file-mod) | 450 | 5 | 0 | yes (WorkspaceManager) | **Move** |
| planning | 2637 | 3 | 0 | no | **Trait-inject** |
| services | 2724 | 3 | 0 | no | **Trait-inject** |
| session | 922 | 1 | 0 | no | **Trait-inject** |
| catalog (file-mod) | 853 | 1 | 0 | no | **Trait-inject** |
| catalog_classifier (file-mod) | 54 | 1 | 0 | no | **Trait-inject** |
| capability_probe (file-mod) | 3 | 3 | 0 | no | **Trait-inject** |
| context_envelope (file-mod) | 696 | 1 | 0 | no | **Trait-inject** |
| gate (file-mod) | 477 | 4 | 0 | no | **Trait-inject** |
| grounding (file-mod) | 740 | 4 | 0 | no | **Trait-inject** |
| handoff (file-mod) | 822 | 4 | 0 | no | **Trait-inject** |
| orchestration_feature_flags (file-mod) | 124 | 1 | 0 | no | **Trait-inject** |
| orchestrator_policy (file-mod) | 516 | 1 | 0 | no | **Trait-inject** |
| runtime (file-mod) | 903 | 2 | 0 | no | **Trait-inject** |
| schema (file-mod) | 10 | 1 | 0 | no | **Trait-inject** |
| search_bridge (file-mod) | 19 | 1 | 0 | no | **Trait-inject** |
| usage (file-mod) | 756 | 1 | 0 | no | **Trait-inject** |
| validation (file-mod) | 251 | 2 | 0 | no | **Trait-inject** |
| attention | 985 | 0 | 0 | no | **Stay** |
| drain_oplog | 42 | 0 | 0 | no | **Stay** |
| generated | 107 | 0 | 0 | no | **Stay** |
| hopper | 617 | 0 | 0 | no | **Stay** |
| legacy | 397 | 0 | 0 | no | **Stay** |
| mcp_tools | 50 | 0 | 0 | no | **Stay** |
| memory | 1093 | 0 | 0 | no | **Stay** |
| orch_daemon | 948 | 0 | 0 | no | **Stay** |
| preregistration | 1320 | 0 | 0 | no | **Stay** |
| retrieval | 90 | 0 | 0 | no | **Stay** |
| routing | 526 | 0 | 0 | no | **Stay** |
| spot_check | 31 | 0 | 0 | no | **Stay** |
| attachment_manifest (file-mod) | 38 | 0 | 0 | no | **Stay** |
| attention_tracker (file-mod) | 235 | 0 | 0 | no | **Stay** |
| bootstrap (file-mod) | 84 | 0 | 0 | no | **Stay** |
| cache_predictor (file-mod) | 162 | 0 | 0 | no | **Stay** |
| calibration (file-mod) | 406 | 0 | 0 | no | **Stay** |
| circuit_breaker (file-mod) | 335 | 0 | 0 | no | **Stay** |
| clarification_db_inbox_poll (file-mod) | 94 | 0 | 0 | no | **Stay** |
| compaction (file-mod) | 481 | 0 | 0 | no | **Stay** |
| compaction_trigger (file-mod) | 103 | 0 | 0 | no | **Stay** |
| confidence_fusion (file-mod) | 308 | 0 | 0 | no | **Stay** |
| continuation (file-mod) | 391 | 0 | 0 | no | **Stay** |
| entropy_scorer (file-mod) | 69 | 0 | 0 | no | **Stay** |
| fatigue_monitor (file-mod) | 96 | 0 | 0 | no | **Stay** |
| jj_backend (file-mod) | 384 | 0 | 0 | no | **Stay** |
| json_vcs_facade (file-mod) | 209 | 0 | 0 | no | **Stay** |
| lsp (file-mod) | 80 | 0 | 0 | no | **Stay** |
| mesh (file-mod) | 495 | 0 | 0 | no | **Stay** |
| mesh_federation_poll (file-mod) | 344 | 0 | 0 | no | **Stay** |
| mesh_federation_poll_noop (file-mod) | 20 | 0 | 0 | no | **Stay** |
| mode (file-mod) | 71 | 0 | 0 | no | **Stay** |
| observer (file-mod) | 467 | 0 | 0 | no | **Stay** |
| occ (file-mod) | 286 | 0 | 0 | no | **Stay** |
| pii_filter (file-mod) | 54 | 0 | 0 | no | **Stay** |
| privacy_classifier (file-mod) | 246 | 0 | 0 | no | **Stay** |
| rebalance (file-mod) | 206 | 0 | 0 | no | **Stay** |
| registry_model_resolve (file-mod) | 267 | 0 | 0 | no | **Stay** |
| research_gate (file-mod) | 38 | 0 | 0 | no | **Stay** |
| risk_matrix (file-mod) | 374 | 0 | 0 | no | **Stay** |
| route_policy (file-mod) | 28 | 0 | 0 | no | **Stay** |
| security (file-mod) | 444 | 0 | 0 | no | **Stay** |
| skill_runtime_inproc (file-mod) | 50 | 0 | 0 | no | **Stay** |
| snapshot_tests (file-mod) | 167 | 0 | 0 | no | **Stay** |
| state (file-mod) | 143 | 0 | 0 | no | **Stay** |
| subagent_dispatch (file-mod) | 295 | 0 | 0 | no | **Stay** |
| tier_cascade (file-mod) | 324 | 0 | 0 | no | **Stay** |
| usage_policy (file-mod) | 110 | 0 | 0 | no | **Stay** |
| victory (file-mod) | 105 | 0 | 0 | no | **Stay** |

**Summary:** 31 Move, 17 Trait-inject, 49 Stay. Total candidates: 97 (was 23). New rows added: 74 (23 Move, 14 Trait-inject, 37 Stay). Total Move-decision LoC: ~20,810 (original 8 dir-modules: ~12,345; new file-mod Moves: ~8,465).

Trait-inject detail: `planning`, `services`, and `session` each have 0 `pub fn` in their `mod.rs` (all functions are in sub-files) and 1–3 imports from `src/orchestrator/`. Their mod.rs public interfaces are re-export-only, meeting the ≤5-pub-fn criterion technically, though the total module pub fn count is high (25, 24, 30 respectively). D3 should validate the Trait-inject classification with a speculative `cargo check` move before committing.

### D2 — Skeleton (2h)

Create `crates/vox-orchestrator-core/`:
```
crates/vox-orchestrator-core/
  Cargo.toml      # name = "vox-orchestrator-core", layer 3, max_loc = 40_000
  src/
    lib.rs        # empty pub use surface initially
```

Add to:
- `Cargo.toml` workspace members (via glob if active; manual otherwise)
- [`docs/src/architecture/layers.toml`](../../src/architecture/layers.toml) — new `[crates.vox-orchestrator-core]` block with `layer = 3`, `max_loc = 40_000`, `max_dependents = 30`
- [`docs/src/architecture/where-things-live.md`](../../src/architecture/where-things-live.md) — new row in L3 section
- [`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml) — initial floor `vox-orchestrator-core = 40.0` (matches `vox-orchestrator`)

### D3 — Move co-move modules (1–2 days)

For each module in D1's manifest marked **Move**:
1. `git mv crates/vox-orchestrator/src/<mod> crates/vox-orchestrator-core/src/<mod>` (preserves blame)
2. `use crate::<mod>` references inside `vox-orchestrator-core` keep working (still `crate::` after move)
3. In `crates/vox-orchestrator/src/lib.rs`: add `pub use vox_orchestrator_core::<mod>;` for any types that were in the public API surface
4. Add `vox-orchestrator-core = { path = "../vox-orchestrator-core" }` to `crates/vox-orchestrator/Cargo.toml`
5. Run `cargo check -p vox-orchestrator-core && cargo check -p vox-orchestrator` after each module move; fix errors before continuing

Trait-inject candidates from D1 follow a different path: define `trait <X>` in `vox-orchestrator-core`, implement in `vox-orchestrator`, inject as `Box<dyn <X>>` field on `Orchestrator`. Do these AFTER the bulk move so the trait shapes are obvious.

### D4 — Move `orchestrator/` subdir + the `Orchestrator` struct (4–8h)

1. `git mv crates/vox-orchestrator/src/orchestrator crates/vox-orchestrator-core/src/orchestrator`
2. `git mv crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator-core/src/orchestrator.rs` (the struct definition)
3. Fix `crate::` references within moved files
4. In `crates/vox-orchestrator/src/lib.rs`: replace any direct `Orchestrator` use with `pub use vox_orchestrator_core::Orchestrator;`
5. Iterate `cargo check -p vox-orchestrator-core` and `cargo check -p vox-orchestrator` until both compile
6. Iterate `cargo check -p vox-orchestrator-mcp` to confirm the 3 external `Orchestrator` consumers still compile (they should, via the re-export)

### D5 — Integration glue (2–4h)

- `crates/vox-orchestrator/src/runtime.rs` and `crates/vox-orchestrator/src/orch_daemon/` construct `Orchestrator` instances — verify `Orchestrator::new` is accessible through the re-export
- `vox-cli` known inversion (`vox-cli → vox-orchestrator`) is unchanged; verify [`layers.toml`](../../src/architecture/layers.toml) `[[known_inversions]]` entries still match
- Smoke check: `cargo build -p vox-orchestrator-mcp` succeeds; `cargo nextest run -p vox-orchestrator-mcp model_route_policy` passes

### D6 — Tests (2h)

- Unit tests inside moved `.rs` files travel with the file automatically (`#[cfg(test)] mod tests`)
- Integration tests in `src/orchestrator/tests/` move with the subdir to `vox-orchestrator-core/src/orchestrator/tests/`
- Workspace-level integration tests under `crates/vox-orchestrator/tests/` stay put if they use `vox_orchestrator::Orchestrator` (covered by re-export); switch to `vox_orchestrator_core::` if they import internal modules
- **Test count check**: record `cargo nextest list --workspace | wc -l` before D2 and after D6; the count must increase by exactly the number of new test files created in D2 (likely zero) OR match (no tests lost in the shuffle)
- Run: `cargo nextest run --workspace --no-fail-fast` must pass; `cargo run -p vox-arch-check` must report clean

### D7 — Cleanup (1h)

- [`docs/src/architecture/layers.toml`](../../src/architecture/layers.toml): lower `vox-orchestrator` `max_loc` from `70_000` to `35_000` (or per-actual-measurement +20% headroom)
- [`docs/src/architecture/where-things-live.md`](../../src/architecture/where-things-live.md): update `vox-orchestrator` row, add `vox-orchestrator-core` row
- Append final post-split LoC numbers to this spec doc
- Mark the 2026-05-15 plan and this spec as `status: "completed"`

Total estimate when gate fires: **2–4 working days** (mostly D3 + D4).

## 5. Test strategy

Three layers:

1. **Per-file unit tests** — travel automatically with `git mv`.
2. **`src/orchestrator/tests/` integration tests** — exercise the struct; move with the struct to `vox-orchestrator-core/src/orchestrator/tests/`.
3. **`crates/vox-orchestrator/tests/` workspace integration tests** — exercise the public API; stay in `vox-orchestrator` and use the re-export.

Regression-safety bar:
- Total nextest workspace test count must not decrease
- `cargo nextest run --workspace --run-ignored default` exit 0 (4,989 pass / 1 fail is the current baseline; the 1 failure is the pre-existing `vox-oratio peak_normalize` flake, unrelated)
- `cargo run -p vox-arch-check` clean
- `vox ci coverage-gates --summary-json=…` passes including the new `vox-orchestrator-core` floor

## 6. Risk register (updated for current state)

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| D1 misclassifies a helper module (no impl, but trait/macro dep) | Medium | High — compile failure mid-D4 | D1 uses `cargo check` iteration, not grep alone. Plan accepts D1 taking 2h instead of 1h. |
| 3 `vox-orchestrator-mcp` model_route_policy files break | Low | Medium | They use `use vox_orchestrator::Orchestrator` — re-export covers them. D5 includes explicit smoke build. |
| MENS / mesh work lands during D3–D4 | High if uncoordinated | High | Block start of D2 on an active-sprint check. Don't begin during a sprint touching `vox-orchestrator/src/`. |
| Tests silently dropped in the shuffle | Medium | Medium | D6 enforces test-count check before/after. |
| `serial_test`-annotated integration tests break (89 in workspace) | Low | Medium | Verify post-D6: `cargo nextest run -p vox-orchestrator-core serial -- --test-threads=1` passes. |
| Coverage gates fail because new crate has no floor | Low | Low | D2 adds `vox-orchestrator-core = 40.0` to `coverage-gates.toml`. |
| Trait-injected modules introduce dispatch overhead in hot paths | Low | Low | Trait-inject candidates (attention, snapshot, oplog, socrates) are not in the dispatch hot path. Verify with a microbench post-D6 if performance was previously characterized. |

**Rollback**: D3–D4 happen on a feature branch. If `cargo nextest run --workspace` regresses or `vox-arch-check` flags new errors, revert the branch in one operation; no partial state lands on `main`.

## 7. Success criteria

The extraction is complete when **all** of these hold simultaneously:

- `cargo nextest run --workspace --no-fail-fast` exits with the same pass/fail/skip counts as the pre-extraction baseline (modulo the pre-existing flakes)
- `cargo run -p vox-arch-check` reports clean
- `vox ci coverage-gates --summary-json=./target/coverage-summary.json` passes the workspace floor (50%) and all per-crate floors (including the new `vox-orchestrator-core = 40.0`)
- `cargo build -p vox-orchestrator-mcp` succeeds without source edits beyond what D5 specifies
- `vox-orchestrator` LoC < 35,000
- `vox-orchestrator-core` LoC < 40,000
- The 2026-05-15 plan doc and this design doc are marked `status: "completed"` with actual post-split LoC appended

## 8. Open items (deferred to plan)

These are not design decisions; the writing-plans pass will pin them down:

- Exact list of D1 candidate modules to audit (the 19 listed in §4-D1 is the starting set; D1 may add or remove)
- Exact ordering of D3 module moves (alphabetical vs leaf-first; doesn't affect correctness, affects compile-fix cadence)
- Branch name and commit message style (workspace convention is `feat(arch): extract ...` per dei_shim precedent)
- Whether D7 should also delete the old 2026-05-15 plan or keep it as historical (recommend: keep, mark `superseded_by`)
