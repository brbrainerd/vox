---
title: CI runner unexpected-death visibility + reap hardening
description: "Detect and surface self-hosted CI runner containers that exit unexpectedly mid-job (not OOM, not a normal ephemeral job-complete exit), and harden the autoscaler's reap logic against the most likely cause: a stale/racy GitHub runners-API busy flag."
category: "Architecture SSOTs"
status: "approved"
training_eligible: false
---

# CI runner unexpected-death visibility + reap hardening — design

## Context

`vox ci runner-scale` (`crates/vox-cli/src/commands/ci/runner_scale.rs`) autoscales an ephemeral self-hosted GitHub Actions runner pool: each container registers with `--ephemeral`, takes exactly one dispatched job, self-deregisters, and exits. A Windows scheduled task (`VoxCIRunnerScale`) invokes `vox ci runner-scale --apply` every 2 minutes.

During this session's live investigation of PR #460's CI (2026-07-22), the fleet was found down (all `vox-runner-auto-*` containers exited) after being healthy shortly before. Manually triggering the scheduled task brought runners back up — but three GitHub Actions jobs (`docs-quality`, `generator-drift`, `stdlib-coverage parity`) were later found stuck reporting `in_progress` while every backing container had already exited with code 143 (`SIGTERM`) minutes earlier. This is a real, reproduced bug, not a one-off: a runner died while its assigned job was still running, and nothing surfaced that fact anywhere the operator would see it in the moment. `docker logs` on the affected container (captured before it was pruned by a later autoscaler tick) showed it successfully registered, connected, and started running `docs-quality` — then went silent; there was no crash trace, consistent with an external SIGTERM rather than the job's own process dying.

Exit code 143 rules out the failure mode this codebase already has a mature detector for: `oom_watch.rs` (`crates/vox-cli/src/commands/ci/oom_watch.rs`) detects genuine memcg OOM-kills via a `dmesg` `oom-kill:constraint=CONSTRAINT_MEMCG` line (which corresponds to exit code 137, `SIGKILL`) and posts a PR comment naming the killed job/process/evidence. It correctly did not fire here, because this wasn't an OOM event.

Tracing every reap path in `runner_scale.rs`'s reconcile loop (`run_scale`) found the code already structurally protects busy runners: the scale-down reap (`if total_keep > desired`) and the idle-timeout reap both operate exclusively on `idle_runners`, built by classifying `managed_busy_map(&rows)`'s `Some(false)` entries — a runner GitHub's `/actions/runners` API reports as `busy: true` is never added to that set. The phantom-registration prune (§3 of `run_scale`) is separately guarded to only target registrations with *no* backing container at all. None of the three reap paths, as written, should remove a container GitHub currently reports as busy.

That leaves two real hypotheses, not a single confirmed root cause:

