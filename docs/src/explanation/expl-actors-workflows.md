---
title: "Actors & Workflows"
description: "Deep dive into Vox's primary concurrency primitives: stateful actor functions for message-passing and durable workflow functions for reliable orchestration."
category: "Concepts"
last_updated: "2026-04-26"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---
# Actors & Workflows

Vox provides two first-class concurrency primitives: **Actors** for lightweight message-passing and **Workflows** for orchestrating activities. Both are expressed as standard `fn` declarations — the runtime provides the mailbox dispatch, journaling, and replay infrastructure automatically.

---

## Actors

Actors are isolated processes with their own state and a mailbox for receiving messages. They communicate exclusively via message passing — no shared memory.

### Defining an Actor

An actor is modeled as one function per handler. The naming convention `ActorName_HandlerName` makes handlers discoverable and groups them semantically:

```vox
fn CounterActor_Increment(current: int, amount: int) to int {
    return current + amount
}

fn CounterActor_GetCount(current: int) to int {
    return current
}

fn CounterActor_Reset() to int {
    return 0
}
```

Key concepts:
- Each handler takes current state as the first parameter and returns the new state
- The runtime maintains the mailbox and dispatches messages to the appropriate handler
- No shared memory — state flows through parameters and return values

### Messages

Define typed messages for inter-actor communication using ADTs:

```vox
type CounterMsg =
    | Increment(amount: int)
    | GetCount
    | Reset
```

### Durable Actors

State persistence is handled by the interpreted runtime (ADR-019). The function receives the last-known state and returns the next state — the runtime checkpoints it automatically.

```vox
fn PersistentCounter_Increment(current: int) to int {
    return current + 1
}
```

This pattern compiles to database-backed state management — the actor's count survives process restarts because the runtime journals the state after each handler invocation.

### How Actors Compile

| Vox Concept | Compiled Output (Rust) |
|-------------|----------------------|
| `fn ActorName_Handler(state: T, ...) to T` | Tokio task + `mpsc::channel` mailbox |
| Actor spawn (runtime) | `ProcessHandle` via `ProcessRegistry` |
| Message send (runtime) | Channel send + optional `oneshot` for reply |
| State parameter | Struct field with default, checkpointed by runtime |

---

## Activities

Activities are retryable units of work that may fail. They are the recommended place for side effects within workflows.

```vox
fn fetch_user_data(user_id: str) to Result[str] {
    return Ok("User data for " + user_id)
}

fn send_notification(email: str, body: str) to Result[bool] {
    return Ok(true)
}
```

Activities must always return a `Result` type, since they represent operations that can fail.

---

## Quick Comparison

| Concept | Pattern | Survival | State |
|---|---|---|---|
| Actor | `fn ActorName_Handler(state: T, ...) to T` | Lives in memory; revive with same ID | Runtime checkpoints return value |
| Workflow | `fn workflow_name(...) to Result[T]` | Interpreted runtime can replay completed steps | Journal in Codex |
| Activity | `fn activity_name(...) to Result[T]` | Individual retryable step within a workflow | None (idempotent) |

---

## Workflows

Workflows orchestrate activities with retry and journaling intent. A workflow is a plain function — the runtime provides durability.

Current state:

- **Implemented semantics:** function-based workflow pattern, `with { ... }` parsing/typechecking, generated async Rust functions, interpreted workflow planning/journaling, stored step-result replay, and retry/backoff for interpreted `mesh_*` activities.
- **Planned semantics:** full durable state-machine execution for the generated Rust path and richer replay models for branching/loops.
- **Escape hatch / current durable path:** the interpreted workflow runtime used by `vox mens workflow ...`.

```vox
// vox:skip — `return` in match arm body is a parser limitation; illustrative only
fn onboard_user(user_id: str, email: str) to Result[str] {
    let profile = fetch_user_data(user_id)
    match profile {
        Error(msg) => return Error("profile fetch failed: " + msg)
        Ok(data) => {
            let notif = send_notification(email, "Welcome! " + data)
            match notif {
                Error(msg) => return Error("notification failed: " + msg)
                Ok(_) => return Ok("Onboarding complete for " + user_id)
            }
        }
    }
}
```

