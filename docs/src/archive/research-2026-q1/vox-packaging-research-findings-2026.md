---
title: "Vox packaging research findings 2026"
description: "Hard-cut research findings for Cargo-first Vox package management, command namespace unification, and Python/UV retirement."
category: "Architecture SSOTs"
last_updated: "2026-03-26"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

## Decision context

This revision applies the following product decisions as hard constraints:

- Python/UV is not retained as a Vox platform packaging/runtime lane.
- `vox install` is removed from package-management semantics (Phase B).
- Vox uses a hybrid package command model:
  - Top-level common dependency verbs (`add/remove/update/lock/sync`).
  - Advanced and governance operations under `vox pm ...`.
- `update` and `upgrade` cannot remain semantic synonyms.

## Why this document was rewritten

The prior draft captured useful benchmarking, but it underweighted three repo-critical areas:

- Package storage and repository lifecycle details (`.vox_modules`, local DB usage, CAS boundaries).
- Existing namespace policy conflict already documented in CLI design rules (`update` vs `upgrade`).
- Current state of Python retirement (some surfaces already retired, others still active in code/docs).

This rewrite corrects those gaps and converts findings into implementation-grade requirements.

## Method and evidence quality

- Repo audit focused on active code paths and command contracts:
  - [crates/vox-cli/src/lib.rs](../../../crates/vox-cli/src/lib.rs)
  - [crates/vox-cli/src/commands/lock.rs](../../../crates/vox-cli/src/commands/lock.rs)
  - [crates/vox-cli/src/commands/update.rs](../../../crates/vox-cli/src/commands/update.rs)
  - [crates/vox-cli/src/commands/add.rs](../../../crates/vox-cli/src/commands/add.rs)
  - [crates/vox-cli/src/commands/remove.rs](../../../crates/vox-cli/src/commands/remove.rs)
  - [crates/vox-cli/src/build_service.rs](../../../crates/vox-cli/src/build_service.rs)
  - [crates/vox-cli/src/commands/run.rs](../../../crates/vox-cli/src/commands/run.rs)
  - [crates/vox-pm/src/lib.rs](../../../crates/vox-pm/src/lib.rs)
  - [contracts/cli/command-registry.yaml](../../../contracts/cli/command-registry.yaml)
- External benchmark pass: 24 web searches (Cargo, registries, lockfile systems, supply-chain controls).
- Source weighting:
  - Tier A: canonical specs and official docs.
  - Tier B: project-maintainer docs.
  - Tier C: ecosystem analyses.

## Current-state architecture map

### Command surface and namespace

- **Phase B:** `vox install` is **not** a CLI subcommand; it does not appear in [crates/vox-cli/src/lib.rs](../../../crates/vox-cli/src/lib.rs) or [contracts/cli/command-registry.yaml](../../../contracts/cli/command-registry.yaml) (use **`vox add`** / **`vox lock`** / **`vox sync`** / **`vox pm`** — see [pm-migration-2026.md](../reference/pm-migration-2026.md)).
- **Historical (pre‑2026 wave):** `Install` had been a hidden migration-error variant; that shim is removed.
- `add/remove/update/lock/sync/pm` are first-class in [crates/vox-cli/src/commands/mod.rs](../../../crates/vox-cli/src/commands/mod.rs).
- CLI design rules already call out the anti-pattern of near-synonyms (`update` vs `upgrade`) in [docs/src/reference/cli.md](../reference/cli.md).

### PM core capabilities already present

`vox-pm` already provides foundational pieces:

- Manifest parsing (`Vox.toml`) in [crates/vox-pm/src/manifest.rs](../../../crates/vox-pm/src/manifest.rs).
- Lockfile model (`vox.lock`) in [crates/vox-pm/src/lockfile.rs](../../../crates/vox-pm/src/lockfile.rs).
- Registry client in [crates/vox-pm/src/registry.rs](../../../crates/vox-pm/src/registry.rs).
- Workspace model in [crates/vox-pm/src/workspace.rs](../../../crates/vox-pm/src/workspace.rs).
- Artifact cache in [crates/vox-pm/src/artifact_cache.rs](../../../crates/vox-pm/src/artifact_cache.rs).

Gap: the user-visible lifecycle is not coherently exposed through stable top-level commands.

### Package storage and repository blind spots