1. **GitHub API staleness** — the `busy` field on `/actions/runners` is known to lag briefly behind a runner actually starting a job (the runner's own agent process has to report "running" back to GitHub before the flag flips). A reconciler tick landing in that window would misclassify a truly-busy runner as idle and reap it.
2. **External termination** — WSL2 VM memory pressure, a Docker daemon hiccup, or another host-level event killing the container directly, independent of the reconciler's own decision-making.

This codebase already has a proven pattern for exactly the first class of problem: `zombies_for_force_cancel`'s doc comment documents a prior incident ("the PR #334 innocent-kill pattern") where a single offline+busy sample was insufficient — "a network blip flips a live runner offline" — and the fix was requiring the bad state to be observed in **two consecutive ticks** before acting. This design reuses that established pattern rather than inventing a new one.

## Approach

**Revision note (post adversarial review):** the first draft of this design had a critical wiring gap (a blocked scale-down candidate would have been dropped from the tracked-idle set entirely, regardless of whether it was actually reaped, permanently defeating scale-to-zero for exactly the runners the hardening was meant to protect) and an unreachable, causally-backwards "near-miss" diagnostic. Both are corrected below; see the two `**Revision:**` notes.

### 1. Reap hardening (addresses hypothesis 1, and reduces exposure to hypothesis 2)

Two independent, complementary guards added to the idle-classification/reap path in `run_scale`:

**a. Corroborating busy-check.** Before either reap path (`scale-down`, `idle-timeout`) removes a container, cross-check the runner's name against a fresh jobs-API lookup — reusing `oom_watch.rs`'s `fetch_recent_job_rows`/`find_matching_job` shape. If the runner's name is currently assigned to an `in_progress` job per this independent signal, the tick's "idle" classification (from the `runners` API's `busy` flag) is treated as stale: skip the reap for this runner this tick, and log it (see §3) rather than silently self-correcting with no trace. This **narrows** the staleness window (from "however long GitHub's `busy` flag lags" down to "the wall-clock gap between this tick's `runners`-API and `jobs`-API snapshots") — it does not close it outright, and the design does not claim otherwise.

**Revision — a blocked reap must not be lost.** Skipping a reap is not enough on its own: the candidate must still flow into the tracked-idle state that the *next* tick's decisions (both the 2-tick check below, and the separate idle-timeout path) depend on. A candidate that's blocked this tick and then silently dropped from that state looks freshly-idle again next tick, gets blocked again, forever — see Task 3 of the implementation plan for the exact wiring fix (a blocked scale-down candidate falls back into the same tracked-idle set the idle-timeout path consumes, rather than being removed unconditionally).

**b. Two-consecutive-tick requirement.** The scale-down reap path currently reaps on a single tick's snapshot with no history check at all — unlike the idle-timeout path, which already has a real multi-minute grace via `should_reap_idle`'s `DEFAULT_IDLE_REAP_SECS = 300`. Require a runner to have also been idle-tracked as of the *prior* tick's persisted state before it's scale-down-eligible — reusing the existing `idle_since`/`read_state()` mechanism (no new state file for this part), and matching `zombies_for_force_cancel`'s existing 2-consecutive-tick rationale in this same file ("a single ... sample is insufficient").

Cost: one shared `gh api` jobs-fetch per apply-tick (see the single-shared-fetch note in §2 below — this is not "one fetch per candidate," and not "one fetch per detector" either); (b) is pure state-diffing, no extra IO. Neither guard touches the scale-up/spawn path — only reap decisions are affected. Ephemeral runners that genuinely finish their one job and self-deregister/exit are unaffected either way (they're never in the reap-candidate set to begin with).

### 2. New detector: unexpected mid-job runner death

A new module, `crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs`, structured like `oom_watch.rs` (same seen-list persistence pattern under `~/.vox/`, same PR-comment posting mechanism) but for a different signal.

**Single shared fetch per tick.** `run_scale` fetches `fetch_recent_job_rows()` **once**, near the top of the `apply` tick, and passes the same `Vec<JobRow>` to all three consumers that need it this tick: the OOM scan, this detector, and §1's reap-hardening (reused later in the same tick, not re-fetched). `oom_watch::scan_and_report_oom_events` changes from fetching its own copy internally to accepting the rows as a parameter. This closes the redundant-fetch risk a naive per-consumer-fetch design would have (up to 3 independent multi-call `gh api` fans-out per tick, worst case coinciding exactly during an incident, when rate-limit headroom matters most) down to one fetch, and gives the whole tick one consistent snapshot of job state instead of two or three that could disagree with each other.

**Detection.** Each tick, after the existing exited-container enumeration (`run_scale` step 1) but *before* those containers are pruned, diff this tick's newly-exited managed containers against a persisted "was seen running" set (new state file, `~/.vox/ci-runner-running-seen.json`, written each tick with the current `managed_containers("running")` set — mirrors `phantom_seen`'s persistence style). Any container present in last tick's running-seen set that's now exited is a transition to investigate. For each:

- Capture `docker inspect`'s `.State.ExitCode` **before** this tick's own cleanup (`docker rm -f`) removes it — ordering matters, so this detector's scan must run before step 1's cleanup loop, not after.
- Correlate the container name against the tick's shared job-rows fetch (above) via `find_matching_job`. If that runner's assigned job is still `in_progress` on GitHub (not completed, not cancelled), this is an unexpected-exit event.
- **Skip anything `oom_watch` already claimed this tick** — check the same container name against `oom_watch`'s fresh event list (passed in from `run_scale`, which already calls both scanners in the same tick) so a genuine OOM death is only ever reported once, under the OOM framing, not also under this generic one.

**Reporting.** Same PR-comment mechanism as `oom_watch::post_pr_comment` (reused directly, not reimplemented), phrased to be honest about uncertain cause:

> **CI runner exited unexpectedly** — job `{job_name}` (run `{run_id}`) did not complete or get cancelled normally: its runner container exited (code `{exit_code}`) while the job was still `in_progress`. This was not a memcg OOM-kill (see the separate OOM-visibility check). Two known causes: (1) GitHub's runners-API `busy` flag briefly lagging a runner that just started a job, or (2) an external cause (WSL2 VM memory pressure, a Docker daemon hiccup) unrelated to the autoscaler's own decisions. This detector cannot distinguish between them from a single event; if this recurs, check whether it correlates with autoscaler reap activity around the same timestamp.
>
> Auto-detected by the host-side runner autoscaler (`vox ci runner-scale`).

**Revision — no same-tick "near-miss" correlation.** An earlier draft of this design proposed correlating a same-tick blocked-reap event (from §1a's hardening) into this comment as evidence pointing toward hypothesis 1. Adversarial review found this unreachable in practice given the real tick ordering (the exited-container scan runs *before* reap decisions are made in the same tick, so no near-miss evidence can exist yet when the comment is composed) **and** backwards even when reachable: a *blocked* reap by construction did not execute, so it cannot be why a container died — if anything, a blocked reap in the same tick is evidence *against* the autoscaler's own reap logic being the cause, not for it. Rather than build a diagnostic that would either never fire or fire with the wrong conclusion, this design states both hypotheses neutrally in every report and leaves cross-referencing autoscaler activity to a human, at least for this first cut.

### 3. Rich console output (both detectors)

Both `oom_watch::scan_and_report_oom_events` and the new detector currently emit (or would emit) only a bare count to stdout (`"runner-scale: reported {n} OOM-killed job(s) this tick"`). Change both call sites in `run_scale` to also print full per-event detail to stdout on every tick where anything fires — job name, run id, container name, exit code, and cause — not just the count. This is a pure logging change (no new IO, the data is already being fetched to build the PR comment body) that makes the same information visible to:

- Anyone watching `vox ci runner-scale --apply` run interactively.
- The Windows scheduled task's own captured output, if `VoxCIRunnerScale`'s action is configured to log to a file (out of scope for this spec to configure — noted as a natural follow-up, not required for this design to be complete, since the console output itself is the deliverable; whether Task Scheduler captures it to a durable log is an operational choice the user can make independently once the output exists).

### 4. `ScaleLock` heartbeat must cover the new IO

`run_scale`'s existing single-instance lock (`ScaleLock`) is refreshed at specific points during a tick so a slow tick doesn't let a concurrent invocation see it as stale (`LOCK_STALE_SECS = 90`) and steal it — exactly the kind of double-execution this design's own reap-hardening is trying to reduce exposure to. The shared job-rows fetch this design adds (§2) is itself a multi-call `gh api` fan-out that takes real wall-clock time; the lock's refresh calls must be extended to cover it (and any other new IO this design adds), not just the pre-existing OOM-visibility step. See the implementation plan for the exact refresh call sites.

## What this does not include

- No change to the spawn/scale-up path, warm pool, or phantom-registration pruning logic — only the two reap paths gain the corroborating check and 2-tick requirement.
- No Windows-native toast/balloon notification or Event Log integration — explicitly scoped out during brainstorming in favor of extending the existing, proven PR-comment mechanism plus console output.
- No attempt to definitively prove which of the two root-cause hypotheses (API staleness vs. external termination) explains any given future incident — see the revised §2, which states both neutrally rather than claiming a same-tick diagnosis.
- No change to `oom_watch.rs`'s own OOM-detection logic (the `dmesg` parsing, event correlation, dedup) — only its job-rows-fetching (now a parameter instead of an internal call, §2) and console-output (§3) call sites change; the detection logic itself is reused as-is.
- No retroactive investigation/backfill of the specific PR #460 incident's true cause — by the time this design is implemented, the evidence (dmesg, docker events window, container inspect data) will likely have rotated out. This design is forward-looking: the next occurrence will have real, captured evidence.

## Testing

- **Reap hardening (§1a, §1b)**: unit tests for the new corroborating-check logic (pure function taking a runner name + job rows, returning whether a reap should be blocked) and the 2-tick eligibility check (pure function taking a runner name + the prior tick's persisted idle state). **Additionally, an integration-style test of the scale-down block's combined decision logic** (extracted into a testable helper rather than left inline in `run_scale`) covering the specific regression adversarial review found in this design's first draft: a candidate that's blocked this tick must still be present in the set the idle-timeout path consumes afterward, not silently dropped — a test asserting this end-to-end (given a blocked candidate, its `idle_since` ends up in `new_state` after a simulated tick) is required, not optional, precisely because the bug that motivated it was a wiring/integration bug, not a pure-function bug, and this file's pure-function tests alone would not have caught it.
- **New detector (§2)**: mirror `oom_watch.rs`'s existing test structure directly — pure-function tests for the running→exited transition diff, the OOM-event-already-claimed skip, and `full_pipeline`-style tests parsing/correlating/composing a comment body end to end, matching that file's `full_pipeline_parses_correlates_and_composes_a_correct_comment` test as the template.
- **Console output (§3)**: since this is a formatting change to existing `println!` call sites, verify by inspection/a snapshot-style assertion on the composed string (matching whatever minimal testing convention, if any, the existing bare-count `println!` already has — likely none, in which case this stays untested code per this file's own established convention of testing the *pure logic* functions and leaving `println!`/`eprintln!` call sites themselves untested).
- **Manual verification**: once implemented, run a real `--apply` tick against a live (or realistically staged) fleet and confirm the console output and PR-comment path both fire with accurate, readable content — this repo's established practice (per `oom_watch.rs`'s own test fixtures being "verbatim shape of real ... output captured during the 2026-07-07 investigation") of grounding tests in real captured evidence rather than synthetic-only fixtures.
