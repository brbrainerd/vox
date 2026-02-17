# Vox Programming Language

**Vox** is an AI-native, full-stack programming language that compiles to both Rust (backend) and TypeScript (frontend). Write once, deploy everywhere — from single binaries to WASM modules.

> [!NOTE]
> For detailed architectural documentation, roadmap, and contribution guidelines, see [AGENTS.md](./AGENTS.md).

## Vision

The future of software development involves humans and AI collaborating seamlessly. Vox is built from the ground up to be the ideal medium for this collaboration:
-   **Type-Safe & Predictable**: Strong static typing and ADTs prevent hallucinations.
-   **Full-Stack Cohesion**: Define your data model, API, and UI in a single file.
-   **Durable Execution**: Integrated workflow engine ensures reliability.

## Quick Start

### Installation

```bash
cargo install --path crates/vox-cli
```

### Hello World

```vox
@server fn greet(name: str) to str:
    ret "Hello, " + name + "!"

@component fn App() to Element:
    let name = use_state("World")
    let greeting = greet(name)
    ret <div>
        <h1>{greeting}</h1>
        <input value={name} onChange={fn(e) set_name(e.value)} />
    </div>
```

### Build & Run

```bash
# Development build
vox build app.vox -o dist

# Production bundle (single binary)
vox bundle app.vox --release
```

## Architecture Overview

Vox compiles to industry-standard targets:
-   **Backend**: High-performance Rust (Axum, Tokio).
-   **Frontend**: Modern React (TypeScript).

For a deep dive into the compiler pipeline (Lexer -> CST -> AST -> HIR -> Typeck -> Codegen), please refer to [AGENTS.md](./AGENTS.md).

## Roadmap

- [x] Phase 1: Foundation (Core Compiler, CLI)
- [x] Phase 2: Core Language Features (ADTs, Inference)
- [ ] Phase 3: Ecosystem (LSP, Formatter)
- [ ] Phase 4: Advanced Features (WASM, Durable Objects)

See [AGENTS.md](./AGENTS.md) for the detailed roadmap.

## License

Apache-2.0
