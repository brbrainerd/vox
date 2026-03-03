# Crate API: vox-parser

## Overview

Recursive descent parser for the Vox programming language. Produces a lossless syntax tree with full error recovery.

## Purpose

Transforms a token stream from `vox-lexer` into a typed AST (`vox-ast`). Designed for error resilience — the parser continues after encountering invalid syntax, which is critical for LSP support where the user is actively typing.

## Key Files

| File | Purpose |
|------|---------|
| `parser.rs` | Core recursive descent parser — `parse()` entry point |
| `error.rs` | `ParseError` type with source spans and recovery info |
| `indent.rs` | Indentation-aware formatting and scope detection |

## Features

- **Error recovery** with synchronization points — never panics on invalid input
- **Trailing comma** support in function parameter lists
- **Duplicate parameter** name detection with clear error messages
- **Indentation-aware** parsing for Python-style block structure

## Usage

```rust
use vox_parser::parse;
use vox_lexer::lex;

let tokens = lex("fn hello(): ret 42");
let (module, errors) = parse(&tokens);
// module: Module with one FnDecl
// errors: Vec<ParseError> (empty for valid input)
```

## Design

The parser produces `vox_ast::Module` structures and collects all errors into a `Vec<ParseError>` rather than failing on the first error. This design enables:
- Real-time diagnostics in the LSP
- Partial compilation of files with syntax errors
- Better error messages by continuing to parse after mistakes

---

### `struct ParseError`

A parse error with detailed context.


## Module: `vox-parser\src\indent.rs`

Indentation tracking for the parser.
The lexer already injects Indent/Dedent/Newline tokens,
so this module provides utilities for the parser to track block depth.


### `struct IndentTracker`

Tracks indentation context during parsing.


## Module: `vox-parser\src\lib.rs`

# vox-parser

Recursive descent parser for the Vox language. Produces a typed AST
with full error recovery — critical for LSP support where the user
is actively typing and the source may be temporarily invalid.


