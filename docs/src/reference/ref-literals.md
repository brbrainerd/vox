---
title: "Reference: literals"
description: "Numeric, decimal, string, and character literal lexing rules for Vox source (UTF-8)."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Grounds tooling and docs in lexer truth."
schema_type: "TechArticle"
---

# Reference: literals

Normative lexer: [`crates/vox-compiler/src/lexer/token.rs`](../../../crates/vox-compiler/src/lexer/token.rs) (`#[regex(...)]` on `Token`).

## Source encoding

- Vox source files are interpreted as **UTF-8** text (Rust `str`), consistent with the `str` primitive in [syntax](./ref-syntax.md).

## Integers

- Pattern: ASCII digits `[0-9]+`, parsed as **`i64`** (`Token::IntLit`).
- No `0x` / `0o` / binary integer literals are defined in the current lexer.

## Floating-point

- Pattern: `[0-9]+.[0-9]+` optionally suffixed with `dec` for fixed-precision literals (`Token::FloatLit` vs `Token::DecLit`).
- See lexer comments in `token.rs` for the exact regex split between `FloatLit` and `DecLit`.

## Strings

- **Double-quoted** (`"…"`): `Token::StringLit` with escapes: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`; unknown escapes preserve `\` + char.
- **Single-quoted** (`'…'`): `Token::SingleStringLit` with the same escape set.

## Booleans and keywords

- `true` / `false` are keywords, not numeric literals.

## See also

- [Operator precedence](./ref-operator-precedence.md)
- [Type system](./ref-type-system.md)
