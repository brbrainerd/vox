---
title: "Explanation: The Vox Runtime"
description: "Understand the inner workings of the Vox runtime—the engine that powers AI-native, stateful applications."
category: "Concepts"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---
# Explanation: The Vox Runtime

Understand the inner workings of the Vox runtime—the engine that powers AI-native, stateful applications.

## Implementation map

The runtime-facing story in today’s codebase is split across:

- `crates/vox-actor-runtime/src/lib.rs`: actor/process/runtime primitives and exported runtime modules.
- `crates/vox-actor-runtime/src/builtins/mod.rs`: standard builtin implementations used by generated Rust code.
- `crates/vox-compiler/src/codegen_rust/emit/http.rs`: generated Axum app host for routes/server/query/mutation handlers.
- `crates/vox-compiler/src/app_contract.rs`: app-surface contract projection used to keep route/RPC/server config mapping centralized.

## 1. Actor-Based Concurrency and Tokio

At its core, Vox is an actor-based system. Unlike traditional shared-memory concurrency (threads + locks), Vox processes communicate via message passing.

- **Isolation**: Each actor has its own private state.
- **Mailbox**: Messages are queued and processed sequentially, eliminating race conditions by design.
- **Tokio Foundation**: The Vox runtime is built natively on top of the Tokio async runtime, allowing it to take full advantage of Rust's modern asynchronous ecosystem for IO and task scheduling.

## 2. Process Registry and Channels

When Vox code spans actors and sends messages, the compiler lowers these operations to specific Rust primitives:

- **Processes**: Vox actors compile to Tokio tasks running independently.
- **ProcessRegistry**: The runtime tracks running actors using a `ProcessRegistry`, which associates a typed `ProcessHandle` with the underlying Tokio task.
- **mpsc Channels**: Actor mailboxes are implemented using bounded `mpsc::channel` structures. Backpressure is naturally handled by the channel bounds.
- **Replies**: When an actor expects a return value (like `.send()`), an inner `oneshot` channel is used to cleanly route the response back to the caller.

## 3. Technical Unification

Vox achieves "Technical Unification" by abstracting the boundary between frontend and backend.

- **RPC-as-Function**: Calling an `@endpoint` fn from a `component` looks like a local function call but is actually a type-safe API call generated into the UI layer via `vox-client.ts`.
- **State Synchronization**: Backend state updates interact directly with the client code through standard HTTP routes built on top of Axum, managed under the hood by the compiler's output.

## 4. Workflows and Journaling

While actors handle live state and passing messages, **Workflows** provide durability for orchestration tasks. The runtime provides a secondary interpreted path for `vox mens workflow ...` executions that allows for persistent step journaling. In standard compiled operation, workflows act as normal async functions coordinating `Result`-returning activities.

---

**Related Reference**:
- [Actors & Workflows Explanation](expl-actors-workflows.md) — Dive deeper into the runtime behavior of actors and workflows.
- [Language Reference](../reference/ref-syntax.md) — The core syntax for actors and state.
