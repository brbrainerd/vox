---
title: "Ecosystem & Tooling"
description: "Official documentation for Ecosystem & Tooling for the Vox language. Detailed technical reference, architecture guides, and implementatio"
category: "How-To Guides"
training_eligible: true

schema_type: "HowTo"
---
# Ecosystem & Tooling

> **Note:** This page describes the **intended** developer experience. The **`crates/vox-cli`** binary implements a **subset** of commands today (`build`, `check`, `test`, `run`, `bundle`; `fmt` / `install` fail until wired; `lsp`). Authoritative current flags: **[`ref-cli.md`](../reference/cli.md)**.

Vox ships with a complete development toolchain: compiler, bundler, test runner, formatter, package manager, and language server — converging on the **`vox`** CLI as the primary entry point.

---

## CLI Commands

### `vox build`

Compile a `.vox` file to Rust and TypeScript:

```bash
# Basic build
vox build app.vox -o dist
```

Watch mode and other flags may land later; use `vox build --help` and [`ref-cli.md`](../reference/cli.md) for what the binary exposes **now**.

**Typical output layout (minimal CLI)** — filenames vary by program; Rust lands under `target/generated/`:
```text
dist/
├── backend/      # Generated Rust (Axum server)
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
└── frontend/     # Generated TypeScript (React)
    ├── src/
    │   └── App.tsx
    └── package.json
```

### `vox bundle`

Ship a single statically-linked binary containing frontend + backend + SQLite:

```bash
# Release build targeting Linux
vox bundle app.vox --release --target x86_64-unknown-linux-musl

# Debug build (default)
vox bundle app.vox
```

### `vox test`

Run `@test` decorated functions:

```bash
vox test tests.vox
```

This compiles the test functions to Rust `#[test]` blocks and runs them with `cargo test`.

### `vox fmt`

**Minimal binary today:** `vox fmt` **exits with an error** until `vox-fmt` matches the current AST. Formatting work lives in the `vox-fmt` crate.

```bash
vox fmt app.vox
```

See [`ref-cli.md`](../reference/cli.md).

### `vox lsp`

Launch the Language Server Protocol server:

```bash
vox lsp
```

See [Language Server](#language-server-lsp) below for details.

### Package management (`vox add` / `vox sync` / `vox pm`)

**`vox install` is removed** (no CLI subcommand). Use **`vox add`**, **`vox lock`**, **`vox sync`**, and **`vox pm`** per [`reference/cli.md`](../reference/cli.md); see the full mapping in [`pm-migration-2026.md`](../reference/pm-migration-2026.md).

### `vox vendor`

Offline trees: use **`vox pm vendor`**. Populate `.vox_modules/dl/` with **`vox sync`** first.

---

## Language Server (LSP)

The `vox-lsp` crate provides IDE support via the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/).

### Current Features

| Feature | Status |
|---------|--------|
| Syntax error diagnostics | ✅ Implemented |
| Type error diagnostics | ✅ Implemented |
| Go to Definition | 🔜 Planned |
| Completion | 🔜 Planned |
| Hover info | 🔜 Planned |

### Setup

1. Build the LSP server:
   ```bash
   cargo build --release -p vox-lsp
   ```

2. Configure your editor:

   **VS Code** (with the `vox-vscode` extension or manual configuration):
   ```json
   "vox.lsp.serverPath": "/path/to/target/release/vox-lsp"
   ```

The LSP server integrates the full compiler pipeline — when you save a file, it re-runs the lexer, parser, and type checker to provide real-time diagnostics.

---

## Package Manager (`vox-package`)

The Vox package manager uses a **Content-Addressable Store (CAS)** backed by libSQL/Turso.

### How It Works

```text
store(data) → SHA3-256 hash
get(hash)   → data
```

All artifacts are stored by their content hash:
- **Deterministic** — same content always produces the same hash
- **Deduplication** — identical artifacts share a single stored copy
- **Integrity** — content can be verified against its hash at any time

### Database Backends

| Mode | Use Case |
|------|----------|
| Remote (Turso) | Production — cloud-hosted database |
| Local SQLite | Development — local file storage |
| In-Memory | Testing — ephemeral database |
| Embedded Replica | Hybrid — local cache with cloud sync |

### Semantic Code Search

The package manager includes a **de Bruijn indexing** normalizer that strips identifier names from AST nodes and replaces bound variables with positional indices. This enables detection of semantically identical code regardless of naming differences.

```text
bind_name(namespace, name, hash)    # Map a name to content
lookup_name(namespace, name) → hash # Resolve a name to content
search_code_snippets(query, limit)  # Vector-similarity search
```

### Agent Memory

The store also manages agent memory for AI-powered features:

```text
recall_async(agent, type, limit, min_importance)  # Query with relevance filtering
```

---

## Installation

Canonical page: [Installing Vox](../reference/installation.md).

### Automated (recommended)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://voxlang.org/voxup | sh
```

From a checkout, the same scripts run directly — they take **no arguments**:

```bash
sh scripts/install.sh        # Linux / macOS
.\scripts\install.ps1        # Windows (PowerShell)
```

### Manual

Prerequisites: Rust 1.96 (`rust-toolchain.toml` pins `1.96.0`), Node.js >= 18, C compiler (gcc/clang/MSVC). Full workspace + **Turso** crates: **clang** on Linux/macOS; **clang-cl** (LLVM) on Windows.

```bash
cargo install --locked --path crates/vox-cli
```

> **Note:** Node.js and npm are required at runtime for `vox bundle` and `vox run` (frontend scaffolding). Copy `.env.example` to `.env` to configure optional API keys.

---

## Development

### Building

```bash
cargo build --workspace
```

### Testing

```bash
cargo test --workspace
```

### Linting

```bash
cargo fmt --all -- --check    # Format check
cargo clippy --workspace      # Lint check
```

---

## Next Steps

- [Language Guide](../reference/ref-syntax.md) — Full syntax and feature reference
- [Compiler Architecture](../explanation/expl-architecture.md) — Pipeline internals
- [Actors & Workflows](../explanation/expl-actors-workflows.md) — Concurrency and durable execution
- [Examples](examples-corpus.md) — Annotated example programs

