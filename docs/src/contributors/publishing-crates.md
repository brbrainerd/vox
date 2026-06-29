---
title: "Publishing crates to crates.io"
description: "Repeatable hakari-aware recipe for publishing workspace crates externally."
category: "Contributors"
status: "current"
---

# Publishing a crate to crates.io

Vox uses [cargo-hakari](https://docs.rs/cargo-hakari) for workspace build optimization. Publishing
external crates requires a few extra steps to satisfy crates.io closure requirements.

## Checklist

1. **Mark the crate publishable** in `docs/src/architecture/layers.toml`:

   ```toml
   my-crate = { layer = 1, publishable = true }
   ```

2. **Run the publishability gate**:

   ```bash
   cargo run -p vox-arch-check
   ```

   Rule 18 must pass — no `publish = false` deps in the workspace-crate closure. If it fires,
   either mark each flagged dep publishable (and add it to this pipeline) or remove it from the
   crate's dependencies.

3. **Remove `workspace-hack`** from the crate's `Cargo.toml`:

   ```toml
   # Remove this line from crates intended for external publishing:
   # workspace-hack = { workspace = true }
   ```

   `workspace-hack` is a build-optimization-only dep managed by hakari. It has no code — only
   feature flags. `cargo publish` cannot resolve it (our repo version is 0.6.0; no matching
   version on crates.io). Removing it has no effect on crate behaviour.

4. **Add a version** to each workspace path-dep that is in this crate's closure (in the root
   `Cargo.toml` `[workspace.dependencies]` section):

   ```toml
   my-dep = { path = "crates/my-dep", version = "0.6.0" }
   ```

5. **Verify name availability**:

   ```bash
   cargo search my-crate --limit 5
   ```

   crates.io has no namespaces — use a `vox-` prefix to avoid collisions.
   Record the decision in `contracts/reports/crates-io-names.v1.json`.

6. **Add publish metadata** to the crate's `Cargo.toml`:

   ```toml
   readme = "README.md"
   keywords = ["..."]   # max 5, snake_case
   categories = ["..."] # must match https://crates.io/category_slugs
   ```

   `license`, `repository`, `version`, `authors` are inherited from the workspace. Add a
   `README.md` (becomes the crates.io front page).

7. **Dry-run**:

   ```bash
   cargo publish -p my-crate --dry-run
   ```

   If this fails with "no matching package named X", the dep `X` is not yet on crates.io.
   Publish it first (leaf-first ordering).

8. **Publish leaf-first** — each crate must be on crates.io before its dependents can publish:

   ```bash
   cargo publish -p my-leaf-crate    # publish deps first
   cargo publish -p my-crate         # then the crate
   ```

   **Every `cargo publish` (without `--dry-run`) is irreversible — crates.io allows yanking
   but not deletion. Requires `cargo login` with a maintainer token. Never run from CI.**

## Current publishable crates (leaf-first order)

| Crate | Status | Notes |
| --- | --- | --- |
| `vox-crypto` | prepared, ready to publish | no workspace deps |
| `vox-mesh-types` | prepared | no workspace deps |
| `vox-scaling-policy` | prepared | requires `vox-mesh-types` live |
| `vox-bounded-fs` | prepared | requires `vox-scaling-policy` live |
| `vox-secrets` | prepared | requires `vox-crypto` + `vox-bounded-fs` live |

## See also

- `contracts/reports/crates-io-names.v1.json` — approved published names
- `contracts/reports/closure-budgets.v1.json` — Rule 19 dep-closure budgets
- `docs/src/architecture/layers.toml` — `publishable = true` registry
