# Orchestrator Chat Fast-Path, Reliability, and Latency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give chat messages a fast, non-agentic reply path; fix 6 real reliability
gaps found by a fresh orchestrator audit (5 from the original investigation, 1
found by a subsequent adversarial review of this plan itself); wire an
already-built compaction engine into the one place it's needed most.

**Architecture:** Three independently landable phases (A: chat fast-path via a new
`TaskCategory::Chat` + dedicated processor behind a thin dispatching wrapper; B:
6 reliability fixes, including one — Task B0 — that MUST land before Task B2
since B2 would otherwise re-trigger the same bug B0 fixes; C: 2 latency fixes,
one shared with Phase B). Every task is TDD'd, zero paid LLM calls in any new
test (mirrors the `StubTaskProcessor` / `chat_round_trip.rs` pattern already in
the codebase from this session's earlier fix). This plan was adversarially
audited against the live codebase after initial approval (2026-07-20) by four
parallel reviewers; every "Corrected by adversarial review" callout below
records a real, verified gap between the original design and what actually
compiles/runs against this repo — treat those sections as more load-bearing
than the surrounding prose, not as optional polish.

**Tech Stack:** Rust (`crates/vox-orchestrator`), TypeScript/React
(`crates/vox-gui/ui`), Tauri commands (`crates/vox-gui/src/commands`).

**Design doc:** [`docs/superpowers/specs/2026-07-20-orchestrator-chat-latency-reliability-design.md`](../specs/2026-07-20-orchestrator-chat-latency-reliability-design.md)

**Ground-truth line numbers below were read directly from the current worktree
(`C:\Users\Owner\vox\.worktrees\axis-frontend-remediation`) — if they've drifted by
the time you implement, re-locate by content match, not blind line offset.**

---

## Phase B — reliability hardening (do this first: small, independent, no schema changes)

### Task B0: `fail_task` silently no-ops after `abort_interrupted_task` — fix this BEFORE B1/B2

**Found by adversarial review, not the original investigation — this is a real,
currently-live bug, more severe than B1/B2 individually, and blocks both of
them from working as designed.**

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/agent/lifecycle_ops.rs`
  (`abort_interrupted_task`, verified at `:340-391`, removes
  `task_assignments` at `:383`)
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/fail.rs`
  (`fail_task_with_audit`, looks up `agent_id` from `task_assignments` at
  `:25-28` and returns `Err(OrchestratorError::TaskNotFound(task_id))` if
  absent)
- Modify: `crates/vox-orchestrator/src/runtime.rs` (the three phase-loop exit
  arms: cancel `:533-536`, stream-error `:571-575`, and the `HaltAgent` arm
  Task B2 below extends)
- Test: new test(s) in whichever of the above files already has a
  `#[cfg(test)]` module suited to it (`lifecycle_ops.rs` or `fail.rs`)

**Verified bug:** `abort_interrupted_task` (`lifecycle_ops.rs:383`) does
`crate::sync_lock::rw_write(&self.task_assignments).remove(&task_id);` as part
of its cleanup. `runtime.rs`'s dispatcher always follows an `Err` return from
the phase loop with a call to `fail_task` (`runtime.rs:877-887` per this
session's earlier investigation). But `fail_task_with_audit` (`fail.rs:25-28`)
re-derives `agent_id` by looking `task_id` up in that SAME `task_assignments`
map — which `abort_interrupted_task` just emptied. The lookup returns `None`,
`fail_task_with_audit` bails with `Err(OrchestratorError::TaskNotFound(task_id))`,
and that error is only logged (`tracing::error!` at the dispatcher, swallowed,
not propagated anywhere user-visible). **Net effect: every task that hits the
existing cancel path or the existing stream-error path today already fails to
be properly recorded as failed** — the task's terminal state, audit report,
and any budget/oplog bookkeping `fail_task_with_audit` was supposed to do
never happen. This is a pre-existing defect, not something Phase A/B/C
introduces — it just happens to also be exactly why B1's `reset_drift()` (if
inserted inside `fail_task_with_audit`, as originally planned below) would
never actually run for the scenarios it's meant to protect, and why B2's
planned `HaltAgent` fix would inherit the same silent-no-op.

- [ ] **Step 1: Write the failing test proving the no-op**

```rust
#[tokio::test]
async fn fail_task_after_abort_interrupted_task_does_not_silently_no_op() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    let agent_id = orch.spawn_agent("a1").unwrap();
    // Adapt to submit_task_with_agent's REAL 8-parameter signature (read it
    // fresh at task_dispatch/submit/task_submit.rs:106 before writing this -
    // do not copy a guessed signature from elsewhere in this plan).
    let task_id = /* submit a task on agent_id, get its TaskId */;
    orch.abort_interrupted_task(task_id, agent_id);
    let result = orch.fail_task(task_id, "boom".into()).await;
    // Today this is Err(TaskNotFound) and the caller only logs it - assert
    // it should instead succeed (or at minimum not silently vanish).
    assert!(result.is_ok(), "fail_task must not silently no-op after abort_interrupted_task already ran: {result:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator fail_task_after_abort_interrupted_task_does_not_silently_no_op`
Expected: FAIL — confirms `fail_task` returns `Err(TaskNotFound)`.

- [ ] **Step 3: Decide and implement the fix — read both functions fully first, this needs a real judgment call, not a guess**

Two candidate fixes, in order of preference — **read `abort_interrupted_task`
and `fail_task_with_audit` in FULL, including every other caller of both
(`grep -rn "abort_interrupted_task\|fail_task_with_audit\|fail_task\b"
crates/vox-orchestrator/src/`) before picking one**, since either could have
side effects on callers this plan hasn't traced:

  - **(a) Preferred if safe:** stop `abort_interrupted_task` from removing
    `task_assignments` itself — that removal is `fail_task`'s job (marking a
    task's terminal state), and `abort_interrupted_task`'s actual
    responsibility (per its own code) is file-lock/scope/interrupt-flag
    cleanup. If nothing else in the codebase relies on `task_assignments`
    being empty immediately after `abort_interrupted_task` returns (check via
    the grep above), delete the `task_assignments.remove()` line from
    `lifecycle_ops.rs:383` and let the subsequent `fail_task` call remove it
    as part of its own normal bookkeeping.
  - **(b) Fallback if (a) has other dependents:** give `runtime.rs`'s
    phase-loop exit arms an agent-id-aware failure path that doesn't
    re-derive `agent_id` from `task_assignments` at all — e.g. a new
    `fail_task_for_agent(task_id, agent_id, reason)` that skips the
    now-redundant lookup (the caller already has `agent_id` in scope in all
    three exit arms). This avoids touching `abort_interrupted_task`'s
    contract for its OTHER callers (`lifecycle_ops.rs:112`, `:151`, `:300`
    reference `task_assignments` too — read whether those are the same
    function or different call sites before assuming (a) is universally
    safe).

- [ ] **Step 4: Run test to verify it passes, then the full suite**

Run: `cargo test -p vox-orchestrator --lib`
Expected: all pass. Pay special attention to any EXISTING test asserting on
current cancel-path or stream-error-path behavior that this fix changes (a
task that previously silently stayed "un-failed" now gets correctly marked
failed — a test relying on the old, buggy behavior needs its assertion
flipped, not preserved).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/orchestrator/agent/lifecycle_ops.rs \
  crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/fail.rs \
  crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(orchestrator): fail_task no longer silently no-ops after abort_interrupted_task

abort_interrupted_task removed the task's task_assignments entry as
part of its cleanup; fail_task_with_audit re-derives agent_id from
that same map and bails with a swallowed TaskNotFound when it's
already gone. Every task hitting the EXISTING cancel or stream-error
phase-loop exit paths was silently never marked failed - found via
adversarial review while designing the HaltAgent-parity fix (Task B2),
which would have inherited the same defect."
```

---

### Task B1: reset drift state when a task completes or fails

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs`
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/fail.rs`
- Test: same files' existing `#[cfg(test)]` modules (or the crate's integration
  tests directory if these modules have none — check first with
  `grep -n "#\[cfg(test)\]" <file>`)

