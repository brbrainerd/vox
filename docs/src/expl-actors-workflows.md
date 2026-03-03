# Actors & Workflows

Vox provides two first-class concurrency primitives: **Actors** for lightweight message-passing and **Workflows** for durable, fault-tolerant execution.

---

## Actors

Actors are isolated processes with their own state and a mailbox for receiving messages. They communicate exclusively via message passing — no shared memory.

### Defining an Actor

```vox
# Skip-Test
actor Counter:
    state count: int = 0

    on increment(amount: int) to int:
        count = count + amount
        count

    on get_count() to int:
        count

    on reset() to Unit:
        count = 0
```

Key concepts:
- **`state`** fields hold mutable internal data
- **`on`** handlers define message responses
- Each handler returns a typed result

### Spawning and Messaging

```vox
# Skip-Test
fn main():
    # spawn() creates a new actor instance, returns a handle (Pid)
    let counter = spawn(Counter)
    let greeter = spawn(Greeter)

    # .send() dispatches a message to the actor's mailbox
    let new_count = counter.send(increment(5))
    let greeting  = greeter.send(greet("Alice"))

    # Actors can receive multiple messages
    let _ = counter.send(increment(3))
    let total = counter.send(get_count())   # returns 8
```

### Messages

Define typed messages for inter-actor communication:

```vox
# Skip-Test
message Greeting:
    from_name: str
    text: str
```

### Durable Actors

Actors can persist state across restarts using `state_load` and `state_save`:

```vox
# Skip-Test
actor PersistentCounter:
    on increment() to int:
        let current = state_load("counter")
        let next = current + 1
        state_save("counter", next)
        ret next
```

This compiles to database-backed state management — the actor's count survives process restarts.

### How Actors Compile

| Vox Concept | Compiled Output (Rust) |
|-------------|----------------------|
| `actor Counter` | Tokio task + `mpsc::channel` mailbox |
| `spawn(Counter)` | `ProcessHandle` via `ProcessRegistry` |
| `counter.send(msg)` | Channel send + optional `oneshot` for reply |
| `state count: int = 0` | Struct field with default |
| `state_load` / `state_save` | Database read/write via `ProcessContext` |

---

## Activities

Activities are retryable units of work that may fail. They are the **only** place for side effects within workflows.

```vox
# Skip-Test
activity fetch_user_data(user_id: str) to Result[str]:
    # Would call an external API in production
    ret Ok("User data for " + user_id)

activity send_notification(email: str, body: str) to Result[bool]:
    # External email service call
    ret Ok(true)
```

Activities must always return a `Result` type, since they represent operations that can fail.

---

## Workflows

Workflows orchestrate activities with **durable execution** guarantees. Each step is recorded so execution survives crashes and restarts.

```vox
# Skip-Test
workflow onboard_user(user_id: str, email: str) to Result[str]:
    # Step 1: Fetch user profile
    let profile = fetch_user_data(user_id) with { retries: 3, timeout: "30s" }

    # Step 2: Send welcome email
    let _ = send_notification(email, "Welcome! " + profile) with { retries: 5, timeout: "60s" }

    # Step 3: Return success
    ret Ok("Onboarding complete for " + user_id)
```

### The `with` Expression

The `with` expression configures retry and timeout policies for activity calls:

| Option | Type | Description |
|--------|------|-------------|
| `retries` | `int` | Maximum retry attempts |
| `timeout` | `str` | Maximum time for the activity (e.g., `"30s"`) |
| `initial_backoff` | `str` | Backoff delay between retries (e.g., `"500ms"`) |
| `activity_id` | `str` | Explicit ID for deduplication |

### Durable Execution

If a process crashes after step 1 completes, the workflow resumes at step 2 when restarted — no work is lost. This is achieved by recording each step's result in a durable store before proceeding.

### How Workflows Compile

| Vox Concept | Compiled Output (Rust) |
|-------------|----------------------|
| `workflow` | State machine with step tracking |
| `activity` | Async function with retry wrapper |
| `with { retries: 3 }` | Retry loop configuration |
| Step completion | Recorded to durable store before proceeding |

---

## Full Example: Order Processing

A complete workflow combining activities with different retry policies:

```vox
# Skip-Test
type OrderResult =
    | Ok(order_id: str)
    | Error(message: str)

activity validate_order(order_data: str) to Result[str]:
    let validated = "validated-" + order_data
    ret Ok(validated)

activity charge_payment(amount: int, card_token: str) to Result[str]:
    let tx = "tx-" + card_token
    ret Ok(tx)

activity send_confirmation(recipient: str, order_id: str) to Result[str]:
    let msg = "Order " + order_id + " confirmed for " + recipient
    ret Ok(msg)

workflow process_order(customer: str, order_data: str, amount: int) to Result[str]:
    # Validate with a short timeout and no retries
    let validated = validate_order(order_data) with { timeout: "5s" }

    # Charge payment with retries and backoff
    let payment = charge_payment(amount, "card-123") with { retries: 3, timeout: "30s", initial_backoff: "500ms" }

    # Send confirmation with basic retry
    let confirmation = send_confirmation(customer, "order-001") with { retries: 2, activity_id: "confirm-order-001" }

    ret confirmation
```

---

## Next Steps

- [Language Guide](ref-language.md) — Full syntax and type system reference
- [Compiler Architecture](expl-architecture.md) — How actors and workflows compile
- [Examples](examples.md) — All example programs with annotations
