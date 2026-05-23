---
title: "Tutorial: Workflow Durability"
description: "Learn how to build resilient, long-running processes using Vox workflows."
category: "Tutorials"
status: "current"
sort_order: 6
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---
# Tutorial: Workflow Durability

Learn how to build resilient, long-running processes using Vox workflows. This tutorial explains the durability story Vox supports today: interpreted workflow step replay, stable activity ids, and idempotent activities.

> [!WARNING]
> Interpreted workflow runtime durability and generated-Rust workflow durability are different things. The durable replay and recovery story shown here uses the interpreted path (`vox mens workflow ...`), not compiled native async functions.

## 1. The Challenge of Long-Running Tasks

Traditional async functions lose their state if the server restarts or a network error occurs. Vox workflows are intended to solve that by recording progress in a database.

## 2. Defining a Workflow

Use the bare `activity` and `workflow` keywords to describe long-running orchestration. 

Use the bare `activity` and `workflow` keywords to describe long-running orchestration. 

```vox
{{#include ../../../examples/golden/getting_started.vox:logic}}
```

The `with` block provides execution options for the activity:
- `retries`: Number of attempts before failing the workflow step
- `timeout`: Maximum duration allowed for a single execution
- `initial_backoff`: Delay before the first retry attempt

## 3. How It Works

1. **Step tracking**: The interpreted runtime records activity progress in Codex workflow tracking tables.
2. **Recovery**: If the workflow is restarted with the same run identity, the runtime skips steps that completed successfully by reading their result from the journal.
3. **Idempotency**: Activities should still be safe to retry on timeout or failure. Durable step replay is not the same thing as a universal exactly-once guarantee.

## 4. Workflows vs. Tasks

| Feature | Regular Task | Vox Workflow |
|---------|--------------|--------------|
| Survival | Dies on reboot | Interpreted workflow runtime resumes steps |
| Retry | Manual `try/catch`| `with { retries }` support |
| State | In-memory | Durable step tracking |

## 5. Best Practices

- **Idempotency**: Activities should be idempotent since they might be retried after a failure.
- **Deterministic**: Workflow logic must be deterministic. Avoid using `rand()` directly inside the workflow body; use an activity instead.
- **Stable step ids**: Use explicit `activity_id` values for steps you expect to resume safely across restarts. `with { id: "..." }` sets this.

---

**Next Steps**:
- [Language Syntax](../reference/ref-syntax.md) — Explore advanced workflow expressions.
- [First App](tut-first-app.md) — Integrate a workflow into your task list.