**Verified current code:**
`budget/mod.rs:565-572` already has the method we need:
```rust
pub fn reset_drift(&self, agent_id: AgentId) {
    let mut drift_map = sync_lock::rw_write(&*self.drift);
    if let Some(state) = drift_map.get_mut(&agent_id) {
        state.drift_streak = 0;
        state.cost_since_drift_start = 0.0;
        state.consecutive_tool_calls = 0;
    }
}
```
`orchestrator/accessors.rs:137-141` exposes the budget manager handle:
```rust
pub fn budget_manager_handle(&self) -> std::sync::Arc<std::sync::RwLock<crate::budget::BudgetManager>> {
    std::sync::Arc::clone(&self.budget_manager)
}
```
`complete_task_with_attestation` (`success/mod.rs:72+`) and
`fail_task_with_audit` (`fail.rs:19-...`) both resolve `agent_id` early via
`crate::sync_lock::rw_read(&*self.task_assignments).get(&task_id).copied()...`
(fail.rs:25-28; success/mod.rs has the equivalent — read it to confirm the exact
local variable name before editing, it may not be identically named).

**Corrected by adversarial review — three compile-blocking errors in the
original test sketch, now fixed:**
1. `record_agent_iteration` is a method on `Orchestrator`
   (`self.orchestrator.record_agent_iteration(...)` at `runtime.rs:579`, per
   the design doc's Ground Truth section), **not** on `BudgetManager` —
   `BudgetManager`'s own drift-recording method is `record_iteration_output`,
   a different name/signature. Call it via `orch.record_agent_iteration(...)`,
   not through a `budget_manager_handle()` guard.
2. `submit_task_with_agent` takes **8** parameters, not 7 as the original
   sketch assumed — read its real current signature
   (`task_dispatch/submit/task_submit.rs:106`) before writing the call;
   it's missing a `tenant_id`-shaped argument.
3. It returns `Result<TaskId, _>` directly, not a tuple — do not `.0` into it;
   `.unwrap()` alone gives the `TaskId`.
4. **Prerequisite: Task B0 must land first.** `fail_task` currently no-ops
   silently in scenarios reached via `abort_interrupted_task` (see Task B0)
   — but the plain `orch.fail_task(task_id, reason)` call this test uses does
   NOT go through `abort_interrupted_task` (that's only called from
   `runtime.rs`'s phase-loop exit arms, not from a direct `fail_task` call on
   a freshly-submitted, never-started task), so this specific test is safe to
   write independent of B0. B0 is still a hard prerequisite for **Task B2**
   (below), which fixes a phase-loop exit arm that DOES call
   `abort_interrupted_task` before the dispatcher's `fail_task`.

- [ ] **Step 1: Write the failing test**

In `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/fail.rs`'s test
module (or a new one in the same file if none exists — follow the crate's existing
`#[cfg(test)] mod tests { use super::*; ... }` convention seen elsewhere in this
crate, e.g. `queue/drain.rs`'s `semcov_drain_tests`):
```rust
#[tokio::test]
async fn fail_task_resets_drift_state_for_the_agent() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    let agent_id = orch.spawn_agent("a1").unwrap();
    // Manufacture a drift record the same way runtime.rs's phase loop does,
    // via the Orchestrator method (NOT a BudgetManager method - see the
    // correction above).
    orch.record_agent_iteration(agent_id, "same output", false); // 1st: drift_streak stays 0 (no prior match)
    orch.record_agent_iteration(agent_id, "same output", false); // 2nd: matches -> drift_streak = 1

    // Read submit_task_with_agent's REAL current 8-parameter signature
    // (task_dispatch/submit/task_submit.rs:106) before finalizing this call -
    // do not copy a guessed arg list from elsewhere in this plan.
    let task_id = orch.submit_task_with_agent(/* fill in from the real signature */).await.unwrap();

    orch.fail_task(task_id, "boom".into()).await.unwrap();

    // A fresh drift check for this agent must start clean, not inherit drift_streak.
    let decision = orch.record_agent_iteration(agent_id, "same output", false);
    assert!(matches!(decision, crate::budget::DriftDecision::Continue));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator fail_task_resets_drift_state_for_the_agent`
Expected: FAIL — `record_agent_iteration` on step 3 sees the still-present drift
record from before `fail_task`, producing `WarnUser` or worse instead of `Continue`.

- [ ] **Step 3: Add the reset call in `fail_task_with_audit`**

In `fail.rs`, immediately after `agent_id` is resolved (the `let agent_id = ...`
line near :25-28), add:
```rust
self.budget_manager_handle();
crate::sync_lock::rw_read(&*self.budget_manager_handle()).reset_drift(agent_id);
```
(Read the exact surrounding code first — `self.budget_manager` may already be a
directly-accessible field rather than needing the handle-accessor round-trip; use
whichever access pattern the rest of this file already uses for budget-manager
reads, for consistency.)

- [ ] **Step 4: Add the same reset call in `complete_task_with_attestation`**

Same pattern, in `success/mod.rs`, right after that function's `agent_id`
resolution.

- [ ] **Step 5: Run test to verify it passes, then the fail.rs/success module's full local test suite**

Run: `cargo test -p vox-orchestrator fail_task_resets_drift_state_for_the_agent`
Expected: PASS.
Run: `cargo test -p vox-orchestrator --lib`
Expected: all pass (this touches a shared completion path — watch for any test
that submits+fails/completes a task and separately asserts drift state persisted,
which would be relying on the buggy behavior and needs its assertion flipped, not
its behavior "fixed back").

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/orchestrator/task_dispatch/complete
git commit -m "fix(orchestrator): reset drift state when a task completes or fails

DriftState was keyed only by AgentId with no reset on task boundaries -
reset_drift() existed (budget/mod.rs:565) but had zero call sites. An
agent reused for a second, unrelated task after finishing/being halted
for drift on the first could trip a false-positive drift warning if the
new task's early output happened to fingerprint-collide with a leftover
record from the prior task."
```

---

### Task B2: `HaltAgent` exit path gets the same cleanup as cancel/stream-error

**Prerequisite: Task B0 must land first.** Without B0's fix, adding
`abort_interrupted_task` to the `HaltAgent` arm (this task's whole point)
would make the dispatcher's subsequent `fail_task` call silently no-op for
EVERY drift-halted task, exactly the way it already silently no-ops for the
cancel and stream-error paths today — landing this task before B0 would add
a third instance of the same live bug instead of fixing anything.

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`
- Test: extend `AiTaskProcessor`'s existing test coverage, or add a focused test
  in the crate's test module if `AiTaskProcessor` isn't already unit-tested
  directly (check first: `grep -n "AiTaskProcessor" crates/vox-orchestrator/src/runtime.rs | grep -i test`)

**Verified current code** (`runtime.rs:533-536`, `:571-575`, `:585-593`):
```rust
// cancel path (line ~533-536)
if cancel.load(Ordering::Acquire) {
    self.orchestrator.abort_interrupted_task(task.id, agent_id);
    return Err(anyhow::anyhow!("task interrupted"));
}
// ...
// stream-error path (line ~571-575)
Err(e) => {
    self.orchestrator.abort_interrupted_task(task.id, agent_id);
    return Err(e);
}
// ...
// HaltAgent path (line ~585-593) — MISSING the same call
crate::budget::DriftDecision::HaltAgent { reason } => {
    tracing::error!(agent_id = agent_id.0, %reason, "halted agent due to semantic drift");
    self.event_bus.emit(AgentEventKind::DoubtReported {
        agent_id,
        task_id: task.id,
        reason: reason.clone(),
    });
    return Err(anyhow::anyhow!("Safety Halt: {}", reason));
}
```

- [ ] **Step 1: Add the missing cleanup call**

Change the `HaltAgent` arm to:
```rust
crate::budget::DriftDecision::HaltAgent { reason } => {
    tracing::error!(agent_id = agent_id.0, %reason, "halted agent due to semantic drift");
    self.event_bus.emit(AgentEventKind::DoubtReported {
        agent_id,
        task_id: task.id,
        reason: reason.clone(),
    });
    self.orchestrator.abort_interrupted_task(task.id, agent_id);
    return Err(anyhow::anyhow!("Safety Halt: {}", reason));
}
```

- [ ] **Step 2: Verify `abort_interrupted_task` is idempotent / safe to call from this position**

Read `abort_interrupted_task`'s implementation (grep
`fn abort_interrupted_task` in `crates/vox-orchestrator/src/`) to confirm it's
safe to call before the `fail_task` that `handle_command` triggers downstream on
this `Err` return (i.e. it releases locks/scope but doesn't itself mark the task
failed — if it does something that would conflict with the later `fail_task`
call, note it and adjust; the cancel/stream-error paths already do this exact
sequence successfully, so parity should be safe, but confirm rather than assume).

- [ ] **Step 3: Add a regression test proving the parity**

If `AiTaskProcessor` has an existing test harness with a fake/controllable client
that can force a drift halt (check for one first), add a test asserting
`abort_interrupted_task`'s observable effect (whatever that is — read its impl
in Step 2 to know what to assert, e.g. a scope/lock released, an event emitted)
fires on the `HaltAgent` path the same way it does on the cancel path. If no such
test harness exists and building one is disproportionate to this small fix, it's
acceptable to skip an automated test here and rely on the full suite + manual
code-review parity check — note this explicitly in the commit if so, don't
silently skip.

