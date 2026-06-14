---
title: "tree-sitter-vox Grammar SSOT"
description: "Single source of truth for the Vox tree-sitter grammar vocabulary and parity rules"
category: "tooling"
---

# tree-sitter-vox Grammar SSOT

## Canonical Vocabulary

The vocabulary for `tree-sitter-vox` is owned by **two** files in `crates/vox-compiler/src/lexer/`:

| Surface | Lexer SSOT |
|---------|-----------|
| All `@decorator` tokens | `token.rs` — every `#[token("@...")]` attribute |
| Keywords | `token.rs` — `#[token("keyword")]` attributes |
| Operators | `token.rs` — `Token::Plus`, `Token::Eq`, etc. |
| Reactive surface | `language_surface.rs` |

## Parity Enforcement

The CI gate is in `crates/vox-grammar-export/tests/export_test.rs`:

- **`all_decorators_appear_in_grammar_js`** — asserts every `#[token("@...")]` from `token.rs` appears in `grammar.js` (either as a literal string in the SSOT comment block, or matched by the `decorator` regex rule).
- **`grammar_js_uses_return_not_ret`** — asserts `grammar.js` uses `'return'` not `'ret'` (the retired spelling).

## Decorator Rule

All 54+ decorators are covered by a single regex rule:

```js
decorator: $ => /@[a-z_][a-z0-9_]*(?:\.[a-z_][a-z0-9_]*)*/
```

This handles plain (`@server`, `@auth`) and dotted (`@mcp.tool`, `@mcp.resource`) forms.
The SSOT comment block in `grammar.js` lists every decorator explicitly so the parity
test (which uses `contains()`) can find them.

## Regenerating the Parser

After editing `grammar.js`, run:

```sh
tree-sitter generate   # requires tree-sitter CLI in PATH
```

Or via npm: `cd tree-sitter-vox && npx tree-sitter generate`

**Known pre-existing conflict:** `block` vs `block_repeat1` in the `http_route` production
causes a generation error. This is a pre-existing grammar ambiguity; add a `prec.left`
or a `conflicts:` entry in grammar.js to resolve it before releasing a new parser binary.
Until resolved, the generated `grammar.json` is correct but the C parser (`src/parser.c`)
cannot be regenerated.

## Keyword List

Vox keywords recognized by the compiler (`token.rs`):

`fn`, `let`, `mut`, `return`, `if`, `else`, `for`, `in`, `while`, `loop`,
`break`, `continue`, `match`, `type`, `import`, `export`, `pub`, `async`,
`await`, `spawn`, `actor`, `workflow`, `activity`, `component`, `view`,
`state`, `derived`, `effect`, `mount`, `cleanup`, `fragment`, `extern`,
`routes`, `agent`, `environment`, `http`, `get`, `post`, `put`, `delete`
