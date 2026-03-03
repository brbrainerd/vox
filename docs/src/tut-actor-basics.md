# Tutorial: Actor Basics

Learn how to manage stateful concurrency in Vox using the Actor model. This tutorial covers defining actors, handling messages, and using the `spawn()` function.

## 1. What is an Actor?

In Vox, an **Actor** is a lightweight, stateful unit of concurrency. It has private state and a mailbox for receiving messages. Actors are the primary way to handle concurrent tasks without shared-memory locks.

## 2. Defining an Actor

Use the `@actor` decorator to define an actor. You specify its state in a struct and its message handlers as functions.

```vox
# Skip-Test
@actor type Counter:
    state:
        count: int

    @handler fn increment(amount: int):
        self.state.count += amount
        print("Count is now: " + self.state.count)

    @handler fn get_count() to int:
        ret self.state.count
```

## 3. Spawning Actors

Use the `spawn()` function to create an instance of an actor and get its handle.

```vox
# Skip-Test
@server fn main():
    # Create the actor
    let c = spawn Counter(count: 0)

    # Send a message (non-blocking)
    c.send(increment(5))

    # Call a handler (awaits return value)
    let final_count = c.call(get_count())
```

## 4. Message Semantics

- **`send()`**: Asynchronous, fire-and-forget. The sender does not wait for the actor to process the message.
- **`call()`**: Synchronous request-response. The sender awaits the result of the handler.

## 5. Summary

Actors provide:
- **Isolation**: State is private and only accessible via handlers.
- **Concurrency**: Many actors can run in parallel.
- **Reliability**: Errors in one actor don't necessarily crash the whole system.

---

**Next Steps**:
- [Workflow Durability](tut-workflow-durability.md) — Combine actors with durable execution.
- [UI Integration](tut-ui-integration.md) — Bind actor state to React components.
