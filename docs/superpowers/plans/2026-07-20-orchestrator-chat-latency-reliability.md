# Orchestrator Chat Fast-Path, Reliability, and Latency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give chat messages a fast, non-agentic reply path; fix 5 real reliability
gaps found by a fresh orchestrator audit; wire an already-built compaction engine
into the one place it's needed most.

**Architecture:** Three independently landable phases (A: chat fast-path via a new
`TaskCategory::Chat` + dedicated processor behind a thin dispatching wrapper; B:
5 small reliability fixes; C: 2 latency fixes, one shared with Phase B). Every
task is TDD'd, zero paid LLM calls in any new test (mirrors the `StubTaskProcessor`
/ `chat_round_trip.rs` pattern already in the codebase from this session's earlier
fix).

**Tech Stack:** Rust (`crates/vox-orchestrator`), TypeScript/React
(`crates/vox-gui/ui`), Tauri commands (`crates/vox-gui/src/commands`).

**Design doc:** [`docs/superpowers/specs/2026-07-20-orchestrator-chat-latency-reliability-design.md`](../specs/2026-07-20-orchestrator-chat-latency-reliability-design.md)

**Ground-truth line numbers below were read directly from the current worktree
(`C:\Users\Owner\vox\.worktrees\axis-frontend-remediation`) — if they've drifted by
the time you implement, re-locate by content match, not blind line offset.**

---

## Phase B — reliability hardening (do this first: small, independent, no schema changes)

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
    // Manufacture a drift record directly through the budget manager, the
    // same way runtime.rs's record_agent_iteration would after two
    // fingerprint-matching phase outputs.
    {
        let bm = orch.budget_manager_handle();
        let bm = crate::sync_lock::rw_write(&*bm);
        bm.record_agent_iteration(agent_id, "same output", false); // 1st: drift_streak stays 0 (no prior match)
        bm.record_agent_iteration(agent_id, "same output", false); // 2nd: matches -> drift_streak = 1
    }
    let task_id = orch.submit_task_with_agent(
        "t1".into(), vec![], None, Some("a1".into()), None, None, None,
    ).await.unwrap().0; // adapt to submit_task_with_agent's real signature/return
    // fail it
    orch.fail_task(task_id, "boom".into()).await.unwrap();
    // A fresh drift check for this agent must start clean, not inherit drift_streak.
    let bm = orch.budget_manager_handle();
    let bm = crate::sync_lock::rw_read(&*bm);
    let decision = bm.record_agent_iteration(agent_id, "same output", false);
    assert!(matches!(decision, crate::budget::DriftDecision::Continue));
}
```
Read `record_agent_iteration`'s real signature (`budget/mod.rs`, the function
containing the code shown in the design doc's "Ground truth" section, just above
`reset_drift`) and `submit_task_with_agent`'s real signature/return type
(`task_dispatch/submit/task_submit.rs:106+`) before finalizing this test — the
sketch above captures the intent (drift state must not survive across a
fail_task boundary), adapt exact argument shapes to what actually compiles.

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

- [ ] **Step 1: Read the exact `notes`-building code path**

Read `runtime.rs` lines 520-665 in full again immediately before editing (the
line numbers above are from investigation, not a diff-safe anchor) to confirm
exactly where `notes` is read for prompt-building (inside `run_phase_stream`'s
call at `~540-566`, passing `notes.as_str()`) versus where it's mutated
(`~607-610`, `~660-661`).

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

- [ ] **Step 4: Convert `notes` accumulation to compact before each phase's prompt build**

Sketch (adapt to whatever `Turn` construction/config-sourcing pattern Step 1's
read of `session/state.rs:257` revealed):
```rust
// Before building this phase's prompt, compact the accumulated history if
// it has grown past the configured trigger.
let history_turns: Vec<crate::compaction::Turn> = /* map notes' per-phase
    segments into Turn entries — one per completed phase's [phase_name]\n...
    block, role "assistant" (or whatever session/state.rs's convention is) */;
let compacted = compaction_engine.compact(&history_turns)?;
let notes_for_prompt = /* re-flatten compacted.retained_turns back into the
    "[Phase]\n..." string shape the prompt template expects */;
