---
title: "Self-Hosted CI Runner Autoscaling"
description: "Ephemeral, demand-scaled self-hosted CI runner pool: how it works, how to roll it out, and how to recover when runners are down. Replaces the two always-on vox-runner containers."
category: "CI & Quality"
status: "current"
training_eligible: true
training_rationale: "Documents the runner autoscaler design + rollout so the single-box CI fleet can be operated and recovered reliably."
---

# Self-Hosted CI Runner Autoscaling

Queue clearing, the agent-facing queue signal, and the async failure signal
are documented in [local-first-ci](local-first-ci.md).

## Why

The merge gate (required check `Check, Build, and Test (Rust)`, plus most of
`ci.yml`) runs on `[self-hosted, linux, x64]` — historically **two always-on**
Docker containers (`vox-runner-1/2`) on a single Windows i9-14900KS (32 threads,
64 GB) via Docker Desktop/WSL2. That setup had three failure modes:

1. **Slow tests.** WSL2 was capped at **8 processors** (`.wslconfig`), so the
   whole fleet shared 8 cores while `cargo` was configured for `jobs=24` →
   oversubscription.
2. **OOM outages.** WSL2 was capped at **16 GB**; two *unbounded* runners running
   heavy builds exceeded it → `exit 137` (SIGKILL) → fleet down, gate stalls,
   *all* merges blocked (the hosted fallback `ci-fallback-hosted.yml` is not a
   required context, so it can't satisfy branch protection).
3. **Wasted "idle" CPU.** With `cancel-in-progress: true` on most workflows and
   many agents pushing, jobs ran 15–30 min then got canceled and re-queued — the
   runners ground a never-draining backlog 24/7 (this is the "700% CPU when
   nothing's running": real, but wasted, work).

## The model: ephemeral, scale 0 ↔ N

- `vox ci runner-scale` reconciles a pool of **ephemeral** runners to demand:
  when CI work is queued it spawns one-shot `--ephemeral` containers (each runs a
  **single** job, then self-deregisters and exits); when the queue empties, the
  pool drains to **0 runners = 0 CPU/RAM**. Containers are spawned with **no
  restart policy**, so a Docker/host restart never resurrects a stale fleet that
  storms the queue at boot.
- **Demand = queued jobs**, not workflow runs: each tick enumerates queued and
  in-progress runs and counts jobs with `status: queued` whose label set the
  pool can serve (`self-hosted,linux,x64,docker,browser`). One PR fanning out a
  dozen jobs registers a dozen demand, capped at the pool max.
- **Bounded:** up to `VOX_RUNNER_MAX` (default 2), each `--cpus=4
  --memory=14000m` → at most 8 vCPU / 28 GB when fully saturated (fits the
  WSL2 24-cpu / 32-GB ceiling); tune `VOX_RUNNER_MAX` to leave headroom for
  Windows. The per-runner memory budget was raised from an earlier `5000m` to
  `14000m` (and the runner count correspondingly dropped from 6) after a real
  build (`cargo doc --workspace --exclude vox-gui --no-deps`) was measured
  peaking at ~12GB RSS — the old budget was silently causing runner
  containers to be killed by their own memory cgroup limit mid-build. See
  `docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md`.
- **OOM-visible:** if a runner container is ever hard-killed by its own
  memory cgroup limit again (e.g. a future job's build exceeds the current
  14GB budget), `vox ci runner-scale`'s host-side autoscaler tick detects it
  via `dmesg` on the next `--apply` run, correlates it to the PR/job that was
  executing on that runner, and posts a comment directly on the affected PR
  with the evidence — no manual `dmesg` archaeology needed.
- **Warm:** every runner mounts a shared `vox-ci-runner-cache` volume with
  `sccache` (`SCCACHE_DIR=/cache/sccache`), so ephemeral cold starts reuse
  compiler output instead of rebuilding the world.
- **Never force-kills** a running job; running runners exit on their own after
  their single job (`spawn_count = max(0, desired - current)`). The only reap is
  a short **idle grace window** (`VOX_RUNNER_IDLE_REAP_SECS`, default 300 s) for
  runners that registered but never got a job (e.g. the queued run was
  cancelled). Stale **offline** GitHub registrations with no backing container
  (crashed ephemeral runners) are pruned each tick.
- **Optional warm pool:** `VOX_RUNNER_WARM_POOL` (default 1) keeps N idle
  runners registered for instant dispatch; set to `0` for pure scale-to-zero.
- **Registry cache:** ephemeral runners symlink `~/.cargo/registry`, `~/.cargo/git`,
  and `~/.cargo/advisory-db` into the shared `vox-ci-runner-cache` volume
  (`/cache/cargo-registry`, etc.) so cold starts skip re-downloading crates and
  the cargo-deny advisory DB.

### Components (this repo)
| Piece | Path |
|---|---|
| Reconcile + preflight logic | `crates/vox-cli/src/commands/ci/runner_scale.rs` |
| Reproducible runner image | `infra/ci-runner/Dockerfile` + `infra/ci-runner/entrypoint.sh` |
| Schedulable bring-up glue | `scripts/ci-runners-up.vox` |
| Host CPU/RAM ceiling | `~/.wslconfig` (`processors=24`, `memory=32GB`) |

### Commands
```bash
vox ci runner-scale            # DRY-RUN: print the reconcile plan, mutate nothing
vox ci runner-scale --apply    # spawn/reap to match demand
vox ci runner-preflight        # exit non-zero if NO self-hosted runner is online
vox run scripts/ci-runners-up.vox   # one schedulable reconcile tick (apply)
```

`runner-scale` is **dry-run by default** so it can never silently spawn a
runaway pool. Demand = count of `queued` **jobs** matching the pool's labels
(`gh api` runs + per-run jobs).

### Knobs (env, optional — registered in `contracts/config/env-vars.v1.yaml`)

| Env var | Default | Meaning |
|---|---|---|
| `VOX_RUNNER_MAX` | `2` | Hard ceiling on concurrent managed runners (2 × 4 cpu / 14000m = 8 vCPU / 28 GB, fits WSL2 24/32) |
| `VOX_RUNNER_IDLE_REAP_SECS` | `300` | Grace window before reaping a never-assigned runner |
| `VOX_RUNNER_WARM_POOL` | `1` | Idle runners to keep registered for instant dispatch |

## Fail-fast (surfacing "runners are down")

Before relying on the gate, run `vox ci runner-preflight`. If the fleet is down
it **errors immediately** instead of letting work queue forever, and points at
`scripts/ci-runners-up.vox`. Wire it into agent/merge workflows so a runner
outage surfaces in seconds, not after a 20-minute stall.

## Rollout (one-time, do in a quiet window)

These steps are disruptive (they restart WSL / replace the fleet), so run them
when no critical CI is mid-flight.

1. **Build the image** from the now-reproducible source. On this Windows host run
   it **inside WSL2** — `docker build` hangs over the `docker-wsl` SSH context (see
   the note at the end of this page):
   ```bash
   wsl -d Ubuntu -- bash -c "cd /mnt/c/Users/Owner/vox && \
     docker build -t vox-ci-runner-local:latest -f infra/ci-runner/Dockerfile ."
   ```
   The build context is ~1.2G; `.dockerignore` must keep excluding `.worktrees/`
   (~85G) and `mens/runs`+`mens/data` (~23G), or this balloons. Do **not** exclude
   all of `mens/` — `mens/config/templates.yaml` is `include_str!`'d by
   `vox-corpus` and the root `Dockerfile`'s build needs it.
2. **Apply the WSL ceiling** (the `.wslconfig` bump to 24 cpu / 32 GB):
   ```powershell
   wsl --shutdown    # kills all containers; systemd restarts docker.service on next `wsl`
   ```
3. **Retire the always-on pair** (the autoscaler replaces them):
   ```bash
   docker rm -f vox-runner-1 vox-runner-2   # they deregister on stop
   docker volume create vox-ci-runner-cache # shared sccache (first time only)
   ```
4. **Schedule the reconcile tick** every ~2 min so the pool tracks demand.
   The task definition is versioned at `scripts/ci/voxcirunnerscale.task.xml`
   (Windows Task Scheduler XML, `MultipleInstancesPolicy=IgnoreNew` so a slow
   tick that overruns 2 min is dropped rather than queued). Install or
   re-install it with:
   ```bash
   vox run scripts/ci/install-runner-schedule.vox
   ```
   Verify the registration: `schtasks /Query /TN VoxCIRunnerScale /FO LIST`
5. Verify: `vox ci runner-scale` (dry-run) shows the plan; after a push,
   `docker ps` shows `vox-runner-auto-*` appear and disappear; idle → 0.

## Recovery (fleet down right now)

- Quick check: `vox ci runner-preflight`.
- Daemon up, just need runners: `vox run scripts/ci-runners-up.vox`.
- Legacy always-on containers merely stopped: `docker start vox-runner-1 vox-runner-2`.
- WSL2 Docker Engine itself down (the usual cause of multi-hour outages — restart
  policy can't help when the daemon is gone): start it with
  `wsl -d Ubuntu -u root -- service docker start` (Docker Desktop is **not** used
  on this host — see the note below), then the above.

> **Docker on this host = WSL2-native Docker Engine, not Docker Desktop.** Docker
> Desktop's Windows service is permanently wedged on this machine (an
> un-deletable Unix-socket-emulation reparse point survives reboot + factory
> reset), so Docker Engine (`docker-ce`) is installed **inside** the WSL2 Ubuntu
> distro (systemd-managed `docker.service`). Ignore any older instruction to
> "start Docker Desktop" or enable its "WSL Integration" toggle — neither applies.
>
> **Windows tooling needs no special invocation — except `docker build`.** The
> Windows `docker` CLI reaches the WSL2 daemon through the active `docker-wsl`
> **SSH context** (`docker context show` → `docker-wsl`), so `docker info` / `run` /
> `ps` / `rm` — and every `Command::new("docker")` callsite in this repo, including
> the autoscaler — just work. The context authenticates with a dedicated key
> (`~/.ssh/id_ed25519_docker_wsl`) whose `authorized_keys` entry is `restrict`ed to
> `command="docker system dial-stdio"`. There is deliberately **no TCP daemon
> socket** (no `2375`): the Docker API is root-equivalent and unauthenticated over
> TCP, so it is not exposed.
>
> **⚠️ `docker build` hangs over the SSH context.** BuildKit cannot negotiate its
> session over `docker system dial-stdio`; the CLI blocks indefinitely at ~0% CPU
> with no output (reproduced with a one-line `FROM hello-world` context, so it is
> not a context-size problem). **Build inside WSL2 instead** — see the build step
> below. Only `build` is affected.
>
> Caveats: the context and key are **per-user** (`%USERPROFILE%`), so a scheduled
> task or service running as another account (e.g. `SYSTEM`) will not see them and
> must be run as the owning user. Direct `wsl.exe docker <args>` remains a working
> fallback — from Git Bash set `MSYS_NO_PATHCONV=1` first, or POSIX paths get
> mangled.

## Adding a new nightly (or other scheduled) workflow

A new `schedule:`-triggered workflow shares the SAME demand pool as every
other nightly job and all PR/merge-queue CI — `runner_scale.rs`'s
`query_queued_job_demand` sums queued+in-progress jobs across the whole repo
with no per-workflow reservation, capped at `VOX_RUNNER_MAX` (currently 2;
see the capacity math above — do not raise this without also lowering
`MEM_PER_RUNNER` in `runner_scale.rs`, or you will overcommit the WSL2 memory
ceiling; a 2026-07-27 incident found the cap had drifted to 4 without that
check, risking a 56GB request against a 32GB budget).

Before adding one:
- Give the job an explicit `timeout-minutes` — GitHub's 360-min default lets
  a hang squat a scarce runner slot for hours, starving the other nightlies
  (two of the three existing nightlies were missing this until 2026-07-27).
- Pick a cron time — existing nightlies run at 03:17 and 06:20 UTC, with the
  06:00 hour additionally carrying `gitleaks` (05:00), `cr-l-gates`,
  `ci-fallback-hosted`, and `link_checker` (all 06:00); `mutation-nightly.yml`
  was moved from 05:17 to 13:00 UTC in 2026-09 after measuring 24/29 runs
  cancelled by contention in that cluster. Avoid clustering more jobs into
  the 02:00–06:00 UTC window, since they'd all compete for the same 2-runner
  pool.
- A GPU-requiring job (labels including `gpu`, like `qwen35-native-nightly.yml`)
  draws from a wholly separate capacity pool — see `runner-contract.md` — so
  its demand doesn't compete with `linux`/`docker`-labeled jobs, but does
  compete with any other `gpu`-labeled job.
- No reusable workflow template exists yet — each nightly is a hand-rolled
  YAML file; copy an existing one (`bench-nightly.yml` is a reasonable
  starting point) rather than starting from scratch.
- **Evaluated and declined (2026-07-27):** extracting a shared `workflow_call`
  reusable workflow or composite action for the common
  checkout/toolchain/cache/timeout/`runs-on` prologue. Read all three
  nightlies (`mutation-nightly.yml`, `bench-nightly.yml`,
  `qwen35-native-nightly.yml`) in full first. The truly shared block is only
  ~10-12 lines (checkout + `dtolnay/rust-toolchain@stable` + optional
  `actions/cache@v5`), and even that isn't uniform: cache keys differ per job
  (`cargo-mutants` vs `cargo-bench`), the Qwen3.5 job has no cache step at
  all, and the toolchain step there adds `components: rustfmt, clippy` that
  the others don't. Meanwhile each job's unique body dominates the file —
  mutation testing's `taiki-e/install-action` + `cargo mutants` invocation,
  bench's two `cargo bench` runs plus a bespoke bencher-output-parsing
  threshold gate plus two artifact uploads, and the Qwen job's four distinct
  build/corpus-prep/train/upload steps. A composite action would save at most
  ~10 lines per file while adding a fourth file to keep in sync, an extra
  level of indirection when reading "what does this job actually do," and a
  place for the per-job cache-key/component differences to get silently
  flattened. Net: boilerplate reduction is marginal and the abstraction cost
  is real, so keep the three nightlies hand-rolled. Revisit only if a fourth
  near-identical nightly is added (rule-of-three) or if the shared prologue
  itself needs a coordinated fix (e.g. bumping `actions/cache` across all
  three) more than once.
- `ci-health-watchdog.yml`'s nightly-health check auto-discovers any workflow
  file with a `schedule:` trigger, so a new nightly job is covered by the
  failure-alerting from its first scheduled run — no watchdog edit needed.

## Cross-refs
- Runner contract: [`runner-contract.md`](runner-contract.md).
- Hosted fallback: `.github/workflows/ci-fallback-hosted.yml` (**manual only** via
  `workflow_dispatch`; not a required check and does not run on PR/push).

## Required-check policy (2026-06-15)

`Cross-Platform (Win/macOS/Ubuntu)` (workflow `cross-platform-check.yml`) is a
**weekly** scheduled + `workflow_dispatch` check. It is intentionally **not** a
required status check for merging: it has no PR/merge_group trigger, so requiring
it permanently blocked the merge queue. Keep it scheduled; do not re-add it to
branch protection or the merge-queue ruleset's required checks. To enforce it
per-batch instead, add a `merge_group:` trigger to the workflow first, then
re-require it — never require it while it is schedule-only.

## CI Health Watchdog invariants

The watchdog (`.github/workflows/ci-health-watchdog.yml`) and the autoscaler guard
each other. These invariants prevent a repeat of the 2026-06-29 silent cascade — do
not relax them without understanding why they exist:

- **`VoxCIRunnerScale` `ExecutionTimeLimit` must stay `< the repetition interval`**
  (currently `PT2M` limit vs `PT2M` interval — keep the limit ≤ the interval). With
  `MultipleInstancesPolicy=IgnoreNew`, a tick that outlives the interval silently
  drops every later tick → no reaping → runaway zombies. This is what hung for 4h.
- **A reconcile tick must never block on a build.** It only queries GitHub and
  `docker run`/`rm` a pre-built image (seconds). If a tick triggers `cargo`/image
  builds inline (observed during the incident), a cold build can exceed the limit and
  get killed — fix the build path, do not raise the limit.
- **The scheduler `ExecutionTimeLimit` frees the task *slot*, not necessarily the
  child tree.** Authoritative worker-kill is the in-process `VOX_RUNNER_TICK_TIMEOUT_SECS`
  timeout that kills the child process group.
- **Reap only `vox-runner-auto-` runners past the grace window.** The watchdog's
  managed-prefix gate mirrors the Rust reaper's `MANAGED_PREFIX`; never deregister an
  arbitrary offline runner (a real or rebooting runner would be destroyed).
- **Failover uses the `fleet-down` PR label, not a `--ref main` dispatch.** `main` is
  merge-queue-gated and `ci.yml` has no `push:main`; a main-ref status does not clear a
  queue entry's required context. Recovery (`online>0`) removes the label.
- **The watchdog needs an authenticated push endpoint and a dead-man's-switch
  heartbeat.** A public ntfy topic leaks CI internals; a silently-disabled or
  PAT-expired watchdog is the original blind spot one level up. The runner-admin PAT
  (`SSOT_AUTOREGEN_TOKEN`) has an expiry — record and rotate it; an expired PAT 401s
  the watchdog into silence.
- **PAT expiry date:** _record here when set_ (fine-grained, this repo only,
  Administration r/w + Actions w).
