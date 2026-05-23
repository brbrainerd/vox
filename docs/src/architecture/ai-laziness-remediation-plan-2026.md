---
title: "AI-Laziness Audit — Findings & Pivot (2026-05-16)"
category: "Architecture SSOTs"
status: "current"
---

# AI-Laziness Audit — Findings & Pivot (2026-05-16)

**Status:** Audit investigation complete; no Phase 1 work shipped. Read this as an audit-of-audit, not as an execution plan.

**Origin:** On 2026-05-16, 9 parallel audit agents scanned the Vox codebase for AI-laziness (aspirational features, stubs, placeholders, dual-brain implementations). Findings were synthesized into a 4-track remediation plan (Phase 1 retirements → telemetry trace + dedup → `vox-code-audit` stub strip → MENS Batch 3). Verification before any execution revealed that the audit's signal-to-noise was poor enough that no track in the original plan should be executed as designed.

This doc records what was found, what was verified, and where the user should redirect effort instead.

---

## 1. Audit-of-audit summary

| Track (as audit recommended) | Verification outcome |
|---|---|
| Phase 1 retirements (10 candidates) | **0 / 10 safely deletable.** 5 were outright false positives (real production systems mis-flagged). 2 are stubs but tied to compile-time tests and generated bundle docs (feature-gate, don't delete). 1 ties to a documented anti-hallucination research initiative. 1 is wired into CI checks and frontend dependency policy. 1 is a real test fixture that needs careful relocation, not deletion. |
| Telemetry trace propagation | **Duplicate of existing in-flight plan.** Phase C of the team's 4-phase telemetry plan (`docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-c.md`) explicitly states the audit's finding (`infer.rs` reading `current_trace_ctx()` but getting a default empty context) is the **deliberate Phase B end-state** that Phase C is designed to fix. The work is already a 13-task TDD plan. |
| `vox-code-audit` stub strip | 27 `todo!()`/`unimplemented!()` claim verifiable by grep; this is the most reliable audit finding for build-out work. Not yet planned in detail. |
| MENS Batch 3 | Real, narrow `bail!("not yet wired (SP3 stub)")` claims in `vox-plugin-mens-candle-cuda/src/{checkpoint,model,inference,merge}.rs`. Phase 4 of `mesh-mens-distributed-training-and-execution-plan-2026.md` covers some of this. |

**Key meta-finding:** Audit agents are good at finding "function X bails because not yet wired" (specific, file:line-grounded). They are unreliable for "this crate is worthless" (broad scope, requires reading tests/, contracts/, CI configs). On this codebase the broad-scope claims were wrong 5+ times in a row.

---

## 2. The 10 retirement candidates — verified verdicts

| # | Item | Real status | Verdict | Evidence |
|---|---|---|---|---|
| 1 | `vox-orchestrator-d` | ADR 022 Phase B TCP daemon; imports `orch_daemon::*`; integration test at `crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs`; 81 cross-workspace references | NOT a retirement | grep across env vars / deployment / contracts |
| 2 | `vox-plugin-mens-candle-metal` | Real `MlBackend` plumbing in `src/backend.rs` (load_model, train_step, save_checkpoint, run_full_training, run_inference, merge_adapter); SP3 stub state mirrors CUDA exactly | NOT a retirement (revisit in MENS Batch 3) | Read `src/backend.rs` end-to-end |
| 3 | `apps/interop/marquee_app` | Canonical Slot 1 v1.0 marquee app, ratified by council 2026-05-15 in `contracts/marquee/manifest.v1.yaml`; load-bearing for CR-P1 / CR-P3 / CR-E2 / CR-L0 / CR-L7; audit-claimed `dist/` directory does not exist; placeholder endpoint bodies in `main.vox` are deliberate canonical showcase | NOT a retirement | Read `contracts/marquee/manifest.v1.yaml`; verify no `dist/` directory |
| 4 | `vox-integration-tests` | "Empty 4-LoC harness" referred only to `src/lib.rs`; `tests/` directory contains ~100 integration test files / 9,670 lines (orchestrator E2E 682, codegen snapshots, workflow recovery, LSP capabilities, speech audit, MCP roundtrips, Playwright golden routes) — this is Cargo's standard integration-test layout | NOT a retirement (first delete attempt was reverted) | `find crates/vox-integration-tests/tests -name "*.rs"` |
| 5 | `voxup` | Omnibus installer used in `.github/workflows/release-installers.yml` and `.github/workflows/setup-e2e.yml`; spec doc at `docs/src/architecture/voxup-omnibus-installer-spec-2026.md` | NOT a retirement | grep `.github/workflows/` |
| 6 | `vox-plugin-cloud` | Confirmed SP7 scaffold (89 LoC, all methods return `"not yet implemented"`) — BUT referenced in `vox-plugin-catalog/tests/catalog_load.rs`, `vox-plugin-api/tests/cloud_sync_compile.rs`, auto-generated bundle docs | Feature-gate, not delete | Read `src/sync.rs`; grep tests + catalog |
| 7 | `vox-plugin-script-execution` | Confirmed SP7 scaffold (82 LoC) — BUT referenced in `examples/mesh-compose.yml`, catalog tests, `docs/src/reference/feature-builds.md`, generated bundles | Feature-gate, not delete | Read `src/executor.rs`; grep examples + catalog |
| 8 | LSP "proximity alert" stub at `crates/vox-lsp/src/main.rs:107-118` | Deliberate prototype of an anti-hallucination feature; symbol pair (`resolveArenaRound` / `combatRoundResolver`) is the canonical KCH example from `docs/src/archive/research-2026-q1/research-semantic-proximity-split-brain-2026.md` and feeds `crates/vox-corpus/src/synthetic_gen/kch_anticonflation.rs:21` | NOT a retirement | grep symbol names across `docs/` and `crates/vox-corpus/` |
| 9 | `vox-plugin-noop-skill` | Real test fixture used by `vox-cli/tests/plugin_commands_smoke.rs`, `vox-plugin-host/tests/load_noop_skill.rs`, `vox-plugin-catalog/tests/catalog_load.rs` | Careful relocation (not in this round) | grep usage sites |
| 10 | `apps/experimental/visualizer` | Referenced in `contracts/frontend/surface-ownership.v1.yaml`, `contracts/frontend/dependency-policy.v1.yaml`, `contracts/marquee/manifest.v1.yaml`, `contracts/ci/check-targets.v1.yaml`, `.github/workflows/docs-quality.yml`, `.github/workflows/ci.yml`, `biome.json` | Move would break CI + frontend dep policy; not a Phase 1 move | grep `contracts/` and `.github/workflows/` |

---

## 3. Verified-real audit findings (narrow scope, file:line-grounded)

These survive the verification gate. Each one is a real bug or stub at a specific location, not a broad "delete this thing" claim.

1. **Telemetry trace context not populated in `ModelCallEvent` success path.**
   - File: `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs:504-507`
   - Emit hardcodes `task_id: None, parent_task_id: None, trace_id: None, caller_agent_id: None` even though the error path (line 612–623) correctly calls `vox_telemetry::current_trace_ctx()`.
   - **However:** this is the deliberate Phase B end-state per the existing telemetry plan (`docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-c.md`). Phase C of that plan, already authored, fixes it as a 13-task TDD plan. The same `None`-quartet pattern also appears in `crates/vox-cli/src/telemetry_corpus_feedback_sink.rs` (verified via grep).
   - **Recommendation:** Execute the existing Phase C plan rather than write a new one.

2. **arXiv submission `.tex` placeholder ignores rendered LaTeX.**
   - File: `crates/vox-publisher/src/submission/arxiv.rs:26`
   - Generates a stub `\documentclass{article}` from `PublicationManifest` instead of using `vox-manuscript-latex::render_latex()` output.
   - "Operator-assisted" is the documented submission mode; arXiv API automation is explicitly deferred. The "bug" is a pipeline gap, not a regression.
   - **Recommendation:** Decide whether arXiv automation is in v0.6 scope before fixing; if so, write a focused plan for a `SubmissionArtifact { manifest, rendered_tex, arxiv_bundle }` type.

3. **`vox-code-audit` has 27 `todo!()` / `unimplemented!()` panics across 83 files.**
   - Audit summary claims; not personally verified line-by-line yet.
   - These turn audit rule failures into crashes rather than reportable findings.
   - **Recommendation:** First task is to grep + read each one; group by rule family; classify implement-now / convert-to-warn / delete-rule.

4. **MENS plugin save/load/merge/eval-local stubs.**
   - Files: `crates/vox-plugin-mens-candle-cuda/src/{checkpoint,model,inference,merge}.rs`, `crates/vox-ml-cli/src/commands/mens/eval_local.rs:77-80`
   - Each returns `bail!("not yet wired (SP3 stub)")`.
   - The plumbing in `vox-plugin-mens-candle-metal` mirrors the CUDA shape.
   - **Recommendation:** Coordinate with `mesh-mens-distributed-training-and-execution-plan-2026.md` Mn-T1..T15 rather than writing a separate Batch 3 plan.

---

## 4. What changed in this branch

- **Nothing of substance shipped.** One commit (`d8110c6a`) was made by a fast-model subagent that deleted `vox-integration-tests`. That commit was reverted (`4ac0d0dbc`) within minutes after the diff stat revealed the deletion was 9,670 lines instead of the claimed 4. No further code changes were committed under this plan.
- This doc itself is the only artifact added by the audit cycle.

---

## 5. Recommended pivot

The user's chosen pivot was "Track 4: Telemetry trace propagation." Since that work is **already a 13-task TDD plan as Phase C of the existing telemetry plan**, the recommended action is:

1. **Read** `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-c.md` end-to-end.
2. **Verify the prerequisite** — Phase B is described as merged; confirm by checking that `ModelCallEvent` exists in `vox-telemetry/src/types.rs:386` (it does) and that infer.rs has the partial `current_trace_ctx()` integration (it does, in the error path).
3. **Execute Phase C tasks** under whichever workflow skill is appropriate (`superpowers:executing-plans` or `superpowers:subagent-driven-development` with manual verification of each implementer commit).

Other potential pivots (in rough priority order):

- **`vox-code-audit` stub strip** — verify the "27 panics across 83 files" claim by hand first; group by rule family; only then write a TDD plan.
- **MENS Batch 3** — pair with the existing `mesh-mens-distributed-training-and-execution-plan-2026.md` rather than freelancing.
- **arXiv submission dedup** — only if arXiv automation is in v0.6 scope.

---

## 6. Lessons (also captured in the auto-memory feedback file)

1. **Audit agent summaries are hypotheses, not verdicts.** On this run, broad-scope retirement claims were wrong ≥5 times out of 10.
2. **`src/lib.rs` LoC is misleading.** Cargo integration-test crates put the work in `tests/`. CLI/binary crates put work in `src/bin/`. Plugin crates put work in `src/<extension>.rs` files.
3. **Cargo.toml caller graph is necessary but not sufficient.** Real consumers can live in `.github/workflows/`, `contracts/`, `examples/`, ADRs, and generated docs.
4. **Read `tests/` before deleting a crate.** And check `find <crate>/tests -name "*.rs" -o -name "*.snap"`.
5. **Check the team's existing plans before writing your own.** This codebase has 280+ architecture docs and 4+ live in-flight plans (telemetry, mesh, codegen unification, language rules). Any audit-driven plan should explicitly check `docs/superpowers/plans/` and `docs/src/architecture/` for prior art.
6. **Fast-model subagents will trust the task description.** They will not re-verify a "0 callers, 4 LoC" claim. Pre-stage all verification yourself.
7. **`git revert` of a delete commit removes anything else the commit added.** If a subagent ran `git add -A` and swept up an unstaged plan doc, the revert removes the plan too. Stage explicitly when possible.

---

## 7. Companion artifacts

- Memory: `~/.claude/projects/C--Users-Owner-vox/memory/feedback_verify_audit_retirement_claims.md` — verification checklist (7 items) for any future retirement-style work.
- Memory: `~/.claude/projects/C--Users-Owner-vox/memory/project_ai_laziness_remediation_plan_2026.md` — pointer to this doc.
- Companion audit: `docs/src/architecture/comprehensive-audit-v2-2026.md` (April 2026 governance-crisis diagnosis).
- Existing in-flight plans that subsume audit findings:
  - `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-{a,b,c,d}.md`
  - `docs/src/architecture/mesh-mens-distributed-training-and-execution-plan-2026.md`
  - `docs/src/architecture/external-frontend-interop-plan-2026.md`
  - `docs/src/architecture/vox-language-rules-and-enforcement-plan-2026.md`

---

## 8. Status

- ✅ Audit completed
- ✅ Verification of all 10 retirement candidates completed
- ✅ One incorrect delete attempted, reverted, no impact
- ✅ Findings + lessons recorded in this doc + auto-memory
- ⏭️ User to choose next track; recommendation is to execute existing telemetry Phase C plan, not freelance
