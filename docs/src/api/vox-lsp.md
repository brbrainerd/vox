# Crate API: vox-lsp

## Overview

# Vox Language Server

This crate provides the Language Server Protocol (LSP) implementation for Vox.

## Features
- **Diagnostics**: Reports syntax errors (parser) and type errors (checker).
- **Go to Definition**: (Planned)
- **Completion**: (Planned)
- **Hover**: (Planned)

## Installation

1. Build the LSP server:
   ```bash
   cargo build --release -p vox-lsp
   ```
2. The binary will be at `target/release/vox-lsp`.

## Editor Configuration

### VS Code
Use the `vscode-vox` extension (TBD) or configure manually:
```json
"vox.lsp.serverPath": "/path/to/target/release/vox-lsp"
```

---

## Module: `vox-lsp\src\lib.rs`

# vox-lsp

Language Server Protocol implementation for the Vox language.

This crate provides the core LSP analysis functions: diagnostics,
document symbols, hover, completion, go-to-definition, formatting,
code actions, rename, semantic tokens, and workspace symbol search.

The public API operates on source text (`&str`) and returns
`tower_lsp_server::ls_types` structs, making them easy to wire into
any LSP transport layer.


### `fn document_symbols`

B-050: Return all top-level declarations as DocumentSymbols.


### `fn hover_at`

B-047: Return hover info for the identifier at the given position.


### `fn completions_at`

B-049: Return completion items at the given position.


### `fn format_document`

B-051: Format the entire document by delegating to vox-parser's formatter.
Currently returns the source unchanged (formatter not yet implemented).


### `fn definition_at`

B-048: Return the definition location of the identifier at the given position.


### `fn code_actions_at`

B-052: Return code actions (quick fixes) based on diagnostics.


### `fn rename_at`

B-053: Rename symbol across file.


### `fn semantic_tokens`

Return semantic tokens for syntax highlighting.


### `fn workspace_symbols`

B-055: Workspace symbols — search across all declarations by name.


### `fn find_references`

Phase 3.3 — Find all references to the identifier at the given position.


### `fn inlay_hints`

Phase 3.4 — Inlay hints: show inferred types for `let` bindings without annotations.


### `fn signature_help`

Phase 3.5 — Signature help: show function signature when inside a call.


### `fn search_workspace_symbols`

Phase 3.7 — Workspace symbol search with fuzzy matching


## Module: `vox-lsp\src\main.rs`

# vox-lsp server binary

LSP transport for the Vox language. Provides: diagnostics, hover,
completion, go-to-definition, find-references, document symbols,
formatting, code actions, rename, semantic tokens, inlay hints,
and signature help.


