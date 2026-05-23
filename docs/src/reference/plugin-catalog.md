---
title: "Plugin Catalog"
description: "What the Vox plugin catalog is and how it relates to per-plugin manifests."
category: "Language Reference"
status: "current"
training_eligible: true
---

# Plugin Catalog

The plugin catalog is the SSOT of every first-party Vox plugin. It lives at [`crates/vox-plugin-catalog/catalog.toml`](../../../crates/vox-plugin-catalog/catalog.toml) and is exposed to the rest of the codebase via the `vox-plugin-catalog` crate.

## What the catalog is for

- Source of truth for `vox plugin list` (shows installed + available + incompatible).
- Source of truth for `vox plugin install <id>` (resolves the install URL).
- Source of truth for distribution bundle composition.
- Source of truth for the auto-generated reference docs ([plugin catalog](plugin-catalog.generated.md), [distribution bundles](distribution-bundles.generated.md)).

## What the catalog is **not**

- Not the plugin manifest itself. Each installed plugin ships its own [`Plugin.toml`](plugin-manifest.md). The catalog is the directory of *known* plugins; the manifest is what an installed plugin declares about *itself*.
- Not a marketplace. There is no central server, no ratings, no provenance beyond the URL in `default-source`. Third-party plugins are not in the catalog (and are not yet supported in v1; see the parent design spec).
- Not a feature-flag table. Catalog entries describe *plugins* — units that can be installed at runtime — not Cargo features.

## Editing the catalog

1. Edit `catalog.toml`.
2. `cargo build -p vox-plugin-catalog` — `build.rs` validates the file and fails the build with a clear error if any invariant is violated.
3. `vox ci generate-plugin-catalog-docs` — regenerates the two `.generated.md` files.
4. Commit the catalog change and the regenerated docs in the same commit.

CI guards (`vox ci plugin-catalog-parity`, `vox ci generate-plugin-catalog-docs --check`) enforce both invariants on every PR.