```
The exact shape of "map notes into Turns and back" depends on how granular the
existing `notes` string's internal structure is (it's built as
`"[{phase}]\n{phase_out}"` blocks joined by `\n\n`, per `:607-610` — one Turn per
phase block is the natural mapping). Do not lose the lossless-archival contract
`compact()` documents (`compaction.rs:136-141`) — if any dropped turns need to
be durably logged (matching how `session` persists `dropped_turns`), check
whether that's necessary for phase-loop notes too or whether losing them is
acceptable here (session transcripts are user-facing history; phase notes are
an internal scratchpad the task discards after completion anyway — likely
acceptable to NOT persist `dropped_turns` for this use, but confirm by reading
how `session/state.rs` treats them, and make a deliberate choice either way,
noting it in the commit).

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

- [ ] **Step 1: Write the failing test**

Construct a scenario with 2+ agents whose sends would each need the full 5s
timeout to fail (e.g. mock/fake handles that never respond — check whether
`vox_actor_runtime::mailbox`'s `Envelope`/handle types are fakeable in a unit
test, or whether this needs a coarser integration-level timing assertion in
`crates/vox-orchestrator/tests/`). Assert the TOTAL time `nudge_queued_agents`
takes is close to ONE timeout window (e.g. `< 6s` for N=3 failing agents),
not `N × 5s`.

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL (or times out very slowly) under the current serial
implementation — may need a shortened timeout constant in the test harness to
keep this fast; check whether `D_5S` is overridable/injectable for tests, or
scale the test's failure-simulation and assertion window accordingly (e.g. use
a much shorter configured timeout in a test-specific fleet construction if the
timeout is parameterized anywhere; if it's hardcoded to `vox_config::timeouts::D_5S`
globally, this test may need to accept a proportionally longer real wall-clock
run — note this tradeoff rather than skip the test).

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
Check whether `futures_util` (or `futures`) is already a `vox-orchestrator`
dependency before adding it — this crate almost certainly already depends on
`futures`/`futures_util` transitively via tokio's ecosystem; check `Cargo.toml`
first and reuse whatever's already there rather than adding a new dependency if
avoidable.

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

- [ ] **Step 1: Read `AiTaskProcessor`'s client-usage pattern**

Read `runtime.rs:180-260` (the `AiTaskProcessor` struct + `new`/
`with_tool_dispatcher` constructors + the start of `run_phase_stream`) to see
exactly how `vox_gamify::ai::FreeAiClient` is constructed and invoked for a
single generation call — `ChatTaskProcessor` should reuse the SAME client type
and construction pattern (`FreeAiClient::auto_discover().await`), not invent a
new LLM-calling mechanism.

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
written byte-exact without the implementer re-reading `AiTaskProcessor::
run_phase_stream_with_bus`'s real streaming-call body (`runtime.rs:259-318+`)
at implementation time — not because the logic is unclear, but because the
exact `FreeAiClient` method name/signature, the exact `AgentEventKind` variant
used per streamed chunk, and how `notes`/token accounting get recorded
(`record_ai_usage`, called at the end of `AiTaskProcessor::process` per this
plan's Ground Truth excerpt) all need to be copied verbatim from that function
rather than re-derived. Concretely, `process()` must, in order:
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

Find every call site constructing `AgentFleet` with `Arc::new(AiTaskProcessor::...)`
(grep as noted above). Change each to construct both processors and wrap in
`RoutingTaskProcessor`:
```rust
let agentic = Arc::new(AiTaskProcessor::with_tool_dispatcher(...).await); // or ::new, per existing call
let chat = Arc::new(ChatTaskProcessor::new(event_bus.clone(), orchestrator.clone()).await);
let processor: Arc<dyn TaskProcessor> = Arc::new(RoutingTaskProcessor::new(agentic, chat));
let fleet = AgentFleet::new(scheduler, orchestrator, processor);
```
Read each real call site's exact existing variable names/construction order
before editing — do not assume they're identical across `vox-orchestrator-d`'s
main and `vox-gui`'s daemon.rs if both construct a fleet.

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
    .and_then(|s| s.parse::<TaskCategory>().ok()); // TaskCategory::FromStr,
      // generated by build.rs (Task A1) - confirm the exact generated
      // signature/error type before assuming `.parse()` works this way.
```
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

- [ ] **Step 6: Thread the field through the GUI layers (frontend -> Tauri command)**

Add `task_category: 'chat'` (or the exact string `TaskCategory::FromStr`
expects, confirmed in Step 4) to:
- `ChatPayload` (or equivalent) in `types/tauri.ts`.
- `handleLoquelaSubmit`'s payload construction in `App.tsx` — set it
  unconditionally to `'chat'` for every submission through this handler
  (confirmed in the design doc: nothing else needs to distinguish sub-cases
  within `handleLoquelaSubmit` — slash commands and skill deploys going
  through the SAME handler get `'chat'` too unless there's a reason found
  during Step 1's read to special-case them; if slash-command dispatch
  clearly wants agentic-not-chat routing for some commands, note that as a
  found design wrinkle and handle it explicitly rather than silently
  defaulting everything through this one handler to chat — read
  `handleLoquelaSubmit`'s full body first to judge this).
- `SubmitTaskInput` in `control_plane.rs` (new `Option<String>` field) and
  `submit_task_params`'s payload-building (pass it through to the daemon RPC
  params as `"task_category"`).

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
