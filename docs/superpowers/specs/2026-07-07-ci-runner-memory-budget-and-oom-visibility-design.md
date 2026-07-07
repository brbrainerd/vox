# CI runner memory budget + OOM visibility — design

## Problem

Self-hosted CI runner containers (`vox-runner-auto-*`, spawned by `vox ci runner-scale`)
were being hard-killed mid-job, well before their job's own `timeout-minutes` fired.
GitHub Actions reports this identically to an external cancellation
(`conclusion: cancelled`, `##[error]The operation was canceled.`), so from the job log
alone it's indistinguishable from a real fleet event (crash, reap, etc.).

### Root cause (confirmed, not inferred)

Cross-referencing `docker events`, WSL2 kernel logs (`dmesg -T`), and the provisioning
source settled it:

- `docker events` showed the runner container dying with `exitCode=137` (SIGKILL) and
  being destroyed/recreated by the autoscaler within ~2 seconds.
- `dmesg -T` showed 7 `Memory cgroup out of memory` kill events over a 14-hour window,
  every one `constraint=CONSTRAINT_MEMCG` (a *per-container* cgroup limit, not host-wide
  pressure), always killing `cargo`, `rustc`, or `rustdoc`.
- One event — a `rustdoc` process killed at 07:53:04 EDT — lands 15 seconds before the
  PR's "Lints (clippy + rustdoc + ...)" job was marked `cancelled` at 11:53:19 UTC. Direct
  causal match.
- `crates/vox-cli/src/commands/ci/runner_scale.rs:42` sets `MEM_PER_RUNNER = "5000m"`.
  The comment at lines 57-59 explains the budget was derived by dividing total host RAM
  by max concurrency (`6 runners × 5000m = 30GB < 32GB WSL2 cap`) — never by measuring
  what a real build in this workspace actually needs.
- The host was never under real memory pressure: `free -h` inside WSL2 showed 22-28GB
  free even while a runner was actively being OOM-killed. The ceiling is just set too
  low for a single container's actual peak.
- No existing tooling anywhere in `vox-cli` records OOM/cgroup-kill evidence
  (`grep -rl "OOMKilled\|docker stats" crates/vox-cli/src` → zero hits). This gap is why
  root-causing this required manual `dmesg` archaeology instead of reading the job's own
  log.

### Measured peak (empirical, not guessed)

Ran the exact command the Lints job runs —
`cargo doc --workspace --exclude vox-gui --no-deps` (`RUSTDOCFLAGS='-D warnings'`,
`sccache`-warmed, same cache volume as the real fleet, 4 CPUs) — inside a throwaway
container built from the same `vox-ci-runner-local:latest` image, with a generous
memory ceiling instead of 5GB, sampling `docker stats` every 5 seconds. It completed
successfully in 9m21s with a measured peak of **~12.06GB** RSS — 2.4x the current
5GB-per-runner budget. This fully explains the repeated OOM-kills with room to spare.
(clippy's peak was not separately measured; it shares most of the same compile graph as
`cargo doc` and is expected to be in a similar range — the OOM-visibility logging in
Part B is the safety net if that assumption turns out wrong.)

## Design

### Part A: memory budget fix

File: `crates/vox-cli/src/commands/ci/runner_scale.rs`

- `MEM_PER_RUNNER`: `"5000m"` → `"14000m"` (≈2GB headroom over the measured 12.06GB
  peak).
- `DEFAULT_MAX_RUNNERS`: `6` → `2`. On this 31GB host, `2 × 14GB = 28GB`, leaving ~3GB
  for the WSL2 VM/Docker daemon itself. This is a real throughput reduction (CI jobs
  that used to fan out across up to 6 runners now queue behind 2) — an accepted
  trade-off: correct over fast. `CPUS_PER_RUNNER` (`"4"`) is unchanged; there is no
  evidence it contributes to this failure mode.
- The existing `fleet_budget_fits_wsl2_ceiling` test (line ~1266) already asserts
  `runners × mem ≤ 32GB` and will keep passing. Extend it with a floor assertion —
  `MEM_PER_RUNNER` (parsed as GB) `> 12` — tied to the measured peak, so a future
  edit can't silently shrink the budget back below the known-real requirement without
  the test failing.
- Update the stale comment at lines 57-59 to cite the measured peak as the basis for
  the new numbers, not just even division of host RAM.

### Part B: OOM visibility, posted to the affected run

Two implementation constraints ruled out the original in-job design and changed where
this has to live:

1. **`dmesg` is not readable from inside the runner container.** Verified directly:
   `docker exec <runner> dmesg -T` → `dmesg: read kernel buffer failed: Operation not
   permitted` (containers lack `CAP_SYSLOG` by default, correctly so — granting it would
   widen what every CI job running on that container can read from the host kernel, a
   real privilege increase not worth this one diagnostic feature). `/proc/self/cgroup`
   inside the container also shows the namespaced view (`0::/`), not the host-side
   `/docker/<id>` path `dmesg`'s `oom_memcg=` field uses — so even with read access, the
   container can't cheaply self-identify which OOM lines are its own.
