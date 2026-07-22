---
title: Orchestrator daemon launch reliability + v1.0 readiness audit
status: approved
---

# Orchestrator daemon launch reliability + v1.0 readiness audit — design

## Context

Vox Axis (the GUI, `crates/vox-gui`) already auto-launches and supervises the orchestrator daemon (`vox-orchestrator-d`) when run standalone — this is more mature than initially assumed. Investigation of `crates/vox-gui/src/commands/daemon.rs` confirmed:

- `PersistentDaemon::reensure` adopts a reachable, token-authenticated daemon or spawns a fresh one, with a 15s connect timeout (100ms poll interval), killing the spawned child on timeout and surfacing a real error string (`daemon.rs:136-232`).
- A `reensure_lock` mutex serializes concurrent ensure/spawn attempts within the GUI process, and a background supervisor re-pings every 5s and self-heals a mid-session daemon death (`daemon.rs:71-76, 250-265`).
- Token-authenticated ping-before-adopt rejects an unrelated process squatting on the daemon's port rather than mistaking it for "the" daemon (`daemon.rs:150-166`).
- Frontend (`useOrchestratorStatus.ts:126-134`) shows a reconnecting/stale state rather than crashing when the initial stream registration fails; workspace DB connection is best-effort with a friendly "Database busy" fallback (`gui_db_pool.rs:34-58`).

So the "always properly launched" concern is largely already solved at the mechanism level. The real gaps are: (1) the one smoke test that proves this end-to-end (`crates/vox-gui/tests/gui_relaunch_smoke.rs`, mapping to criterion **CR-U6** in `docs/src/architecture/v1-foundation-criteria-research-2026.md`) is `#[ignore]`d behind an env var and not confirmed to run in CI — meaning regressions in this exact subsystem could ship undetected; and (2) per the user's explicit choice, this spec is scoped broadly: also catalog the *other* CR-U1–U5 criteria in that same foundation document to build a real picture of what's blocking a v1.0 release, not just the launch-specific piece.

This spec is the first of three related specs from the same brainstorming session (bottom status bar redesign; orchestrator/GUI build-version parity) — this one owns daemon-launch reliability and the overall v1.0 gap inventory; the version-parity mechanism itself is spec'd separately (`2026-07-22-build-version-parity-design.md`) since it has its own design surface, though this spec's CI work and that spec's CI work should land as one coordinated CI change, not two competing ones (flagged for the implementation plan to sequence).

## Approach

### 1. Promote CR-U6's smoke test to a required CI gate

`gui_relaunch_smoke.rs` already does the right thing (spawns the real daemon binary the same way `PersistentDaemon::ensure` does, pings it, calls `orchestrator_status`/`agent_ids`) but is gated behind `#[ignore] + VOX_GUI_RELAUNCH_SMOKE=1`, and CI's own vox-gui-related jobs largely `--exclude vox-gui` (confirmed at multiple lines in `.github/workflows/ci.yml`) because its frontend build is heavy. Fix: add a CI job (or extend an existing one) that builds both `vox-gui` and `vox-orchestrator-d` from the same commit, sets `VOX_GUI_RELAUNCH_SMOKE=1`, and runs this specific test — not the whole `--exclude`d vox-gui suite, keeping the job narrow and fast. This directly satisfies CR-U6 as a *required*, not optional, gate.

### 2. v1.0 readiness inventory (broad scope)

Read `docs/src/architecture/v1-foundation-criteria-research-2026.md` in full (not just the CR-U6 excerpt already found) and produce a concrete status table for every CR-F/CR-K/CR-U criterion it defines: built-and-verified / built-but-unverified (like CR-U6 before this spec) / genuinely unbuilt. This is a documentation/audit deliverable, not new product code — its output directly informs which of the "unbuilt" items become their own follow-up specs (out of scope for *this* spec to build, in scope to identify and hand off). Cross-reference against project memory's prior note that "CR-F/K/U harnesses UNBUILT" to confirm whether that's still accurate or whether some have shipped since (this session's own work today, e.g., may have inadvertently satisfied or partially satisfied some UI-tier criteria — check).

### 3. Defense-in-depth: explicit cross-process singleton awareness (informational, not new locking)

Investigation found today's "one-daemon" invariant is enforced implicitly (TCP bind + token-ping-before-adopt), with no explicit named lock file guarding against two independent processes racing to bind the daemon's port. Given the existing mechanism already handles the realistic case (GUI restart, GUI+CLI both wanting a daemon) correctly via adopt-if-reachable, and given the user's approved narrow-vs-broad question was specifically about *launch reliability scope*, not about re-architecting an already-working locking mechanism — this spec does **not** add a new lock file. It documents the existing mechanism's actual guarantee (safe for concurrent same-machine callers; not designed for, and not claiming to protect against, two entirely separate machines somehow sharing a port) as part of the v1.0 readiness writeup, so this is a documented-and-accepted design rather than an unexamined gap.

## What this does not include

- No changes to the daemon-spawn/supervision logic itself (`daemon.rs`'s `reensure`/`spawn_supervisor`) — it already works; this spec is about *proving* it works in CI, not rebuilding it.
- No new lock-file/OS-mutex primitive (see Approach §3).
- Version-parity detection between GUI and daemon binaries is specified separately in `2026-07-22-build-version-parity-design.md` — this spec's CI job should be written with awareness that a second, related CI job is landing alongside it (coordinate in the implementation plan, don't duplicate CI infrastructure).
- Building out any of the "genuinely unbuilt" CR items discovered by the §2 audit — those become candidate follow-up specs, not this spec's job to implement.

## Testing

`gui_relaunch_smoke.rs` itself already tests the right thing; this spec's job is making it *run*, in CI, on every relevant PR — verified by confirming the new/extended CI job actually executes it (not just exists) and fails when the test is intentionally broken (a CI-config smoke test of the smoke test, run once during implementation, not kept as a permanent double-test).

**"Required" means wired into the required-checks aggregator, not merely "a job that runs and passes"**: this repo has a real, file-based required-checks gate (`.github/workflows/ci.yml`'s `ci-summary:` job, which fails unless every job in its own `needs:` list succeeded). Adding a new job that builds and runs this test, without also adding that job's name to the aggregator's `needs:` list, would leave CR-U6 exactly as unverified-in-practice as it was before this effort — a green job nobody's merge depends on. Verification of this spec's core claim must include confirming that specific wiring, not just confirming the job's own pass/fail status in isolation.
