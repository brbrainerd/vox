# Vox CLI & Toolchain — Execution Layer

The `vox` binary is the **canonical tool surface** for all compilation, analysis, and
training operations. It is stateless (no long-running daemon) and can be invoked by
humans, CI systems, and the Orchestrator alike.

## Crate Architecture

```
crates/
  vox-lexer/         ← Tokeniser (logos-based)
  vox-ast/           ← AST node types
  vox-parser/        ← Recursive-descent parser
  vox-hir/           ← High-level IR (desugared AST)
  vox-typeck/        ← Type checker
  vox-codegen-ts/    ← TypeScript code generator
  vox-codegen-rust/  ← Rust code generator
  vox-fmt/           ← Formatter
  vox-lsp/           ← Language Server (stdio)
  vox-pm/            ← Package manager (vox add/remove/publish)
  vox-runtime/       ← Runtime builtins (hash, stdlib)
  vox-tensor/        ← Burn-based ML training (feature-gated)
  vox-toestub/       ← TOESTUB architectural analysis engine
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
| `vox dev` | Build + watch + hot-reload |
| `vox lsp` | Start Language Server over stdio |
| `vox stub-check` | TOESTUB scan (alias: `vox toestub self`) |
| `vox review` | AI code review |
| `vox train` | Orchestrate fine-tuning |
| `vox train --native` | Burn-based Rust training loop |
| `vox learn` | Behavioral learning / dogfood export |
| `vox corpus` | Training corpus management |
| `vox generate` | LLM code generation |
| `vox chat` | Interactive AI chat |
| `vox config get/set` | Read/write VoxConfig SSOT |
| `vox agent status` | Query Orchestrator agent states (headless) |
| `vox doctor` | Environment diagnostic |
| `vox setup` | First-run wizard |
| `vox init / new` | Project scaffolding |
| `vox add / remove` | Dependency management |
| `vox publish` | Publish to Vox registry |
| `vox container` | OCI container management |
| `vox orchestrator` | Orchestrator task/lock management |
| `vox db` | VoxDB management |
| `vox gamify` | Gamification profile/quests/battles |
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

The CLI enforces its own architectural rules:
```
vox toestub self           # scan the vox workspace
vox stub-check --path .    # equivalent
```
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
