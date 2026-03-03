# @require

**Category:** function

## Syntax

```vox
# Skip-Test
@require(expr)
```

## Description

Add a precondition assertion. Panics at runtime if the expression is false.

## Code Generation
### Rust
```rust
assert!(expr, message)
```
### TypeScript
```typescript
if (!expr) throw new Error(message)
```

[← Back to Decorator Index](../decorators.md)