- [ ] **Step 4: Run the full orchestrator suite**

Run: `cargo test -p vox-orchestrator --lib`
Expected: all pass, no regression.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(orchestrator): HaltAgent drift-halt path gets the same cleanup as cancel/stream-error

The cancel and stream-error phase-loop exits both call
abort_interrupted_task before returning Err; the HaltAgent arm (the
one path explicitly documented as an emergency circuit-breaker) was
the only one that didn't - a latent asymmetry of the same shape that
produced the earlier ProcessQueue stall bug."
```

---

### Task B3: `sync_fleet` skips one agent's registry-lookup failure instead of aborting the whole tick

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`
- Test: `crates/vox-orchestrator/src/runtime.rs`'s own test module, or a new
  integration test in `crates/vox-orchestrator/tests/` mirroring
  `chat_round_trip.rs`'s in-process-daemon pattern if a multi-agent scenario is
  easier to construct there.

**Verified current code** (`runtime.rs:966-977`):
```rust
let already_running = match self.scheduler.registry().lookup_name(&proc_name) {
    Ok(opt) => opt.is_some(),
    Err(e) => {
        tracing::error!(
            error = %e,
            proc_name = %proc_name,
            "process registry poisoned during fleet sync; aborting sync_fleet"
        );
        return;
    }
};
```
This `return` exits `sync_fleet` entirely — for-loop context is
`for (agent_id, name) in agent_info { ... }` (`:963`), so a lookup failure for
ANY one agent skips syncing every other agent in the same tick, forever, as long
as the registry stays poisoned for that name.

- [ ] **Step 1: Write the failing test**

This requires simulating a registry lookup failure for one agent among several.
Read `self.scheduler.registry()`'s type (`vox_actor_runtime`'s process registry)
to determine whether it's realistically fake-able/poisonable in a unit test, or
whether this is better proven by a smaller, more targeted refactor-and-inspect
test: extract the per-agent loop body into a small pure function
`fn sync_one_agent(...) -> SyncOutcome` (or similar) that returns an enum
(`Synced | LookupFailed`) instead of `return`ing from the whole method, and unit
test THAT function's behavior directly (does not silently propagate a "stop
everything" signal), then verify by code reading that `sync_fleet`'s loop calls
`continue` rather than `return` on `LookupFailed`. Choose whichever approach
actually compiles cleanly against the real registry API — read it first.

- [ ] **Step 2: Run test to verify it fails** (if a test was constructed per Step 1)

- [ ] **Step 3: Change `return` to `continue`**

```rust
let already_running = match self.scheduler.registry().lookup_name(&proc_name) {
    Ok(opt) => opt.is_some(),
    Err(e) => {
        tracing::error!(
            error = %e,
            proc_name = %proc_name,
            "process registry lookup failed for this agent during fleet sync; skipping it this tick"
        );
        continue;
    }
};
```

- [ ] **Step 4: Run test to verify it passes, then the full suite**

Run: `cargo test -p vox-orchestrator --lib`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(orchestrator): one agent's registry-lookup failure no longer aborts the whole fleet-sync tick

sync_fleet's per-agent loop 'return'ed on a single lookup_name error,
silently skipping every OTHER agent in the same tick too. A poisoned
registry entry for one agent name could permanently stall fleet
convergence for the entire fleet, with only one log line as the trail."
```

---

### Task B4: parallelize the nudge fan-out (shared with Phase C — implement once)

See **Task C2** below — implemented once, satisfies both this task's reliability
framing (bound blast radius of a wedged mailbox) and Phase C's latency framing
(bound tick stall time). Do not duplicate; this entry exists so Phase B's task
list is complete for tracking purposes. Mark this task's checkbox complete when
Task C2 lands.

- [ ] Task C2 landed (see below) — closes this task too.

---

### Task B5: `ScaleDown` requeue logs a failure instead of silently discarding it

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`
- Test: same file's test module (or a new focused test if `check_scaling`'s
  `ScaleDown` branch isn't currently unit-testable in isolation — check first)

**Verified current code** (`runtime.rs:1181-1187`):
```rust
for id in agent_ids {
    if let Ok(remaining) = self.orchestrator.retire_agent(id).await {
        for task in remaining {
            let _ = self.orchestrator.submit_existing_task(task).await;
        }
    }
}
```

- [ ] **Step 1: Write the failing test**

If `submit_existing_task` can be made to fail in a controlled test (check its
signature/error conditions — grep `fn submit_existing_task`), construct a
scenario: an agent with 1+ queued tasks gets retired, the requeue is forced to
fail, and assert a tracing/log event fires (or, if the codebase has a test
pattern for asserting `tracing::error!` was called — check for one, e.g. a test
subscriber — use it; otherwise this may need to be a code-reading verification
rather than an automated assertion, same caveat as Task B2 Step 3). If forcing a
realistic failure is impractical, it is acceptable to skip the red/green cycle
here and just implement + note why in the commit, but check for a forcing
mechanism first (e.g. can `submit_existing_task` be called with an already-
duplicate task ID to force a benign, testable failure path?).

- [ ] **Step 2: Add the logging**

```rust
for id in agent_ids {
    if let Ok(remaining) = self.orchestrator.retire_agent(id).await {
        for task in remaining {
            if let Err(e) = self.orchestrator.submit_existing_task(task.clone()).await {
                tracing::error!(
                    task_id = task.id.0,
                    error = %e,
                    "failed to requeue task from a retiring agent during scale-down; task is now untracked"
                );
            }
        }
    }
}
```
(Matches the existing logging pattern at `runtime.rs:867-887` for
`complete_task`/`fail_task` failures — confirm the exact style there and mirror
it, e.g. field names, before finalizing.)

- [ ] **Step 3: Run the full suite**

Run: `cargo test -p vox-orchestrator --lib`

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(orchestrator): log ScaleDown requeue failures instead of silently discarding them

A retiring agent's remaining tasks were re-submitted with the Result
discarded (let _ = ...) - a failed requeue meant the task vanished from
tracking with zero trace. Now logged, matching the existing
complete_task/fail_task failure-logging pattern."
```

---

## Phase C — latency fixes (apply to every task, chat or not)

### Task C1: wire `CompactionEngine` into the phase loop

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`
- Test: same file, or `crates/vox-orchestrator/tests/` for an integration-level
  proof

**Verified current code:**
- The phase loop builds `notes` as raw concatenation (`runtime.rs:520, 607-610,
  660-661`) and resends it whole via the `"Known notes:\n{}"` prompt slot
  (`runtime.rs:291`).
- `CompactionEngine::compact(&self, history: &[Turn]) -> Result<CompactionResult, CompactionError>`
  (`compaction.rs:229`) takes a `&[Turn]`, not a raw string — `Turn` is
  `{ role: String, content: String, token_estimate: usize }` (`compaction.rs:103-110`),
  constructed via `Turn::new(role, content)` which fills `token_estimate` via
  `CompactionEngine::estimate_tokens` automatically (`compaction.rs:112-122`).
  `compact()` is a no-op (`compacted: false`, everything retained) below its
  configured trigger threshold (`should_compact`, `compaction.rs:196-198`) — so
  wiring this in is safe for short conversations/early phases; it only starts
  trimming once `notes` genuinely grows large, which is exactly the intended
  behavior.
- `session/state.rs:257` already calls `.compact(...)` on session history — read
  that call site for the exact construction pattern (how `CompactionEngine` is
  instantiated/configured there, e.g. `CompactionEngine::new(config)` — find
  where `config` comes from in that context and mirror it, or find/use a shared
  instance if the orchestrator already holds one) before writing the phase-loop
  version, for consistency rather than inventing a second config source.

