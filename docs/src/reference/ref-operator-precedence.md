---
title: "Reference: operator precedence"
description: "Binary and postfix operator precedence for Vox expressions, sourced from the Pratt parser binding powers."
category: "Language Reference"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Prevents incorrect parentheses assumptions in generated and hand-written Vox."
schema_type: "TechArticle"
---

# Reference: operator precedence

Normative implementation: [`crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs`](../../../crates/vox-compiler/src/parser/descent/expr/pratt_ops.rs) (`infix_bp` and postfix `?` handling).

## Binding strength (loose → tight)

Higher binding power means the operator groups **more tightly** with its operands.

| Precedence (conceptual) | Operators / forms | Notes |
| --- | --- | --- |
| 1–2 | `\|>` pipe | Left-associative chain: `a \|> f \|> g` |
| 3–4 | `or` | Phonetic logical OR |
| 5–6 | `and` | Phonetic logical AND |
| 5–6 | `with` (expression form) | Parsed as infix between operand and options block |
| 7–8 | `is`, `isnt`, `==`, `!=` | Equality-style (`EqEq` / `NotEq` tokenized same as `is` / `isnt`) |
| 9–10 | `<`, `>`, `<=`, `>=` | Ordering comparisons |
| 11–12 | `+`, `-` | Arithmetic |
| 13–14 | `*`, `/`, `%` | Arithmetic multiplicative |
| Postfix (tight) | `?` | Try / early-return on `Option` / `Result` |

Unary `not` and primary expressions are parsed before binary operators (see parser descent).

## Unary and unary-like forms

- Prefix `not` applies to the following primary expression; combine with parentheses when mixing with `and` / `or`.

## See also

- [Language syntax](./ref-syntax.md)
- [Literals](./ref-literals.md)
