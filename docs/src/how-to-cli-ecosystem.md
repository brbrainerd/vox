# Ecosystem & Tooling

Vox ships with a complete development toolchain: compiler, bundler, test runner, formatter, package manager, and language server — all accessed through the `vox` CLI.

---

## CLI Commands

### `vox build`

Compile a `.vox` file to Rust and TypeScript:

```bash
# Basic build
vox build app.vox -o dist

# Watch mode with live-reload
vox build app.vox -o dist --watch
```

**Output structure**:
```
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

Format `.vox` source files using the integrated lexical span formatter:

```bash
vox fmt app.vox
```

The formatter preserves inline indentation logic while normalizing whitespace and alignment.

### `vox lsp`

Launch the Language Server Protocol server:

```bash
vox lsp
```

See [Language Server](#language-server-lsp) below for details.

### `vox install`

Install packages from the Vox registry:

```bash
# Install a package
vox install my-package

# Install with offline-only mode (cached packages only)
vox install my-package --offline
```

### `vox vendor`

Bundle all dependencies into a local `vendor/` directory for fully offline builds:

```bash
vox vendor
```

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

## Package Manager (`vox-pm`)

The Vox package manager uses a **Content-Addressable Store (CAS)** backed by libSQL/Turso.

### How It Works

```
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

```
bind_name(namespace, name, hash)    # Map a name to content
lookup_name(namespace, name) → hash # Resolve a name to content
search_code_snippets(query, limit)  # Vector-similarity search
```

### Agent Memory

The store also manages agent memory for AI-powered features:

```
recall_memory(agent, type, limit, min_importance)  # Query with relevance filtering
```

---

## Installation

### Automated (recommended)

```bash
# Linux / macOS
./scripts/install.sh          # End-user install
./scripts/install.sh --dev    # Full contributor setup

# Windows (PowerShell)
.\scripts\install.ps1         # End-user install
.\scripts\install.ps1 -Dev    # Full contributor setup
```

### Manual

Prerequisites: Rust >= 1.75, Node.js >= 18, C compiler (gcc/clang/MSVC)

```bash
cargo install --path crates/vox-cli
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

- [Language Guide](ref-language.md) — Full syntax and feature reference
- [Compiler Architecture](expl-architecture.md) — Pipeline internals
- [Actors & Workflows](expl-actors-workflows.md) — Concurrency and durable execution
- [Examples](examples.md) — Annotated example programs
