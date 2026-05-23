---
title: "Vox Syntax Highlighting: Universal SSOT Strategy (2026)"
description: "Research synthesis on how tree-sitter-vox, TextMate grammars, and Markdown injection queries combine to provide consistent, IDE-agnostic Vox syntax coloring everywhere code appears."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
last_updated: "2026-04-16"
archived_date: 2026-04-18
---

# Vox Syntax Highlighting: Universal SSOT Strategy

## Problem Statement

Vox code blocks appear in three distinct rendering contexts that each use
incompatible highlighting engines:

| Context | Default Engine | How it injects a custom language |
|---|---|---|
| **VS Code / Cursor / Windsurf** | TextMate regex (`.tmLanguage.json`) + LSP semantic tokens | VS Code TS extension adds semantic highlights on top |
| **Neovim / Helix / Zed** | Tree-sitter (`.scm` query files) | `injections.scm` in Markdown grammar triggers `tree-sitter-vox` |
| **GitHub.com** | Linguist → Tree-sitter or TextMate fallback | PR to `github-linguist/linguist` |
| **mdBook docs portal** | Highlight.js (browser JS) | `additional-js` in `book.toml` OR Shiki preprocessor |
| **README.md / raw Markdown** | Varies by viewer (same as above) | language identifier on fenced code block |

The historical workaround — using ` ```rust ` as a proxy — gives approximate
coloring but silently hides real Vox-specific constructs (`actor`, `workflow`,
`@mcp.tool`, `@island`, `component`, `ret`, `activity`).

---

## SSOT Architecture: Two Artifacts, Maximum Coverage

Research shows no single file serves all five contexts. However a **two-artifact
strategy** provides a true single source of truth per rendering class while
sharing the same underlying token taxonomy.

### Artifact 1 — `tree-sitter-vox` ← Neovim, Helix, Zed, GitHub (modern)

Already live at `tree-sitter-vox/`. What is **missing** to unlock Markdown
injection in these editors:

```
tree-sitter-vox/
  queries/
    highlights.scm  ← exists ✓
    locals.scm      ← exists ✓
    injections.scm  ← MISSING — needed for Markdown fenced blocks