2. **A job that gets OOM-killed cannot run its own `if: always()` report step.** The
   runner agent process (the thing that would execute that step and report status back
   to GitHub) dies along with the container. There is no "after" for that same job —
   its whole execution environment is destroyed. Evidence has to come from *outside*
   the job that failed.

Given both, detection and reporting move to the **host-side autoscaler**
(`vox ci runner-scale --apply`, already invoked every 2 minutes by the
`VoxCIRunnerScale` Task Scheduler entry — confirmed via `schtasks /query /tn
VoxCIRunnerScale /fo LIST /v`), which already runs with full host-level `dmesg` access
(confirmed directly: `wsl -e dmesg -T` from the host works fine).

On each 2-minute tick, in addition to its existing reconcile logic:

1. Scan `dmesg -T` for `Memory cgroup out of memory` lines newer than a persisted
   cursor (mirrors the existing `~/.vox/ci-runner-idle.json` state pattern — new file
   `~/.vox/ci-runner-oom-cursor.json` storing the last-seen kernel timestamp).
2. Resolve each new line's `oom_memcg=/docker/<id>` to a runner container **name**
   (== `RUNNER_NAME`, which `spawn_one` already sets as an env var on every container —
   see `runner_scale.rs:563`). Since the container may already be destroyed and
   replaced by the time the next tick runs, maintain a short-lived ID→name cache built
   from `docker events` history (the same event stream that showed `container destroy`
   / `container create` pairs during this investigation) rather than only the current
   `docker ps` snapshot.
3. Once the container name is known, find which job/run was executing on it: GitHub
   Actions job objects expose a `runner_name` field once a job is assigned to a runner.
   Query recent workflow runs' jobs (`gh api repos/{REPO_SLUG}/actions/runs?...` →
   `.jobs[]`) for a `runner_name` match around the kill's timestamp.
4. Post a PR comment (`gh pr comment <number> --body "..."`, matching this repo's
   existing pattern of using `gh api`/`gh pr edit` for out-of-band CI signaling in
   `ci-health-watchdog.yml`) on the PR associated with that run: which job died, the
   killed process name, and the raw `dmesg` line as evidence — so anyone looking at the
   failed run sees the real cause within about 2 minutes, without needing to run this
   same manual investigation by hand.
- A check-run annotation (attached directly to the specific failed check, rather than a
  general PR comment) would be a nicer landing spot but needs the check-run ID, which
  is more API calls to resolve reliably — noted as a possible v2 improvement, not
  required for this pass.

### Testing

- `fleet_budget_fits_wsl2_ceiling` extension is a real Rust unit test in
  `runner_scale.rs`, following the existing pattern in that file.
- The dmesg-scan-and-correlate logic is pure enough to unit test without a live
  Docker/GitHub environment: the dmesg-line parser (extracting timestamp, killed
  process, `oom_memcg=` id), the cursor persistence (don't re-report the same line
  twice across ticks), and the container-name resolution (id → name via the events
  cache) are each testable against fixture input strings, mirroring this file's
  existing unit-test style rather than needing an integration harness.
- The `gh api`/`gh pr comment` posting step is the one piece that needs a live-ish
  check: a dry-run mode (`--dry-run`, matching the existing flag already used elsewhere
  in this command) that prints the resolved PR/comment body instead of posting, so this
  is verifiable locally without spamming a real PR during development.
- This directly guards against the same class of bug this whole investigation fixed in
  the CI-timeout-diagnostics design earlier this session: a detector that never
  actually fires on the failure case it exists to catch.

### Non-goals

- Not touching `CPUS_PER_RUNNER` — no evidence CPU, as opposed to memory, contributed to
  this failure mode.
- Not capping build parallelism inside each container (e.g. `CARGO_BUILD_JOBS`) to fit
  more, smaller runners into the same host RAM — considered and explicitly rejected in
  favor of fewer, bigger runners (simpler, no new build-invocation tuning surface, and
  the measured peak is a property of compiling this workspace, not of how many runners
  happen to be scheduled concurrently).
- Not adding host RAM or otherwise changing the WSL2 memory cap — out of scope for a
  CI-config fix; the 31GB host ceiling is treated as a given constraint.
- Not separately measuring clippy's peak in this round — `cargo doc`'s measured 12.06GB
  already forced the concurrency answer (2 runners) regardless of whether clippy's own
  peak is somewhat higher or lower; Part B's OOM-visibility logging is the intended
  catch-all if clippy (or any other job) turns out to need more than the 14GB budget
  covers.
- Not granting runner containers `CAP_SYSLOG`/elevated privileges to read `dmesg`
  directly — considered and rejected in favor of host-side detection, which needs no
  container privilege change at all.
- Not implementing a check-run annotation in this pass — a PR comment is the v1 landing
  spot; annotating the specific failed check directly is a possible follow-up, not
  required to make the failure visible on the affected PR.
