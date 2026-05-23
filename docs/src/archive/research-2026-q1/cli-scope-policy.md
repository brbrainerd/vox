---
title: "CLI scope policy"
description: "Official documentation for CLI scope policy for the Vox language. Detailed technical reference, architecture guides, and implementation p"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# CLI scope policy

## Shipped binary

The **`vox`** executable built from `crates/vox-cli` is the **minimal compiler CLI**. Its command surface is defined in code (`Cli` in `src/lib.rs`, invoked from `src/main.rs`) and documented in [ref-cli.md](../reference/cli.md). The legacy monolithic dispatch source file was removed to avoid drift; extend the shipped surface only via `lib.rs` / `commands/mod.rs` and feature flags.

**Canonical decision:** The product ships this **minimal** surface by default. A larger command tree under `crates/vox-cli/src/commands/**` exists for future integration; most of it stays **out of** `commands/mod.rs` until wired into `lib.rs` / `main.rs`. **`commands::runtime`** (dev / info / tree / run+test shims / shell) and **`commands::info`** are compiled as **library-visible** modules for reuse; they do **not** add subcommands to the minimal `Cli` until explicitly dispatched.

## Feature-gated commands (minimal `Cli`)

Some variants exist only when Cargo features are enabled (see `crates/vox-cli/Cargo.toml`):

- **`ars`** — `vox openclaw` / `oc` (OpenClaw gateway client; `vox-skills`) and `vox skill` (ARS registry / promote / context). Build with `cargo build -p vox-cli --features ars`.
- **`extras-ludus`** — `vox ludus` (gamification; `vox-ludus`). Build with `cargo build -p vox-cli --features extras-ludus`.
- **`live`** — `vox live` (orchestrator demo bus).
- **`populi`** — `vox populi status` / `vox populi serve` (`vox-populi` registry + HTTP control plane). Build with `cargo build -p vox-cli --features populi`.
- **`workflow-runtime`** — interpreted `vox mens workflow run` + `commands::workflow` when enabled; implies **`mens-dei`**. Build with `cargo build -p vox-cli --features workflow-runtime`.

## Documentation

- **Shipped commands** — `ref-cli.md` must match `lib.rs` (`Cli`) / `commands/mod.rs`.
- **Registry + parity** — `contracts/cli/command-registry.yaml` is the machine SSOT; run **`vox ci command-compliance`** (see [`cli-design-rules.md`](../reference/cli.md), [`command-compliance.md`](../reference/command-compliance.md)).
- **Broader narrative** — `how-to-cli-ecosystem.md` may describe workspace-wide or planned tooling; it must state clearly when a command is **not** in the minimal binary.

## Tests and scripts

Integration tests and scripts must not assume subcommands that are absent from the minimal `Cli` enum. Prefer `cargo run -p vox-cli -- …` against documented commands only.

## Script migration exceptions

- **Allowed in GitHub workflows without Rust rewrite {** paths under `scripts/` that are **data artifacts** or **explicitly allowlisted** in `docs/agents/workflow-script-allowlist.txt`. CI enforces this via `vox ci workflow-scripts`.
- **Thin shell / PowerShell shims** (`scripts/check_*.sh`, `scripts/populi/*_gate.*`, legacy `scripts/mens/release_training_gate.*`, …) are **delegates** to `vox ci …` or `cargo run -p vox-cli -- ci …` — keep them one-liners to avoid drift.
- **Host-only tooling** (GPU installers, external marketplace actions, third-party ML stacks) may stay outside `vox ci`; record them in [`docs/agents/script-registry.json`](../../agents/script-registry.json) with `status: "external"` when added.

## Governance

- New **`scripts/...` references** in `.github/workflows/*.yml` must either match the allowlist or the PR must update `workflow-script-allowlist.txt` with an owner note.
- Prefer extending **`vox ci`** for new guards instead of adding long bash matrices.


