---
title: Affected-Crate Selective CI
description: PR-time CI builds only the crates a change affects; the merge-queue gate and nightly run the full workspace for soundness.
category: "CI & Quality"
---

# Affected-Crate Selective CI

> **Status (2026-06-15):** the `vox ci affected-crates` subcommand, the reverse-dep
> closure logic (`vox-cli-ci::affected`), and the committed `crate-graph.v1.json` SSOT
> are landed. The `.github/workflows/ci.yml` wiring that consumes them (scoping the
> PR-lane `tests`/`lints`/`check` jobs to the affected set) is a **follow-up** — until
> it lands, every PR still runs the full workspace. This document describes the target
> behavior the wiring will enable; the tooling below is usable now.

## The rule
- **`pull_request` push:** build/test/clippy only changed crates + reverse-dep closure. Fast feedback.
- **`merge_group` + `push:main` + nightly:** full `--workspace`. Authoritative soundness gate.

## Sentinels (force full)
`Cargo.toml` (root), `Cargo.lock`, `.cargo/config.toml`, `rust-toolchain.toml`, `crates/workspace-hack/**`, `.config/hakari.toml`.

## The graph
`contracts/ci/crate-graph.v1.json` — regenerate with `vox ci affected-crates --regen`,
verify with `vox ci affected-crates --check` (hard-fails on drift vs `cargo metadata`).
Adding a crate requires regenerating. The drift `--check` is opt-in today; it becomes a
blocking gate when the ci.yml wiring (see Status above) lands.

## Intentional gaps
- PR-time runs (once wired) do NOT run doctests. Covered at merge gate + nightly.
- After wiring, new-crate PRs must regenerate the graph or the `--check` gate fails CI.
