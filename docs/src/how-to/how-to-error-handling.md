---
title: "How-To: Handle Errors Gracefully"
description: "Learn the best practices for error management in Vox to build robust applications."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---
# How-To: Handle Errors Gracefully

Learn the best practices for error management in Vox to build robust, fault-tolerant applications.

## 1. The `Result` Type

Vox uses the functional `Result[T, E]` type for operations that can fail, rather than standard exceptions.

```vox
// vox:skip
fn find_user(id: str) to Result[str] {
    if id == "" {
        return Error("Invalid ID")
    }
    return Ok(id)
}
```

## 2. Using the `?` Operator

The `?` operator provides ergonomic error propagation. If an expression evaluates to `Error`, the surrounding function returns that error immediately.

```vox
// vox:skip
fn process_order(id: str) to Result[bool] {
    let user = find_user(id)?
    // `check_balance` might also return a Result
    // let balance = check_balance(user)?
    return Ok(true)
}
```

## 3. Error Handling

Vox allows you to handle `Result` types directly using exhaustive pattern matching. (Error display in UI is covered in the islands tutorial).

```vox
// vox:skip
let result = find_user("123")

match result {
    Ok(user)   -> println("Found { " + user)
    Error(msg) -> println("Failed: " + msg)
}
```

## 4. Converting Errors with `Result[T, E]`

You can transform results using functional combinators or explicit pattern matching.

```vox
// vox:skip
fn get_user_name(id: str) to Result[str] {
    let user = find_user(id).map_err(|e| "User fetch failed: " + e)?
    return Ok(user.name)
}
```

## 5. Preconditions with `@require`

For invariant safety (assertions that must hold for a type to be valid), use the `@require` decorator. This acts as a construction-time guard.

```vox
// vox:skip
@require(self.age >= 18)
type Adult {
    name: str
    age: int
}
```

If the condition fails during instantiation, a panic is triggered (or an error returned if used within a fallible constructor context).

---

## Best Practices

1. **Surface Results Early**: Always surface the `Result` type rather than attempting to `unwrap()` or panic inside production web routes.
2. **Contextualize Errors**: Use `.map_err()` to add context to low-level errors (e.g., "Database error" -> "Failed to save user").
3. **Use `?` for Flow**: The `?` operator is the preferred way to maintain a "happy path" while handling fallibility.

---

## Summary
- Use `Result` for operations that can gracefully fail.
- Use `?` to easily propagate `Error` up the call stack.
- Use pattern matching with `match` blocks to unwrap and inspect the branches safely.

## Related
- [Language Syntax](../reference/ref-syntax.md) — Syntax for `match` and `?`.
- [Durable Workflows](../tutorials/tut-workflow-durability.md) — Automatic error recovery in long-running tasks.
