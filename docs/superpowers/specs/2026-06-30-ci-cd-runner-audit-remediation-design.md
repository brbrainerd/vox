# CI/CD Runner Audit Remediation — Design (v2, adversarially verified)

**Date:** 2026-06-30
**Status:** Approved design, pending implementation plan
**Scope:** Phase 0 (measure) + Phase A (confirmed quick wins) + Phase B
(structural) now; Phase C (elastic retreat) designed but gated on measurement.

> **v2 note.** Every claim in v1 was adversarially re-verified against the code
> by a 16-agent refute-first workflow. The central v1 thesis — "the queue stalls
> from bugs B1/B2/B4" — was **wrong**: B1 is a non-bug, B2/B4 are real config
> facts that do not cause the stall. The real levers are one confirmed config
> bug (W1), **structural oversubscription on `merge_group`** (a gap v1 missed),
> a confirmed cancellation gap, and **missing queue-wait observability** (also
> missed). Five v1 items are rejected as false positives or hazards. See
> "Rejected" below — each with its disproof.

## Problem

The merge gate runs on a single Windows box's WSL2 Docker self-hosted runners
(`vox-runner-auto-*`), a **hard ceiling of 6 concurrent runners**
(`runner_scale.rs:60` `DEFAULT_MAX_RUNNERS=6`, pinned to the WSL2 .wslconfig
caps by `fleet_budget_fits_wsl2_ceiling` at `runner_scale.rs:1183`), warm pool 1
(`:68`). Each runner is ephemeral and takes exactly one job (`:6-8,:395`).

Three symptoms were reported: (1) the queue never drains, (2) green merges take
too long, (4) CI has accumulated bloat. The audit + adversarial verification
found:

- The **stall** is not caused by code bugs. The autoscaler being **disabled**
  (an ops state) plus **structural oversubscription** explains it. On
  `merge_group`, `full=true` is forced (`ci.yml:151-159`) and the gate fans out
  **~36 self-hosted jobs** — 12 singletons (`setup`, `guards-fast`, `lints`,
  `compiler-gates`, `tests`, `audits`, + 6 smokes) **plus `all-features-matrix`'s
  24 matrix legs** (`ci.yml:1591-1610`, each a self-hosted job) — *plus* three
  workflows the first count missed that also fire self-hosted on `merge_group`:
  `cross-platform-check.yml` (Linux leg `:38`, different concurrency group so
  neither cancels the other), `gui-cross-build.yml` (`matrix-setup`, ungated),
  `mobile-eas-build.yml` (`bundle`), `mutation-pr.yml` (`cargo-mutants`). Against
  a **6-runner ceiling** that is ~**6 waves**, not 2 — the oversubscription is
  worse than v1 stated. (Note: the `browser`-labelled jobs draw from a distinct
  label subset, so ceiling math must bucket by required label set, not a flat
  count.)
- The **wall-clock** is dominated by one confirmed config bug (W1, sccache
  disabled on the gate) compounded by that 2× oversubscription (each wave is
  ~6 jobs, each recompiling the workspace).
- The **bloat** is mostly illusory: a routine no-label PR fires only ~2
  self-hosted workflows on the critical path; several proposed cuts would have
  *lost* coverage.

The reframe that drives this design: **measure the real ceiling first** (queue
wait is currently unmeasured), fix the one confirmed config bug and the
structural oversubscription, and only then decide whether to add paid hosted
minutes or a second host. We do not add capacity to paper over a misconfiguration.

## Verified findings (with disproofs)

### Confirmed (keep, sharpened)