**Corrected by adversarial review — two critical gaps in the original sketch:**
1. `AiTaskProcessor`'s real fields (`runtime.rs:180-192`:
   `client/event_bus/orchestrator/provider/model/tool_dispatcher`) do **not**
   include a `CompactionEngine`. Every real `compact()` call site in this
   crate receives an externally-owned engine as a parameter
   (`session/manager/mutations.rs:304`) — nothing constructs one for the
   phase loop today. This task must ADD a `compaction: CompactionEngine`
   field to `AiTaskProcessor` and construct it in `new`/`with_tool_dispatcher`,
   not assume one already exists.
2. The original sketch computed a compacted `notes_for_prompt` local but
   never reassigned it back to the persistent `notes` string that keeps
   accumulating at `:607-610`/`:660-661` — as written, compaction would be
   silently discarded every phase and `notes` would keep growing exactly as
   unbounded as before, defeating the entire point of this task. The fix
   below reassigns `notes` itself, mirroring `session/state.rs:290`'s
   reassignment pattern.

- [ ] **Step 1: Read the exact `notes`-building code path AND `session/state.rs`'s reassignment pattern**

Read `runtime.rs` lines 520-665 in full again immediately before editing (the
line numbers above are from investigation, not a diff-safe anchor) to confirm
exactly where `notes` is read for prompt-building (inside `run_phase_stream`'s
call at `~540-566`, passing `notes.as_str()`) versus where it's mutated
(`~607-610`, `~660-661`). Also read `session/state.rs` around line 290 (the
line immediately after its `:257` `compact()` call) to see EXACTLY how that
call site takes `CompactionResult::retained_turns` and reassigns it back into
the live working set — this is the pattern to mirror, not invent a new one.

- [ ] **Step 2: Write the failing test**

Add a test (in `runtime.rs`'s test module or a new integration test) that:
1. Constructs a `notes`-equivalent history long enough to exceed
   `CompactionEngine`'s default trigger threshold (read `CompactionConfig`'s
   default `trigger_at()` value to size this realistically, e.g.
   `compaction.rs` near the `CompactionConfig` struct definition).
2. Asserts that the ACTUAL PROMPT TEXT built for a later phase (you may need to
   extract prompt-building into a small testable helper if it's inline —
   consider whether `run_phase_stream_with_bus`'s prompt-format call
   (`runtime.rs:290-299`) can be isolated into a `fn build_phase_prompt(...) -> String`
   free function for direct unit testing, since testing the full streaming
   call end-to-end would need a real/fake LLM client) is SHORTER than the raw
   concatenated history would have produced, once compaction is wired in.

This is the one task in this plan where "write the failing test first" may
require a small structural extraction (pulling prompt-building into a testable
function) as a legitimate first step, not scope creep — do it if the inline
version genuinely can't be tested in isolation otherwise.

- [ ] **Step 3: Run test to verify it fails**

Expected: FAIL — the prompt contains the full raw history, uncompacted.

- [ ] **Step 3b: Add a `CompactionEngine` field to `AiTaskProcessor`**

In `runtime.rs`'s `AiTaskProcessor` struct (`:180-192`), add a `compaction:
crate::compaction::CompactionEngine` field. Construct it in both `new` and
`with_tool_dispatcher` using `CompactionEngine::new(config)` — source `config`
the same way `session/state.rs`'s caller does (read that call site's config
provenance in Step 1; if it's a cheap `Default`-able config rather than
something requiring disk/DB access, construct it directly; if it needs
threading in from elsewhere, add it as a constructor parameter to both
`AiTaskProcessor::new`/`with_tool_dispatcher` and update their call sites —
this may interact with Task A3's fleet-construction wiring, since Task A3
already touches these same constructors' call sites; sequence C1 and A3
accordingly if implementing both, or note the merge point explicitly in
whichever commit lands second).

- [ ] **Step 4: Convert `notes` accumulation to compact before each phase's prompt build, AND reassign `notes` itself**

Sketch (adapt to whatever `Turn` construction pattern Step 1's read of
`session/state.rs:257-290` revealed, mirroring its reassignment exactly):
```rust
// Before building this phase's prompt, compact the accumulated history if
// it has grown past the configured trigger. `notes` is built as
// "[{phase}]\n{phase_out}" blocks joined by "\n\n" (runtime.rs:607-610) -
// one Turn per phase block is the natural mapping, role "assistant" (this
// is all agent-output; compaction.rs's trim strategies only special-case
// role=="system", never "user" - confirmed safe by adversarial review, no
// "user turns protected from trimming" concern applies here).
if !notes.is_empty() {
    let history_turns: Vec<crate::compaction::Turn> = notes
        .split("\n\n")
        .map(|block| crate::compaction::Turn::new("assistant", block))
        .collect();
    if let Ok(result) = self.compaction.compact(&history_turns) {
        if result.compacted {
            // REASSIGN notes itself - mirrors session/state.rs:280-289's
            // exact pattern (self.turns = result.retained_turns...collect()).
            // Without this reassignment, compaction has no effect: notes
            // keeps accumulating raw and unbounded regardless of what
            // compact() computed (this was the original sketch's bug).
            notes = result
                .retained_turns
                .iter()
                .map(|t| t.content.clone())
                .collect::<Vec<_>>()
                .join("\n\n");
        }
    }
}
```
Do not lose the lossless-archival contract `compact()` documents
(`compaction.rs:136-141`) without a deliberate decision — if any dropped
turns should be durably logged (matching how `session` persists
`dropped_turns`), check whether that's necessary for phase-loop notes too, or
whether losing them is acceptable (session transcripts are user-facing
history; phase notes are an internal scratchpad the task discards after
completion anyway — likely acceptable to NOT persist `dropped_turns` here,
but make the choice deliberately and note it in the commit, not by omission).

**Known limitation, not blocking (flagged by adversarial review):** with 6
phases per task and unbounded individual `phase_out` size, a single very
large phase output can itself exceed `CompactionConfig`'s
`tail_preserve_tokens` (default 8000 per the review) and get trimmed even
though it's the most recent, most relevant turn. Not a regression (today's
uncompacted behavior has no protection at all), but worth a code comment
noting it as a known edge case for future tuning, not something this task
needs to solve.

- [ ] **Step 5: Run test to verify it passes, then the full suite**

Run: `cargo test -p vox-orchestrator --lib`
Expected: all pass. Also run any existing phase-loop-behavior tests
specifically (grep for tests referencing `AiTaskProcessor`/phase execution) to
confirm no regression in short-conversation (below-trigger) behavior, where
compaction should be a complete no-op.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(orchestrator): compact phase-loop notes before each phase's prompt, instead of resending raw and unbounded

CompactionEngine::compact already existed and was proven correct via
the session-transcript path (session/state.rs:257), but runtime.rs's
phase loop only used CompactionEngine::estimate_tokens for post-hoc
cost telemetry after all 6 phases ran - the notes string resent as
'Known notes' in every phase's prompt grew raw and unbounded. This was
the single highest-leverage latency/cost fix found by the performance
audit: the fix already existed, it just wasn't invoked from the one
place that needed it most."
```

---

