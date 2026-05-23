---
title: "Actor GC Implementation Blueprint"
description: "Detailed operational blueprint for bridging actor boundaries in the Vox Rust-lowering compiler to support Per-Actor Garbage Collection, isolating LLM code generation from strict borrow-checking."
category: "Architecture SSOTs"
status: "roadmap"
sort_order: 11
last_updated: "2026-04-16"
training_eligible: false
training_rationale: "Maps the high-level GC concepts down to precise crate, struct, and module mutations within vox-runtime and vox-tensor."
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Actor GC Implementation Blueprint (2026)

This blueprint documents the explicit implementation strategy for bridging actor-boundaries inside the Rust-lowered Vox backend to support the **Per-Actor Garbage Collector**. By executing this blueprint, Vox abstracts memory management into localized, single-threaded arenas, reducing K-Complexity for LLM generators while safely bypassing Rust's strict lifetime bounds.

## Phase 1: Local Arena Allocation (`vox-runtime/src/gc.rs`)

We must introduce an `ActorHeap` representing a thread-local or context-local memory arena.

1. **Create the `vox-gc` Module**
   Create `crates/vox-runtime/src/gc.rs` containing the `ActorHeap` struct. This arena acts as a localized semi-space copying collector or mark-and-sweep bump allocator.
   
2. **Define explicit `!Send` / `!Sync` Pointers**
   Design the `Gc<T>` smart pointer. It must strictly **not** implement `Send` or `Sync`. This utilizes Rust's core borrow checker to formally guarantee that no actor can accidentally send a GC pointer through a `tokio::sync::mpsc` mailbox, effectively trapping the memory within the actor's boundaries.

## Phase 2: Intercepting the Execution Context (`vox-runtime/src/process.rs`)

Actors execute via a standard Tokio `JoinHandle`. We must inject the local heap into the execution context.

1. **Embed the Heap:** Add `pub heap: ActorHeap` to the `ProcessContext` struct.
2. **Schedule GC Sweeps:** Embed collection sweeps inside `ProcessContext::receive()`. Just before the actor yields cooperatively to Tokio (`tokio::task::yield_now()`), check allocation thresholds: `if self.heap.should_collect() { self.heap.collect(); }`. This utilizes the natural pauses in actor scheduling to hide GC latency.

## Phase 3: Bridge the Actor Mailbox Boundary (`vox-runtime/src/mailbox.rs`)

Because `Gc<T>` cannot be sent across a `tokio` channel, we must serialize passing data.

1. **Implement `DeepCloneToOwned`**
   The `vox-compiler` must derive a `DeepCloneToOwned` trait for lowered structs. When an actor sends a message containing a `Gc<String>`, the compiler invokes a complete deep-copy of the string into an owned `String` allocation.
2. **Recipient Re-Allocation**
   When the recipient actor pulls the `String` from its mailbox, the lowered code immediately re-allocates it into its own local `ActorHeap`.

## Phase 4: Compute/Math Handling (`vox-tensor`)

The integration of `vox-tensor` natively wraps `burn::tensor`, meaning the tensors hold GPU-backed memory handles rather than internal Vec buffers. A GC should not move or manage megabytes of raw tensor data.

1. **Unmanaged Compute Resources:** 
   `Tensor<B>` handles will be excluded from the `ActorHeap` byte-tracking. We will manage memory references using `Gc<Tensor<B>>`, where the GC only tracks the pointer handle.
2. **Handle Drop Hooks:** 
   When the `ActorHeap` clears a `Gc<Tensor>`, it must correctly invoke `.drop()` on the tensor so WGPU/CUDA frees the hardware memory. 
3. **Actor Boundary Transit:** 
   Mathematical operations (`matmul`, `add`, `sub` in `crates/vox-tensor/src/tensor/elemwise.rs`) do not change. When passing a Tensor to another Actor, we do not deep-copy the matrix. The internal `burn` backend naturally supports cross-thread RC pointers. The sending actor invokes `tensor.clone()` (which only increments the GPU reference count) and sends the `burn` handle across the mailbox.

## Sequence / Summary of Execution
The implementation sequence traces from the core data structures outward to the compiler interface:
1. `vox-runtime/src/gc.rs`: `ActorHeap` and `Gc<T>`
2. `vox-runtime/src/process.rs`: Context integration and yielding
3. `vox-runtime/src/mailbox.rs`: `DeepCloneToOwned` serialization
4. `vox-tensor/src/tensor/tensor.rs`: RAII unmanaged handle integration
5. `vox-compiler/src/codegen.rs`: Re-routing AST lowering to `ctx.heap.allocate(...)`


