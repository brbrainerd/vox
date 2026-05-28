---
title: "Cli Toolchain"
description: "Agent support documentation for cli toolchain"
category: "contributor"
status: "current"
training_eligible: true
---
# Vox CLI & Toolchain — Execution Layer

The `vox` binary is the **canonical tool surface** for all compilation, analysis, and
training operations. It is stateless (no long-running daemon) and can be invoked by
humans, CI systems, and the Orchestrator alike.

## Crate Architecture

```
crates/
  vox-compiler/       ← Lexer, Parser, AST, HIR, Typeck, Fmt
  vox-codegen/        ← Code generation (TypeScript / JS emit)
  vox-ssg/            ← Static site generation
  vox-lsp/           ← Language Server (stdio)
  vox-package/       ← Package manager (vox add/remove/publish)
  vox-actor-runtime/ ← Runtime builtins (hash, stdlib)
  vox-tensor/        ← Pure-CPU JSONL data loaders / training-pair types (Burn extracted 2026-05-08)
  vox-code-audit/    ← TOESTUB architectural analysis engine
  vox-container/     ← Container/OCI build support
  vox-ssg/           ← Static site generation
  vox-eval/          ← Expression evaluator
  vox-config/        ← Config SSOT (Vox.toml + global + env)
  vox-cli/           ← Binary entrypoint, all subcommands
```

## CLI Command Surface

| Command | Description |
|---|---|
| `vox build` | Compile `.vox` → TypeScript |
| `vox check` | Type-check without output; optional TOESTUB |
| `vox fmt` | Format in-place |
| `vox test` | Run `@test` functions |
| `vox run` | Execute a Vox source file |
| `vox bundle` | Full-stack bundle |
| `vox dev` | Watch + rebuild via **`vox-compilerd`** (stdio JSON-RPC; daemon on `PATH` or next to `vox`) |
| `vox lsp` | Start Language Server over stdio |
| `vox stub-check` | TOESTUB scan — **`cargo build -p vox-cli --features stub-check`** (see `docs/src/reference/cli.md`) |
| `vox review` | AI code review |
| `vox train` | Orchestrate fine-tuning |
| `vox learn` | Behavioral learning / dogfood export |
| `vox mens corpus` | Training corpus (extract, validate, pairs, mix, eval) |
| `vox generate` | LLM code generation |
| `vox chat` | Interactive AI chat |
| `vox config get/set` | Read/write VoxConfig SSOT |
| `vox agent status` | Query Orchestrator agent states (headless) |
| `vox doctor` | Environment diagnostic (`tokio::process` / `tokio::fs` from the async handler) |
| `vox setup` | First-run wizard |
| `vox init / new` | Project scaffolding |
| `vox add / remove` | Dependency management |
| `vox publish` | Publish to Vox registry |
| `vox container` | OCI container management |
| `vox orchestrator` | Orchestrator task/lock management |
| `vox db` | VoxDB management |
| `vox ludus` | Gamification profile/quests/battles (`extras-ludus`) |
| `vox skill` | Skill install/list/search |
| `vox architect` | Governance checks |
| `vox completions` | Shell completions |

## Hard Rules

- **Stateless**: No daemon. Each invocation is idempotent.
- **No orchestrator imports**: CLI does not link against `vox-orchestrator`.
  Communication is: Orchestrator calls `vox` as a subprocess.
- **Single binary**: All subcommands live in `vox-cli::commands/`. Never split
  into separate binaries unless performance mandates it.
- **Config reads through `vox-config`**: No hard-coded defaults in command handlers.

## TOESTUB Self-Enforcement

Canonical CI/agents path: **`bash scripts/quality/toestub_scoped.sh`** (or `cargo run -p vox-code-audit --bin toestub -- <PATH>`).

With **`cargo build -p vox-cli --features stub-check`**, the same rules are available as:

```text
vox stub-check --path .
```

See `docs/src/reference/cli.md` for flags (`--suggest-fixes`, not `--fix`).
God-object thresholds (from `vox-schema.json`):
- Files > 500 lines → warning
- Structs > 12 methods → warning
- Directories > 20 files → warning

## Build Environment Notes

On Windows, always invoke via full path in agent shells:
```powershell
& "$env:USERPROFILE\.cargo\bin\cargo.exe" check --workspace
```
Prefer `cargo check` over `cargo build` for fast iteration in agent sessions.