### Task C2: parallelize the fleet-tick nudge fan-out

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`
- Test: same file's test module

**Verified current code** (`runtime.rs:1029-1075`, the full `nudge_queued_agents`
body already shown in the design doc's Ground Truth section) — sends are
sequential: `for agent_id in self.orchestrator.agent_ids() { ... match
tokio::time::timeout(D_5S, handle.send(env)).await { ... } }`.

- [ ] **Step 1: Write the failing test — deterministic, NOT timing-based**

**Corrected by adversarial review:** a wall-clock assertion ("total time is
close to one timeout window") is flaky by construction under CI scheduler
noise, and can false-pass on a fast machine even if a regression accidentally
reintroduces serial sends. Use a concurrency-counting proof instead: a fake
handle whose `send()` increments an `AtomicUsize` on entry, sleeps briefly,
records the peak concurrent value, then decrements on exit. Assert the
recorded peak was `> 1` — this proves concurrent dispatch deterministically,
with no dependency on absolute timing:
```rust
#[tokio::test]
async fn nudge_sends_are_concurrent_not_serial() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let in_flight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    // Construct 2+ agents with ready tasks and fake/stub handles that, on
    // send(), do: in_flight.fetch_add(1); update peak via fetch_max-style
    // CAS loop; tokio::time::sleep(short); in_flight.fetch_sub(1). Exact
    // handle-faking mechanism depends on vox_actor_runtime::mailbox's real
    // API - read it first (Step 0 below) to find the right seam; if handles
    // aren't directly fakeable, this may need to be an integration test in
    // crates/vox-orchestrator/tests/ using real (but artificially slow, via
    // a test-only actor) agents instead of a unit-level fake.
    // ... call fleet.nudge_queued_agents().await ...
    assert!(peak.load(Ordering::SeqCst) > 1, "sends must overlap, not run one-at-a-time");
}
```

- [ ] **Step 1b: Read `vox_actor_runtime::mailbox`'s handle/`Envelope` types first**

Before finalizing Step 1's test, read the real handle type `nudge_queued_agents`
sends through (`vox_actor_runtime::mailbox`) to determine whether it's
directly fakeable in a unit test (implement a test double satisfying whatever
trait/interface `handle.send(env)` requires) or whether proving concurrency
needs an integration-level test with real actors instead. Adjust Step 1's
exact construction to whichever is actually feasible.

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — the current serial `for` loop with sequential `.await`s
never has more than 1 send in flight at once, so `peak` stays `1`.

- [ ] **Step 3: Fan the sends out concurrently**

```rust
use futures_util::future::join_all; // add to Cargo.toml deps if not already present — check first

pub async fn nudge_queued_agents(&self) {
    let candidates: Vec<crate::types::AgentId> = self
        .orchestrator
        .agent_ids()
        .into_iter()
        .filter(|&agent_id| match self.orchestrator.agent_queue(agent_id) {
            Some(queue_lock) => {
                let queue = crate::sync_lock::rw_read(&*queue_lock);
                queue.has_ready_task() && !queue.has_in_progress()
            }
            None => false,
        })
        .collect();

    let sends = candidates.into_iter().filter_map(|agent_id| {
        let handle = crate::sync_lock::rw_read(&*self.orchestrator.agent_handles)
            .get(&agent_id)
            .cloned()?;
        Some(async move {
            let json = serde_json::to_string(&AgentCommand::ProcessQueue).unwrap_or_else(|e| {
                tracing::warn!("serialize ProcessQueue: {e}");
                "{}".to_string()
            });
            let env = vox_actor_runtime::mailbox::Envelope::Message(
                vox_actor_runtime::mailbox::Message {
                    from: vox_actor_runtime::Pid::new(),
                    payload: MessagePayload::Json(json.into()),
                },
            );
            match tokio::time::timeout(vox_config::timeouts::D_5S, handle.send(env)).await {
                Ok(Err(e)) => tracing::warn!("fleet tick: ProcessQueue nudge to agent {agent_id} failed: {e:?}"),
                Err(_) => tracing::warn!("fleet tick: ProcessQueue nudge to agent {agent_id} timed out"),
                Ok(Ok(())) => {}
            }
        })
    });
    join_all(sends).await;
}
```
**Confirmed by adversarial review:** `futures-util = { workspace = true }` is
already present at `crates/vox-orchestrator/Cargo.toml:71` — no dependency
edit needed, `use futures_util::future::join_all;` will resolve directly.

- [ ] **Step 4: Run test to verify it passes, then the full suite**

Run: `cargo test -p vox-orchestrator --lib`
Also re-run the existing `chat_round_trip.rs` integration test (from the
earlier session fix) to confirm the fan-out change doesn't break the original
stall-fix behavior: `cargo test -p vox-orchestrator --test chat_round_trip`

- [ ] **Step 5: Commit** (this closes Task B4 too)

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "perf(orchestrator): fan out the fleet-tick ProcessQueue nudge instead of sending serially

N agents each needing the full 5s send-timeout could stall a single
fleet tick (and everything after it - check_scaling/sync_fleet/
rebalance) by up to N x 5s. Concurrent sends bound this to one timeout
window regardless of N. Closes the reliability concern (bounded blast
radius of a wedged mailbox) and the latency concern (bounded tick
stall time) with one change."
```

---

## Phase A — chat fast-path

### Task A1: add `Chat` to the `TaskCategory` config + generated enum

**Files:**
- Modify: `contracts/orchestration/model-routing.v1.yaml`
- (No manual Rust edit — `crates/vox-orchestrator/build.rs:109-146` regenerates
  `TaskCategory`, its `Display`, and `FromStr` impls automatically from this
  file's `task_categories` list at `:55-68` on the next build.)

- [ ] **Step 1: Read the current list**

Run: `sed -n '50,70p' contracts/orchestration/model-routing.v1.yaml` to see the
exact current `task_categories:` block (design doc confirmed `Research`,
`General` [default], `Visus` are present at lines 60/63/68 — read the live file
before editing since exact ordering/formatting matters for a clean diff).

- [ ] **Step 2: Add `Chat`**

Add a `- Chat` entry to the `task_categories` list, matching the existing
entries' YAML formatting exactly.

- [ ] **Step 3: Rebuild and verify the generated enum**

Run: `cargo build -p vox-orchestrator 2>&1 | tail -20`
Expected: clean build. Then confirm the new variant exists:
Run: `cargo doc -p vox-orchestrator --no-deps 2>&1 | tail -5` (or simpler:
`grep -rn "TaskCategory::Chat" target/debug/build/vox-orchestrator-*/out/generated.rs`
after the build, to directly inspect the generated code).

- [ ] **Step 4: Commit**

```bash
git add contracts/orchestration/model-routing.v1.yaml
git commit -m "feat(orchestrator): add TaskCategory::Chat to the routing contract

Config-generated (build.rs regenerates the enum/Display/FromStr impls
from this file) - no manual Rust enum edit needed. Prerequisite for the
chat fast-path processor."
```

---

### Task A2: `ChatTaskProcessor` — single-call, non-phased task processor

**Files:**
- Create: `crates/vox-orchestrator/src/chat_processor.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs` (add `mod chat_processor;` +
  re-export, matching how `runtime` is declared/exported — check the exact
  pattern first)
- Test: same new file's `#[cfg(test)]` module

**Context:** implements the existing `TaskProcessor` trait
(`runtime.rs:98-109`), the same seam `StubTaskProcessor` (`runtime.rs:112-129`)
already demonstrates is a clean, minimal extension point. Unlike
`AiTaskProcessor`, this makes exactly ONE LLM call with a prompt written from
scratch for a single-shot conversational reply — it must NOT reuse the 6-phase
prompt template (`runtime.rs:290-299`), which is built around "Known notes"
phase accumulation that doesn't exist for a single call and would read
incoherently to the model.

- [ ] **Step 1: Use `FreeAiClient::generate_stream`, NOT `AiTaskProcessor`'s heavier call path**

**Corrected by adversarial review:** `AiTaskProcessor::process` does
routing/budget/model-registry work (`runtime.rs:180-442`) before ever calling
its underlying generation method — none of that machinery belongs in a
single-shot chat processor. `FreeAiClient` has a directly-reusable, simpler
method for exactly this shape of call: `generate_stream(prompt)`
(`crates/vox-gamify/src/ai/client/ctor.rs:291-302` — read its real signature
before using it). `ChatTaskProcessor` should call this method directly
instead of mirroring `AiTaskProcessor`'s full pattern. Still reuse
`FreeAiClient::auto_discover().await` for construction (same as
`AiTaskProcessor::new`, `runtime.rs:201`) — only the per-call generation path
differs, not client construction.

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, AgentTask, TaskCategory, TaskId, TaskPriority};
    use std::sync::atomic::AtomicBool;

    // Uses whatever fake/stub LLM client mechanism AiTaskProcessor's own
    // tests already use (check runtime.rs's test module first, or
    // vox_gamify::ai::FreeAiClient's test-construction helpers, e.g. a
    // deterministic/offline provider) rather than inventing a new one.
    #[tokio::test]
    async fn process_makes_exactly_one_call_not_a_phase_loop() {
        // ... construct a ChatTaskProcessor with a fake client that records
        // call count, run process() on a Chat-category task, assert the
        // fake client's call counter is exactly 1 (not 6).
    }
}
```
Adapt to whatever fake-client mechanism actually exists in this codebase — search
for one (`grep -rn "impl.*FreeAiClient\|fn.*fake.*client\|fn.*mock.*client" crates/vox-gamify/src/ai/`)
before assuming one needs to be built from scratch.

- [ ] **Step 3: Run test to verify it fails**

Expected: FAIL — `ChatTaskProcessor` doesn't exist yet (compile error), or once
stubbed minimally, fails the call-count assertion.

- [ ] **Step 4: Implement `ChatTaskProcessor`**

Struct + constructor (direct copy of `AiTaskProcessor`'s equivalent shape from
Step 1's read, minus the phase-loop-specific fields it doesn't need):
```rust
//! A single-call, non-phased [`crate::runtime::TaskProcessor`] for
//! conversational (chat-origin) tasks. Unlike [`crate::runtime::AiTaskProcessor`]'s
//! 6-phase Inspect/Localize/Hypothesize/Act/Verify/Decide pipeline (built for
//! genuine multi-step agentic work), this makes exactly one LLM call with a
//! prompt written for a single-shot conversational reply.

