# Vox Grammar SSOT

This document defines the canonical vocabulary for the Vox programming language. Both `tree-sitter-vox` and `apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json` must align with these tokens.

## Keywords

### Control Flow
`fn`, `let`, `mut`, `if`, `else`, `match`, `for`, `in`, `to`, `return`, `while`, `loop`, `break`, `continue`

### Declaration
`type`, `import`, `actor`, `workflow`, `activity`, `spawn`, `http`, `pub`, `with`, `on`, `state`, `derived`, `effect`, `mount`, `cleanup`, `view`, `component`, `agent`, `async`, `migrate`, `env`, `dec`

### Web & Reactive (Path C)
`and`, `or`, `not`, `is`, `true`, `false`, `get`, `post`, `put`, `delete`, `table`, `index`, `query`, `mutation`, `server`, `tool`, `resource`, `form`

## Primitive Types
`int`, `str`, `bool`, `float`, `Unit`, `Element`

## Collection Types
`List[T]`, `Map[K, V]`, `Set[T]`, `Result[T, E]`, `Option[T]`

## Constants
`true`, `false`

## Decorators
`@deprecated`, `@tool`, `@resource`, `@pure`, `@traced`, `@require`, `@scheduled`, `@ensure`, `@invariant`, `@forall`, `@fuzz`, `@test`, `@example`, `@server`, `@query`, `@mutation`, `@table`, `@index`, `@placeholder`, `@place`, `@loading`, `@ai`

## Operators
`->`, `|>`, `==`, `!=`, `<=`, `>=`, `<`, `>`, `=`, `+=`, `-=`, `*=`, `/=`, `+`, `-`, `*`, `/`, `%`

## Comments
- Single line: `//`
