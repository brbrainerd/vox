# match

## Syntax

```vox
# Skip-Test
match expr:
    | pattern -> body
```

## Description

Exhaustive pattern matching on values, typically ADTs.

## Code Generation
### Rust
```rust
match expr { pattern => body }
```
### TypeScript
```typescript
switch (expr.type) { case ... }
```

[← Back to API Index](../index.md)