use crate::runtime::TaskProcessor;
use crate::types::{AgentId, AgentTask, TaskId};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ChatTaskProcessor {
    client: vox_gamify::ai::FreeAiClient,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<crate::orchestrator::Orchestrator>,
}

impl ChatTaskProcessor {
    pub async fn new(
        event_bus: crate::events::EventBus,
        orchestrator: Arc<crate::orchestrator::Orchestrator>,
    ) -> Self {
        let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
        Self { client, event_bus, orchestrator }
    }
}
```

`process()`'s body is the one piece of this plan that genuinely cannot be
written byte-exact without the implementer re-reading two things at
implementation time — not because the logic is unclear, but because exact
signatures need to be copied verbatim rather than re-derived: (1)
`FreeAiClient::generate_stream`'s real signature
(`vox-gamify/src/ai/client/ctor.rs:291-302`, per this task's corrected Step
1 — use this, NOT `AiTaskProcessor::run_phase_stream_with_bus`'s heavier
routed-call path at `runtime.rs:259-318`), and (2) how `AiTaskProcessor::
process` records usage at its tail (`record_ai_usage`, shown in this plan's
Ground Truth excerpt) — mirror that call's shape for token/cost accounting,
substituting this processor's single-call numbers. Concretely, `process()`
must, in order:
1. Check `cancel` and call `abort_interrupted_task` + return `Err` if set —
   identical to `AiTaskProcessor`'s cancel-path shape shown in this plan's
   Task B2 excerpt.
2. Build a single prompt string for the task's `description` — NOT the
   6-phase `"Task: {}\n\n{}{}\nPhase: {}\n...\nKnown notes:\n{}"` template
   (`runtime.rs:291`), since there is no phase/prior-notes context to fill;
   write a minimal chat-appropriate template instead, e.g.
   `format!("You are a helpful assistant responding to a chat message.\n\n{}", task.description)`.
3. Call the same streaming generation method `AiTaskProcessor` calls (read
   its exact name/signature at `runtime.rs:259-318`), emitting the same
   `AgentEventKind::TokenStreamed` events per chunk so the existing frontend
   chat-bubble streaming (`chatCorrelation.ts`, wired to these events since
   before this plan) keeps working unmodified.
4. On completion, emit whatever start/completion event pairing signals the
   frontend's `pending → streaming → done` bubble transition (read
   `chatCorrelation.ts`'s event-name expectations, established earlier this
   session, to confirm exactly which events must fire and in what order —
   likely a single `TaskPhaseChanged` plus the terminal `task_completed`/
   `task_failed` framing the daemon layer already wraps every processor's
   return value in, rather than anything ChatTaskProcessor emits directly).
5. Call `self.orchestrator.record_ai_usage(...)` with the single call's real
   token/cost numbers, mirroring `AiTaskProcessor::process`'s end-of-function
   usage recording (this plan's Ground Truth section shows the tail of that
   call).
6. Return `Ok(task.id)` on success, propagate the streaming call's error
   (with the same `abort_interrupted_task` cleanup-before-return the cancel
   path uses) on failure.

Write this body directly against the real, freshly-read `AiTaskProcessor`
implementation — every piece above has a concrete real-code analog to copy
from, this is a mechanical adaptation task, not an open design question.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator chat_processor`

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/chat_processor.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): ChatTaskProcessor - single-call task execution for chat-origin tasks

Implements the existing TaskProcessor trait (the same seam
StubTaskProcessor already proves is a clean extension point) with
exactly one LLM call and a prompt written from scratch for a
single-shot conversational reply, instead of AiTaskProcessor's 6-phase
pipeline built for multi-step agentic work."
```

---

### Task A3: `RoutingTaskProcessor` — dispatch by task category

**Files:**
- Create: `crates/vox-orchestrator/src/routing_processor.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs`
- Modify: wherever `AgentFleet` is constructed with its `Arc<dyn TaskProcessor>`
  (find via `grep -rn "AiTaskProcessor::new\|AiTaskProcessor::with_tool_dispatcher"
  crates/`  — likely `vox-orchestrator-d`'s main and/or `crates/vox-gui/src/commands/daemon.rs`,
  both of which construct the fleet's processor at startup)
- Test: same new file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{StubTaskProcessor, TaskProcessor};
    use crate::types::{AgentId, AgentTask, TaskCategory, TaskId, TaskPriority};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    // Two counting stub processors distinguishable by which one got called.
    struct CountingProcessor(std::sync::atomic::AtomicUsize);
    #[async_trait::async_trait]
    impl TaskProcessor for CountingProcessor {
        async fn process(&self, _a: AgentId, task: AgentTask, _c: Arc<AtomicBool>) -> anyhow::Result<TaskId> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(task.id)
        }
    }

    #[tokio::test]
    async fn chat_category_routes_to_chat_processor_others_to_agentic() {
        let chat_calls = Arc::new(CountingProcessor(Default::default()));
        let agentic_calls = Arc::new(CountingProcessor(Default::default()));
        let router = RoutingTaskProcessor::new(agentic_calls.clone(), chat_calls.clone());

        let mut chat_task = AgentTask::new(TaskId(1), "hi", TaskPriority::Normal, vec![]);
        chat_task.task_category = TaskCategory::Chat;
        router.process(AgentId(1), chat_task, Arc::new(AtomicBool::new(false))).await.unwrap();
        assert_eq!(chat_calls.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(agentic_calls.0.load(std::sync::atomic::Ordering::SeqCst), 0);

        let agentic_task = AgentTask::new(TaskId(2), "do a thing", TaskPriority::Normal, vec![]);
        router.process(AgentId(1), agentic_task, Arc::new(AtomicBool::new(false))).await.unwrap();
        assert_eq!(agentic_calls.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
```
(Confirm `task_category` is a public, directly-settable field on `AgentTask`
by reading `types/tasks.rs`'s struct definition before assuming this exact
mutation syntax compiles.)

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — `RoutingTaskProcessor` doesn't exist.

- [ ] **Step 3: Implement**

```rust
//! Dispatches a task to the chat processor or the full agentic processor
//! based on `task.task_category`, while presenting exactly one
//! [`crate::runtime::TaskProcessor`] to [`crate::runtime::AgentFleet`] (which
//! holds only one `Arc<dyn TaskProcessor>` for the whole fleet).

use crate::runtime::TaskProcessor;
use crate::types::{AgentId, AgentTask, TaskCategory, TaskId};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct RoutingTaskProcessor<A: TaskProcessor, C: TaskProcessor> {
    agentic: Arc<A>,
    chat: Arc<C>,
}

impl<A: TaskProcessor, C: TaskProcessor> RoutingTaskProcessor<A, C> {
    pub fn new(agentic: Arc<A>, chat: Arc<C>) -> Self {
        Self { agentic, chat }
    }
}

#[async_trait::async_trait]
impl<A: TaskProcessor, C: TaskProcessor> TaskProcessor for RoutingTaskProcessor<A, C> {
    async fn process(
        &self,
        agent_id: AgentId,
        task: AgentTask,
        cancel: Arc<AtomicBool>,
    ) -> anyhow::Result<TaskId> {
        match task.task_category {
            TaskCategory::Chat => self.chat.process(agent_id, task, cancel).await,
            _ => self.agentic.process(agent_id, task, cancel).await,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator routing_processor`

