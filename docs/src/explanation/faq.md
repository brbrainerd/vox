---
title: "Vox FAQ: Frequently Asked Questions"
description: "Answers to common questions about Vox, its current architecture, generated outputs, MCP support, and Mens training lanes."
category: "Concepts"
status: "current"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "FAQPage"
keywords: ["Vox FAQ", "Vox language questions", "AI native language runtime", "null safety programming"]
---

# Vox Frequently Asked Questions (FAQ)

This page answers product and architecture questions.

For operational fixes, environment issues, or command failures, use the [Troubleshooting FAQ](../how-to/troubleshooting-faq.md).

## Language Basics

### What is Vox?
Vox is a full-stack programming language and toolchain that aims to keep more of the application structure in one place. The current repository documents a compiler and CLI that generate Rust and TypeScript artifacts, plus a wider ecosystem of orchestration, MCP, and Mens-related tooling.

### Is Vox statically typed?
Yes. Vox uses bidirectional type inference: you rarely need explicit types inside function bodies, but all signatures are validated at compile time.

### How does Vox handle null?
Null is completely banned. Absent values use `Option[T]` (`Some(value)` or `None`); fallible operations use `Result[T, E]` (`Ok(value)` or `Error(e)`). Both must be explicitly handled — the compiler rejects unhandled cases. See [Type System Reference](../reference/ref-type-system.md) for details.

## Installation & Toolchain

### How do I install and update Vox?
Build from source with `cargo install --locked --path crates/vox-cli`.

To discover what your installed binary actually supports, run `vox commands --recommended` and `vox commands --format json --include-nested`. The docs intentionally distinguish between the current compiled CLI surface and broader workspace capabilities.

### What does `vox build` do?
`vox build` lexes, parses, and type-checks your `.vox` file, then generates Rust and TypeScript output.  
Why use it: it gives you a deterministic compile artifact you can inspect before running or bundling.

### Can I use existing Rust or NPM libraries?
Yes. Use `import rust:<crate>` (for example `import rust:serde_json as json`) for Rust crates and standard NPM imports in frontend blocks.

## Architecture & Runtime

- **Actor** — a stateful unit of concurrency with a private mailbox. Processes one message at a time; no shared-state races.
- **Workflow** — a long-running orchestration construct. Today, the interpreted workflow runtime provides the repo's durable step-replay path, while generated Rust workflows are not yet full durable state machines (see [ADR-021](../adr/021-generated-workflow-durability-parity.md)).

### What is the Mens?
In current repo language, **Mens** refers to the model-training lane and local model generation pipeline, while **Populi / mesh** refers to coordination, inference serving, and distributed execution surfaces. Older docs sometimes used the terms loosely; newer docs keep those lanes separate.

### What is the difference between `activity` and `workflow`?
A **workflow** is an overarching orchestrator that tracks progress durably across steps, whereas an **activity** is an individual, retryable unit of work that performs side effects (like an API call). Workflows run activities but are not meant to contain side effects directly.

### What about `@island`?
`@island` was retired on 2026-05-03. Vox now compiles `component` declarations to plain React/TSX and `@endpoint` declarations to a generated `vox-client.ts`; an external React app imports the components or calls the endpoints over RPC. See [external-frontend-interop-plan-2026](../architecture/external-frontend-interop-plan-2026.md). Sources containing `@island` will be a hard parser error.

### What is `Codex` and how does it relate to SQLite?
**Codex** is the logical data environment — the unified data and knowledge store in Vox that application code interacts with. It acts as a high-level facade over **Arca**, which handles the actual physical storage (SQLite/Turso layer under the hood).

### How is Vox different from Go or Erlang/Elixir?
Vox is opinionated about generated outputs, durable workflows, and keeping more application structure in one language. Its design language overlaps with actor and workflow systems, but the repo also includes code generation, contracts, and web-facing lanes that are not trying to be a drop-in clone of Go or Erlang/Elixir.

## AI & ML Integration

### How does Vox support AI agents?
The repo has native [Model Context Protocol (MCP)](https://modelcontextprotocol.io) integration and a growing set of tool-registry contracts. In the current documentation set, the canonical sources are the MCP registry contract pages and the `vox-mcp` workspace surfaces, not older duplicate reference tables.

### What is Mens, and how do I fine-tune a model?
Mens is the repo's native model-training lane. The current default production mix is still code-oriented; documentation prose extraction exists, but architecture Q&A is not the default training objective today.

For the canonical training entrypoint:

```bash
vox mens train --backend qlora
```

See [Mens native training SSOT](../reference/mens-training.md), [Mens training data contract](../reference/mens-training-data-contract.md), and [How To: Train Mens Models](../how-to/how-to-train-mens-4080.md).

### What is the Socrates Protocol?
An orchestration-layer reasoning protocol (SOP). Before generating or approving code, Vox uses structural prompts to force the underlying LLM to evaluate confidence and structure its reasoning via the MCP control plane.

## Deployment & Community

### How do I deploy a Vox app?
Deployment surfaces exist, but they are not all equivalent in maturity. Treat the deployment and portability docs as the current source of truth for the lane you are using rather than assuming every repo path is equally production-ready.

### Is Vox open source? How do I contribute?
Yes, Apache-2.0 licensed. Start with the [Contributor hub](../contributors/contributor-hub.md), follow `STYLE.md`, and use the relevant `vox ci` guards for the area you changed.