- **W1 (HIGH, one line) — sccache disabled on the gate.** `ci.yml:25-29` sets
  `RUSTC_WRAPPER: sccache` + `SCCACHE_DIR` but **not** `CARGO_INCREMENTAL: "0"`.
  Verified absent at every effective level *for the gate's workflow-env / non-
  container path*: workflow env, all 6 gate jobs, `.cargo/config.toml`
  `[build]`/`[env]`, and all runner Dockerfiles. (The autoscaler *does* inject
  `CARGO_INCREMENTAL=0` into spawned containers at `runner_scale.rs:432`, but
  only when MinIO is reachable — so the 0%-hit applies to the workflow-env path
  and to any tick where the S3 cache is down.) The lone
  repo occurrence (`ci.yml:320`) is on the non-required `toolchain-lint-wave`
  job and is paired with `RUSTC_WRAPPER: ""` + `unset RUSTC_WRAPPER` to
  *defeat* sccache — not a gate mitigation. Sibling workflows set it
  workflow-level (`cross-platform-check.yml:45`, `ci-fallback-hosted.yml:55`).
  Incremental-on means sccache caches nothing → the documented 0%-hit.
- **W2 (confirmed) — cache hit rate invisible.** No `sccache --show-stats` /
  `SCCACHE_LOG` anywhere in `ci.yml` (present only in three sibling workflows).
- **CANCEL_GAP (confirmed) — zombie pins a slot forever.** No `gh run cancel`
  anywhere in the repo. The watchdog reap DELETEs offline managed runners
  (`ci-health-watchdog.yml:82`) but a busy runner returns 422 and the step only
  *defers* (`:87`) — nothing cancels the pinned run. An offline-but-busy zombie
  holds its queue slot across every 10-min tick until a human applies
  `fleet-down` (which only relabels PRs to hosted; it does not cancel the run).
- **B3 (confirmed) — `CostCircuitBreaker` is dead code.** Zero non-test callers
  tree-wide (`cost_defense.rs:146/165`, re-exported `lib.rs:11-12`); the 5
  dependent crates import only `ScalingPolicy`/path constants. A 2026-06-15 plan
  independently records "the cost-defense layer has no live consumer." The
  documented daily-budget kill switch protects nothing. (Relevant to Phase C.)
- **W6 (confirmed, scope corrected) — apt stack reinstalled per job.** The
  GTK/webkit/dbus/soup stack (`libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev`) is `apt-get install`ed
  identically in **5 jobs** (`ci.yml:105,626,774,876,1217` — `setup` + the 4
  heavy Rust jobs, *not* "every stage-2 job") and is **not** baked into
  `infra/ci-runner/Dockerfile` (which bakes only `libglib2.0-dev` + `pkg-config`
  of that set).

### Rescoped (real, but not what v1 said)

- **B2 (partial) — autoscaler hard-kill is not the stall.** `ExecutionTimeLimit=
  PT2M` + `AllowHardTerminate=true` + `IgnoreNew` are all real
  (`voxcirunnerscale.task.xml:54,40,37`) and the worst-case demand probe really
  is ~42 sequential `gh` calls (`runner_scale.rs:273-299`). **But:** `IgnoreNew`
  *drops* a late tick (does not "block the next tick"); the apply path
  early-exits at `total >= max` (`:293`) so a real backlog short-circuits the
  42-call worst case; and a hard-killed tick is fully recovered next tick by the
  90s stale-steal `ScaleLock` (`:592`) + RAII cleanup + idempotent reconcile.
  → Optional hardening, **not** a stall cause.
- **B4 (partial) — undercount hits telemetry, not spawning.** `per_page=20` with
  no `--paginate` (`:77,:279`) and `.unwrap_or_default()` on per-run errors
  (`:291`) are real. **But** the spawn path calls `query_queued_job_demand(max)`
  (`:768`), early-exits at `total >= max`, and `desired_runner_count` clamps via
  `.min(max)` (`:119`) — so spawning saturates at max regardless. The undercount
  only bites the status-table telemetry path (`:995`, called with `u32::MAX`).
  → An **observability** fix, not a provisioning fix.