- Current `update` path uses `.vox_modules/local_store.db` through `vox_db::VoxDb` in [crates/vox-cli/src/commands/update.rs](../../../crates/vox-cli/src/commands/update.rs).
- Vendor trees: **`vox pm vendor`** (or copy `.vox_modules/dl` manually) after **`vox sync`**; the old unwired `commands/vendor.rs` helper was removed as duplicate.
- The relationship between:
  - manifest (`Vox.toml`),
  - lock (`vox.lock`),
  - local materialization (`.vox_modules`),
  - and cache/CAS (`artifact_cache`)
  is not enforced as one canonical contract yet.

### Cargo invocation architecture

- Cargo orchestration service exists in [crates/vox-cli/src/build_service.rs](../../../crates/vox-cli/src/build_service.rs).
- Direct cargo spawning still exists in [crates/vox-cli/src/commands/run.rs](../../../crates/vox-cli/src/commands/run.rs).
- This split undermines consistent policy enforcement (target-dir, telemetry, retries, lock handling).

### Python/UV retirement status (hard-cut baseline)

- `vox mens train-uv` is already retired by runtime bail in [crates/vox-cli/src/commands/mens/populi/dispatch.rs](../../../crates/vox-cli/src/commands/mens/populi/dispatch.rs) and marked `retired` in registry.
- But UV/Python code remains in active crate surfaces (for example [crates/vox-container/src/env.rs](../../../crates/vox-container/src/env.rs)).
- Docs still describe active Python integration (for example `how-to-pytorch`, `api/vox-py` pages listed by doc inventory).

Conclusion: retirement is policy-correct but code/docs are not fully converged.

## Critique of prior draft

### What the prior draft got right

- Correctly identified Cargo as the stable substrate.
- Correctly identified `vox install` as a stub and namespace confusion source.
- Correctly identified Docker reproducibility and provenance as strategic requirements.

### What it missed or under-specified

- Did not reflect user intent to hard-retire Python/UV.
- Did not specify a concrete hybrid command taxonomy with migration-level detail.
- Did not map `.vox_modules` and local store behavior into the PM lifecycle model.
- Did not handle `update` vs `upgrade` with explicit namespace ownership and policy.
- Treated UV patterns as adoption candidates instead of retirement impacts.

### Corrected stance

- Python/UV is a removal target, not a retained compatibility strategy.
- `vox install` is retired; top-level `add/remove/update/lock/sync` become the common package lane.
- `upgrade` is reserved for Vox toolchain/self-update semantics only.

## Namespace unification requirements (hard constraints)

### Canonical meaning per verb

- `add`: add project dependency declaration to `Vox.toml`.
- `remove`: remove project dependency declaration from `Vox.toml`.
- `update`: update resolved package graph and lock entries for the project.
- `lock`: create or refresh `vox.lock` without necessarily materializing.
- `sync`: materialize dependencies to local storage from lock/manifest policy.
- `upgrade`: upgrade Vox binary/toolchain/source distribution, never project dependencies.

### Advanced `pm` scope

Use `vox pm ...` only for advanced, operator, or governance actions:

- registry/search/publish/yank,
- vendor/offline packs,
- provenance verify,
- policy checks,
- cache maintenance and diagnostics.

### `install` retirement rule

- `vox install` is removed as a package verb.
- Any transitional alias must fail with explicit migration guidance to the new verbs.

## Cargo-first PM lifecycle to implement

### Required lifecycle stages

1. Read and validate `Vox.toml`.
2. Resolve version graph.
3. Write deterministic `vox.lock`.
4. Fetch artifacts with digest checks into canonical cache/store.
5. Materialize local working set (for build/runtime).
6. Build/ship from lock-bound inputs.

### Policy modes required

- `--locked`: forbid lock mutation.
- `--offline`: forbid network.
- `--frozen`: locked + offline.

These modes must be consistently enforced in local workflows, CI lanes, and Docker build paths.

## Python hard-retirement impact matrix

### Code targets (remove or gate-to-error)

- UV/Python environment code in [crates/vox-container/src/env.rs](../../../crates/vox-container/src/env.rs).
- Python-oriented container generation in `vox-container` python Dockerfile paths.
- Any remaining command flags or branches that imply Python package setup.

### Command contracts and registry

- Ensure command registry reflects no active Python package-management lane.
- Keep historical retired rows only where needed for migration diagnostics.

### Documentation targets

- Remove or rewrite Python integration pages so they no longer describe supported paths.
- Keep historical context only in ADR/changelog sections where explicitly marked as superseded.

