---
description: Vox compiler pipeline walkthrough — stages, file paths, and data flow
---

# Vox Compiler Pipeline

## Stage Overview

```
Source (.vox) → Lexer → Parser → AST → HIR → TypeCheck → CodeGen → Output
```

## Stage Details

| Stage | Crate | Entry Point | Input | Output |
|---|---|---|---|---|
| Lexer | `vox-lexer` | `cursor::lex(text)` | Source text | `Vec<Token>` |
| Parser | `vox-parser` | `parser::parse(tokens)` | Tokens | `Module` (CST) |
| AST | `vox-ast` | (typed wrappers) | CST GreenTree | Typed AST |
| HIR | `vox-hir` | `lower_module(&module)` | AST Module | `HirModule` |
| TypeCheck | `vox-typeck` | `typecheck_module(&module, src)` | AST Module | `Vec<Diagnostic>` |
| Codegen Rust | `vox-codegen-rust` | varies | Typed HIR | Rust source |
| Codegen TS | `vox-codegen-ts` | varies | Typed HIR | TypeScript source |

## Adding a Language Feature

1. **Grammar**: Update `crates/vox-parser/src/grammar.rs`
2. **AST**: Add node wrappers in `crates/vox-ast`
3. **Lowering**: Map AST to HIR in `crates/vox-hir`
4. **Type Checking**: Add inference rules in `crates/vox-typeck`
5. **Codegen**: Implement emission in both `vox-codegen-rust` and `vox-codegen-ts`
6. **Test**: Add end-to-end test in `crates/vox-integration-tests/tests/`

## Pipeline Validation (LSP)

The LSP runs the full pipeline on every edit:
```rust
let tokens = lex(text);
let module = parse(tokens)?;
let type_errors = typecheck_module(&module, text);
let hir_module = lower_module(&module);
let hir_errors = validate_module(&hir_module);
let dead_code = check_dead_code(&hir_module);
```
