---
title: "Compiler Architecture"
description: "Official documentation for Compiler Architecture for the Vox language. Detailed technical reference, architecture guides, and implementat"
category: "Concepts"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Compiler Architecture

The Vox compiler follows a modular pipeline architecture with conceptual stages. The current implementation is consolidated under `crates/vox-compiler/src/`, where each stage is represented by explicit modules.

Current implementation note: the practical pipeline is currently consolidated under `crates/vox-compiler/src/` for lexer, parser, AST, HIR, typecheck, and emitters. This document keeps conceptual stage boundaries while implementation modules may live in one crate.

---

## Pipeline Overview

```text
Source Code (.vox)
    │
    ▼
┌────────────────┐
│     Lexer      │  Tokenization (logos)
└──────┬─────────┘
       │ Vec<Token>
       ▼
┌────────────────┐
│     Parser     │  Recursive descent parser → AST Module
└──────┬─────────┘
       │ Module (AST root)
       ▼
┌────────────────┐
│      AST       │  Strongly-typed AST wrappers
└──────┬─────────┘
       │ Module (Decl, Expr, Stmt, Pattern)
       ▼
┌────────────────┐
│      HIR       │  Desugaring + name resolution + dead code detection
└──────┬─────────┘
       │ HirModule
       ▼
┌────────────────┐
│    Typeck      │  Bidirectional type checking + HM inference
└──────┬─────────┘
       │ Typed HIR + Vec<Diagnostic>
       ▼
┌────────────────┐
│     Web IR     │  HIR→WebIR lower + validate
└──────┬─────────┘
       │ WebIrModule
       ▼
┌────────────────┐
│  App Contract  │  HIR→AppContract (HTTP/RPC/server config)
└──────┬─────────┘
       │ AppContractModule
       ▼
┌────────────────┐
│ Runtime Proj   │  HIR→RuntimeProjection (DB/task capability hints)
└──────┬─────────┘
       │ RuntimeProjectionModule
       ▼
┌──────────────────┬─────────────────────┐
│ vox-codegen-rust │  vox-codegen-ts     │
│  (quote! → .rs)  │  (string → .ts/tsx) │
└──────────────────┴─────────────────────┘
```

Current path note:

- `codegen_ts` is still the production TS emitter path.
- `VOX_WEBIR_VALIDATE` defaults **on** (WebIR lower/validate gate); set `=0` / `false` / `no` / `off` to skip.
- `app_contract::project_app_contract` is the SSOT for route/RPC/server-config codegen inputs (via [`projection_bundle`](../../../crates/vox-codegen/src/projection_bundle.rs) in emit paths).
- `runtime_projection::project_runtime_from_hir` is the SSOT for orchestration-facing DB capability projection (also bundled).
- Reactive `view:` uses the Web IR TSX bridge when validation is clean; **`VOX_WEBIR_EMIT_REACTIVE_VIEWS` was removed** — there is no legacy-only emit path (see [`reactive.rs`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs)).

---

## ML Training Pipeline

