# @deprecated

**Category:** function

## Syntax

```vox
# Skip-Test
@deprecated
```

## Description

Mark a function as deprecated. Emits a warning at every call site.

## Code Generation
### Rust
```rust
#[deprecated]
```
### TypeScript
```typescript
/** @deprecated */
```

[← Back to Decorator Index](../decorators.md)