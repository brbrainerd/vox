//! Shared test fixtures for the `vox-wasm-engine` integration tests.
//!
//! Cargo treats each `.rs` file under `tests/` as a separate integration target,
//! so the `mod common;` declaration must appear in every test that uses these
//! helpers. The `#[allow(dead_code)]` is required because Cargo type-checks
//! the module against each target individually and warns if any single test
//! file doesn't use a given helper.

#![allow(dead_code)]

/// Minimal WASI module that calls `proc_exit(0)`. Exported to confirm WASI
/// imports are satisfied and the module runs to completion.
pub fn minimal_wasi_exit_success_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
  (import "wasi_snapshot_preview1" "proc_exit" (func (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    i32.const 0
    call 0
  )
)"#,
    )
    .expect("wat parse minimal WASI module")
}
