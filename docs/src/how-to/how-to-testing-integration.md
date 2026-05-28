---
title: "How-To: Testing Integration"
description: "How to structure tests for Vox functions, workflows, and UI using the built-in testing framework."
category: "How-To Guides"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "HowTo"
---

# How-To: Testing Integration

Testing in Vox focuses on unit tests and bounded integration tests using the `@test` decorator. Note that the legacy `@mock` and `@fixture` features have been removed or placed into aspirational scope for v0.3.

## Structuring a Test

Any function annotated with `@test` will be executed during a `vox test` invocation. The `assert` global built-in is used to evaluate conditions.

```vox
// vox:skip
fn calculate_total(subtotal: int, tax: int) to int {
    return subtotal + tax
}

@test
fn test_calculate_total() to Unit {
    let result = calculate_total(100, 10)
    assert(result == 110)
}
```

## Testing `Result` Returns

When testing functions that return `Result[T, E]`, you typically use `match` to assert the correct execution branch.

```vox
// vox:skip
@test
fn test_database_insert_validation() to Unit {
    let invalid_data = { title: "", owner: "alice" }
    
    // Assuming db.Task.insert has a length requirement on title
    match db.Task.insert(invalid_data) {
        Ok(_) -> assert(false) // Should fail
        Error(_) -> assert(true) // Expected
    }
}
```

## Testing Asynchronous Workflows

Workflows and Activities evaluate sequentially and synchronously from the tester's perspective because the execution context blocks until the workflow concludes or hits a checkpoint limit.

```vox
// vox:skip
@test
fn test_order_workflow() to Unit {
    // Run the workflow natively
    let result = process_order("alice", 500)
    
    match result {
        Ok(tx) -> assert(len(tx) > 0)
        Error(_) -> assert(false)
    }
}
```

## Running Tests

Execute all tests in the workspace {

```bash
vox test
```

Execute tests targeting a specific module:

```bash
vox test src/domain/tasks.vox
```

You can view the specific failures via standard error stack traces emitted by the V0.3 compiler pipeline.
