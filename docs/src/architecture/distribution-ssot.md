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

- **Tiers** — `minimal` / `default` / `full`, each with its binary set, bundle
  cross-reference, build deps, and runtime-optional deps. `voxup install <tier>`
  and `vox doctor` read this.
- **Publish set** — the crates.io publish list (leaf-first), reconciled against
  `crates/_public.toml`. `publish.enabled: false` keeps the public flip deferred.
- **Binaries** — what release + nightly build (`vox`, `vox-langtool`,
  `vox-ml-cli`, `voxup`). Nightly tagging/pruning and local nightly install are
  documented in [nightly-builds-ssot.md](nightly-builds-ssot.md), not here.
- **agy containment** — `agy` is runtime-optional in the `full` tier only;
  `vox-orchestrator-mcp` is non-publishable. Enforced by the parity test.

## Tiers

| Tier      | Description                                                                             | Binaries                          | Bundle          |
|-----------|------------------------------------------------------------------------------------------|------------------------------------|-----------------|
| `minimal` | Language toolchain only: check, fmt, run, build. No orchestrator, no model catalog, no GPU. | `vox-langtool`                     | `vox-base`      |
| `default` | Agentic CLI plus desktop GUI (Axis).                                                     | `vox`                               | `vox-fullstack` |
| `full`    | Everything: CLI, GUI, ML, agy delegation, curated plugins.                                | `vox`, `vox-ml-cli`, `voxup`        | `vox-dev`       |

## Two orthogonal taxonomies (Ruling R4)

This repo has two taxonomies for "what do I get if I install Vox," and both are
kept — they answer different questions and are not substitutable:

- **Layer 1 — tiers** (`contracts/distribution/profiles.v1.yaml` `tiers`, this
  file's subject): audience -> package identity. "Who is this install for?"
- **Layer 2 — bundles** (`crates/vox-plugin-catalog/catalog.toml` `[[bundle]]`):
  capability -> plugin set. "Which plugins does this install carry?"

They are joined by exactly one cross-reference: each tier's `bundle:` key names
a bundle id defined in `catalog.toml`. `minimal` maps to `vox-base` ("bare host
binary, no plugins") because a language-only audience must not get a plugin
host; `default` maps to `vox-fullstack` ("default developer experience with all
built-in skill plugins"); `full` maps to `vox-dev` ("contributor / power-user
development environment"). `crates/vox-cli/tests/distribution_tier_bundle_xref.rs`
asserts every tier's `bundle` resolves via `vox_plugin_catalog::bundle_resolved`,
so the two files cannot drift apart silently. This decision is recorded in
`docs/superpowers/plans/2026-09-05-00-INDEX.md` §2.1 and is binding: neither
taxonomy is deleted, and no third bundle-resolution code path is introduced.

## Enforcement

`crates/voxup/tests/distribution_parity.rs` (CI: `distribution-parity.yml`)
fails when the manifest drifts from: the toolchain contract, `_public.toml`,
the agy-containment rule, or the on-disk crate dirs.

## Consumers (Tracks A–D)

- Track A — `voxup install <tier>` + `vox doctor` per-tier dep enable.
- Track B — release + nightly read `binaries`.
- Track C — publish automation reads `publish`.
- Track D — supply-chain trust.
