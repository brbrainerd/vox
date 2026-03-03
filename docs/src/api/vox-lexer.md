# Crate API: vox-lexer

## Overview

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

---

### `fn compact`

Compacts Vox source code to be more token-efficient for LLMs.
Removes comments, minimizes whitespace, and preserves only essential indentation.


### `struct Spanned`

A located token with its source span.


### `fn lex`

Lex source code into a flat vector of spanned tokens.
Handles indentation tracking: raw newlines from logos are processed
to emit synthetic Indent, Dedent, and Newline tokens based on
leading whitespace at each line start.


## Module: `vox-lexer\src\lib.rs`

# vox-lexer

High-performance tokenizer for the Vox language, built on [`logos`](https://docs.rs/logos).

Converts source code into a stream of [`Token`]s consumed by the parser.


### `enum Token`

All tokens in the Vox language.
Keywords are phonetically distinct English words.
Operators use English keywords (and, or, not, is, isnt) instead of symbols.


