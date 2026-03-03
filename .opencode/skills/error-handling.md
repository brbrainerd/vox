---
description: Error handling patterns for the Vox codebase — miette, Result, Option
---

# Vox Error Handling Patterns

## Core Principles

1. **ZERO Null States** — `null` is banned. Use `Option<T>`, `Result<T, E>`, or typed discriminated unions.
2. **No `.unwrap()`** in production code — use `.expect("descriptive message")` or `?`.
3. **`miette`** for user-facing errors — provides rich diagnostic output with source spans.
4. **`thiserror`** for library error types — derives `Error` trait with `#[error(...)]` formatting.

## Error Type Pattern

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("description of error: {0}")]
    VariantName(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
}
```

## Result Propagation

```rust
// Good — propagates with ?
pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

// Bad — panics on error
pub fn load_config(path: &Path) -> Config {
    let content = std::fs::read_to_string(path).unwrap(); // ❌
    toml::from_str(&content).unwrap() // ❌
}
```

## Diagnostic Pattern (miette)

```rust
use miette::{Diagnostic, SourceSpan};

#[derive(Debug, Diagnostic, Error)]
#[error("type mismatch")]
#[diagnostic(code(vox::typeck::mismatch))]
pub struct TypeMismatch {
    #[source_code]
    pub src: String,
    #[label("expected {expected}, found {found}")]
    pub span: SourceSpan,
    pub expected: String,
    pub found: String,
}
```

## Optional Values

```rust
// Good
fn find_user(id: UserId) -> Option<User> { ... }

// Good
match find_user(id) {
    Some(user) => process(user),
    None => log::warn!("user not found"),
}

// Bad
fn find_user(id: UserId) -> User { ... } // might return null ❌
```
