---
description: Testing patterns and conventions for the Vox codebase
---

# Vox Testing Patterns

## Test Types

### Unit Tests (inline)
Place inside `#[cfg(test)] mod tests { ... }` at the bottom of each source file.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptive_test_name() {
        // Arrange
        let input = ...;
        // Act
        let result = function_under_test(input);
        // Assert
        assert_eq!(result, expected);
    }
}
```

### Async Tests
Use `#[tokio::test]` for async code:

```rust
#[tokio::test]
async fn async_operation_completes() {
    let result = some_async_fn().await;
    assert!(result.is_ok());
}
```

### Integration Tests
Place in `crates/vox-integration-tests/tests/*.rs`. These test the full pipeline.

### Property Tests
Use `proptest` for invariant checking:

```rust
proptest! {
    #[test]
    fn roundtrip_serialization(input in any::<MyType>()) {
        let serialized = serde_json::to_string(&input).unwrap();
        let deserialized: MyType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(input, deserialized);
    }
}
```

## Commands

```bash
# Full workspace
cargo test --workspace

# Single crate
cargo test -p vox-orchestrator

# Single test
cargo test -p vox-orchestrator -- submit_task

# With output
cargo test -p vox-orchestrator -- --nocapture
```

## Naming Convention

Use long, descriptive names: `test_<action>_<condition>_<expected_result>`

Example: `test_submit_task_with_conflicting_locks_returns_lock_conflict_error`

## Rules

1. Every new feature needs a test.
2. Tests must compile and pass.
3. Use `.expect("message")` not `.unwrap()`.
4. Test both happy paths and error cases.
5. Include doc comments explaining what each test verifies.