- [ ] **Step 5: Wire it into fleet construction**

**Corrected by adversarial review: there is exactly ONE production call site,
not two as originally guessed.** `crates/vox-gui/src/commands/daemon.rs`
does **not** construct `AgentFleet`/`AiTaskProcessor` at all — the GUI's
`PersistentDaemon` adopts/spawns the standalone `vox-orchestrator-d` process
rather than embedding the fleet in-process. The one real construction site is
`spawn_agent_fleet_if_enabled_with_dispatcher`
(`crates/vox-orchestrator/src/runtime.rs:1247-1279`), called from
`crates/vox-orchestrator-d/src/bin/vox_orchestrator_d.rs:267`. Edit ONLY this
site:
```rust
let agentic = Arc::new(AiTaskProcessor::with_tool_dispatcher(...).await); // or ::new, per existing call
let chat = Arc::new(ChatTaskProcessor::new(event_bus.clone(), orchestrator.clone()).await);
let processor: Arc<dyn TaskProcessor> = Arc::new(RoutingTaskProcessor::new(agentic, chat));
let fleet = AgentFleet::new(scheduler, orchestrator, processor);
```
Read `spawn_agent_fleet_if_enabled_with_dispatcher`'s real current body
(`runtime.rs:1247-1279`) before editing — do not assume the sketch above
matches its exact existing variable names/construction order. Also grep
`AiTaskProcessor::new\|AiTaskProcessor::with_tool_dispatcher` across the whole
repo one more time at implementation time (not just trusting this plan's
claim) in case something changed between this audit and implementation.

- [ ] **Step 6: Run the full orchestrator + vox-gui Rust suites**

Run: `cargo test -p vox-orchestrator --lib`
Run: `cargo test -p vox-gui --bin vox-gui`
Expected: all pass (fleet construction call sites now build both processors —
verify no test relies on the concrete `AiTaskProcessor` type where it now gets
a `RoutingTaskProcessor` instead, e.g. via `downcast` or similar — unlikely
given the trait-object design, but check).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator/src/routing_processor.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): RoutingTaskProcessor dispatches Chat-category tasks to the fast path

AgentFleet holds exactly one Arc<dyn TaskProcessor> for every agent and
task; this thin wrapper owns both AiTaskProcessor (agentic, 6-phase)
and ChatTaskProcessor (single-call) and dispatches on
task.task_category, so resolve_route/spawning/queueing/task-history/
dedup/cost-tracking/event-bus all stay untouched - chat keeps every
piece of infrastructure it already gets 'for free' today."
```

---

### Task A4: thread a `task_category` hint from the chat composer through to `AgentTask`

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (`handleLoquelaSubmit`)
- Modify: `crates/vox-gui/ui/src/types/tauri.ts` (`ChatPayload`, or wherever the
  submit-input type used by `handleLoquelaSubmit` lives — confirm exact type
  name before editing)
- Modify: `crates/vox-gui/src/commands/control_plane.rs` (`SubmitTaskInput`,
  `submit_task_params`)
- Modify: `crates/vox-orchestrator/src/orch_daemon/mod.rs` (`SUBMIT_TASK`
  handler)
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`
  (`submit_task_with_agent`) and/or `crates/vox-orchestrator/src/types/tasks.rs`
  (`AgentTask::new`)
- Test: frontend — extend `App.test.tsx` or a chat-submit-focused test file;
  backend — extend the daemon's `SUBMIT_TASK` test coverage (the
  `submit_task_treats_explicit_null_priority_and_file_manifest_as_omitted`
  test added earlier this session, in `orch_daemon/mod.rs`'s test module, is
  the right neighborhood and pattern to follow)

**Design constraint (from the design doc):** do NOT reuse the existing
`[[category:X]]` description-text-marker convention for chat — that text is
NOT stripped from `description` before being embedded verbatim in the 6-phase
prompt template's `"Task: {}"` slot (`runtime.rs:291`) for other categories
today (pre-existing, harmless there since agentic tasks' descriptions aren't
directly user-facing chat bubbles), but chat's `description` IS the literal
text shown back to the user in the transcript — a leaked `[[category:chat]]`
marker would be visible in the UI. Use a genuine explicit field instead.

- [ ] **Step 1: Read every real hop's current exact shape**

Read, in order, the current real code at:
1. `crates/vox-gui/ui/src/App.tsx`'s `handleLoquelaSubmit` (search for it —
   note in an earlier session investigation it was cited near line 674-798;
   confirm current location) and whatever payload type it builds.
2. `crates/vox-gui/ui/src/types/tauri.ts`'s `ChatPayload` (or current
   equivalent type name).
3. `crates/vox-gui/src/commands/control_plane.rs`'s `SubmitTaskInput` struct
   and `submit_task_params` function.
4. `crates/vox-orchestrator/src/orch_daemon/mod.rs`'s `SUBMIT_TASK` handler
   (the null-safe `priority`/`file_manifest` parsing pattern added earlier
   this session, ~line 467-545, is the exact idiom to extend).
5. `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`'s
   `submit_task_with_agent` signature (large function — read enough to find
   where it calls `AgentTask::new` or equivalent construction).
6. `crates/vox-orchestrator/src/types/tasks.rs`'s `AgentTask::new` (already
   quoted in this plan's Ground Truth section, `:704-725`).

- [ ] **Step 1b: Add `task_category` to the `LIST_TASKS` response — confirmed missing, blocks Step 2's test**

**Found by adversarial review:** the `LIST_TASKS` handler
(`orch_daemon/mod.rs:644-681`) does not serialize `task_category`/`category`
in its response JSON today. Step 2's test below asserts on a `"category"`
field that doesn't exist yet — this is a real, separate gap this task must
close first, not an assumption to adapt around. Read the handler's response-
building code (`orch_daemon/mod.rs:644-681`, the same block that already
serializes `"priority"`/`"lifecycle"` per this plan's Ground Truth excerpt at
`:656-660`) and add a `"category": t.task_category` (or however the existing
fields are named/formatted — match the exact style) entry alongside them.

- [ ] **Step 2: Write the failing backend test first**

Following the `submit_task_treats_explicit_null_priority_and_file_manifest_as_omitted`
test's exact pattern in `orch_daemon/mod.rs`'s test module:
```rust
#[tokio::test]
async fn submit_task_with_explicit_chat_category_routes_to_chat_processor_category() {
    let orch = Arc::new(Orchestrator::new(OrchestratorConfig::for_testing()));
    orch.spawn_agent("a1").unwrap();
    let resp = dispatch_request(
        "rid",
        Arc::clone(&orch),
        &req(
            orch_daemon_method::SUBMIT_TASK,
            serde_json::json!({
                "description": "hi there",
                "task_category": "chat",
            }),
        ),
    ).await;
    let task_id = result_value(&resp)["task_id"].as_u64().unwrap();
    let list_resp = dispatch_request(
        "rid", orch,
        &req(orch_daemon_method::LIST_TASKS, serde_json::json!({})),
    ).await;
    let tasks = result_value(&list_resp)["tasks"].as_array().unwrap();
    let t = tasks.iter().find(|t| t["id"].as_u64() == Some(task_id)).unwrap();
    assert_eq!(t["category"].as_str(), Some("Chat")); // adapt field name to
      // whatever LIST_TASKS actually serializes task_category as — read that
      // handler's response-building code first (grep "fn.*list_tasks\|LIST_TASKS"
      // in orch_daemon/mod.rs).
}
```

- [ ] **Step 3: Run test to verify it fails**

Expected: FAIL — no `task_category` param is parsed yet, or it's silently
ignored.

- [ ] **Step 4: Add null-safe `task_category` parsing to `SUBMIT_TASK`**

