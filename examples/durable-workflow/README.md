# Durable Workflow Example

This example showcases Vox's **durable execution model** — a workflow framework where activity calls are automatically retried, replayed on crash, and journaled for exactly-once semantics.

---

## What It Demonstrates

| Feature | Syntax | Semantics |
|---------|--------|-----------|
| Activity declaration | `activity fn(...) to Result[T]` | Retriable, isolated unit of work |
| Workflow declaration | `workflow fn(...) to Result[T]` | Orchestrator with at-least-once guarantees |
| Retry policy | `with { retries: 3 }` | Exponential backoff, per-call |
| Timeout per attempt | `with { timeout: "30s" }` | Cancels single attempt, not retry chain |
| Initial backoff | `with { initial_backoff: "500ms" }` | Base delay before first retry |
| Idempotent activity | `with { activity_id: "unique-id" }` | Memoizes result — safe on crash replay |

---

## Order of Execution

```
process_order
 ├── validate_order        (timeout: 5s, no retries — fast-fail)
 ├── charge_payment        (retries: 3, timeout: 30s, backoff: 500ms, idempotent)
 ├── send_confirmation     (retries: 2, timeout: 15s, idempotent)
 └── record_audit_log      (timeout: 5s, best-effort)
```

---

## Crash Recovery

If the process crashes after `charge_payment` succeeds but before `send_confirmation` runs:

1. Workflow journal is replayed from the last successful step.
2. `charge_payment` is **not re-executed** — its result is loaded from the journal via `activity_id`.
3. `send_confirmation` runs from scratch (with retries if needed).

This prevents double-charges even if the process crashes in the middle.

---

## Running the Example

```bash
# Parse and type-check the workflow
vox check examples/durable-workflow/src/main.vox

# Run the workflow (stub runtime — shows activity graph)
vox workflow run examples/durable-workflow/src/main.vox process_order

# Inspect the workflow definition
vox workflow inspect examples/durable-workflow/src/main.vox process_order
```

---

## Testing Failure Scenarios

To test retry behavior, modify `charge_payment` to simulate failures:

```vox
activity charge_payment(amount: int, card_token: str) to Result[str]:
    # Simulate a transient failure (remove in production)
    ret Error("Payment gateway timeout")
```

With `retries: 3`, the runtime will attempt this 4 times total (1 initial + 3 retries)
before propagating the error to the workflow.

---

## `with` Clause Reference

```vox
some_activity(args) with {
    retries: 3,           # Number of retry attempts after initial failure
    timeout: "30s",       # Timeout per single attempt (s = seconds, m = minutes)
    initial_backoff: "500ms",  # Base delay before retry 1 (doubles each attempt)
    activity_id: "unique-stable-id",  # Enables idempotent replay on workflow restart
}
```
