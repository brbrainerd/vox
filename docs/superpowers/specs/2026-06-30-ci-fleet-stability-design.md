---
title: CI self-hosted fleet stability — freshness-guard starvation fix + self-heal
date: 2026-06-30
status: approved-design (in implementation)
---

# CI fleet stability: stop the autoscaler starving to zero

## Problem (observed 3×)

The self-hosted CI fleet repeatedly drops to **0 online runners**, stalling every PR's
required self-hosted checks indefinitely (PR #409 hit this across many sessions). The
dead-man watchdog correctly alarms, but nothing recovers.

## Root cause (verified)

The autoscaler is a Windows Task (`VoxCIRunnerScale`, every 2 min) → `vox run
scripts/ci-runners-up.vox` → `vox ci runner-scale --apply`. Its design is sound:
**ephemeral runners, scale 0↔`VOX_RUNNER_MAX` (6) to match queued jobs, warm-pool 1,
idle-reap 300 s.** Each runner takes one job and exits.

But `vox ci` applies a **blanket freshness guard** — `crate::freshness::enforce_for_ci`
at `crates/vox-cli/src/commands/ci/run_body.rs:56`, **before** the subcommand match —
that hard-aborts when the *installed binary's build commit* lags the *working-tree
commit*. With multiple agents committing/merging, the tree races ahead within minutes,
so the guard trips, `runner-scale` exits 1 **every tick**, ephemeral runners exit and
are never replaced, and the fleet starves to 0. The guard exists to prevent stale
*guard verdicts* — but `runner-scale` produces **no verdict**; it reconciles Docker
containers. It should never have been gated.

Verified: with `VOX_SKIP_FRESHNESS_CHECK=1`, the scaler runs perfectly
(`queued_jobs=6 → desired=6 → spawn 6`). Without it, it aborts.

## Design

### Fix A — exempt infra commands from the freshness guard (the cure)
In `run_body.rs::run`, skip `enforce_for_ci` for the infra reconcile/read commands,
which carry no correctness verdict:

```rust
let is_infra = matches!(
    cmd,
    CiCmd::RunnerScale { .. } | CiCmd::RunnerPreflight | CiCmd::RunnerStatus
);
if !is_infra {
    crate::freshness::enforce_for_ci(&root)?;
}
```

All *guard* subcommands (lint/parity/audit gates) keep the freshness guard — their
verdicts depend on current source. Only the runner-fleet ops are exempt.

**Bootstrap:** rebuild + install the binary (`cargo install --locked --path
crates/vox-cli --force`) so the *running* `vox` carries the exemption. Thereafter the
existing 2-min Task self-maintains the fleet **regardless of how far the tree drifts**
— no env hacks, no per-commit rebuilds.

### Fix D — watchdog local remediation (defense in depth)
The current watchdog (`VoxCIHealthWatchdog` → `ci-health-watchdog.yml` +
`ci-health-assess`) **assesses + alerts** but runs GitHub-hosted, so it cannot spawn
local runners. Add a **local** remediation tick (extend the watchdog Task or a new
`vox ci runner-heal`) that, when it sees `online==0 && queued>0`:
1. ensure Docker is up (`docker info`; if down, surface a clear error — starting Docker
   Desktop may need the user),
2. run `vox ci runner-scale --apply` (now freshness-exempt),
3. if the installed binary is grossly stale (e.g. > N commits or > M days), kick a
   background `cargo install` to refresh it,
4. escalate to the existing alert only if it still can't recover after K ticks.

This catches the rarer failure modes A doesn't (Docker/WSL crash, image missing) so a
starved fleet recovers within a tick instead of waiting for a human.

## Queuing & multi-agent concurrency (answering the design question)
The ephemeral **scale-to-queued-demand** model is correct and already multi-PR/
multi-agent friendly: GitHub's queue dispatches one job per ephemeral runner; the
autoscaler simply keeps the pool sized to `min(queued, MAX)`. Concurrency across PRs/
agents is bounded only by `VOX_RUNNER_MAX` (default 6; 6 × 5 GB = 30 GB < 32 GB WSL2
cap). Raising throughput = raise `VOX_RUNNER_MAX` on a bigger host, or move the fleet
off the dev box. No queuing redesign is needed — the fix is keeping the scaler alive.

## Testing
- Unit: `run_body::run` does **not** call `enforce_for_ci` for `RunnerScale/
  RunnerPreflight/RunnerStatus`, and **does** for a sample guard command (e.g.
  `SsotDrift`). (Refactor the gate into a testable `fn should_enforce_freshness(&CiCmd)
  -> bool`.)
- Manual: with a deliberately stale installed binary, `vox ci runner-scale` succeeds
  (no abort); the `VoxCIRunnerScale` Task `Last Result` returns to 0; fleet self-tops
  to demand across a source-tree advance.

## Out of scope
- Changing the freshness guard for actual `vox ci` *guard* gates (intentionally kept).
- The `cookie`/`time` break in `vox run`'s JIT script-compile path (separate; `vox-cli`
  itself checks clean).
