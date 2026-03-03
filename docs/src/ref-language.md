# Vox Language Guide

This guide provides a technical overview of Vox, focusing on architectural patterns and the "Single Source of Truth" philosophy.

---

## Type System & Patterns

Vox uses a bidirectional type system that prioritizes safety and explicitness.

### 🧩 Algebraic Data Types (ADTs)

ADTs (Tagged Unions) are used to represent complex state without "boolean soup."

```vox
# Skip-Test
type RequestStatus[T] =
    | Idle
    | Loading
    | Success(data: T)
    | Error(code: int, msg: str)

# Exhaustive pattern matching ensures all states are handled
match current_status:
    | Loading -> <Spinner />
    | Success(res) -> <DataView data={res} />
    | Error(404, _) -> <NotFound />
    | Error(c, m) -> <GeneralError code={c} message={m} />
    | Idle -> <Placeholder />
```

---

## The Three Pillars of Vox

Vox provides specialized decorators to lower declarations into their respective target environments.

### 1. Data (`@table`)
The `@table` decorator defines the persistence schema. Vox generates the Rust migrations and the typed database access layer.

```vox
@table type Article:
    title: str
    content: str
    published: bool

@index Article.by_title on (title)
```

### 2. Server (`@server`)
Server functions are transformed into backend API endpoints (Rust) and typed client wrappers (TypeScript).

```vox
# Skip-Test
@server fn publish_article(id: id[Article]) to Result[Unit]:
    db.execute("UPDATE Article SET published = true WHERE id = ?", id)?
    ret Ok(())
```

### 3. UI (`@component`)
Components are compiled to React/TypeScript. They look like functional components and support inline JSX.

```vox
# Skip-Test
@component fn ArticleFeed() to Element:
    let articles = use_query(db.all_articles)
    ret <div className="space-y-4">
        for a in articles key a.id:
            <Card title={a.title} />
    </div>
```

The `key <expr>` modifier on a `for` loop tells the compiler to emit a stable React key from the item's identity rather than its position. This is the correct default for mutable lists.

For form inputs, use `bind={var}` to declare two-way binding — the compiler expands it to `value` + `onChange`:

```vox
# Skip-Test
@component fn Login() to Element:
    let (email, set_email) = use_state("")
    ret <input bind={email} />
```


---

## 🛠️ Performance & Ergonomics

### Error Propagation
Vox uses the `?` operator for clean error propagation, similar to Rust. It works for both `Result` and `Option`.

```vox
# Skip-Test
fn get_user_email(id: int) to Option[str]:
    let user = db.find_user(id)?
    ret Some(user.email)
```

### Durable Async Logic
`workflow` functions ensure that long-running tasks remain durable across system interruptions.

```vox
# Skip-Test
workflow fn TransactionFlow(userId: int, amount: float):
    await reserve_funds(userId, amount)
    await process_payment(userId, amount)
    await notify_user(userId, "Success")
```

---

## ⚡ Patterns & Performance

For high-performance engineering, follow these established Vox patterns.

### Efficient Data Access
Avoid "N+1" query patterns. Use `@query` functions that return joined datasets rather than iterating over IDs in the frontend.

```vox
# Skip-Test
# PREFERRED: One query for the whole feed
@query fn get_activity_feed() to list[ActivityItem]:
    ret db.query("SELECT * FROM Activity JOIN User ON ...")

# AVOID: Iterating over IDs in a component
```

### Actor Communication
Actors communicate via asynchronous messaging. Use `spawn()` to create lightweight tasks that handle specific responsibilities (e.g., interacting with an LLM or processing a stream).

- **Mailbox Pressure**: Keep actor handlers fast. Delegate long-running computations to `workflow` functions.
- **State locality**: Keep data local to the actor and expose it only via message handlers.

> [!TIP]
> For deep technical details on all decorators and their compiler mappings, see the [Reference Gallery](api/decorators.md).

---

## 📚 Technical Reference

- **[Compiler Pipeline](expl-architecture.md)** — Understanding HIR, LIR, and emission.
- **[Distributed Actors](expl-actors-workflows.md)** — State management and message passing.
- **[CLI & Tooling](how-to-cli-ecosystem.md)** — Command-line interface and LSP features.
