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

### 1. Reap hardening (addresses hypothesis 1, and reduces exposure to hypothesis 2)

Two independent, complementary guards added to the idle-classification/reap path in `run_scale`:

**a. Corroborating busy-check.** Before either reap path (`scale-down`, `idle-timeout`) removes a container, cross-check the runner's name against a fresh jobs-API lookup — reusing `oom_watch.rs`'s `fetch_recent_job_rows`/`find_matching_job` shape (fetched once per tick, shared across every candidate, matching that module's existing efficiency pattern rather than one `gh api` call per candidate). If the runner's name is currently assigned to an `in_progress` job per this independent signal, the tick's "idle" classification (from the `runners` API's `busy` flag) is treated as stale: skip the reap for this runner this tick, and emit a near-miss event (see §2) rather than silently self-correcting with no trace.

**b. Two-consecutive-tick requirement.** Extend the idle-since tracking (`next_idle_since`/`should_reap_idle`) so a runner must be classified idle in the busy-map on **two consecutive ticks** — not one — before it becomes eligible for either reap path. This mirrors `zombies_for_force_cancel`'s existing pattern verbatim (same 2-tick intersection requirement, same rationale: a single bad sample is insufficient evidence).

Both guards are cheap: (a) is one extra `gh api` call per tick, only when there's a reap candidate; (b) is pure state-diffing, no extra IO. Neither touches the scale-up/spawn path — only reap decisions are affected. Ephemeral runners that genuinely finish their one job and self-deregister/exit are unaffected either way (they're never in the reap-candidate set to begin with — the exited-container cleanup in step 1 of `run_scale` already removes them by a completely separate path that isn't touched by this change).

### 2. New detector: unexpected mid-job runner death

A new module, `crates/vox-cli/src/commands/ci/unexpected_exit_watch.rs`, structured like `oom_watch.rs` (same seen-list persistence pattern under `~/.vox/`, same PR-comment posting mechanism) but for a different signal.

**Detection.** Each tick, after the existing exited-container enumeration (`run_scale` step 1) but *before* those containers are pruned, diff this tick's newly-exited managed containers against a persisted "was seen running" set (new state file, `~/.vox/ci-runner-running-seen.json`, written each tick with the current `managed_containers("running")` set — mirrors `phantom_seen`'s persistence style). Any container present in last tick's running-seen set that's now exited is a transition to investigate. For each:

- Capture `docker inspect`'s `.State.ExitCode` **before** this tick's own cleanup (`docker rm -f`) removes it — ordering matters, so this detector's scan must run before step 1's cleanup loop, not after.
- Correlate the container name against the same jobs-API fetch §1a already performs this tick (shared, not re-fetched) via `find_matching_job`. If that runner's assigned job is still `in_progress` on GitHub (not completed, not cancelled), this is an unexpected-exit event.
- **Skip anything `oom_watch` already claimed this tick** — check the same container name against `oom_watch`'s fresh event list (passed in from `run_scale`, which already calls both scanners in the same tick) so a genuine OOM death is only ever reported once, under the OOM framing, not also under this generic one.

**Reporting.** Same PR-comment mechanism as `oom_watch::post_pr_comment` (reused directly, not reimplemented), phrased to be honest about uncertain cause rather than overclaiming a diagnosis this detector can't make on its own:

> **CI runner exited unexpectedly** — job `{job_name}` (run `{run_id}`) did not complete or get cancelled normally: its runner container exited (code `{exit_code}`) while the job was still `in_progress`. This was not a memcg OOM-kill (see the separate OOM-visibility check). {If §1a's corroborating check blocked a related near-miss this tick: "The autoscaler's reap-hardening caught a related stale-busy-flag reap attempt this tick, which may explain this — investigate GitHub API busy-flag lag around this timestamp." Else: "No related near-miss was caught this tick — likely an external cause (WSL2/Docker), not the autoscaler's own reap logic."}
>
> Auto-detected by the host-side runner autoscaler (`vox ci runner-scale`).

The conditional second paragraph is the key diagnostic value: it tells a human reading the PR comment which of the two hypotheses from Context is more likely *for this specific incident*, using real per-tick evidence, rather than making them re-derive it from `docker ps`/`gh api` by hand the way this session's investigation had to.

### 3. Rich console output (both detectors)

Both `oom_watch::scan_and_report_oom_events` and the new detector currently emit (or would emit) only a bare count to stdout (`"runner-scale: reported {n} OOM-killed job(s) this tick"`). Change both call sites in `run_scale` to also print full per-event detail to stdout on every tick where anything fires — job name, run id, container name, exit code, and cause — not just the count. This is a pure logging change (no new IO, the data is already being fetched to build the PR comment body) that makes the same information visible to:

- Anyone watching `vox ci runner-scale --apply` run interactively.
- The Windows scheduled task's own captured output, if `VoxCIRunnerScale`'s action is configured to log to a file (out of scope for this spec to configure — noted as a natural follow-up, not required for this design to be complete, since the console output itself is the deliverable; whether Task Scheduler captures it to a durable log is an operational choice the user can make independently once the output exists).

## What this does not include

- No change to the spawn/scale-up path, warm pool, or phantom-registration pruning logic — only the two reap paths gain the corroborating check and 2-tick requirement.
- No Windows-native toast/balloon notification or Event Log integration — explicitly scoped out during brainstorming in favor of extending the existing, proven PR-comment mechanism plus console output.
- No attempt to definitively prove which of the two root-cause hypotheses (API staleness vs. external termination) explains *every* future incident — the design surfaces per-incident evidence (via the near-miss correlation in §2) so each occurrence can be individually diagnosed, rather than claiming to solve the ambiguity once and for all with this one change.
- No change to `oom_watch.rs`'s own detection logic — only its console-output call site in `run_scale` changes (§3); the module itself is reused as-is via the shared job-rows fetch and PR-comment helper.
- No retroactive investigation/backfill of the specific PR #460 incident's true cause — by the time this design is implemented, the evidence (dmesg, docker events window, container inspect data) will likely have rotated out. This design is forward-looking: the next occurrence will have real, captured evidence.

## Testing

- **Reap hardening (§1a, §1b)**: unit tests for the new corroborating-check logic (pure function taking a runner name + job rows, returning whether a reap should be blocked) and the 2-tick state transition (extending the existing `next_idle_since`/`should_reap_idle` test coverage with a "busy on tick 1, idle-classified but corroborated-busy on tick 2 → not reaped" case, and a "idle-classified on two consecutive ticks with no corroborating busy job → reaped" case, mirroring `zombies_for_force_cancel`'s own existing test shape).
- **New detector (§2)**: mirror `oom_watch.rs`'s existing test structure directly — pure-function tests for the running→exited transition diff, the OOM-event-already-claimed skip, and `full_pipeline`-style tests parsing/correlating/composing a comment body end to end, matching that file's `full_pipeline_parses_correlates_and_composes_a_correct_comment` test as the template.
- **Console output (§3)**: since this is a formatting change to existing `println!` call sites, verify by inspection/a snapshot-style assertion on the composed string (matching whatever minimal testing convention, if any, the existing bare-count `println!` already has — likely none, in which case this stays untested code per this file's own established convention of testing the *pure logic* functions and leaving `println!`/`eprintln!` call sites themselves untested).
- **Manual verification**: once implemented, deliberately reproduce a near-miss (e.g., temporarily shrink the busy-check corroboration window or simulate a stale busy=false read in a controlled dry-run) to confirm the console output and PR-comment path both fire with accurate, readable content — this repo's established practice (per `oom_watch.rs`'s own test fixtures being "verbatim shape of real ... output captured during the 2026-07-07 investigation") of grounding tests in real captured evidence rather than synthetic-only fixtures.
