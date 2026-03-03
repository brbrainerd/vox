---
title: "Vox Compiler Architecture"
category: explanation
constructs: [function, actor, workflow, component, table, query]
last_updated: 2026-03-02
training_eligible: true
difficulty: intermediate
---

# Compiler Architecture

The Vox compiler follows a modern, modular pipeline architecture. Each stage is implemented as an independent Rust crate within the `crates/` workspace.

---

## Pipeline Overview

```
Source Code (.vox)
    │
    ▼
┌──────────────┐
│  vox-lexer   │  Tokenization (logos)
└──────┬───────┘
       │ Vec<Token>
       ▼
┌──────────────┐
│  vox-parser  │  Recursive descent → lossless GreenTree (Rowan)
└──────┬───────┘
       │ GreenTree / CST
       ▼
┌──────────────┐
│   vox-ast    │  Strongly-typed AST wrappers
└──────┬───────┘
       │ Module (Decl, Expr, Stmt, Pattern)
       ▼
┌──────────────┐
│   vox-hir    │  Desugaring + name resolution + dead code detection
└──────┬───────┘
       │ HirModule
       ▼
┌──────────────┐
│  vox-typeck  │  Bidirectional type checking + HM inference
└──────┬───────┘
       │ Typed HIR + Vec<Diagnostic>
       ▼
┌──────────────────┬─────────────────────┐
│ vox-codegen-rust │  vox-codegen-ts     │
│  (quote! → .rs)  │  (string → .ts/tsx) │
└──────────────────┴─────────────────────┘
```

---

## ML Training Pipeline

Vox has a native ML training loop powered by [Burn](https://burn.dev) (a pure-Rust deep learning framework):

```
docs/src/*.md + examples/*.vox
    │
    ▼
vox corpus extract     # produces validated.jsonl
    │
    ▼
vox corpus pairs       # produces train.jsonl (instruction-response pairs)
    │
    ▼
vox training native    # Burn 0.19 / Wgpu backend
    │
    ▼
populi/runs/v1/model_final.bin
```

The training loop is defined in `crates/vox-cli/src/training/native.rs`.

---

## Stage Details

### 1. Lexer (`vox-lexer`)

**Purpose**: Converts source text into a flat stream of tokens.

**Implementation**: Uses the [`logos`](https://docs.rs/logos) crate for high-performance, zero-copy tokenization.

**Output**: `Vec<Token>` — each token carries its kind and span. See [vox-lexer API](api/vox-lexer.md) for details.

---

### 2. Parser (`vox-parser`)

**Purpose**: Transforms a token stream into a lossless Concrete Syntax Tree (CST).

**Implementation**: A hand-written recursive descent parser producing a [Rowan](https://docs.rs/rowan)-based GreenTree. The parser is **resilient to errors**, meaning it continues parsing after encountering invalid syntax — this is critical for LSP support, where the user is actively typing.

**Key features**:
- Error recovery with synchronization points
- Trailing comma support in parameter lists
- Duplicate parameter name detection
- Indentation-aware formatting (`indent.rs`)

See [vox-parser API](api/vox-parser.md) for implementation details.

**Output**: `GreenTree` — a lossless syntax tree preserving all whitespace and comments.

---

### 3. AST (`vox-ast`)

**Purpose**: Strongly-typed wrappers around the untyped CST nodes.

See [vox-ast API](api/vox-ast.md) for the full node hierarchy.

---

### 6. Code Generation

#### Rust Codegen (`vox-codegen-rust`)

Emits Rust source using the [`quote!`](https://docs.rs/quote) macro. Each decorator maps to specific Rust constructs:

| Vox | Generated Rust |
|-----|---------------|
| `@server fn` | Axum handler + route registration |
| `@table type` | Struct + SQLite schema |
| `@test fn` | `#[test]` function |
| `@deprecated` | `#[deprecated]` attribute |
| `actor` | Tokio task + mpsc mailbox |
| `workflow` | State machine with durable step recording |

#### TypeScript Codegen (`vox-codegen-ts`)

Emits TypeScript/TSX in modular files:

| Module | Output |
|--------|--------|
| `jsx.rs` | React JSX components |
| `component.rs` | Component declarations and hooks |
| `activity.rs` | Activity/workflow client wrappers |
| `routes.rs` | React Router route definitions |
| `adt.rs` | TypeScript discriminated union types |

---

## Supporting Crates

| Crate | Purpose |
|-------|---------|
| `vox-cli` | `vox` command-line entry point (build, bundle, test, fmt, lsp, install) |
| `vox-lsp` | Language Server Protocol implementation |
| `vox-runtime` | Tokio/Axum runtime: actors, scheduler, subscriptions, storage |
| `vox-pm` | Package manager: CAS store, dependency resolution, caching |
| `vox-db` | Database abstraction layer |
| `vox-gamify` | Gamification system |
| `vox-orchestrator` | Multi-agent orchestration |
| `vox-toestub` | AI anti-pattern detector |
| `vox-tensor` | Native ML tensors via Burn 0.19 (Wgpu/NdArray backends) |
| `vox-eval` | Automated evaluation of training data quality |
| `vox-doc-pipeline` | Rust-native doc extraction + SUMMARY.md generation |
| `vox-integration-tests` | End-to-end pipeline tests |

---

## Adding a Language Feature

The full checklist for adding a new language construct:

1. **Lexer** — Add tokens to `vox-lexer/src/token.rs`
2. **Parser** — Add grammar rules in `vox-parser/src/grammar.rs`
3. **AST** — Add node types in `vox-ast/src/`
4. **HIR** — Map AST → HIR in `vox-hir/src/lower.rs`
5. **Type Check** — Add inference rules in `vox-typeck/src/check.rs`
6. **Codegen** — Emit code in both `vox-codegen-rust` and `vox-codegen-ts`
7. **Test** — Add an integration test in `vox-integration-tests/tests/`
8. **Docs** — Add frontmatter + code example in `docs/src/`
9. **Training** — Run `vox corpus extract` to include the new construct in ML data

---

## Next Steps

- [Language Guide](ref-language.md) — Full syntax and feature reference
- [Actors & Workflows](expl-actors-workflows.md) — Durable execution system
- [Ecosystem & Tooling](how-to-cli-ecosystem.md) — CLI commands, package manager, LSP
