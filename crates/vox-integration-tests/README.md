# vox-integration-tests

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
