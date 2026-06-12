---
title: "Semantic Gap Implementation Plan (2026-05-16)"
category: "Architecture SSOTs"
status: "current"
---

# Semantic Gap Implementation Plan (2026-05-16)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Remediate the 7 HIGH-confidence semantic gaps found in `docs/src/architecture/semantic-gap-audit-2026.md` plus the 1 LOW-severity plan-doc fix. Findings F2–F6 collapse into one engineering motion (silent-DB-error policy in the orchestrator); F1 is its own surgical fix; F7 is a one-line edit; F8 is documentation.

**Architecture:** Three independent batches (A, B, C) that can ship as separate PRs. Order is by leverage: Batch B first (most user-visible — broken route manifests silently passing codegen), then Batch A (correctness of audit/budget/reliability trail), then Batch C (cosmetic plugin consistency + plan-doc cleanup).

**Tech Stack:** Rust 1.83 workspace, `tracing` crate for structured logging, `vox_telemetry` for event emit, `cargo test --workspace` for verification.

---

## Batch B — Wire the dead-code validator in vox-codegen (F1)

**Why first:** the gap is user-visible at build time. Anyone authoring `routes { }` with a typo in a component name today gets clean codegen and a runtime error; after this batch, they get a fail-fast build error pointing at the bad symbol.

**Files:**
- Modify: `crates/vox-codegen-ts/src/route_manifest.rs:25-68`
- Create: `crates/vox-codegen/tests/route_manifest_validation.rs` (or extend an existing test file)

### Task B1: Write the failing test

- [ ] **Step 1:** Identify or create a test file location. Search for existing tests of `try_emit_route_manifest_from_web_ir` in `crates/vox-codegen/tests/` and `crates/vox-codegen-ts/src/`. If a test file exists for route_manifest, extend it. Otherwise create `crates/vox-codegen/tests/route_manifest_validation.rs`.

- [ ] **Step 2:** Write a failing test that constructs a `WebIrModule` with a `RouteContract` referencing a component name `"NonExistentComponent"`, a `HirModule` that does NOT define `NonExistentComponent`, then calls `validate_manifest_symbols(&web, &hir)` and asserts `Err(...)` with a message containing the component name.

Reference for the WebIrModule + HirModule construction: look at existing tests in `crates/vox-codegen-ts/src/` for the canonical fixtures pattern (search `WebIrModule::new`, `HirModule::default`, or wherever route fixtures are built).

The test body in skeleton form:

```rust
#[test]
fn validate_manifest_symbols_flags_missing_component() {
    // Build a minimal WebIrModule with one route referencing "NonExistentComponent".
    let web = /* ... construct using existing fixture helpers ... */;
    let hir = /* ... HirModule that defines no components ... */;

    let result = vox_codegen::codegen_ts::route_manifest::validate_manifest_symbols(&web, &hir);

    let err = result.expect_err("expected validation error for missing component");
    assert!(
        err.contains("NonExistentComponent"),
        "error should name the missing component; got: {err}"
    );
}
```

- [ ] **Step 3:** Run the test and verify it fails.

Run: `cargo test -p vox-codegen --test route_manifest_validation -- validate_manifest_symbols_flags_missing_component`

Expected: test FAILS with "expected validation error for missing component" because the current body returns `Ok(())` unconditionally.

### Task B2: Wire the existing dead-code validator into the public entry

- [ ] **Step 1:** In `crates/vox-codegen-ts/src/route_manifest.rs`, replace the body of `validate_manifest_symbols` (lines 25–27) so it:
  1. Extracts `component_names: BTreeSet<String>` from `hir` — i.e., the names of every component/JSX function defined in the HIR. (Locate the relevant accessor by looking at how `emit_route_manifest_from_web_ir` consumes `hir` — there will be a similar walk available, or one can be added in a small helper.)
  2. Extracts `query_names: BTreeSet<String>` from `hir` — names of every `@query`-annotated function.
  3. Calls `route_tree_top_contracts(web)` to get the top-level route contracts.
  4. Recursively calls the existing `validate_contract_branch` for each top contract, accumulating errors in a `Vec<String>`.
  5. If the error vector is non-empty, returns `Err(errors.join("\n"))`. Otherwise returns `Ok(())`.

- [ ] **Step 2:** Remove the `#[allow(dead_code)]` from `validate_contract_branch` (line 29).

- [ ] **Step 3:** Run the failing test from B1 and verify it PASSES.

