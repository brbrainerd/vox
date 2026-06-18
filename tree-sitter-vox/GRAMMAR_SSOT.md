# Vox Grammar SSOT

This document defines the canonical vocabulary for the Vox programming language. Both `tree-sitter-vox` and `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` must align with these tokens.

## Keywords

### Control Flow
`fn`, `let`, `mut`, `if`, `else`, `match`, `for`, `in`, `to`, `return`, `while`, `loop`, `break`, `continue`, `type`, `import`, `actor`, `workflow`, `activity`

### Declaration
`spawn`, `http`, `pub`, `with`, `on`, `state`, `derived`, `effect`, `mount`, `cleanup`, `view`, `component`, `and`, `or`, `not`, `is`, `isnt`

### Web & Reactive (Path C)
`true`, `false`, `get`, `post`, `put`, `delete`

## Primitive Types
`int`, `str`, `bool`, `float`, `Unit`, `Element`

## Collection Types
`List[T]`, `Map[K, V]`, `Set[T]`, `Result[T, E]`, `Option[T]`

## Constants
`true`, `false`

## Decorators
`@deprecated`, `@tool` (canonical; replaces deprecated `@mcp.tool`), `@resource` (canonical; replaces deprecated `@mcp.resource`), `@pure`, `@require`, `@scheduled`, `@ensure`, `@invariant`, `@forall`, `@fuzz`, `@test`, `@server`, `@query`, `@mutation`, `@table`, `@index`, `@v0`, `@mobile.native`, `@loading`

## Operators
`->`, `|>`, `==`, `!=`, `<=`, `>=`, `<`, `>`, `=`, `+=`, `-=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `%`

## Comments
- Single line: `//`
