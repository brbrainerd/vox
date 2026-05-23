---
title: "Workspace root `Cargo.toml` (fix forward)"
description: "Official documentation for Workspace root `Cargo.toml` (fix forward) for the Vox language. Detailed technical reference, architecture gui"
category: "CI & Quality"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Workspace root `Cargo.toml` (fix forward)

There is **no** reliance on `git restore` or old commits to recover this file. The **root `Cargo.toml`** is the **single source of truth** for:

- `[workspace]` — `members`, `exclude`, `default-members`
- `[workspace.package]` — shared `version`, `edition`, `license`, **`repository`**, `rust-version`, etc. (member crates use `*.workspace = true` where applicable)
- `[workspace.dependencies]` — **every** dependency referenced as `{ workspace = true }` in a member crate **must** appear here with either a `path = "crates/…"` (internal) or a crates.io `version` / `features` (external)

## When Cargo errors with "not found in `workspace.dependencies`"

1. Open the member `crates/<crate>/Cargo.toml` and note the dependency key (e.g. `vox-oratio`, `turso`).
2. Add to root `[workspace.dependencies]`:
   - **Internal:** `vox-oratio = { path = "crates/vox-oratio" }` (and add the crate to `members` if it is new — usually covered by `members = ["crates/*"]` plus `exclude` for exceptions).
   - **External:** `some-crate = { version = "x.y", features = [...] }` — align versions with sibling deps in the same table when possible.
3. If you changed versions, update **`Cargo.lock`**: `cargo update -p <crate>` or a full `cargo check --workspace` on a machine with disk space.
4. Verify resolution without a full compile: **`vox ci manifest`** (CI runs `cargo run -p vox-cli --quiet -- ci manifest`). Doc drift: **`vox ci check-docs-ssot`** (inventory + stale-ref scan).

## Optional: internal deps as `path` in a member

Some crates use `vox-foo = { path = "../vox-foo" }` instead of `workspace = true`. That is valid and does **not** require an entry in `[workspace.dependencies]`. Prefer **one style per crate** for consistency (most Vox crates use `workspace = true` for shared versions).

## `exclude` vs `members`

With `members = ["crates/*"]`, every `crates/<name>/` with a `Cargo.toml` becomes a member unless listed under **`[workspace].exclude`** (e.g. experimental or broken-out trees). Keep `exclude` in sync when adding such directories.

## Root `Vox.toml` `[workspace]` (not Cargo)

The committed **`Vox.toml`** at the repo root is the manifest for **Vox package / deploy / orchestrator** settings. Its optional **`[workspace].members`** is used only by **`vox-package::VoxWorkspace`** to discover per-crate **`crates/<name>/Vox.toml`** files via a glob (see the comment block in root `Vox.toml`). It does **not** define the Rust workspace graph — that remains **`Cargo.toml`** above.

## Related

- [Runner contract](runner-contract.md) — self-hosted CI labels; canonical **`vox ci`** narrative; optional CUDA compile gate.
- [Workflow enumeration](workflow-enumeration.md) — where `verify_workspace_manifest` runs.


