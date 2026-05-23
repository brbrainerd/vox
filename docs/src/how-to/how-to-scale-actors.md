---
title: "How-To: Scale Actors"
description: "Strategies for distributing and managing Vox actor lifecycles across a cluster."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-26"
training_eligible: true

schema_type: "HowTo"
---

# How-To: Scale Actors

As your application grows beyond a single executable, Vox Actors must scale horizontally across the Populi mesh or large orchestrated deployments.

## The Concept of Actor Affinity

By default, an initialized Actor runs in memory on the node where `spawn` was invoked. In a distributed environment, you rely on the **Codex** to synchronize and persist state securely.

```vox
// vox:skip — state_load/state_save are runtime builtins not yet visible to the parser
fn SessionManager_Login(user: str) to Result[str] {
    let current_sessions = state_load("active_users")
    // logic ...
    state_save("active_users", current_sessions)
    return Ok("Success")
}
```

Because `state_save` natively pushes updates to Codex, another node starting a `SessionManager` actor targeting the same specific state scope can seamlessly resume operations. 

## Load Balancing and Populi

When scaling the inference compute or orchestration logic via Populi Meshes, Vox abstracts message routing.

1. **Local Node Execution**: Functions run via Tokio threads in the core binary.
2. **Distributed GPU Execution**: LLM evaluation or heavy compute tasks explicitly placed on GPU labeled nodes. 

To dispatch an orchestration task externally, the framework determines placement inherently via the resource requests. 

> [!WARNING]
> Manual remote procedure calls (RPC) -> force specific Actor placement remains in active development. As of v0.3, horizontal scaling predominantly operates seamlessly behind standard `routes { }` load-balancing and Turso replicated databases, rather than direct point-to-point remote actor message passing.

## Actor Naming and Discovery

By default, `spawn` produces a random anonymous identity. For singleton services or discoverable workers, you can provide a stable name. 

Stable names allow the system to route messages to the correct instance across a cluster and ensure that only one instance of that specific actor exists.

```vox
let session_key = "user_session_" + user_id
let result = SessionManager_Login(session_key)
```

## Lifecycle and Restart Behavior

Actors in Vox are designed for "Let it Crash" reliability. If an actor panics or its host node fails:

1. **Detection**: The Process Registry (Codex) detects the heartbeat failure.
2. **Re-hydration**: The actor is re-spawned on a healthy node.
3. **Recovery**: The new instance calls `state_load`. Since `state_save` was persistent, no data is lost.
4. **Resumption**: Message ordering is guaranteed; pending messages in the durable mailbox are redelivered to the new instance.

---

## Best Practices for Scale

- **Prefer Workflows**: For long-running business logic, `workflow` is safer than a long-lived actor because it provides step-level journaling.
- **Stateless handlers**: Keep actor handlers as pure as possible between `state_load` and `state_save`.
- **Avoid Large State**: Keep actor state small (under 1MB) to ensure rapid re-hydration across nodes.
