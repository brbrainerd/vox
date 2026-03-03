# vox-lexer

High-performance tokenizer for the Vox programming language, built on [`logos`](https://docs.rs/logos).

## Purpose

Converts Vox source code into a flat stream of typed tokens — the first stage of the compiler pipeline.

## Key Files

| File | Purpose |
|------|---------|
| `token.rs` | `Token` enum — all language tokens (keywords, operators, literals, punctuation) |
| `cursor.rs` | Character-level scanning cursor and `lex()` function |
| `lib.rs` | Public API: re-exports `lex()` and `Token` |

## Usage

```rust
use vox_lexer::{lex, Token};

let tokens = lex("fn hello(): ret 42");
// → [Fn, Ident("hello"), LParen, RParen, Colon, Ret, IntLit(42)]
```

## Design

- **Zero-copy** tokenization via `logos` derive macro
- Tokens carry their source span for error reporting
- Whitespace and comments are preserved as tokens for the lossless parser
