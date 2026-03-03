# Phase 10: Auto-Debugging via Toestub Gate

- **10.1 Diagnostic Integration**: Added `std::process::Command` to execute `cargo test --workspace` automatically in `vox-orchestrator`'s TOESTUB gate when any `.rs` files or `Cargo.toml` change. Also added `tower-lsp` processing to ingest `vox-typeck` and `vox-hir` errors via `vox-lsp::validate_document()` if any `.vox` components undergo modification.
- **10.2 Auto-Debug Loop**: Enhanced the Orchestrator with `debug_iterations` counter per task. Failed checks up to `max_debug_iterations` (configurable) automatically re-emit to the queue directly routing to the exact designated `Agent` along with verbose injected error lines appended straight to the task prompt block strings.
- **10.4 Debugging Tools**: Augmented `vox-mcp` with test routines:
  - `vox_run_tests` (`cargo test -p`) 
  - `vox_test_all` (`cargo test --workspace`)
  - `vox_check_workspace` (`cargo check --workspace`)
  - Registered and cleanly mapping to JSON responses for agents. All endpoints currently integrated and passing local target testing.
