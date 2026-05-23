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
take 30+ min on a 40GB target/, so we don't run them as a routine measurement):
- A clean baseline run is captured at the start of each phase if the phase's
  acceptance criterion would otherwise be unverifiable.
- Otherwise, incremental measurements are sufficient to detect 30%+ wins.

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
