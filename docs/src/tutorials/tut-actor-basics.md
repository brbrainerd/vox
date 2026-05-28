---
title: "Tutorial: Persistent Actors & State"
description: "Master stateful concurrency in Vox. Learn to define, spawn, and persist actor state across system restarts."
category: "Tutorials"
sort_order: 4
last_updated: "2026-04-26"
training_eligible: true

schema_type: "HowTo"
---

# Tutorial: Persistent Actors & State

In Vox, **Actors** are the primary unit of stateful concurrency. Unlike standard functions, an actor has **identity** and **private state**. This tutorial walks through building a persistent counter that survives a system crash.

## 1. Defining the Actor

An actor is defined with the `actor` keyword. Its internal state is private and only accessible via message handlers.

```vox
{{#include ../../../examples/golden/ref_actors.vox:basic_actor}}
```

## 2. Spawning and Identity

To use an actor, you must **spawn** it. This returns an `ActorRef`, which acts as a capability to send messages.

To use an actor, you must **spawn** it. This returns an `ActorRef`, which acts as a capability to send messages.

```vox
fn GlobalCounter_Increment(cur: int, amount: int) to int {
    return cur + amount
}

fn GlobalCounter_Get(cur: int) to int {
    return cur
}

fn demo_actors() to int {
    // Increment the counter by 5
    let next = GlobalCounter_Increment(0, 5)

    // Retrieve the current value
    let val = GlobalCounter_Get(next)

    return val
}
```

## 3. The Lifecycle: Persistence in Action

Vox actors are not just in-memory. By using `state_load` and `state_save`, you tie the actor's life to the **durable runtime**.

1. **Spawn**: The actor is created in the runtime's mailbox registry.
2. **Handle**: A message arrives, `state_load` pulls the latest value from the local SQLite/Codex store.
3. **Save**: `state_save` ensures that even if you `kill -9` the process, the value is safe.
4. **Restart**: When the process resumes and the actor is re-spawned or addressed by its stable ID, it picks up exactly where it left off.

## 4. Patterns: Actor Communication

Actors can talk to each other. Because each actor has its own mailbox, they process messages **sequentially** but run in **parallel** with other actors.

```vox
fn LoggerActor_Log(msg: str) {
    print("[LOG]: " + msg)
}

fn WorkerActor_DoWork() {
    // Delegate logging to another actor
    LoggerActor_Log("Starting work...")
}
```

## 5. Behind the Scenes: How Actors Compile

When you run `vox build`, the compiler lowers actor constructs directly into high-performance Rust primitives:

| Vox Construct | Compiled Rust Equivalent |
| :--- | :--- |
| `actor X` | `struct X` + `enum XMessage` + `async fn run(mailbox)` |
| `state count: int` | Struct field in the actor's private state struct |
| `spawn X()` | `tokio::spawn` + `mpsc::channel` creation |
| `ref.send msg()` | `mpsc::Sender::send` (fire and forget) |
| `await ref.get()` | `oneshot::channel` + `mpsc::send` (request/reply) |
| `state_load(key)` | `Codex::get_actor_state(actor_id, key)` |
| `state_save(key, v)` | `Codex::put_actor_state(actor_id, key, v)` |

## 6. Summary Checklist

- [x] **Isolation**: State is never shared; only messages pass between actors.
- [x] **Persistence**: Use `state_load`/`state_save` for durable state.
- [x] **Concurrency**: Use `spawn` to create independent units of work.
- [x] **Non-blocking**: Use `send` for asynchronous notification.
- [x] **Request-Response**: Use `await ref.handler()` for synchronous calls.

---

**Next Steps**:
- [Workflow Durability](tut-workflow-durability.md) — Orchestrate complex, multi-step long-running processes.
- [Actors & Workflows Explanation](../explanation/expl-actors-workflows.md) — Deep dive into the theory.
- [CLI Reference: vox run](../reference/cli.md#vox-run-file----args) — Run your actor-based applications.


