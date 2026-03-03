# if

## Syntax

```vox
# Skip-Test
if cond:
    body
else:
    body
```

## Description

Conditional branching.

## Code Generation
### Rust
```rust
if cond { body } else { body }
```
### TypeScript
```typescript
if (cond) { body } else { body }
```

[← Back to API Index](../index.md)