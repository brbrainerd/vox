---
title: "Build-Time Baseline (2026-05-08)"
description: "Phase 0 build-time measurements for the 2026-05-08 workspace reorg."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Build-Time Baseline (2026-05-08)

Phase 0 baseline for the workspace reorg. See
[2026-05-08-workspace-reorg-design.md](./2026-05-08-workspace-reorg-design.md).

## Measurement methodology

For incremental scenarios:
1. Touch the target file: `touch <path>`
2. Time the check: `time cargo check -p <crate> --quiet`
3. The reported `real` time is the recorded baseline.

For cold scenarios (captured opportunistically — full `cargo clean` runs
take 30+ min, so we don't run them as a routine measurement):
- A clean baseline run is captured at the start of each phase if the phase's
  acceptance criterion would otherwise be unverifiable.
- Otherwise, incremental measurements are sufficient to detect 30%+ wins.
- Use a fresh `CARGO_TARGET_DIR` for a cold A/B rather than `cargo clean`, and
  hold `.cargo/config.toml` constant across both halves.

**Check free disk first.** Each git worktree gets its own `target/` (per
`.cargo/config.toml`) — ~89 GB across 4 worktrees on the 2026-08-23 host, vs.
the 40 GB single-target assumption this doc was written against. When the disk
fills, cargo fails with `os error 112` and leaves truncated `.rmeta` files whose
downstream errors look like code bugs. Diagnostic signature and cleanup in
[build-time-log.md](./build-time-log.md) §2026-08-23 §0.

For per-crate compile time inside any build, append `--timings` (Cargo writes
`target/cargo-timings/cargo-timings-*.html`).

## Recorded baselines

| Scenario | Command | Real time |
|---|---|---|
| L0 leaf check (cached) | `cargo check -p vox-orchestrator-types` | 0.36s |
| L0 leaf check (cached) | `cargo check -p vox-db-types` | 0.63s |
| Orchestrator incremental (touch lib.rs) | `cargo check -p vox-orchestrator` | **5.59s** |
| Orchestrator incremental (touch mcp_tools/dispatch.rs) | `cargo check -p vox-orchestrator` | **5.06s** |
| CLI incremental (touch vox-cli/src/lib.rs) | `cargo check -p vox-cli` | **26.76s** |

Cached L0 leaf checks bottoming out near 0.5s confirm those crates are already
near-floor — wins on them will be marginal. The real targets are the 5–27s
incremental rebuilds.

## Targets (Phase 9 acceptance)

| Scenario | Today | Target |
|---|---|---|
| Orchestrator incremental after touching `mcp_tools/` file | 5.06s | ≤ 1.5s (only `vox-orchestrator-mcp` rebuilds) |
| Orchestrator incremental after touching coordinator code | 5.59s | ≤ 3s (slimmed coordinator) |
| CLI incremental | 26.76s | ≤ 10s (vox-cli-thin path) |
| L0 leaf clean (will be measured per phase) | TBD | ≤ 5s (post-hack-split, only vox-hack-core) |

## Build-time log

Each phase appends a row to [build-time-log.md](./build-time-log.md) with
post-phase measurements, comparing against this baseline.

## Scope note (2026-08-23)

The figures above are **incremental** rebuilds and are not superseded — nothing
since has re-measured them. They do not, however, cover the cold-scoped-build
cost that dominates fresh worktrees and CI lanes. That axis is measured
separately in [build-time-log.md](./build-time-log.md) §2026-08-23, where a
dependency-weight pass took cold `cargo check -p vox-telemetry` from 590 to 409
compile units (−31 %). Do not carry the wall-clock figures from that section
across as baselines: they were measured on a host with concurrent agent builds,
and `vox-telemetry` is the maximum-benefit crate in the workspace (`vox-cli`
moved −7 % on the same change). When adding a new baseline row, say which axis
it measures.
