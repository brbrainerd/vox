---
title: "Rosetta Inventory: One Scenario, Four Languages"
description: "The same inventory merge across C++, Rust, Python, and Vox — not to show who wins, but to show what each language reveals about the problem."
category: "Concepts"
last_updated: "2026-04-14"
status: "current"
training_eligible: true
keywords: [
  "C++ iterator invalidation", "Python mutable default argument",
  "Rust Arc Mutex concurrency", "actor mailbox pattern",
  "Vox vs Python", "Vox vs Rust", "AI-native language comparison",
  "durable execution idempotency", "inventory stack merge"
]
schema_type: "TechArticle"
---

# Rosetta Inventory: One Scenario, Four Languages

Here is the same task in four languages: merge a stack of potions into a backpack inventory. The goal is not to rank languages, but to see how each language's model of the world frames the problem.

C++ reveals the dangers of container mutation. Rust reveals the architectural cost of thread-safety. Python reveals the silent sharing of state across time. Vox demonstrates a runtime where pure functions grow into concurrent, durable, agent-ready primitives without changing tools or schemas.

## The Scenario

A player with an inventory tries to drag a stack of Potions onto an existing stack of Potions.

| Parameter | Value |
|---|---|
| Hand | `Potion x6` |
| Slot Data | `Potion x7 (max 10)` |
| Expected Output | Primary slot becomes 10. Remaining 3 goes to overflow slot or stays in hand. |
| Edge Case 1 | Merging `Sword ` into `Potion` must be rejected. |
| Edge Case 2 | Max capacity ≤ 0 must be rejected. |

### Navigation: If You're Coming From...

