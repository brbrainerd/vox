# Crate API: vox-integration-tests

## Overview

End-to-end tests for the Vox compiler pipeline. Each test exercises the full path from parsing through code generation.

## Test Files

| Test | Coverage |
|------|----------|
| `pipeline.rs` | Full pipeline: parse → HIR → typeck → codegen for all language features |
| `codegen_rust.rs` | Rust code generation specifics |
| `typeck.rs` | Type checking edge cases |
| `db_typeck.rs` | Database type checking (`@table`, `@index`) |
| `chatbot_integration.rs` | Full chatbot example compilation |
| `workflow_integration.rs` | Workflow and activity compilation |
| `agent_message.rs` | Agent and MCP tool compilation |
| `traits.rs` | Trait/interface compilation |
| `stream_emit.rs` | Streaming code emission |
| `while_trycatch.rs` | While loops and try/catch compilation |
| `test_hang.rs` | Regression test for parser hangs |

## Running

```bash
cargo test -p vox-integration-tests
```

## Adding Tests

Each test follows the pattern:
1. Define Vox source as a string
2. Lex → Parse → Lower → Typecheck → Codegen
3. Assert on the generated output or diagnostics

---

## Module: `vox-integration-tests\src\lib.rs`

Integration Testing Utilities for the Vox Compiler Pipeline.

This crate contains common setup, execution, and verification logic
for end-to-end testing of the Vox toolchain, starting from source code
all the way down to emitted executable code or LSP responses.

# Usage
These utilities are used internally by the `tests/` directory and can
be leveraged for adding custom integration benchmarks or stress-tests.


### `fn parse_str_unwrap`

Compiles source code to a Module, asserting that there are no parser errors.

This provides a quick way to verify that a pipeline handles basic syntax.


### `fn typecheck_str`

Fully typechecks the provided source string and returns the module and diagnostics.


### `fn assert_typechecks_cleanly`

Typechecks the provided string and asserts there are no type errors.
Warnings are permitted.


