# Explanation: Compiler Lowering Phases

Understand how the Vox compiler transforms high-level source code into optimized Rust and TypeScript output.

## 1. Syntax to AST (Abstract Syntax Tree)

The `vox-parser` converts the raw `.vox` file into a tree of declarations. This phase ensures the code is syntactically valid but does not yet understand types or decorators.

## 2. AST to HIR (High-level Intermediate Representation)

The **Lowering** phase begins by transforming the AST into the HIR.
- **Symbol Resolution**: Linking variable names to their definitions.
- **Decorator Processing**: Expanding decorators like `@server` into their underlying architectural primitives (handlers, endpoints, clients).
- **Type Inference**: Deducing types for all expressions.

## 3. HIR to LIR (Low-level Intermediate Representation)

The LIR is a target-specific representation optimized for code generation.
- **Backend LIR**: Optimized for Rust emission, including database queries and actor behaviors.
- **Frontend LIR**: Optimized for TypeScript/React emission, including JSX transformation and RPC client generation.

## 4. Code Generation (Emission)

The final phase where LIR is converted into source files:
- **`vox-codegen-rust`**: Produces `main.rs`, `models.rs`, and `api.rs`.
- **`vox-codegen-ts`**: Produces `App.tsx`, `client.ts`, and `types.ts`.

## 5. Why Lowering Matters?

By having multiple intermediate representations, Vox can perform complex architectural optimizations—like automatically grouping database queries or optimizing actor communication—that would be impossible in a single-pass compiler.

---

**Related Reference**:
- [Architecture Index](expl-architecture.md) — High-level map of the compiler crates.
- [API Reference: vox-hir](api/vox-hir.md) — Details on the HIR data structures.
