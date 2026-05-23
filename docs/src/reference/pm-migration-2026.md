---
title: "Package management migration (2026)"
description: "Old-to-new command mappings for Cargo-first Vox PM, retired install/Python lanes, and tooling upgrades."
category: "Language Reference"
last_updated: "2026-03-27"
training_eligible: true

schema_type: "TechArticle"
---

# Package management migration (2026)

This note is the **operator-facing mapping** for the packaging redesign (hybrid top-level + **`vox pm`**, strict **`update`** vs **`upgrade`**, **`vox install`** removed as a package verb, and **no supported Python/uv PM path**). Authoritative semantics: [`cli.md`](cli.md) § Package management, [`vox-packaging-implementation-blueprint.md`](../archive/research-2026-q1/vox-packaging-implementation-blueprint.md), and `contracts/cli/command-registry.yaml`.

## Command substitutions

| If you used… | Use instead… |
|--------------|--------------|
| **`vox install`** (package graph) | **`vox add`** / **`vox remove`** (manifest), **`vox lock`** (write/check lock), **`vox sync`** (materialize `.vox_modules/dl/`), **`vox update`** (refresh lock from local PM index), **`vox pm …`** (search, publish, vendor, verify, cache). |
| **`vox upgrade`** for dependencies | **`vox update`** and **`vox sync`**. **`vox upgrade`** is **toolchain-only**: default **check-only**; **`--apply --source release`** installs a release binary with **`checksums.txt`**; **`--apply --source repo`** updates a git checkout and runs **`cargo install --locked --path crates/vox-cli`** (see [`cli.md`](cli.md)). |
| **`vox pm vendor`** at old top-level | Unchanged capability: **`vox pm vendor`** (tree under **`vox pm`**). |
| **`vox mens train-uv`** | **`vox mens train --backend qlora`** ([`mens-training.md`](mens-training.md)). |
| **`vox container init`** / **`uv sync`** as the product PM lane | **`Vox.toml`** + **`vox lock`** + **`vox sync`**; container images follow the repo **`Dockerfile`** / **`infra/containers/Dockerfile.populi`** pattern (`cargo … --locked`). Python bridge docs are **historical** only ([`how-to-pytorch.md`](../how-to/how-to-pytorch.md), [`vox-py.md`](../reference/cli.md)). |

## Verification and release posture

- **PM path-deps + lockfile:** `Lockfile::from_str` preserves `source = { path = "…" }` so `vox sync` does not treat path packages as registry (integration: `cargo test -p vox-cli --test pm_lifecycle_integration`).
- **Registry download (`vox sync --registry`):** same test binary stubs `GET …/download` locally (no GitHub or public registry).
- **Frozen sync:** `pm_registry_sync_frozen_matches_manifest_after_lock` seeds `.vox_modules/local_store.db` via `VoxDb::record_pm_registry_mirror`, runs `vox lock`, then `vox sync --frozen` against the stub (validates lock ↔ manifest strict resolve).
- **Operator mirror:** **`vox pm mirror <name> --version <ver> --file <path>`** *or* **`--from-registry <url>`** performs the same index + CAS write (file = air-gap; URL = same download JSON as **`vox sync`**; honors **`VOX_REGISTRY_TOKEN`** when set).
- **CLI / registry / docs parity:** `vox ci command-compliance` (also `cargo run -p vox-cli -- ci command-compliance` from repo root).
- **PM provenance sidecars** (from **`vox pm publish`**): `.vox_modules/provenance/*.json` (`vox.pm.provenance/1`). Enforce in CI with **`vox ci pm-provenance --strict`** when promoting registry artifacts ([`binary-release-contract.md`](../ci/binary-release-contract.md)).
- **Doc inventory drift:** `vox ci doc-inventory verify` after changing substantial docs ([`doc-inventory.md`](doc-inventory.md)).

## See also

- [`how-to-cli-ecosystem.md`](../how-to/how-to-cli-ecosystem.md) — ecosystem entry and **`vox install`** removal note.
- [`cli-command-surface.generated.md`](cli-command-surface.generated.md) — generated status table (`vox ci command-sync --write`).