- **W3 (partial) — recompiles are warm, and sharing target/ is unsafe.** Only the
  `vox` binary is shared as an artifact (not `target/`), and the ~5 heavy jobs
  each re-invoke cargo. **But** they are *not* "5× cold": a shared sccache volume
  (`SCCACHE_DIR=/cache/sccache`) + host-persisted `target/` on the same
  persistent fleet make them warm (the file self-documents this at `ci.yml:286`;
  `toolchain-lint-wave` must explicitly `cargo clean` + unset sccache to force a
  cold build). **Do NOT introduce a shared `CARGO_TARGET_DIR`** (forward-looking
  — no code sets one today; `shared_cache_env` at `runner_scale.rs:418-434` sets
  `SCCACHE_*` + `CARGO_INCREMENTAL=0` but never `CARGO_TARGET_DIR`, and the
  `/cache` volume is used only for the concurrency-safe sccache dir). If anyone
  later points cargo's target dir at the single named volume
  `vox-ci-runner-cache` (`runner_scale.rs:39,:520`), concurrent cargo writers
  corrupt fingerprints (one advisory lock per dir). → rely on warm sccache (W1).
- **W4 (partial) — llvm-cov feeds a hard gate, not just a ratchet.** `tests` runs
  `cargo llvm-cov nextest --workspace` on the required `merge_group` path
  (`ci.yml:996`, in `ci-summary.needs`). It feeds **both** a blocking
  `ci coverage-gates --mode enforce` (`:1172`, no `continue-on-error`) **and** a
  separate advisory ratchet (`:1180`, `continue-on-error:true`). Moving it off
  the gate to cut the double compile is possible **but loses an enforced
  coverage gate** — it is not free.

### Rejected (false positives / hazards — do NOT do)

- **B1 — non-bug.** `set -e` does **not** abort on a false `[ ]` on the left of
  `&&` (disabled for all but the final command of a `&&` list), and the three
  predicate lines are followed by the `>> $GITHUB_OUTPUT` block, so they are not
  even the step's last command. Verified empirically (exit 0, end reached). The
  watchdog does **not** abort on a healthy fleet. Converting to `if`-blocks is an
  optional style nicety, not a fix. **Drop from scope.**
- **Remove inline all-features (was v1 W5 cut) — loses coverage.** The inline
  `cargo check --workspace --all-features` (`ci.yml:1276`) is the **only required
  pre-merge** all-features gate. `all-features-matrix` (`:1580`) is non-required,
  post-merge/full-ci-only, and per-crate (`-p`) — different feature resolution,
  neither subsumes the other. **Keep the inline check.**
- **Cut `compile-matrix.yml` — loses coverage.** It is the **only** workflow
  running `vox compile` end-to-end (native-binary + desktop) over
  `examples/compile-suite` (`:46-53`); `ci.yml` and `cross-platform-check.yml`
  only `cargo check`. **Keep** (its Win/macOS legs are already removed).
- **Merge `cr-l-gates` + `cr-l8-corpus-feedback` — different surfaces.** The
  former is the strict v1.0 audit umbrella (`vox audit --gate all`); the latter
  is the telemetry diagnostic→repair→corpus e2e + the artifact producer the CR-L8
  gate consumes, plus stdlib-coverage + scripts-check. **Keep separate.**
- **Merge `ci-health-watchdog` + `ci-health-deadman` — defeats the design.** The
  deadman is a deliberately *independent* hosted cron that watches the watchdog
  for silent death (`ci-health-deadman.yml:1-3`). Merging reintroduces the
  single-point-of-failure blind spot. (There are **three** ci-health files; the
  third is a PR fixture test.) **Keep separate.**
- **Blind `PT2M→PT10M`** without a lock heartbeat, and **parallelizing the `gh`
  fan-out** before fixing the error-swallow — both are hazards (see Risks).

## Design

### Phase 0 — Measure first (closes two missed observability gaps)

Nothing is "fixed" until it is measured. This phase is also the **gate for Phase
C**.

