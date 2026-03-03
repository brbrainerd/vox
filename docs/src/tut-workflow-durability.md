# Tutorial: Workflow Durability

Learn how to build resilient, long-running processes using Vox Workflows. This tutorial covers checkpointing, state recovery, and durable async operations.

## 1. The Challenge of Long-Running Tasks

Traditional async functions lose their state if the server restarts or a network error occurs. **Workflows** solve this by automatically journaling every step to persistent storage.

## 2. Defining a Workflow

Use the `@workflow` decorator. Inside a workflow, calls to `@activity` functions are automatically checkpointed.

```vox
# Skip-Test
@activity fn process_payment(amount: int) to Result[bool]:
    # This might fail or time out
    ret db.execute("UPDATE accounts...")

@workflow fn order_fulfillment(order_id: str):
    # This step is durable
    let success = process_payment(100)

    if success:
        print("Order " + order_id + " fulfilled!")
```

## 3. How It Works

1. **Journaling**: Every time an activity completes, Vox saves the result to a hidden `_vox_journal` table.
2. **Recovery**: If the process crashes, Vox restarts the workflow and "replays" the journal to skip steps that already finished.
3. **Exactly-Once**: Activities are guaranteed to run only once per successful workflow execution.

## 4. Workflows vs. Tasks

| Feature | Regular Task | Vox Workflow |
|---------|--------------|--------------|
| Survival | Dies on reboot | Resumes on reboot |
| Retry | Manual `try/catch`| Automatic built-in retries |
| State | In-memory | Persisted at every step |

## 5. Best Practices

- **Idempotency**: Activities should be idempotent since they might be retried after a crash.
- **Deterministic**: Workflow logic must be deterministic. Avoid using `rand()` or `Date.now()` directly inside the workflow body; use an activity instead.

---

**Next Steps**:
- [Language Reference](ref-language.md) — Explore advanced workflow decorators.
- [First App](tut-first-app.md) — Integrate a workflow into your todo list.
