---
title: "Explanation: The Vox Memory Model"
description: "How Vox manages memory — pure value semantics with copy-on-write, no garbage collector, and no manual memory management — and what that means per target."
category: "Concepts"
status: "current"
training_eligible: true
---

# Explanation: The Vox Memory Model

## The short version

**Vox has no garbage collector, and you never manage memory by hand.** There is no `malloc`/`free`, no
pointers, no references, no lifetimes in Vox source. Instead, Vox has **pure value semantics**: every value
behaves as if assignment makes an independent copy. Because nothing is shared by reference, *reference cycles
cannot form* — which is exactly the situation a garbage collector exists to clean up. So Vox doesn't need one.

```vox
// vox:skip (illustrative)
let a = [1, 2, 3]
let b = a        // `b` is an independent value
a.push(4)        // mutating `a` does NOT change `b`
// a == [1,2,3,4], b == [1,2,3]
```

## Value semantics, implemented by copy-on-write

Conceptually every assignment copies. Doing that *literally* would be slow — deep-copying a large list on
every pass-by-value. So the interpreter implements value semantics with **copy-on-write (CoW)**:

- Cloning a value (assignment, passing an argument, returning) is an **O(1) reference-count bump**, not a deep
  copy. Internally, collections (`list`, object, tuple), scope frames, and closure bodies are shared via
  `Rc`.
- A value is **deep-copied only at the moment it is mutated *while shared*** (`Rc::make_mut`). If a binding is
  the sole owner, the mutation happens in place.

The observable behavior is identical to "copy on every assignment" — two bindings to the same list are always
independent — but the cost is paid only when it's actually needed. This is the same strategy Clojure and
Swift use. It gives you the safety of value semantics with the speed of sharing, and **no collector and no GC
pauses**.

You never see any of this. There is no `Rc`, no `clone`, no borrow checker in Vox source — it's an
implementation detail of the runtime.

## No shared mutable state; concurrency is actors

Vox deliberately **outlaws shared mutable state**. Concurrency is expressed with actors that own their state
and communicate by message passing (mailboxes), not by sharing memory. This is why value semantics compose
cleanly with concurrency: there is no aliased mutable data to race on. (Actor message payloads are a separate,
serialized wire type — they are not the in-process value type.)

## Memory management is inherited from each target

Vox compiles from one semantic core to several targets, and each target's runtime manages memory in its own
native way. You don't choose or configure this:

| Target | Memory managed by |
|---|---|
| Interpreter (`vox run --interp`) | Rust ownership / `Drop` over the runtime value type |
| Compiled script / server / desktop (Rust, Axum, Tauri) | Rust ownership |
| Web & React Native (emitted TypeScript) | **The JavaScript engine's garbage collector** (V8 / Hermes / JSC) |
| WASI script | Rust allocator over WebAssembly linear memory |

So it isn't true that "Vox only works on Rust memory": when Vox emits web or mobile code, your program rides
the JavaScript engine's GC. Value semantics make the choice of collector invisible to you.

## Why not a garbage collector?

A global tracing GC would add stop-the-world pauses and significant runtime complexity, and it would conflict
with the actor isolation model — all to solve a problem (reference cycles) that Vox's value semantics prevent
from existing. The deliberate choice is **structural sharing, not collection**: keep pure value semantics,
make copies cheap with CoW, and let each target's native runtime reclaim memory deterministically.

## What this means for you

- **You never write memory-management code.** No allocation, no freeing, no ownership annotations.
- **No GC pauses** in the interpreter or Rust-compiled targets; reclamation is deterministic.
- **Mutation is local.** Changing a value never surprises another binding that "had" it.
- **Performance is predictable.** Passing big values around is cheap; the only place a copy happens is when you
  mutate a value that's currently shared.

### Implementation pointers (for contributors)

The runtime value type `VoxValue` lives in `crates/vox-compiler/src/eval/value.rs`
(`List`/`Object`/`Tuple` hold `Rc<Vec<…>>`; `Fn.body` is `Rc`), and scopes live in
`crates/vox-compiler/src/eval/env.rs` (`Rc`-shared frames). `VoxValue` is intentionally `!Send` (a
compile-time tripwire enforces it), since the interpreter is single-threaded. The design rationale, audit, and
benchmarks are in
[`vox-memory-model-audit-and-value-optimization-2026-06-05.md`](../architecture/vox-memory-model-audit-and-value-optimization-2026-06-05.md).