Run: `cargo test -p vox-codegen --test route_manifest_validation -- validate_manifest_symbols_flags_missing_component`
Expected: PASS.

- [ ] **Step 4:** Run the rest of the vox-codegen test suite to ensure no existing route manifest fixtures break.

Run: `cargo test -p vox-codegen`
Expected: no new failures. If existing snapshot tests fail because they used route fixtures with unresolved components (i.e., they were silently passing because of the broken validator), update those fixtures to be valid OR explicitly assert the new error and adjust them with intent.

### Task B3: Add two more validation tests for coverage

- [ ] **Step 1:** Add a test for the loader case — a route with a `loader` meta entry pointing to a non-`@query` symbol should fail.
- [ ] **Step 2:** Add a test for the pending case — a route with a `pending` meta entry pointing to a non-existent component should fail.
- [ ] **Step 3:** Add a test for the recursive case — a child route with a bad component should fail (verifies the recursion at line 65–67 fires).
- [ ] **Step 4:** Add a positive test — a valid route tree returns `Ok(())`.

Run: `cargo test -p vox-codegen --test route_manifest_validation`
Expected: 4 tests pass.

### Task B4: Commit

- [ ] **Step 1:** `git add` only the modified `route_manifest.rs` and the new/modified test file.

- [ ] **Step 2:**
```bash
git commit -m "fix(codegen): wire route-manifest validation that was sitting dead-code

validate_manifest_symbols was a public no-op (Ok(()) only); the real
validation logic existed adjacent as #[allow(dead_code)] validate_contract_branch
but was never called. Broken route manifests with typoed component names
or unresolved loaders were passing codegen and surfacing as runtime errors.

This connects the existing impl to the public entry, removes the dead_code
attribute, and adds four targeted tests (missing component, bad loader,
bad pending, recursion into children).

Refs: docs/src/architecture/semantic-gap-audit-2026.md F1"
```

---

## Batch A — Stop silently dropping `Result`s on critical DB writes (F2–F6)

**Why second:** the gaps are not user-visible at build time but corrupt the audit trail, reliability scoring, and budget enforcement under DB stress. The fix is mechanical: replace `let _ = …await;` with structured `tracing::error!` + event-bus emission so failures are observable, and for budget enforcement add a circuit-breaker hook.

The right *kind* of fix differs per call site:

- **Reliability observations (F2, F3):** These are fire-and-forget event-handler writes; the handler must not block on DB. Right fix: log + emit a `ReliabilityWriteFailed` event so observability captures the loss.
- **Lineage event (F4):** Audit trail. Right fix: log + emit + consider buffering for retry, but at minimum surface to operators.
- **Campaign init (F5):** Setup work that downstream tasks depend on. Right fix: log + return an error to the caller OR emit a `CampaignInitFailed` event that triggers a hold on the task.
- **Budget exec-time (F6):** Cost accounting that the budget gate trusts. Right fix: log + emit + (optionally) trigger circuit-breaker if multiple writes fail in a row.

The common piece is a small helper that wraps the discarded-result pattern. Build it once, apply it everywhere.

### Task A1: Build the `log_and_emit_persistence_failure` helper

**Files:**
- Create: `crates/vox-orchestrator/src/services/persistence_obs.rs` (new module)
- Modify: `crates/vox-orchestrator/src/services/mod.rs` (export new module)
- Test: `crates/vox-orchestrator/src/services/persistence_obs.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1:** Write the failing test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn log_and_emit_persistence_failure_logs_error_with_context() {
        let err: Box<dyn std::error::Error + Send + Sync> =
            "simulated db failure".into();
        log_and_emit_persistence_failure(
            "reliability.endpoint_observation",
            &*err,
            None,
        );
        assert!(logs_contain("reliability.endpoint_observation"));
        assert!(logs_contain("simulated db failure"));
    }
}
```

- [ ] **Step 2:** Run: `cargo test -p vox-orchestrator persistence_obs::tests` → expect FAIL (module not yet defined).

- [ ] **Step 3:** Add `tracing-test = "0.2"` as a `[dev-dependencies]` entry in `crates/vox-orchestrator/Cargo.toml` if not already present.

- [ ] **Step 4:** Implement the helper:

