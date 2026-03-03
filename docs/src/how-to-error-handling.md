# How-To: Handle Errors Gracefully

Learn the best practices for error management in Vox to build robust, fault-tolerant applications.

## 1. The `Result` Type

Vox uses the functional `Result[T, E]` type for operations that can fail, rather than standard exceptions.

```vox
# Skip-Test
fn find_user(id: str) to Result[User, str]:
    if id == "":
        ret Err("Invalid ID")
    ret Ok(User(id: id))
```

## 2. Using the `?` Operator

The `?` operator provides ergonomic error propagation. If an expression evaluates to `Err`, the surrounding function returns that error immediately.

```vox
# Skip-Test
fn process_order(id: str) to Result[bool, str]:
    let user = find_user(id)?
    let balance = check_balance(user)?
    ret Ok(true)
```

## 3. Error Handling in UI

Vox components can handle `Result` types directly mid-render using pattern matching or helper methods.

```vox
# Skip-Test
@component fn UserProfile(id: str) to Element:
    let result = find_user(id)

    <div class="profile">
        match result:
            | Ok(user) => <h1>user.name</h1>
            | Err(msg) => <p class="error">msg</p>
    </div>
```

## 4. Panic vs. Error

- **Errors (`Result`)**: Use for expected failures (e.g., user not found, validation error).
- **Panics**: Use for unrecoverable logic errors or violated invariants (e.g., array out of bounds). Panics trigger actor restarts in stateful systems.

---

**Related Reference**:
- [Language Reference](ref-language.md) — Syntax for `match` and `?`.
- [Durable Workflows](tut-workflow-durability.md) — Automatic error recovery in long-running tasks.