## Docker packaging findings and applied requirements

- Current Docker surfaces package the Vox runtime, but are not yet lockfile-contract strict.
- Applied requirement: every packaging lane that installs Vox dependencies must be lock-aware and reproducible.
- Required checks:
  - lock present or explicitly generated by policy,
  - digest verification at fetch,
  - deterministic materialization path.

## External patterns to apply (post-filtered for hard-cut strategy)

### Cargo patterns

- Resolver + lockfile precedence behavior.
- Source replacement, vendoring, and offline operation.
- Sparse registry metadata model and cache discipline.

### Supply-chain patterns

- Checksum-first install guarantees.
- Provenance attestations on release artifacts.
- Policy verification at CI/release gates.

### Patterns explicitly not adopted

- UV/Python universal lock or environment-resolution features are not strategic under hard-cut retirement.

## Risks and unresolved design questions

### High risk

- Breaking script/tooling users who still invoke `vox install`.
- Incomplete retirement where command registry, docs, and code diverge.
- Operator confusion if **`upgrade`** is documented as touching **`Vox.toml`** / **`vox.lock`** (mitigated: namespace split + CI guard on `upgrade.rs`; binary replacement SSOT is [`binary-release-contract.md`](../ci/binary-release-contract.md) / bootstrap, not the PM lock).

### Toolchain upgrade distribution (packaging wave closure)

- **Namespace / safety:** `vox upgrade` is **toolchain-only** and must not touch `Vox.toml` / `vox.lock` (enforced in CI). The command currently emits **operator guidance** (channel placeholder, rebuild / PATH hints).
- **Binary SSOT for replacing `vox`:** documented artifact layout and triples live in [`binary release contract`](../ci/binary-release-contract.md); first-party install path is [`vox-bootstrap`](../reference/cli.md) (falls back to `cargo install --locked --path crates/vox-cli` when no asset matches).
- **Toolchain self-update (shipped):** `vox upgrade` is **check-only** by default; **`--apply`** uses **`self_update`** + **`checksums.txt`** (same contract as bootstrap) into **`CARGO_HOME/bin`**, with **`--provider github|gitlab|http`**, semver gates, and **`--allow-breaking` / `--allow-prerelease`**. Further hardening (e.g. TUF) remains optional.

## Research-backed acceptance criteria

A successful PM redesign must satisfy all of:

- No active package flow depends on Python/UV.
- No active command uses `install` as dependency-management verb.
- `update` and `upgrade` are semantically disjoint and test-enforced.
- Top-level dependency verbs and advanced `pm` verbs are both documented and contract-tested.
- Lockfile policy modes are implemented and enforced across local, CI, and container lanes.

## Implementation closure (tracked in-tree)

As of the 2026 packaging execution wave: hybrid top-level + **`vox pm`** grammar is shipped; **`vox install`** is **removed** from the CLI and registry (scripts must migrate — see [`reference/pm-migration-2026.md`](../reference/pm-migration-2026.md)); **`update`** vs **`upgrade`** split includes CI validators; **`Lockfile`** TOML round-trips **`path`/`git`/`registry`** sources; **`vox pm mirror`** supports **`--file`** and **`--from-registry`** for the local PM index; integration tests cover path graph, registry stub, frozen **`sync`**, **`pm-provenance`**, and optional **`workflow_dispatch`** fixture workflow — see [`vox-packaging-full-implementation-plan-2026.md`](vox-packaging-full-implementation-plan-2026.md).

## Bibliography (core)

- Cargo resolver: [Dependency Resolution](https://doc.rust-lang.org/cargo/reference/resolver.html)
- Cargo source replacement: [Source Replacement](https://doc.rust-lang.org/cargo/reference/source-replacement.html)
- Cargo vendoring: [cargo vendor](https://doc.rust-lang.org/nightly/cargo/commands/cargo-vendor.html)
- Cargo sparse registry: [RFC 2789](https://rust-lang.github.io/rfcs/2789-sparse-index.html)
- Go transparent checksum model: [sumdb design](https://go.googlesource.com/proposal/+/master/design/25530-sumdb.md)
- SLSA provenance schema: [SLSA provenance](https://slsa.dev/spec/v1.0/provenance)
- Sigstore attest verification: [Cosign in-toto attestations](https://docs.sigstore.dev/cosign/verifying/attestation/)
- in-toto framework: [Getting started](https://in-toto.io/docs/getting-started)