```rust
//! Shared helper for observability around persistence failures in the orchestrator.
//!
//! Use this in place of `let _ = db.something().await;` whenever the discarded
//! Result represents a meaningful write (audit trail, reliability observation,
//! budget accounting, lineage). The helper logs with structured fields and
//! optionally emits a `PersistenceFailed` event on the orchestrator event bus.
//!
//! Refs: docs/src/architecture/semantic-gap-audit-2026.md F2–F6.

use std::error::Error;
use crate::event_bus::AgentEventKind;

/// Log a persistence-layer failure with structured context. Caller passes a
/// short stable id for the operation site (e.g. "reliability.endpoint_observation",
/// "lineage.task_submitted", "budget.exec_time"). If `event_bus` is Some, emits a
/// PersistenceFailed event.
pub fn log_and_emit_persistence_failure(
    op_id: &'static str,
    err: &(dyn Error + 'static),
    event_bus: Option<&crate::event_bus::EventBus>,
) {
    tracing::error!(
        target: "vox.orchestrator.persistence",
        op = op_id,
        error = %err,
        "persistence write failed; data not recorded"
    );
    if let Some(bus) = event_bus {
        bus.emit(AgentEventKind::PersistenceFailed {
            op_id: op_id.to_string(),
            error_message: err.to_string(),
        });
    }
}
```

- [ ] **Step 5:** Add the `PersistenceFailed` variant to `AgentEventKind` in `crates/vox-orchestrator/src/event_bus.rs` (locate the enum and append the variant; if there's a snapshot-based test of the enum's serialization, run it and update the snapshot intentionally).

- [ ] **Step 6:** Run test → expect PASS.

- [ ] **Step 7:** Commit:
```bash
git commit -m "feat(orchestrator): add log_and_emit_persistence_failure helper

Centralizes the discarded-Result pattern around critical DB writes
(reliability observations, audit lineage, budget accounting). Subsequent
commits replace let-underscore patterns with this helper.

Refs: docs/src/architecture/semantic-gap-audit-2026.md F2-F6"
```

### Task A2: Replace silent drops in `services/reliability.rs` (F2, F3)

**Files:**
- Modify: `crates/vox-orchestrator/src/services/reliability.rs:33-41, 43-58, 75-83`

- [ ] **Step 1:** Write a failing test in `reliability.rs` (or its existing test file) that:
  - Constructs a `ReliabilityService` with a mock `store` whose `record_endpoint_observation` always returns `Err`.
  - Emits a `EndpointReliabilityObservation` event.
  - Asserts that `tracing::error!` was emitted with the expected `op_id` field (use `tracing_test::traced_test`).

Run: `cargo test -p vox-orchestrator services::reliability` → FAIL.

- [ ] **Step 2:** In `reliability.rs`, replace each `let _ = self.store.<op>(...).await;` site (lines 33, 45, 49, 53, 57, 75) with:

```rust
if let Err(e) = self.store.<op>(...).await {
    crate::services::persistence_obs::log_and_emit_persistence_failure(
        "reliability.<op_name>", &e, self.event_bus.as_ref(),
    );
}
```

…where `<op_name>` is `endpoint_observation` for line 33/75, `task_completed_obs` for line 45, `task_failed_obs` for line 49, `handoff_accepted_obs` for line 53, `handoff_rejected_obs` for line 57.

If `self.event_bus` isn't currently a field of `ReliabilityService`, either add it (preferred — matches the helper's signature) or pass `None` as a transitional step (deferred event-bus wiring is acceptable here; document with a TODO referencing this plan).

- [ ] **Step 3:** Run the test → PASS.

- [ ] **Step 4:** Commit:
```bash
git commit -m "fix(orchestrator/reliability): stop silently dropping DB write failures

Six sites in reliability.rs (record_endpoint_observation x2 +
record_task_reliability_observation x4 across match arms) were using
\`let _ = ...await;\` to discard DB write Results. Failures now log
structured errors via log_and_emit_persistence_failure.

Refs: docs/src/architecture/semantic-gap-audit-2026.md F2, F3"
```

### Task A3: Replace silent drops in `task_dispatch/submit/task_submit.rs` (F4, F5)

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs:1002-1014` and `:97-105`

- [ ] **Step 1:** For F4 (lineage event at line 1002), write a failing test (in the same crate's tests/ or as an inline `#[cfg(test)]`) that mocks `append_orchestration_lineage_event` to return Err, runs a task submission, and asserts the `tracing::error!` emit.

- [ ] **Step 2:** Replace the F4 site (line 1002–1014) with the `log_and_emit_persistence_failure` pattern.