1. **Queue-wait metric (TDD-first).** Add `created_at: Option<String>` to
   `JobRow` (`job_timings.rs:29`) and a pure `queue_wait_seconds(created_at,
   started_at)` beside `run_seconds` (`:56`). Emit a queue-wait column/annotation
   and mirror in `ci-timings.yml` (`(.started_at - .created_at)`). This is the
   metric that reflects fleet starvation — today it is computed nowhere (only the
   local cargo-shim measures queue wait).
   *TDD:* unit test `queue_wait_seconds` first (pure fn).
2. **Exclude cancelled runs from timings** (`job_timings.rs:101`, `ci-timings.yml`
   jq) so concurrency-cancelled `merge_group` runs stop polluting the dataset.
   *TDD:* unit test the filter over a row set including `conclusion=="cancelled"`.
3. **Cache observability:** `sccache --show-stats` after the build in `setup`
   (W2); log `s3_cache_reachable()` (`runner_scale.rs:522`) into the scale-event
   JSON (`:654`) so cold-cache periods are visible in `ci-runner-history.jsonl`.
4. **Capture baseline:** merge-gate wall-clock, per-job queue wait, sccache hit
   rate, and the merge_group self-hosted job count vs the 6-runner ceiling.

### Phase A — Confirmed quick wins (config/bug, small diffs)

5. **W1 (one line):** add `CARGO_INCREMENTAL: "0"` to `ci.yml:25` env.
   *Check:* `sccache --show-stats` shows non-zero hit rate on the second run.
   *TDD:* add a `vox ci` guard asserting any workflow with `RUSTC_WRAPPER:
   sccache` also sets `CARGO_INCREMENTAL: "0"` — unit-testable (feed it `ci.yml`,
   assert fail pre-fix / pass post-fix), and prevents regression on all siblings.
6. **Re-enable the autoscaler** (the actual stall cause, per the disabled ops
   state) — but only after B2 hardening (step 7) so it does not thrash.
7. **B2 hardening (not a stall fix):** cap + bound the per-run `gh` fan-out in
   `query_queued_job_demand`. Raise `ExecutionTimeLimit` **only together with** a
   lock heartbeat (see Risks) — otherwise leave PT2M.
