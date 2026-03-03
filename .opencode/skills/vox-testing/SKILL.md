---
name: vox-testing
description: Testing patterns and commands for the Vox workspace — unit tests, integration tests, property tests, linting
---

## Quick Commands

| Command | Purpose |
|---------|---------|
| `cargo test --workspace` | Full test suite |
| `cargo test -p vox-orchestrator` | Single crate tests |
| `cargo test -p vox-integration-tests` | Integration tests |
| `cargo clippy --workspace --tests -- -D warnings` | Lint check |
| `cargo fmt --check` | Format check |

## Test Organization

- **Unit tests**: `#[cfg(test)]` modules within each crate source
- **Integration tests**: `crates/vox-integration-tests/tests/*.rs`
- **Property tests**: `proptest` in `crates/vox-orchestrator/tests/stress_tests.rs`
- **E2E orchestrator**: `crates/vox-integration-tests/tests/orchestrator_e2e.rs`

## Patterns

- No `.unwrap()` in non-test code
- Use `miette` for user-facing errors
- Use `proptest` for randomized/stress tests
- Test the full pipeline: parse → HIR → check → codegen for new features