Mirror the existing `priority` parsing exactly (`orch_daemon/mod.rs`, the
`.filter(|v| !v.is_null())` pattern added earlier this session):
```rust
let task_category = req
    .params
    .get("task_category")
    .filter(|v| !v.is_null())
    .and_then(|x| x.as_str())
    .and_then(|s| s.parse::<TaskCategory>().ok());
```
**Gotcha confirmed by adversarial review, not just a hedge to verify:** the
generated `FromStr` impl's fallback arm is `_ => Ok(Self::General)`, never an
`Err` (`build.rs:133-147`) — it does lowercase-match `"chat"` correctly, but
`.parse().ok()` silently maps any TYPO (`"chta"`, `"Chat "` with whitespace,
etc.) to `TaskCategory::General` rather than surfacing an error. This is
safe-by-default for THIS use (a typo'd category silently falls back to
agentic routing, not a crash or a wrong-but-plausible category), but it means
a bug in the frontend's literal `'chat'` string would fail silently — always
route-test this path with the harness's `chat_round_trip.rs` proof (Task A5)
rather than trusting the parse alone.
Thread `task_category` through to wherever `AgentTask` is actually constructed
(Step 1.5/1.6's read tells you exactly where) — if `AgentTask::new`'s
signature can't cleanly take a 5th param without breaking its many other call
sites, prefer setting the field directly after construction
(`task.task_category = category.unwrap_or_default();`) rather than changing
the constructor's signature everywhere, unless the codebase's convention
clearly favors constructor params (check how `priority`/`file_manifest`,
which similarly needed daemon-level plumbing, were threaded through in this
session's earlier `dfe05437bf` fix, and match that precedent).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator submit_task_with_explicit_chat_category`

- [ ] **Step 6: Thread the field through the GUI layers (frontend -> Tauri command) — NOT unconditionally**

**Corrected by adversarial review: this is a confirmed conflict, not a
hypothetical to judge at implementation time.** `App.tsx`'s slash-command
dispatch (`handleLoquelaSlash`, verified at `~:800-888`) — specifically
`/spawn` — calls `handleLoquelaSubmit` with `{ ..., mode: 'act' }`, sharing
the EXACT same code path as free-text chat. Tagging every submission through
`handleLoquelaSubmit` as `'chat'` unconditionally would silently reroute
`/spawn`'s sub-agent dispatch into the one-shot `ChatTaskProcessor`, breaking
real agentic execution triggered from the composer. Add `task_category: 'chat'`
to:
- `ChatPayload` (or equivalent) in `types/tauri.ts`.
- `handleLoquelaSubmit`'s payload construction in `App.tsx` — set it to
  `'chat'` ONLY when `mode !== 'act'` (confirmed conflict case) — e.g.
  `task_category: mode === 'act' ? undefined : 'chat'` (omitted/undefined
  falls through to the daemon's existing `TaskCategory::default()` /
  description-marker-scan behavior for agentic submissions, unchanged). Read
  `handleLoquelaSlash`'s FULL body (`~:800-888`) at implementation time to
  confirm `/spawn` is the only `mode: 'act'` case and there isn't a second,
  differently-shaped conflict this plan hasn't found — do not assume the
  `mode !== 'act'` check alone is sufficient without that read.
- `SubmitTaskInput` in `control_plane.rs` (new `Option<String>` field) and
  `submit_task_params`'s payload-building (pass it through to the daemon RPC
  params as `"task_category"` — the daemon's existing null-safe
  `.filter(|v| !v.is_null())` parsing pattern already handles an
  omitted/`None` value falling back to default category correctly, per this
  session's earlier priority/file_manifest fix).

- [ ] **Step 7: Write a frontend regression test**

Extend whatever test file already covers `handleLoquelaSubmit` (likely part of
`App.test.tsx` — check for existing chat-submit test coverage first) with an
assertion that the `submit_orchestrator_task` invoke call's payload includes
`task_category: 'chat'`.

- [ ] **Step 8: Run full frontend + backend suites**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run && npx tsc --noEmit`
Run: `cargo test -p vox-orchestrator --lib`
Run: `cargo test -p vox-gui --bin vox-gui`

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/src crates/vox-gui/src/commands/control_plane.rs \
  crates/vox-orchestrator/src/orch_daemon/mod.rs \
  crates/vox-orchestrator/src/orchestrator/task_dispatch/submit \
  crates/vox-orchestrator/src/types/tasks.rs
git commit -m "feat(gui,orchestrator): tag chat-composer submissions with TaskCategory::Chat

Threads an explicit task_category field from handleLoquelaSubmit
through SubmitTaskInput -> daemon SUBMIT_TASK params -> AgentTask,
using the same null-safe parsing idiom already established for
priority/file_manifest. Deliberately NOT reusing the existing
[[category:X]] description-text-marker convention: that text isn't
stripped before being embedded in the agentic 6-phase prompt's visible
'Task:' slot, and chat's description IS the literal user-visible
transcript text - a leaked marker would show up in the chat UI."
```

---

### Task A5: end-to-end proof the fast path is actually taken (extends `chat_round_trip.rs`)

**Files:**
- Modify: `crates/vox-orchestrator/tests/chat_round_trip.rs`

- [ ] **Step 1: Read the existing test**

Read `chat_round_trip.rs` in full (added earlier this session — proves
submit → task_started → task_completed with a `StubTaskProcessor` and zero LLM
cost) to match its exact daemon-setup/event-subscription idiom.

- [ ] **Step 2: Write the failing test**

```rust
#[tokio::test]
async fn chat_category_task_emits_exactly_one_phase_change_not_six() {
    // Same in-process daemon + StubTaskProcessor-equivalent setup as
    // chat_submit_round_trip_completes_without_resubmit, but wrapped in a
    // RoutingTaskProcessor pairing a counting-stub "agentic" processor with
    // a counting-stub "chat" processor (per Task A3's test pattern), so this
    // test proves ROUTING without needing a real/fake LLM call either.
    // Submit with "task_category": "chat", subscribe to events, assert
    // exactly one TaskPhaseChanged (or however ChatTaskProcessor signals
    // progress - confirmed during Task A2) event fires, not six.
}
```

- [ ] **Step 3: Run test to verify it fails**

Expected: FAIL before Tasks A1-A4 land (this task should genuinely run LAST,
after everything else in Phase A, so this red state is only real if you're
implementing tasks out of order for some reason — normally by this point in
the plan it will already pass and Step 3/4 collapse into one verification
step; keep the TDD discipline nominal by writing this test's assertion before
checking it, even if the underlying fix already exists from prior tasks).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator --test chat_round_trip`
Expected: all tests in this file pass, including both the original stall-fix
test and this new one.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/tests/chat_round_trip.rs
git commit -m "test(orchestrator): prove chat-category tasks take the fast path, not the 6-phase pipeline

Extends the earlier chat_round_trip.rs (stall-fix proof) with a
routing-proof test: a Chat-category submission emits exactly one
phase-change event through RoutingTaskProcessor's chat side, not six
through the agentic side. Zero LLM cost, same StubTaskProcessor-style
pattern as the rest of this test file."
```

---

## Final task: whole-effort verification + push

- [ ] **Step 1: Full test suites**

```bash
cargo test -p vox-orchestrator --lib
cargo test -p vox-orchestrator --tests
cargo test -p vox-gui --bin vox-gui
cd crates/vox-gui/ui && pnpm exec vitest run && npx tsc --noEmit
```
Expected: all green.

- [ ] **Step 2: Rebuild and relaunch the app for manual confirmation**

```bash
cd crates/vox-gui/ui && pnpm build
cargo build --release -p vox-gui
```
Stop any running `vox-gui.exe`, relaunch (`Start-Process`, working directory
`crates/vox-gui`), verify it's stable (not crashed after 6s).

- [ ] **Step 3: `cargo fmt -p` each touched crate** (never `cargo fmt --all` on
  this Windows workspace — see `AGENTS.md`)

```bash
cargo fmt -p vox-orchestrator
cargo fmt -p vox-gui
```

- [ ] **Step 4: Push**

```bash
git fetch origin main
git push origin HEAD:main
```
Handle pre-push hook failures per this session's established pattern: fmt
failures → `cargo fmt -p <crate>`, commit, retry; contract drift →
`VOX_SKIP_FRESHNESS_CHECK=1 ./target/release/vox.exe ci gui-surface-coverage --write`
and `ci test-inventory --output contracts/reports/test-inventory.v1.json`,
commit, retry. Confirm landed: `git log origin/main -1 --oneline`.

- [ ] **Step 5: Report**

Deliver a final summary to the user: chat now takes a single-call fast path
(measured latency improvement if observable — a rough "N phases → 1 phase" is
honest even without precise timing numbers); the 5 reliability fixes and what
each closes; the compaction wiring and its expected effect (bounded prompt
growth on longer-running agentic tasks too, not just chat); all suites green;
app rebuilt and relaunched; final commit hash.
