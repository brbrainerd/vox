---
title: "Reference: package manager and workspace"
description: "Vox.toml manifests, workspace members, lockfiles, and registry client (vox-package)."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Author-facing map for dependency and workspace crates."
schema_type: "TechArticle"
---

# Reference: package manager and workspace

Implementation crates:

- [`crates/vox-package`](../../../crates/vox-package) — workspace discovery, registry HTTP client, artifact helpers.
- [`crates/vox-package-types`](../../../crates/vox-package-types) — `VoxManifest`, `Lockfile`, semver types.

## Workspace layout

- Root **`Vox.toml`** may declare `[workspace]` with `members` globs; [`VoxWorkspace::load`](../../../crates/vox-package/src/workspace.rs) expands globs and loads member manifests.

## Manifest and lock

- Typed manifest loading and lockfile types live in **`vox_package_types`** (re-exported from `vox-package`).
- Operational CLI verbs for dependency workflows are documented in [`cli.md`](./cli.md) as they ship.

## Storage note

- Internal Codex/Arca storage policies intersect package hashing — see ADR 004 (`docs/src/adr/004-codex-arca-turso-ssot.md`) and [`data-storage-ssot-2026.md`](../architecture/data-storage-ssot-2026.md).

## See also

- [FFI](./ref-ffi.md)
- [Portability SSOT](./vox-portability-ssot.md)
