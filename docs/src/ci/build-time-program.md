---
title: Build-Time Reduction Program
description: How Vox build times are measured (vox ci build-bench), the committed scenario set + baseline, the cycle/coupling guards (vox ci dep-cycles), and how each optimization phase reports its delta.
category: "CI & Quality"
---

# Build-Time Reduction Program

## Instruments

- **`vox ci build-bench`** — runs `contracts/ci/build-bench-scenarios.v1.json`, writes a snapshot, and `--compare`s against `contracts/ci/build-bench-baseline.v1.json` to emit a per-scenario delta. Cumulative report: `graphify-out/build-bench/REPORT.md`.
- **`vox ci dep-cycles`** — Tarjan SCC over `cargo metadata`. HARD-fails on normal-dependency cycles; inventories dev-dep back-edges to `graphify-out/DEP_CYCLES.md`. (Fills the gap that `vox-arch-check` does only pairwise layer-ordering with no cycle detection.)
- **`scripts/crate-build-audit.vox`** — the dependency/blast-radius map (fan-in, LoC, self-time) → `graphify-out/crate_audit.json` + `CRATE_BUILD_AUDIT.md`.

## Refreshing the baseline

After an intentional, accepted change to build cost, regenerate:

```bash
cargo check --workspace   # warm the cache first
vox ci build-bench --label baseline --write contracts/ci/build-bench-baseline.v1.json --repeat 3
git add contracts/ci/build-bench-baseline.v1.json
git commit -m "chore(ci): refresh build-bench baseline"
```

Adding or removing a scenario means editing `build-bench-scenarios.v1.json` and regenerating the baseline in the same PR.

## Selective CI + soundness backstop

PR-time selective CI (PR #348) builds only affected crates on PRs; the merge-queue gate and nightly run the full `--workspace`. No build-time optimization here weakens that backstop.

## Historical log

See [`docs/src/architecture/build-time-log.md`](../architecture/build-time-log.md) for the measured phase-by-phase deltas.
