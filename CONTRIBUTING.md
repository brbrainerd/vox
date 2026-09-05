---
title: "Contributing"
description: "The short golden path for contributors. Deeper policy lives in AGENTS.md and governance.md."
category: "contributor"
status: "current"
training_eligible: true
training_rationale: "Provides the entry point for human and agent contributors."
---
# Contributing to Vox

Welcome. This file is the **short golden path**; deeper policy lives in [AGENTS.md](AGENTS.md) (required for all contributors and agents) and [docs/agents/governance.md](docs/agents/governance.md) (TOESTUB, architecture rules).

## Quick start

1. Install **Rust** (see root `README.md` and [`docs/src/how-to/how-to-cli-ecosystem.md#installation`](docs/src/how-to/how-to-cli-ecosystem.md#installation)).
2. From the repo root:  
   `cargo check --workspace`
3. Before pushing:  
   `cargo run -p vox-cli -- ci line-endings` on your diff (see [runner contract](docs/src/ci/runner-contract.md)).
4. If you touch CLI flags or help text:  
   `cargo run -p vox-cli -- ci command-compliance`

## Beyond `vox` itself

`cargo build -p vox-cli` (and the `voxup` installer) give you the **CLI only**.
These subsystems live in separate binaries or plugins and are *not* installed by
either — `vox doctor` will keep reporting them as missing until you add them:

| Subsystem | Install | Needed for |
|---|---|---|
| ML / mesh CLI | `cargo install --locked --path crates/vox-ml-cli --features populi` | `vox mens`, `vox populi` (both delegate to it; `populi` is **not** a default feature — a bare install yields a mesh-less binary) |
| Orchestrator daemon | `cargo install --path crates/vox-orchestrator-d` | Axis; otherwise the GUI retries the spawn forever |
| Mesh transport | `vox plugin install populi-mesh --yes` | Mesh control plane / A2A dispatch |
| GPU backend | `vox plugin install mens-candle-metal --yes` (Apple Silicon)<br>`vox plugin install mens-candle-cuda --yes` (NVIDIA) | Accelerated inference |
| Language server | `cargo build -p vox-lsp --release` | Editor integration |

The **Axis** GUI (`crates/vox-gui`) additionally needs its frontend built first —
Tauri's `beforeDevCommand` / `beforeBuildCommand` are empty, so nothing builds
`ui/dist` for you:

```bash
cd crates/vox-gui/ui && pnpm install && pnpm build
```

Requires **pnpm 11+**. Settings live in `ui/pnpm-workspace.yaml`, not the `pnpm`
field of `package.json` — pnpm 11 ignores that field once the workspace file
exists, so dependency `overrides` must be kept in **both** or the floor is
silently dropped.

## Pre-commit hooks

Run once after cloning to install generators that auto-maintain `.generated.md` files and ignore-file sync on every commit:

```bash
vox run scripts/install-hooks.vox
```

Requires [lefthook](https://github.com/evilmartians/lefthook): `winget install evilmartians.lefthook` (Windows), `brew install lefthook` (macOS), or `cargo install lefthook` (Linux/other). If you skip this step, CI will show advisory warnings when generated files drift.

## Where things live

| Area | Entry |
|------|--------|
| Compiler (lex → HIR) | [`docs/src/explanation/expl-architecture.md`](docs/src/explanation/expl-architecture.md) |
| CLI | [`docs/src/reference/cli.md`](docs/src/reference/cli.md) |
| Mens / Populi HTTP | [`docs/src/reference/populi.md`](docs/src/reference/populi.md) |
| Secrets | [`docs/src/reference/secrets-ssot.md`](docs/src/reference/secrets-ssot.md) |

## First PR checklist

- [ ] **Write the failing test first.** New `pub fn` in `crates/*/src/**` needs a
      test in the same file *before* the implementation — the `tdd-guard`
      pre-commit hook blocks commits that skip it. See
      [AGENTS.md §Test-First Policy](AGENTS.md). `vox new fn` scaffolds a stub
      paired with a failing `@test` block.
- [ ] `vox run scripts/fmt.vox` to format — **not** `cargo fmt`. At this
      virtual-workspace root a bare `cargo fmt` is the all-members invocation,
      which overflows the Windows `CreateProcess` limit and dies with
      `os error 206`. For one crate: `cargo fmt -p <crate>`.
- [ ] `cargo clippy -p <crate> -- -D warnings` for crates you changed
- [ ] Targeted `cargo test -p <crate>` for crates you changed
- [ ] `vox ci pre-push` before pushing — the aggregate local gate (fmt,
      line-endings, ssot-drift, scoped doc lint). Install the hooks once with
      `vox run scripts/install-hooks.vox`. Note `--complete` runs **no tests**;
      use `--full` when you changed code or tests.
- [ ] Docs SSOT if you changed user-visible behavior (see [`documentation-governance.md`](docs/src/contributors/documentation-governance.md))

## Deep onboarding

- [Contributing — parser & HIR](docs/src/how-to/how-to-contribute-parser-hir.md)
- [Contributing — Populi operators](docs/src/how-to/how-to-contribute-populi.md)
- [Contributing — Mens training](docs/src/how-to/how-to-contribute-mens.md)
- [First `.vox` app (checkpoints)](docs/src/tutorials/tut-first-vox-app-checkpoints.md)
