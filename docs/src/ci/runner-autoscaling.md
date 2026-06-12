---
title: "Self-Hosted CI Runner Autoscaling"
description: "Ephemeral, demand-scaled self-hosted CI runner pool: how it works, how to roll it out, and how to recover when runners are down. Replaces the two always-on vox-runner containers."
category: "CI & Quality"
status: "current"
last_updated: "2026-06-06"
training_eligible: true
training_rationale: "Documents the runner autoscaler design + rollout so the single-box CI fleet can be operated and recovered reliably."
---

# Self-Hosted CI Runner Autoscaling

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
- **Bounded:** up to `VOX_RUNNER_MAX` (default 6), each `--cpus=6
  --memory=6500m` → at most 36 of 32 threads when fully saturated (autoscaler
  caps at host capacity); tune `VOX_RUNNER_MAX` to leave headroom for Windows.
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
| `VOX_RUNNER_MAX` | `6` | Hard ceiling on concurrent managed runners |
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

1. **Build the image** from the now-reproducible source:
   ```bash
   docker build -t vox-ci-runner-local:latest -f infra/ci-runner/Dockerfile .
   ```
2. **Apply the WSL ceiling** (the `.wslconfig` bump to 24 cpu / 32 GB):
   ```powershell
   wsl --shutdown    # kills all containers; Docker Desktop restarts them
   ```
3. **Retire the always-on pair** (the autoscaler replaces them):
   ```bash
   docker rm -f vox-runner-1 vox-runner-2   # they deregister on stop
   docker volume create vox-ci-runner-cache # shared sccache (first time only)
   ```
4. **Schedule the reconcile tick** every ~1 min so the pool tracks demand —
   Windows Task Scheduler running `vox run scripts/ci-runners-up.vox`, or a loop.
5. Verify: `vox ci runner-scale` (dry-run) shows the plan; after a push,
   `docker ps` shows `vox-runner-auto-*` appear and disappear; idle → 0.

## Recovery (fleet down right now)

- Quick check: `vox ci runner-preflight`.
- Daemon up, just need runners: `vox run scripts/ci-runners-up.vox`.
- Legacy always-on containers merely stopped: `docker start vox-runner-1 vox-runner-2`.
- Docker Desktop/WSL itself down (the usual cause of multi-hour outages — restart
  policy can't help when the daemon is gone): start Docker Desktop, then the above.

## Cross-refs
- Runner contract: [`runner-contract.md`](runner-contract.md).
- Hosted fallback: `.github/workflows/ci-fallback-hosted.yml` (**manual only** via
  `workflow_dispatch`; not a required check and does not run on PR/push).