### The `with` Expression

The `with` expression carries workflow activity options when calling activities through the interpreted runtime. Some are honored today, while others only matter on specific runtime paths:

| Option | Type | Description |
|--------|------|-------------|
| `retries` | `int` | Honored for interpreted `mesh_*` activity execution |
| `timeout` | `str` | Parsed for interpreted runtime activity planning |
| `initial_backoff` | `str` | Honored for interpreted `mesh_*` retries |
| `activity_id` | `str` | Explicit durable/journal key |
| `id` | `str` | Alias for `activity_id` |
| `mens` | `str` | Mesh control override for interpreted `mesh_*` activities |

### Durable Execution

The interpreted workflow runtime can skip previously completed activities when restarted with the same workflow, run id, and activity ids because it records journal/tracker data before replay and stores step result payloads for linear replay. Generated Rust workflows do **not** yet compile into a durable state machine.

**Durable spine (today):** the supported replay/idempotency story is the interpreted `vox mens workflow …` runtime (see [ADR-019](../adr/019-durable-workflow-journal-contract-v1.md)). Rust-emitted `async fn` workflows are orchestration helpers only until generated code adopts the same journaling contract. Generated-workflow parity remains intentionally out of scope until Vox has a formal replay model and ADR for it (see [ADR-021](../adr/021-generated-workflow-durability-parity.md)).

### How Workflows Compile

| Vox Concept | Current generated / runtime behavior |
|-------------|------------------------------------|
| `fn workflow_name(...)` | Generated as a plain `async fn` in Rust codegen |
| `fn activity_name(...)` | Generated as a plain `async fn`; `with` lowering adds helper wiring |
| `with { retries: 3 }` | Interpreted runtime honors it for `mesh_*` activity execution |
| Step completion | Interpreted runtime journals versioned events and stores replayable step results |

---

## Full Example: Order Processing

A complete workflow combining activities with different retry policies:

```vox
// vox:skip — `return` in match arm body is a parser limitation; illustrative only
type OrderResult =
    | OrderOk(order_id: str)
    | OrderError(message: str)

fn validate_order(order_data: str) to Result[str] {
    let validated = "validated-" + order_data
    return Ok(validated)
}

fn charge_payment(amount: int, card_token: str) to Result[str] {
    let tx = "tx-" + card_token
    return Ok(tx)
}

fn send_confirmation(recipient: str, order_id: str) to Result[str] {
    let msg = "Order " + order_id + " confirmed for " + recipient
    return Ok(msg)
}

fn process_order(customer: str, order_data: str, amount: int) to Result[str] {
    let validated = validate_order(order_data)
    match validated {
        Error(msg) => return Error(msg)
        Ok(_) => {
            let payment = charge_payment(amount, "card-123")
            match payment {
                Error(msg) => return Error(msg)
                Ok(tx) => send_confirmation(customer, tx)
            }
        }
    }
}
```

---

## Next Steps

- [Language Reference](../reference/ref-syntax.md) — Full syntax and type system reference
- [Compiler Architecture](expl-architecture.md) — How actors and workflows compile

## Durability Taxonomy

Understanding the types of durability is crucial when reasoning about failure recovery in Vox:

1. **Persistent Actors** (runtime checkpointing):
   State survives restarts because the runtime checkpoints the return value of each handler. When the actor respawns, it resumes with the last saved state.
2. **Workflow Durability** (Interpreted Runtime):
   When running via `vox run` or `vox mens` workflow, the engine tracks execution steps natively in the database. If the process dies and restarts, completed activities are short-circuited.
3. **Compiled Rust Workflows** (Future Parity):
   Workflows that are compiled strictly down to standard Rust async equivalents do not automatically benefit from step-level replayable durability yet. This remains an active implementation target for parity with the interpreted path (see ADR-021).
