---
title: "Testing Standard — SSOT"
description: "Official documentation for Testing Standard — SSOT for the Vox language. Detailed technical reference, architecture guides, and implement"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Testing Standard — SSOT

This document is the **Single Source of Truth** for how tests are organized, named, and structured across all 51 crates in the Vox workspace.

> [!IMPORTANT]
> All new tests and test refactors must conform to this standard. PRs that introduce new `dummy_span()` definitions, `_tests.rs` naming, or tests inside `src/` files will be flagged by TOESTUB.

## 1. File Naming

Use the **`_test.rs`** suffix (singular) for all test files:

| Context | Pattern | Example |
|---|---|---|
| Unit (inline) | `#[cfg(test)] mod tests { ... }` at bottom of file | `src/unify.rs` → `mod tests {}` |
| Integration | `tests/<feature>_test.rs` | `tests/scope_test.rs` |
| End-to-end | `vox-integration-tests/tests/<domain>_test.rs` | `tests/pipeline_ts_codegen_test.rs` |

**Never use `_tests.rs`** (plural). Never create `tests_*.rs` source files inside `src/`.

## 2. Test Placement Rules

### Unit tests (`#[cfg(test)] mod tests`)
- Test private internals; live inline in the source file.
- Maximum **150 lines** per inline test module.
- If a module tests only the public API and exceeds 50 lines → extract to `tests/`.

### Integration tests (`tests/*.rs`)
- Test the public API of the crate.
- Each file covers one **feature domain**, not a mix.
- Never put multiple unrelated subsystems in one test file.

### End-to-end tests (`vox-integration-tests/tests/`)
- Cross-crate pipeline scenarios (lex → parse → hir → typeck → codegen).
- Grouped by pipeline phase or language feature area.
- **Do not put 20+ tests in a single file** (sign of a God file).

## 3. Shared Test Infrastructure

All shared test builders and assertion helpers live in **`vox-test-harness`**.

```rust
// ✅ Correct — import from shared harness
use vox_test_harness::spans::dummy_span;
use vox_test_harness::hir_builders::minimal_hir_module;
use vox_test_harness::assertions::{has_error, error_messages};
use vox_test_harness::pipeline::{parse_str_unwrap, typecheck_str};

// ❌ Wrong — define locally
fn dummy_span() -> Span { Span { start: 0, end: 0 } }
```

Never define `dummy_span()`, `minimal_module()`, `module_with_fn()`, or similar helpers locally in test files.

## 4. Test Function Naming

| Location | Pattern | Example |
|---|---|---|
| Inline `mod tests` | `test_<unit>_<scenario>` | `test_unify_simple_int` |
| Integration (`tests/`) | `<feature>_<scenario>` | `scope_affinity_group_routing` |
| B-ticket regression | `b<NNN>_<description>` | `b090_vox_init_creates_expected_scaffold` |

## 5. Anti-Patterns (Banned)

| Anti-Pattern | Resolution |
|---|---|
| `fn dummy_span()` defined locally | Import from `vox_test_harness::spans` |
| `fn minimal_module()` defined locally | Import from `vox_test_harness::hir_builders` |
| Test file named `*_tests.rs` | Rename to `*_test.rs` |
| `tests_*.rs` file inside `src/` | Move to `tests/` directory |
| >20 tests in a single integration test file | Split by feature domain |
| Zero tests in a non-stub crate | Add smoke tests at minimum |

## 6. Crate Test Coverage Requirements

| Crate Tier | Requirement |
|---|---|
| Compiler pipeline (lexer, parser, hir, typeck, codegen) | Full unit + integration coverage |
| Runtime, orchestrator, MCP | Unit coverage of all public API + integration smoke tests |
| CLI commands | Integration test for each subcommand happy path |
| Future/stub crates (`vox-codegen-llvm`, `vox-codegen-wasm`) | Exempt until implementation begins |

## 7. Running Tests

```bash
# All tests
cargo test --workspace

# Single crate
cargo test -p vox-<crate>

# Specific integration test file
cargo test -p vox-integration-tests --test pipeline_ts_codegen_test

# Shared harness
cargo test -p vox-test-harness
```

## 8. References

- [vox-integration-tests API](../reference/cli.md)
- [documentation-rubric](../../agents/documentation-rubric.md)
- [CI runner contract](../ci/runner-contract.md)