- **C++**: your section is first, then jump to [Vox: Types and Pure Functions](#iv1-types-and-pure-functions) to see where that goes.
- **Rust**: your section is second, then jump to [Vox: Concurrency Without a Locking Protocol](#iv3-concurrency-without-a-locking-protocol) to see the direct response.
- **Python**: your section is third, then jump to [Vox: Types and Pure Functions](#iv1-types-and-pure-functions) and [Vox: Schema That Compiles](#iv2-schema-that-compiles) together.

---

## Act I: C++ — What the Container Does When You Aren't Looking

C++ is the historic engine of video games because of deterministic performance and zero-overhead abstractions. Our inventory data model is fine as an array of structs. The problem that emerges is not a flaw in our logic, but the physical reality of contiguous memory.

```cpp
#include <vector>
#include <string>

struct Stack { std::string kind; int qty; int max_stack; };

void merge_potion(std::vector<Stack>& stash) {
    // Find the first Potion stack
    auto it = stash.begin();
    while (it != stash.end() && it->kind != "Potion") { ++it; }

    if (it != stash.end()) {
        int moved = std::min(it->max_stack - it->qty, 6);
        it->qty += moved;

        int overflow = 6 - moved;
        if (overflow > 0) {
            // BUG: reallocation will invalidate 'it'
            stash.push_back({"Potion", overflow, 10}); 
            // UB: Using the dangling iterator
            printf("Primary slot now has: %d\n", it->qty); 
        }
    }
}
```

When `push_back` is called and `size == capacity`, the `std::vector` allocates a new, larger block of memory, moves all existing elements over, and frees the old storage. Any iterator, pointer, or reference pointing to the old block is now dangling. Dereferencing `it->qty` afterward is Undefined Behavior.

The idiomatic fix is simple: replace the iterator with an index before you mutate the container.

```cpp
size_t idx = it - stash.begin();
stash.push_back({"Potion", overflow, 10});
printf("Primary slot now has: %d\n", stash[idx].qty);
```

**The Residual Cost:** The fix works. But the structural vulnerability recurs every time domain logic mutates a container. The safety of indices over iterators is a convention, invisible to the type system. The developer must remember it, or risk silent memory corruption.

Vox makes the logic structurally incapable of holding a container reference across mutation — see [Types and Pure Functions](#iv1-types-and-pure-functions).

---

## Act II: Rust — Safety Pushes Surface Area Outward

Rust eliminates the dangling iterator bug at compile time. The borrow checker rejects the buggy C++ code above because you cannot mutably borrow the vector (`push_back`) while an immutable borrow (`it`) is alive.

But when two players try to merge into the *same* guild chest simultaneously, the pure function runs out of road.

```rust
use std::sync::{Arc, Mutex};
use std::cmp;

struct Stack { kind: String, qty: u32, max_stack: u32 }

// The merge logic is safe, but the signature has exploded
fn merge_concurrent(chest: Arc<Mutex<Vec<Stack>>>, qty: u32) {
    // 1. We must lock the container
    // 2. We must handle lock poisoning (if another thread panicked while holding it)
    let mut stash = chest.lock().expect("mutex poisoned");

    // 3. Now we can do our logic
    if let Some(slot) = stash.iter_mut().find(|s| s.kind == "Potion") {
        let moved = cmp::min(slot.max_stack - slot.qty, qty);
        slot.qty += moved;
        
        let overflow = qty - moved;
        if overflow > 0 {
            stash.push(Stack { kind: "Potion".into(), qty: overflow, max_stack: 10 });
        }
    }
} // MutexGuard is dropped here, unlocking the chest
```

`Arc` allows shared ownership across threads; `Mutex` enforces exclusive, single-thread access to the inner `Vec`. (If reads outnumbered writes, an `Arc<RwLock<T>>` is the lighter choice.) Locking the chest is the correct protocol. 

**The Residual Cost:** The merge logic is correct and thread-safe. But the function is no longer the unit of reasoning. A simple domain operation now requires the caller to provide a locked, reference-counted primitive. The signature has become a proof of a locking protocol.

Vox absorbs shared-state sequencing out of the type signature — see [Concurrency Without a Locking Protocol](#iv3-concurrency-without-a-locking-protocol).

---

## Act III: Python — Evaluation Timing Is the Architecture

Python is fast to write, extremely readable, and the dominant language for AI and data ecosystems. The code below looks obviously correct. The bug is entirely hidden unless you know one specific detail of the CPython interpreter.

```python
def merge_stack(kind, qty, stash={}):
    slot = stash.setdefault(kind, {"qty": 0, "max_stack": 10})
    moved = min(slot["max_stack"] - slot["qty"], qty)
    slot["qty"] += moved
    return stash, qty - moved

alice_stash, overflow = merge_stack("Potion", 6)
bob_stash, _          = merge_stack("Potion", 1)

# alice_stash and bob_stash are the same object!
assert alice_stash is bob_stash  # True. Bob inherited Alice's state.
```

Python evaluates default arguments exactly once: at function *definition* time, stored in `__defaults__`. Because `dict` is mutable, that identical address is shared across every call to `merge_stack` that omits the third argument.

The fix is canonical, and modern linters flag the error immediately:

```python
def merge_stack(kind, qty, stash=None):
    if stash is None:
        stash = {}
```

**The Residual Cost:** The fix works perfectly and is only two extra lines. But the type system cannot express the underlying invariant: "this argument must not share mutable state." The constraint is reduced to a convention enforced by tooling.

Vox's static type system prevents implicit shared mutability — see [Types and Pure Functions](#iv1-types-and-pure-functions).

---

## Act IV: Vox — The Same File, Growing

Vox lets you follow a problem from a pure concept to a durable, networked UI component *without ever switching tooling, frameworks, or schemas*.

We will solve the same inventory problem, watching the Vox file grow.

### IV.1: Types and Pure Functions

Vox prevents Python's default-argument sharing by enforcing static types and copy-by-value on assignment. C++'s dangling iterator is avoided because we use an explicit exhaustive `match` that cannot hold a mutable container reference across an allocation.

```vox
// vox:skip
type MergeError =
    | WrongKind(left: str, right: str)
    | InvalidCap(cap: int)

type MergeOutcome =
    | Applied(primary: int, overflow: int)
    | Rejected(err: MergeError)

fn merge_stacks(kind_a: str, qty_a: int, kind_b: str, qty_b: int, max_stack: int) to MergeOutcome {
    if max_stack <= 0 {
        return Rejected(InvalidCap(max_stack))
    }
    if kind_a != kind_b {
        return Rejected(WrongKind(kind_a, kind_b))
    }

    let total = qty_a + qty_b
    if total <= max_stack {
        return Applied(total, 0)
    }
    return Applied(max_stack, total - max_stack)
}
```

Wrong-kind and invalid-capacity errors are not exceptions or integer codes; they are first-class algebraic values. The compiler enforces that every caller handles both `Applied` and `Rejected`.

```vox
// vox:skip
// In Rust this is an `enum`, in C++ `std::variant`, in Python `Union`.
// Vox enforces exhaustive handling across all variants via `match`.
fn format_error(err: MergeError) to str {
    match err {
        WrongKind(l, r) -> "Cannot merge " + l + " with " + r
        InvalidCap(c) -> "Invalid capacity: " + str(c)
    }
}
```

What about missing items? There is no `nullptr`, `null`, or unchecked `None`. 

```vox
// vox:skip
// No null, no None, no nullptr — the missing case is a compile error.
fn get_stack_safe(id: int) to Option[InventoryStack] {
    let stack = db.InventoryStack.get(id)
    match stack {
        Some(s) -> Some(s)
        None -> None
    }
}
```

### IV.2: Schema That Compiles

Once the logic works, the inventory must persist. Most architectures demand an ORM, a migration script, a schema definition, and a DB boundary map.

In Vox, the struct *is* the schema.

```vox
// vox:skip
@table type InventoryStack {
    kind: str
    qty: int
    max_stack: int
}

@endpoint(kind: query)
fn stack_count(kind: str) to int {
    return len(db.InventoryStack.filter({ kind: kind }))
}

@endpoint(kind: mutation)
fn seed_stack(kind: str, qty: int, max_stack: int) to Result[str] {
    if qty < 0 {
        return Error("invalid stack shape")
    }
    if max_stack <= 0 {
        return Error("invalid stack shape")
    }
    db.InventoryStack.insert({ kind: kind, qty: qty, max_stack: max_stack })
    return Ok("seeded")
}
```

### IV.3: Concurrency Without a Locking Protocol

When multiple players merge into the central chest concurrently, Rust requires the caller to manage an `Arc<Mutex<T>>`.

Vox draws on the Actor Model, popular in Erlang and Elixir precisely for this reason. The actor isolates the state. Callers interact by sending messages to a mailbox linearly, sidestepping deadlocks and lock poisoning natively.

```vox
// The actor model protects shared state via a mailbox, eliminating mutex
// negotiation. The pattern draws directly from Erlang/Akka: one function per
// handler, state flows through parameters, runtime dispatches via mailbox.
fn InventoryActor_MergeRequest(current: int, incoming: int, max_stack: int) to int {
    let total = current + incoming
    if total > max_stack {
        return max_stack
    }
    return total
}
```

### IV.4: Durability Without a Folklore Document

Distributed systems present the double-charge problem: if the process crashes after a trade is authorized but before the inventory saves, restarting the naive async function will charge the user twice. Systems like Temporal and Cloudflare Workers enforce "at-least-once" consistency via durable step journals and idempotency.

Vox embeds execution durability into the language itself via the `workflow` and `activity` primitives.

```vox
// Workflows provide "at-least-once" execution by journaling activities.
// Double-charging is prevented via idempotency and step-skipping on restart
// (implemented in the interpreted runtime via ADR-019).
fn reserve_slots(amount: int) to Result[str] {
    if amount <= 0 {
        return Error("invalid amount")
    }
    return Ok("reserve_ok")
}

fn settle_trade(amount: int) to str {
    let step = reserve_slots(amount)
    match step {
        Ok(code) => "trade-settled:" + code
        Error(msg) => "trade-failed:" + msg
    }
}
```

### IV.5: Agent Surface Without a Schema Registry

If a local AI Agent model wants to propose a stash merge, you would typically write a REST API layer, generate OpenAPI docs, and deploy a tool-schema definition. 

In Vox, the compiler parses the function's strict type signature and generates the Model Context Protocol (MCP) tool dynamically.

```vox
// vox:skip
@mcp.tool "propose_merge: Propose a stack merge and return primary+overflow"
fn propose_merge(kind: str, current: int, incoming: int, max_stack: int) to str {
    let total = current + incoming
    if total <= max_stack {
        return kind + ":" + str(total) + "+0"
    }
    return kind + ":" + str(max_stack) + "+" + str(total - max_stack)
}
```

### IV.6: UI Without Type Drift

When the inventory requires a display, a `component` lowers to plain React/TSX for the external frontend to import. The compiler guarantees the backend types exist in the DOM layer.

```vox
// vox:skip
component StashMeter(values: list[int]) {
    view: <div class="meter">"stash meter"</div>
}

component InventoryView() {
    view: <div className="inventory-view">
        <h1>{"inventory"}</h1>
        <StashMeter values=[7, 9, 2] />
    </div>
}

routes {
    "/inventory" to InventoryView
}
```

### IV.7: Capability-Gated Import

Finally, attempting to bulk-import loot requires reading the host file system. Instead of implicitly depending on an OS directory path, Vox uses fine-grained Platform Capabilities that are passed explicitly as tokens, guaranteeing security boundary visibility.

```vox
// vox:skip
// vox:skip
// Import requires a named platform capability, not just ambient OS access.
// See docs/src/architecture/capability-grants-ssot.md for the runtime model.
fn import_loot_csv(import_cap: cap, path: str) to Result[str] {
    if !has_capability(import_cap) {
        return Error("missing capability token")
    }
    return Ok("imported:" + path)
}
```

### IV.8: Agentic Implementation

Finally, some logic is too complex or fuzzy to write by hand. Vox allows you to delegate function bodies to an LLM while maintaining strict type contracts and telemetry.

```vox
// vox:skip
@llm(model = "claude-3-opus")
fn generate_loot_flavor_text(kind: str, qty: int) to str

@test
fn test_flavor() {
    let text = generate_loot_flavor_text("Potion", 6)
    assert(len(text) > 0)
}
```

The compiler enforces the return type `str`, and the runtime handles the prompt construction and model invocation.

## Concept Mapping Table

To ease the transition across language patterns, here is how the core engineering primitives align.

| Concept | C++ | Rust | Python | Vox |
|---|---|---|---|---|
| Error handling | exceptions / error codes | `Result<T, E>` | exceptions | `Result[T]` + exhaustive `match` |
| Null safety | `nullptr` (unchecked) | `Option<T>` | `None` (unchecked) | `Option[T]` (no null) |
| Sum types / ADTs | `std::variant` | `enum` | `Union[A, B]` (type hints) | `type Foo = \| A \| B` |
| Concurrency | threads + mutexes | `Arc<Mutex<T>>` | `threading` / `asyncio` | `actor` + `workflow` |
| Persistence | ORM / raw SQL | Diesel, SQLx | SQLAlchemy / Django ORM | `@table` |
| Durable execution | manual retry logic | `tokio-retry` + custom | `celery` / `prefect` | `workflow` + `activity` |
| Secret management | env vars / vaults | `dotenv` / custom | `os.environ` / `boto3` | `vox-secrets` (SSOT) |
| AI agent surface | custom HTTP + JSON Schema | custom HTTP + JSON Schema | FastAPI + Pydantic | `@mcp.tool` |
| Test syntax | `gtest`, `catch2` | `#[test]` | `pytest` | `@test` |
| Type-checked schema | manual | Serde+derive | Pydantic | compiler-integrated |
| LLM hallucination guard | none | partial (compile-time types) | none | full (compiler-integrated) |

Your constraints are real, but they are defined by your model of the world. Vox's unified model makes entire classifications of problems disappear natively.

- [Language Reference](../reference/ref-syntax.md) 
- [Golden Examples](../examples/golden.md)
- [Why Vox for AI](why-vox-for-ai.md)