- [ ] **Step 3:** For F5 (line 97 — `begin_reconstruction_campaign`), the policy decision is harder: should a failed campaign init block the task submission, or proceed without a campaign?
  - **Recommendation:** Block. A task submitted with a campaign_id that has no backing campaign row is a downstream lookup bug waiting to happen.
  - Implementation: change the `let _ = self.begin_reconstruction_campaign(...).await;` to:
    ```rust
    self.begin_reconstruction_campaign(...).await
        .map_err(|e| { ...log + return an explicit submission error... })?;
    ```
  - But this changes the function's public error type, so audit the callers. If callers can't tolerate a new error variant, fall back to the log-and-emit pattern PLUS unset `task.campaign_id = None` so downstream code doesn't look up a non-existent campaign.

- [ ] **Step 4:** Add a test for the new F5 behavior (whichever path was chosen).

- [ ] **Step 5:** Run all tests in `vox-orchestrator`:
Run: `cargo test -p vox-orchestrator`
Expected: pass.

- [ ] **Step 6:** Commit:
```bash
git commit -m "fix(orchestrator/submit): stop dropping lineage + campaign-init DB errors

task_submit.rs:1002 (lineage append) and :97 (campaign init) were using
\`let _ = ...await;\` to discard Results from writes that feed the audit
trail and downstream campaign lookups. Lineage failures now log + emit
PersistenceFailed; campaign-init failures now block submission (or clear
task.campaign_id, see code comment) to prevent dangling references.

Refs: docs/src/architecture/semantic-gap-audit-2026.md F4, F5"
```

### Task A4: Replace silent drop in `budget/persistence.rs` (F6)

**Files:**
- Modify: `crates/vox-orchestrator/src/budget/persistence.rs:103`

- [ ] **Step 1:** Write a failing test that mocks `db.record_exec_time` to return Err, runs the budget-persistence path, asserts the log emit AND asserts a circuit-breaker counter (if applicable) incremented.

- [ ] **Step 2:** Replace the F6 site with the `log_and_emit_persistence_failure` pattern. Because exec_time feeds budget enforcement, also add a metric / counter increment so operators can detect ongoing accounting loss. Look for an existing budget-metrics surface in `crates/vox-orchestrator/src/budget/` and emit through that; if none exists, defer the metric to a follow-up and just log + emit for now.

- [ ] **Step 3:** Run tests:
Run: `cargo test -p vox-orchestrator budget::persistence`
Expected: pass.

- [ ] **Step 4:** Commit:
```bash
git commit -m "fix(orchestrator/budget): stop dropping exec-time persistence errors

budget/persistence.rs:103 was using \`let _ = db.record_exec_time(...).await;\`
to discard the Result. Silent loss under-counts agent usage against budgets.
Failures now log + emit PersistenceFailed via log_and_emit_persistence_failure.

Refs: docs/src/architecture/semantic-gap-audit-2026.md F6"
```

### Task A5: Add a clippy lint that bans `let _ = .*await` on calls to known DB write methods (optional follow-up)

**Why:** prevent this regression. Several patterns can be banned at lint level (e.g., `clippy::let_underscore_must_use` already exists for `#[must_use]` types — annotating the DB store methods with `#[must_use]` would surface this automatically).

- [ ] **Step 1:** Audit the public methods on the orchestrator's persistence store (e.g., `crates/vox-orchestrator/src/store/*.rs` or wherever `record_endpoint_observation`, `record_task_reliability_observation`, `append_orchestration_lineage_event`, `record_exec_time` are defined). Add `#[must_use = "persistence write Result must be handled — see semantic-gap-audit-2026.md"]` to each.

