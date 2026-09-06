---
title: "Dependency policy"
description: "How Vox pins Rust and JS dependencies, workspace inheritance, and optional CI insight jobs."
category: "Contributors"
status: "current"
sort_order: 12
training_eligible: true

schema_type: "TechArticle"
---

# Dependency policy

## Rust: single workspace pin

- **Authoritative versions** live in the root `Cargo.toml` `[workspace.dependencies]` using caret semver (`"1"`, `"0.8"`, …). The **lockfile** is the execution pin.
- **Crate manifests** under `crates/*/Cargo.toml` must use `{ workspace = true, … }` for external crates. Keep feature flags and `optional = true` locally; do not duplicate version numbers.
- **Exceptions**: `=` pins are reserved for known-problem upstreams (e.g. `jj-lib`). Document new `=` pins in the PR that introduces them.
- **Aliases**: when two semver-incompatible majors of the same package are required (e.g. `schemars` 0.8 vs 1), add an explicit workspace alias:

  ```toml
  schemars08 = { package = "schemars", version = "0.8", default-features = false }
  ```

  Depend on `schemars08` in the consuming crate; use `schemars` for the 1.x line.

- **Heavy / optional stacks** (Tantivy, scrapers, Wasmtime, Candle, optional ML) belong behind **crate features** so default `cargo check --workspace` does not pull them into every binary unless a consumer opts in.

## After dependency churn

1. `cargo check --workspace`
2. `cargo hakari generate --diff` (must be clean — matches CI)
3. If you changed contracts/schemas consumed by codegen binaries, rerun the documented generator (e.g. `cargo run -p vox-scientia-jsonschema-codegen`)

## Evidence for removals and upgrades

Before deleting a dependency or bumping a major:

1. `rg` (or compiler errors) proving **no remaining usage** in first-party code.
2. For duplicate-version reductions, `cargo tree --workspace -i <crate>@<old>` before/after showing the duplicate is gone or justified.

Store optional snapshots under `.tmp_audit/` (gitignored) for local diffing; do not commit large reports unless a maintainer asks.

## Tooling (informational CI)

- **`cargo shear`** — unused dependency hints; cross-check with `rg` before removing anything.
- **`cargo outdated`** — drift vs crates.io; non-blocking; does not authorize silent major bumps without review.

See also: [Workspace dependency audit findings](../architecture/workspace-dependency-audit-2026.md).

## JavaScript / pnpm

- **No root pnpm workspace** is required; apps keep their own lockfiles.
- Align **`@types/react`** / **`@types/react-dom`** across packages when touching `package.json`, using the same caret baseline as the primary editor app (`apps/editor/vox-vscode`) unless a package is pinned to an older React major intentionally.

## `regress` and SCIENTIA generated types

`vox-research-events` includes **`schema_types.generated.rs`** from typify. That file emits **`::regress::Regex`** for JSON Schema `pattern` constraints, so the **`regress`** crate must stay in `[dependencies]` until codegen switches to another regex backend (would require typify/settings changes and a regeneration PR).

## tiktoken-rs / tokenizer parity

`vox-orchestrator` uses **`tiktoken-rs`** with **`cl100k_base`** for token-accurate budgeting and truncation aligned with GPT-4/o-family tokenization. This is **not** a heuristic-only estimate; replacing it with character or word heuristics would change context limits and truncation behavior. Major upgrades to `tiktoken-rs` require a quick parity check on representative prompts (token counts vs reference).
