---
title: "Distribution SSOT"
description: "The single source of truth for install tiers, dependency closures, the crates.io publish set, and released binaries."
category: "Architecture SSOTs"
status: "current"
---

# Distribution SSOT

`contracts/distribution/profiles.v1.yaml` is the single source of truth for how
Vox is installed, released, and (eventually) published.

## What it governs

- **Tiers** — `minimal` / `default` / `full`, each with its binary set, build
  deps, and runtime-optional deps. `voxup install <tier>` and `vox doctor` read this.
- **Publish set** — the crates.io publish list (leaf-first), reconciled against
  `crates/_public.toml`. `publish.enabled: false` keeps the public flip deferred.
- **Binaries** — what release + nightly build (`vox`, `vox-ml-cli`, `voxup`).
  Nightly tagging/pruning and local nightly install are documented in
  [nightly-builds-ssot.md](nightly-builds-ssot.md), not here.
- **agy containment** — `agy` is runtime-optional in the `full` tier only;
  `vox-orchestrator-mcp` is non-publishable. Enforced by the parity test.

## Enforcement

`crates/voxup/tests/distribution_parity.rs` (CI: `distribution-parity.yml`)
fails when the manifest drifts from: the toolchain contract, `_public.toml`,
the agy-containment rule, or the on-disk crate dirs.

## Consumers (Tracks A–D)

- Track A — `voxup install <tier>` + `vox doctor` per-tier dep enable.
- Track B — release + nightly read `binaries`.
- Track C — publish automation reads `publish`.
- Track D — supply-chain trust.