- [ ] **Step 2:** Run `cargo clippy --workspace --all-targets -- -D warnings -A clippy::all -W clippy::let_underscore_must_use` (or whatever the project's clippy invocation pattern is).

- [ ] **Step 3:** Audit any new warnings that surface — they're additional silent-drop sites this audit didn't reach.

- [ ] **Step 4:** Commit:
```bash
git commit -m "chore(orchestrator): annotate persistence writes with #[must_use]

Adds compile-time enforcement that the Result from DB write methods on
the persistence store cannot be silently discarded. Pairs with the
log_and_emit_persistence_failure helper as the runtime side of the policy.

Refs: docs/src/architecture/semantic-gap-audit-2026.md"
```

---

## Batch C — Plugin trait consistency + plan-doc cleanup (F7, F8)

**Why last:** smallest and most cosmetic. Two unrelated edits packaged together.

### Task C1: Make `CloudSync::list_remote_json` explicit about its scaffold state (F7)

**File:**
- Modify: `crates/vox-plugin-cloud/src/sync.rs:46-48`

- [ ] **Step 1:** Edit the function body from:

```rust
fn list_remote_json(&self, _remote_prefix: RStr<'_>) -> RResult<RString, RBoxError> {
    RResult::ROk(RString::from("[]"))
}
```

…to match the sibling pattern:

```rust
fn list_remote_json(&self, _remote_prefix: RStr<'_>) -> RResult<RString, RBoxError> {
    RResult::RErr(RBoxError::new(std::io::Error::other(
        "not yet implemented; SP7 scaffold",
    )))
}
```

- [ ] **Step 2:** Run `cargo test -p vox-plugin-cloud` and `cargo test -p vox-plugin-catalog`. If any test relied on `list_remote_json` returning `Ok("[]")`, update that test to expect the explicit error (with a comment referencing this fix).

- [ ] **Step 3:** Commit:
```bash
git commit -m "fix(plugin-cloud): list_remote_json returns explicit not-implemented error

Was silently returning Ok(\"[]\") instead of matching its sibling methods
(upload, download) which return RErr with \"not yet implemented; SP7 scaffold\".
Callers can now distinguish \"no remote artifacts\" from \"feature not built.\"

Refs: docs/src/architecture/semantic-gap-audit-2026.md F7"
```

### Task C2: Fix the telemetry Phase B plan internal contradiction (F8)

**File:**
- Modify: `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-b.md`

- [ ] **Step 1:** Locate Task 3 in the Phase B plan. Add a note (one or two lines) that the trace-field population described in this task is forward-looking and that the actual wiring lands in Phase C. Reference the Phase C plan's prerequisite line: *"Phase B is merged (`ModelCallEvent` exists; `current_trace_ctx()` is being read by `infer.rs` but currently returns a default empty context)"*.

- [ ] **Step 2:** Commit:
```bash
git commit -m "docs(telemetry): note that Phase B Task 3 trace fields are wired by Phase C

Phase B Task 3 reads as if the trace_id/task_id/parent_task_id/caller_agent_id
fields are populated during Phase B; the same document's Phase C preview
contradicts that. Code follows the preview. This adds an inline note
pointing at Phase C as the actual wiring task to resolve the contradiction.

Refs: docs/src/architecture/semantic-gap-audit-2026.md F8"
```

---

## Verification gate (run before opening the PRs)

- [ ] `cargo test --workspace` passes (or only pre-existing failures unrelated to these changes).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — passes; in particular, no new `let_underscore_must_use` warnings.
- [ ] `cargo run -p vox-arch-check` passes.
- [ ] No hand-edited auto-generated docs (per project memory: SUMMARY.md / *.generated.md / feed.xml / .cursorignore are tool-regenerated only).

---

## Out of scope for this plan

- **vox-code-audit's 27 `todo!()`/`unimplemented!()` panics** (claim from the prior audit, not personally verified in this round). Worth a separate focused plan after a per-rule inventory.
- **arXiv submission automation** (`vox-publisher/src/submission/arxiv.rs:26`). The "operator-assist" mode is documented; whether to automate it is a v0.6 scope decision, not a defect fix.
- **MENS Batch 3 stubs** in `vox-plugin-mens-candle-cuda/`. These are explicitly tracked as SP3-deferred and belong to the existing MENS distributed plan (`docs/src/architecture/mesh-mens-distributed-training-and-execution-plan-2026.md`).
- **Telemetry Phase C wiring** in `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs:504-507`. Already covered by the existing 13-task Phase C plan.

---

## Estimated effort

| Batch | Effort | Risk | Reviewer time |
|---|---|---|---|
| B (codegen validator wiring) | 2–4 hours | low (additive validation; only breaks malformed inputs) | 30 min |
| A (orchestrator silent-drop fixes) | 6–10 hours | medium (touches hot paths; need to verify event_bus is correctly threaded; budget circuit-breaker is the trickiest) | 60–90 min |
| C (cloud plugin + plan doc) | 30 min | trivial | 15 min |

Total: 1–2 engineering days for one familiar engineer, plus ~2 hours of review.

---

## Companion artifacts

- Findings: `docs/src/architecture/semantic-gap-audit-2026.md`
- Audit-of-audit (prior round's negative-result doc): `docs/src/architecture/ai-laziness-remediation-plan-2026.md`
- Existing plans this work coordinates with: `docs/superpowers/plans/telemetry/2026-05-09-telemetry-phase-{b,c}.md`
- Auto-memory: `~/.claude/projects/C--Users-Owner-vox/memory/feedback_verify_audit_retirement_claims.md` (verification rules used during this round).