```

`injections.scm` for the **Markdown grammar** tells editors that when they see
a fenced code block tagged `vox` (or `tsx`), they should parse its content with
`tree-sitter-vox` instead of treating it as plain text:

```scheme
; vox/  inject into fenced code blocks inside Markdown docs
((fenced_code_block_delimiter) @_lang
 (fenced_code_block_content) @injection.content
 (#match? @_lang "^vox$")
 (#set! injection.language "vox"))

((fenced_code_block_delimiter) @_lang
 (fenced_code_block_content) @injection.content
 (#match? @_lang "^tsx$")
 (#set! injection.language "vox"))
```

This single file is picked up by Neovim `nvim-treesitter`, Helix, Zed, and any
editor using the Tree-sitter Markdown grammar — **zero per-editor configuration**
by the user because the grammar package ships the file.

### Artifact 2 — `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` ← VS Code, Cursor, Windsurf, GitHub (legacy)

VS Code does not yet use Tree-sitter as its default highlighting engine (as of
mid-2026). Highlighting for `.vox` files is served by the `vox-vscode` extension
via TextMate grammar. **What is missing** for Markdown is a second grammar
contribution that activates inside fenced code blocks:

```json
// In apps/editor/vox-vscode/package.json, inside "contributes":
"grammars": [
  {
    "language": "vox",
    "scopeName": "source.vox",
    "path": "./syntaxes/vox.tmLanguage.json"
  },
  {
    "scopeName": "markdown.vox.codeblock",
    "path": "./syntaxes/vox.tmLanguage.json",
    "injectTo": ["text.html.markdown"],
    "embeddedLanguages": {
      "meta.embedded.block.vox": "vox"
    }
  }
]
```

VS Code picks up the `injectTo: ["text.html.markdown"]` contribution and
automatically applies `vox.tmLanguage.json` highlighting inside any
` ```vox ` block in any `.md` file — this covers **VS Code, Cursor, and
Windsurf** (all Electron-based, all ship VS Code extensions).

A `vox.tmLanguage.json` in the correct TextMate format **must first exist** (the
extension currently ships a `language-configuration.json` and snippets but the
tmLanguage itself may be incomplete or absent). The token scope names in the
tmLanguage must mirror those in `highlights.scm` (e.g., `keyword.control.vox`,
`entity.name.function.vox`) to ensure themes render identically.

archived_date: 2026-04-18
---

## Key Insight: An `injections.scm` is NOT the Same as `highlights.scm`

> **This is the source of confusion in the discussion.**

- **`highlights.scm`** is *how to color tokens inside `.vox` source files*.
- **`injections.scm`** (placed inside the **Markdown** grammar's query directory)
  is *how to tell an editor to invoke the Vox grammar when it encounters a
  fenced Vox block inside a `.md` file*.

The LSP already provides semantic tokens for `.vox` files opened as primary
documents. **The LSP does NOT activate inside Markdown fenced code blocks** in
most editors — LSPs are document-scoped, not sub-block-scoped. Therefore
injection queries are the necessary complement.

---

## Implementation Roadmap

### Wave 0 — Enable Neovim/Helix/Zed Markdown injection (High Value, Low Effort)

1. Add `tree-sitter-vox/queries/injections.scm` (see template above).
2. Publish updated `tree-sitter-vox` npm package (`npm publish`).
3. Submit `tree-sitter-vox` to `nvim-treesitter` parsers list (PR to
   `nvim-treesitter/nvim-treesitter`). Once merged, Neovim users get
   `vox` fenced block coloring with zero local configuration.

### Wave 1 — Enable VS Code / Cursor / Windsurf injection (High Value, Medium Effort)

1. Audit / complete `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` to cover all
   tokens in `highlights.scm` (functions, types, keywords, decorators, actors,
   workflows, JSX, etc.).
2. Add the `injectTo: ["text.html.markdown"]` grammar contribution to
   `apps/editor/vox-vscode/package.json`.
3. Publish the updated `vox-vscode` extension to VS Code Marketplace.
4. This automatically propagates to Cursor and Windsurf which consume the same
   extension format.

### Wave 2 — GitHub.com recognition (Medium Value, Gated on Community Size)

GitHub Linguist requires a PR to `github-linguist/linguist` with:
- Entry in `lib/linguist/languages.yml` for `Vox` with file extension `.vox`.
- A grammar source pointing to `tree-sitter-vox`.
- A corpus of representative `.vox` files in `samples/Vox/`.

This gates on GitHub's process not Vox's. File the PR now and iterate.

### Wave 3 — mdBook Shiki Preprocessor (Medium Value, Docs-Only)

For the rendered docs portal, integrate `mdbook-shiki` (or build a thin Rust
preprocessor using the `shiki` WASM bindings). Shiki consumes the same
`vox.tmLanguage.json` from Wave 1 and produces pre-coloured HTML at compile
time. This is the only surface where a separate implementation step is required
beyond the VS Code extension work.

archived_date: 2026-04-18
---

## Scope Names SSOT

The following table is the SSOT for token scope names used in **both**
`highlights.scm` (Tree-sitter) and `vox.tmLanguage.json` (TextMate). Keeping
these aligned guarantees themes render identically across engines.

| Vox construct | Tree-sitter capture | TextMate scope |
|---|---|---|
| `fn`, `let`, `mut`, `if`, `else`, `match`, `ret`, `for`, `in` | `@keyword` | `keyword.control.vox` |
| `actor`, `workflow`, `activity`, `spawn` | `@keyword` | `keyword.other.vox` |
| `@mcp.tool`, `@island`, `@table`, `@query`, `@mutation`, `@component`, `@test`, `@external` | `@attribute` | `storage.modifier.decorator.vox` |
| `component Name()`, `fn name()` | `@function` | `entity.name.function.vox` |
| `TypeIdentifier` | `@type` | `entity.name.type.vox` |
| Type variant names | `@constructor` | `entity.name.function.constructor.vox` |
| Integer / float literals | `@number` | `constant.numeric.vox` |
| String literals | `@string` | `string.quoted.double.vox` |
| `true`, `false`, `None`, `Some` | `@constant.builtin` | `constant.language.vox` |
| `// comment` | `@comment` | `comment.line.double-slash.vox` |
| JSX tags | `@tag` | `entity.name.tag.vox` |
| JSX attributes | `@tag.attribute` | `entity.other.attribute-name.vox` |
| Operators (`+`, `-`, `->`, `|>`) | `@operator` | `keyword.operator.vox` |

---

## References

- Tree-sitter injection query docs: <https://tree-sitter.github.io/tree-sitter/syntax-highlighting#language-injection>
- VS Code TextMate grammar injection: <https://code.visualstudio.com/api/language-extensions/syntax-highlight-guide#injection-grammars>
- GitHub Linguist language definition: <https://github.com/github-linguist/linguist/blob/main/CONTRIBUTING.md>
- nvim-treesitter custom parser setup: <https://github.com/nvim-treesitter/nvim-treesitter?tab=readme-ov-file#adding-parsers>
- Helix languages.toml: <https://docs.helix-editor.com/master/languages.html>
- Shiki TextMate grammar support: <https://shiki.style/guide/load-lang>

