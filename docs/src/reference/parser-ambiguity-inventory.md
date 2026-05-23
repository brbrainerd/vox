---
title: "Parser ambiguity and robustness inventory"
description: "Known parse edge cases, error classes, and fixture locations for the canonical vox-compiler parser"
category: "Language Reference"
last_updated: "2026-03-25"
training_eligible: true

schema_type: "TechArticle"
---

# Parser ambiguity and robustness inventory

The canonical parser is **recursive descent** in [`crates/vox-compiler/src/parser/descent`](../../../crates/vox-compiler/src/parser/descent). It is **not** the `tree-sitter-vox` grammar (highlighting / editor tooling may diverge).

## Error taxonomy

Each [`ParseError`](../../../crates/vox-compiler/src/parser/error.rs) carries a [`ParseErrorClass`](../../../crates/vox-compiler/src/parser/error.rs):

| Class | Typical cause |
|-------|----------------|
| **`expect_token`** | `Parser::expect` mismatch (wrong token at a committed point). |
| **`top_level`** | Token cannot start a module-level declaration. |
| **`declaration`** | `pub` / attribute / item head issues. |
| **`expression` / `statement` / `type_expr`** | Reserved for finer-grained classification in inner parsers. |
| **`other`** | Default for legacy call sites. |

## Fixture corpus (reproducible)

| ID | File | Intent |
|----|------|--------|
| **INV-01** | [`examples/parser-inventory/top-level-garbage.vox`](../../../examples/parser-inventory/top-level-garbage.vox) | Invalid top-level → recovery; subsequent valid decls still parsed when possible. |
| **INV-02** | [`examples/parser-inventory/nested-unclosed.vox`](../../../examples/parser-inventory/nested-unclosed.vox) | Unbalanced braces inside function → parser errors + recovery. |
| **INV-03** | [`examples/parser-inventory/pub-bogus.vox`](../../../examples/parser-inventory/pub-bogus.vox) | `pub` not followed by `fn`/`type` → declaration-class error. |

Automated **no-panic** corpus { [`crates/vox-compiler/tests/parser_corpus_no_panic_test.rs`](../../../crates/vox-compiler/tests/parser_corpus_no_panic_test.rs).

## Related

- [Lexer / tokens — Vox Language Reference](../reference/ref-syntax.md)
- [STYLE / examples](../../../examples/STYLE.md)