8. **B4 (observability):** add pagination + propagate (don't swallow) per-run
   `gh` errors, scoped to the status-table backlog readout. Must land **with or
   before** any fan-out parallelization, never after.
   *TDD:* refactor the accumulate+cap loop into a pure fn taking an iterator of
   per-run label lines; unit-test the cap and the error path. `count_matching_
   queued_jobs` (`:165`) and `desired_runner_count` (`:119`) are already pure +
   tested — extend, don't duplicate.
9. **Safe bloat only:** move `gitleaks.yml` + `link_checker.yml` to
   `ubuntu-latest` (neither has a self-hosted dependency — gitleaks curls its
   binary; lychee's local exclude-paths become harmless no-ops on a clean clone);
   drop `push:main` triggers on `os-compat-report.yml`, `mobile-e2e-ios.yml`,
   `link_checker.yml` (each keeps schedule + dispatch). **No other cuts/merges.**

### Phase B — Structural (the real wall-clock + capacity win)

10. **Tier the non-required smokes off the `merge_group` critical path.** On
    `merge_group`, these self-hosted jobs fire but are **not** `ci-summary.needs`,
    so they consume runners and serialize the gate without gating the merge:
    `visualizer-ingest-smoke`, `web-vite-build-smoke`, `vox-vscode-extension`,
    `docker-vox-image-smoke`, `vox-browser-cdp-smoke`, `gui-playwright-smoke`,
    `all-features-matrix` (`ci.yml:1331,1351,1377,1413,1429,1469,1584`). Move them
    to a post-merge / scheduled lane (or a separate concurrency budget). This is
    THE capacity fix: it shrinks the merge_group self-hosted fan-out from ~12
    toward the ~5 required needs, fitting the 6-runner ceiling in one wave.
    *Check (against Phase 0 baseline):* merge_group self-hosted job count ≤ 6.
11. **Move `cross-platform-check.yml`'s self-hosted Linux leg to `ubuntu-latest`**
    (the Win/macOS legs already are) so it stops competing for the 6-runner pool
    at merge time.
12. **W6:** bake the GTK/webkit/dbus/soup stack into `infra/ci-runner/Dockerfile`;
    delete the 5 per-job `apt-get` steps.
    *TDD:* arch-check `forbidden_pattern` asserting `apt-get install` of
    libgtk/libwebkit/libsoup does not appear in `ci.yml` (fails-first on the
    current 5 occurrences).
13. **CANCEL_GAP — zombie force-cancel, conservatively.** Add a bounded escalation
    to the watchdog: when a `vox-runner-auto-*` runner is offline-but-busy across
    **≥2 consecutive ticks** past a grace window ≥ GitHub's offline-detection
    latency + the longest expected step, `gh api -X POST
    .../actions/runs/$run_id/cancel` the pinned run, then DELETE the runner. Keep
    the 422-defer as the inner guard (a still-cancellable run is never
    force-killed inside the window); log the decision for audit. This avoids the
    PR #334 innocent-kill pattern.
    *Check:* fixture — offline-busy across 2 ticks → cancel; in-window 422 →
    deferred.
14. **Runner-image cache guards:** add `sccache --version && sccache --show-stats`
    smoke to the Dockerfile and an entrypoint assertion that `command -v sccache`
    resolves inside `/usr/local/cargo/bin` (guards the "fake sccache shim"
    gotcha); fail-loud if `/cache` is read-only or the S3 endpoint is configured
    but unreachable.

### Phase C — Elastic retreat (DESIGNED, NOT BUILT)

Documented only. **Trigger to build:** after A+B, if the Phase 0 baseline still
shows queue-wait or merge wall-clock above an agreed threshold (numbers set at
the C gate, once real queue-wait data exists). Then, in priority order:

- (a) promote the hosted fallback from `fleet-down` break-glass to a real
  backlog-overflow path (dispatch locally, retreat to hosted on backlog);
- (b) wire the `CostCircuitBreaker` (`cost_defense.rs`) into demand-based scaling
  as the spend guard before any paid-minute overflow.
  *TDD:* failing integration test in `vox-integration-tests` asserting the
  scale-up path consults `check_before_task` and refuses on a hard block — fails
  today (no producer), passes once wired (the recurring producer/consumer-split
  lesson);
- (c) second self-hosted host → multi-host dispatch.

No code until the gate fires.

## Machine enforcement (AI-first)

Each fix should leave behind a **machine-checkable, AI-greppable guard** so the
invariant cannot silently regress — not a one-time human grep. Reuse existing
surfaces; do not invent harnesses.

- **W1 sccache⇒incremental:** the cross-line rule ("a workflow that sets
  `RUSTC_WRAPPER: sccache` at top-level env must pin `CARGO_INCREMENTAL: "0"`
  there") exceeds a single-line regex, so keep it a **pure `vox ci` guard**
  iterating *all* `.github/workflows/*.yml` (not just `ci.yml`), scoped to each
  file's top-level env block (split before `\njobs:` so the deliberate
  step-level `RUSTC_WRAPPER: ""` opt-out in `toolchain-lint-wave` is neither
  required nor flagged), wired into `guards-fast` (pre-merge, on
  `ci-summary.needs`).
- **W6 no-apt-in-ci:** an exact fit for `vox-arch-check`'s declarative
  `[[forbidden_pattern]]` (`docs/src/architecture/layers.toml`, engine
  `forbidden_patterns.rs:scan_all`; `.github` is not in the prune set). Add a
  rule `pattern = 'apt-get install[^\n]*(libgtk-3-dev|libwebkit2gtk|libsoup-3)'`,
  `file_glob = ".github/workflows/*.yml"` — lands in the same commit as the
  Dockerfile bake (the gate is `error`, so it must go green together).
- **sccache-shim-real:** beyond the build-time/entrypoint check, add a
  `sccache.shadowed_shim` **`vox doctor` diagnosis** (`build_health.rs`,
  registered in `KNOWN_DIAGNOSIS_IDS`) so the "wrapper set but resolves to a
  fake `.cmd` forwarder" pathology is greppable via a `[diag id=…]` tag on any
  host, reusing the existing `sccache_guard` plumbing.
- **merge_group fan-out ≤ ceiling (the biggest passive-YAML gap):** add a pure
  `vox ci` guard `merge_group_self_hosted_fanout(workflow_yaml) -> usize` that
  evaluates each self-hosted job's `if:` against a synthetic
  `event_name=merge_group` context, **buckets by required label set**, and fails
  if any bucket exceeds the runner ceiling. Read the ceiling from a **shared
  SSOT const** (today only `runner_scale.rs:60 DEFAULT_MAX_RUNNERS`; surface it
  so the guard and the autoscaler cannot drift). This is what stops the next
  self-hosted job from silently re-breaching the ceiling — the exact regression
  class this whole effort exists to prevent. (Check whether `fan_in_budget.rs`
  already owns this concept before adding a module.)

## Risks (from adversarial review)

- **PT2M→PT10M without a heartbeat → runaway spawn.** `LOCK_STALE_SECS=90`
  (`runner_scale.rs:592`) is written once at acquire and never refreshed
  (no heartbeat code exists). A legitimately long tick under a 10-min budget
  looks stale to any concurrent invocation (on-demand start, manual re-trigger),
  defeating single-instance and double-running the spawn loop. **Mitigation:**
  add a periodic lock-timestamp heartbeat and raise `LOCK_STALE_SECS` above the
  new budget *before* raising the budget; or keep PT2M.
- **Parallelizing the `gh` fan-out → secondary rate limits + worse B4.** Bursty
  concurrent `gh` calls trip GitHub's abuse limits, and the `.unwrap_or_default()`
  swallow (`:291`) turns a 403 into "zero demand" exactly under load. **Mitigation:**
  fix the error-propagation (step 8) first; cap concurrency 2–4 with jitter.
- **Concurrent `target/` sharing → cache corruption.** See W3. Do not share one
  `CARGO_TARGET_DIR`; warm sccache (W1) delivers the win safely.
- **Zombie force-cancel → innocent kill (PR #334).** Single-sample offline+busy
  is insufficient (a network blip flips a live runner offline). Require the
  multi-sample grace window in step 13.

## Out of scope (YAGNI)

- **B5 (fallback double-post)** beyond a note — only bites during a `fleet-down`
  outage, which Phase C's overflow path supersedes.
- **B7 (cancel-in-progress keyed on ref)** — verified correct, not a bug.
- **Rewriting the autoscaler** — fix the targeted issues; do not redesign.

## Success criteria

- **Phase 0:** queue-wait time measured and surfaced; cancelled runs excluded
  from the timing dataset; sccache hit rate + S3-reachability observable.
- **Phase A:** `sccache --show-stats` shows a non-zero hit rate on the gate (W1);
  autoscaler re-enabled and not thrashing; B4 telemetry reports true backlog.
- **Phase B:** the **required-needs lane** (`setup` + the 5 `ci-summary.needs`
  jobs = 6 self-hosted) fits one wave; the non-required smokes (incl. the 24-leg
  `all-features-matrix`) no longer fire on `merge_group`. (A flat "all merge_group
  self-hosted jobs ≤ 6" is *not* the criterion — `all-features-matrix` alone is
  24 legs post-merge, and `gui-cross-build`/`mobile-eas-build`/`mutation-pr` are
  out of scope here.) Merge-gate wall-clock measurably below the Phase 0
  baseline; zombie jobs have a conservative terminal force-cancel path.
- **Phase C** remains design-only unless the post-A+B measurement gate fires.
- **No coverage lost:** inline all-features, `compile-matrix`, both `cr-l*`
  lanes, and all three `ci-health-*` workflows remain intact.