Vox has a native ML training loop powered by [Burn](https://burn.dev) (a pure-Rust deep learning framework):

```text
docs/src/*.md + examples/*.vox
    │
    ▼
vox mens corpus extract   # produces validated.jsonl
    │
    ▼
vox mens corpus pairs     # produces train.jsonl (instruction-response pairs)
    │
    ▼
vox mens train            # native Burn / HF path (default CLI features)
    │
    ▼
mens/runs/v1/model_final.bin
```

The training loop is defined in `crates/vox-cli/src/training/native.rs`.

---

## Stage Details

### 1. Lexer (`vox-compiler::lexer`)

**Purpose**: Converts source text into a flat stream of tokens.

**Implementation**: Uses the [`logos`](https://docs.rs/logos) crate for high-performance, zero-copy tokenization.

**Output**: `Vec<Token>` — each token carries its kind and span.

---

### 2. Parser (`vox-compiler::parser`)

**Purpose**: Transforms a token stream into an AST module.

**Implementation**: A hand-written recursive descent parser producing `ast::decl::Module`. The parser is **resilient to errors**, meaning it continues parsing after encountering invalid syntax — this is critical for LSP support, where the user is actively typing.

**Key features**:
- Error recovery with synchronization points
- Trailing comma support in parameter lists
- Duplicate parameter name detection
- Indentation-aware formatting (`indent.rs`)

See `crates/vox-compiler/src/parser/descent/mod.rs` for the implementation entrypoint.

**Output**: `Module` (AST root) with source spans on declarations and expressions.

---

### 3. AST (`vox-compiler::ast`)

**Purpose**: Strongly-typed wrappers around the untyped CST nodes.

See `crates/vox-ast/src/` for the node hierarchy.

---

### 6. Code Generation

#### Rust Codegen (`vox-compiler::codegen_rust`)

Emits Rust source using the [`quote!`](https://docs.rs/quote) macro. Each decorator maps to specific Rust constructs:

| Vox | Generated Rust |
|-----|---------------|
| `@endpoint fn` | Axum handler + route registration |
| `@table type` | Struct + SQLite schema |
| `@test fn` | `#[test]` function |
| `@deprecated` | `#[deprecated]` attribute |
| `actor` | Tokio task + mpsc mailbox |
| `workflow` | Plain async function today; interpreted runtime provides partial durable step recording |

#### TypeScript Codegen (`vox-compiler::codegen_ts`)

Emits TypeScript/TSX in modular files:

| Module | Output |
|--------|--------|
| `jsx.rs` | React JSX components |
| `component.rs` | Component declarations and hooks |
| `activity.rs` | Activity/workflow client wrappers |
| `emitter.rs` | TanStack Router trees, optional server fns, islands metadata |
| `adt.rs` | TypeScript discriminated union types |

Normative strategy for reducing frontend emitter complexity while preserving React interop:
[ADR 012 — Internal web IR strategy](../adr/012-internal-web-ir-strategy.md).
Detailed implementation sequencing and weighted task quotas:
[Internal Web IR implementation blueprint](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
Ordered file-by-file execution map:
[WebIR operations catalog](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
Canonical current-vs-target representation mapping:
[Internal Web IR side-by-side schema](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
Quantified K-complexity delta for the canonical worked app:
[WebIR K-complexity quantification](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).
Reproducible per-token-class computation:
[WebIR K-metric appendix](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md).

---

## Supporting Crates

| Crate | Purpose |
|-------|---------|
| `vox-cli` | `vox` command-line entry point — see [`ref-cli.md`](../reference/cli.md) for the implemented subcommand set |
| `vox-lsp` | Language Server Protocol implementation |
| `vox-actor-runtime` | Tokio/Axum runtime: actors, scheduler, subscriptions, storage |
| `vox-package` | Package manager: CAS store, dependency resolution, caching |
| `vox-db` | Database abstraction layer |
| `vox-gamify` | Gamification system |
| `vox-orchestrator` | Multi-agent orchestration |
| `vox-code-audit` | AI anti-pattern detector |
| `vox-tensor` | Native ML tensors via Burn 0.19 (Wgpu/NdArray backends) |
| `vox-eval` | Automated evaluation of training data quality |
| `vox-doc-pipeline` | Rust-native doc extraction + SUMMARY.md generation |
| `vox-integration-tests` | End-to-end pipeline tests |

---

## Adding a Language Feature

The full checklist for adding a new language construct:

1. **Lexer** — Add tokens to `crates/vox-compiler/src/lexer/token.rs`
2. **Parser** — Add grammar rules in `crates/vox-compiler/src/parser/descent/`
3. **AST** — Add node types in `crates/vox-ast/src/`
4. **HIR** — Map AST → HIR in `crates/vox-compiler/src/hir/lower/`
5. **Type Check** — Add inference rules in `crates/vox-compiler/src/typeck/`
6. **WebIR** — Add/update lowering + validation semantics in `crates/vox-codegen/src/web_ir/` when the feature affects web-facing behavior
7. **Codegen** — Emit code in both `crates/vox-compiler/src/codegen_rust/` and `crates/vox-codegen/src/codegen_ts/`
8. **Test** — Add integration coverage in `vox-integration-tests/tests/` and WebIR/parity coverage where applicable
9. **Docs** — Add frontmatter + code example in `docs/src/`
10. **Training** — Run `vox mens corpus extract` to include the new construct in ML data

---

## Next Steps

- [Language Reference](../reference/ref-syntax.md) — Full syntax and feature reference
- [Actors & Workflows](expl-actors-workflows.md) — Workflow durability and actor persistence
- [Ecosystem & Tooling](../how-to/how-to-cli-ecosystem.md) — CLI commands, package manager, LSP
- [Web IR operations catalog](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — numbered compiler/emitter tasks **OP-0001–OP-0320** + supplemental **OP-S049–OP-S220** batch map
- [Web IR acceptance gates G1–G6](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md) — parser, K-metric, parity, and rollout thresholds


