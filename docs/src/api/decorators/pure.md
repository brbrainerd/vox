# @pure

**Category:** function

## Syntax

```vox
# Skip-Test
@pure
```

## Description

Enforce function purity — no side effects allowed in the function body.

## Code Generation
### Rust
```rust
/* @pure */ comment
```
### TypeScript
```typescript
/** @__PURE__ */
```

[← Back to Decorator Index](../decorators.md)