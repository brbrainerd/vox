//! Proves `WasmExecOpts.env` actually crosses the WASI boundary into the guest.
//!
//! The module calls `environ_sizes_get(ptr_count, ptr_size)` then
//! `proc_exit(count)`, so the process exit code equals the number of env vars the
//! guest observes — a clean signal that env forwarding works end-to-end without
//! decoding strings. This underpins the mesh worker's WASM-sandbox secret
//! forwarding (`vox wasm run --env` → here → guest).

use std::io::Write;
use tempfile::NamedTempFile;
use vox_wasm_engine::{WasmExecOpts, WasmHost};

/// `_start`: `environ_sizes_get(0, 4)` then `proc_exit(*0)` ⇒ exit code == env count.
fn env_count_to_exit_module() -> Vec<u8> {
    wat::parse_str(
        r#"(module
  (import "wasi_snapshot_preview1" "environ_sizes_get" (func $sz (param i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
  (memory (export "memory") 1)
  (func (export "_start")
    (drop (call $sz (i32.const 0) (i32.const 4)))
    (call $exit (i32.load (i32.const 0)))))"#,
    )
    .expect("env-count wat module parses")
}

fn run_with_env(env: Vec<(String, String)>) -> i32 {
    let wasm = env_count_to_exit_module();
    let mut f = NamedTempFile::new().expect("tmp");
    f.write_all(&wasm).expect("write wasm");
    let host = WasmHost::new().expect("host");
    let opts = WasmExecOpts {
        args: Vec::new(),
        preopens: Vec::new(),
        fuel_override: None,
        stdin: None,
        env,
    };
    host.execute(f.path(), &opts).expect("execute").exit_code
}

#[test]
fn guest_observes_exactly_the_forwarded_env_vars() {
    assert_eq!(run_with_env(vec![]), 0, "no env forwarded ⇒ guest sees 0");
    assert_eq!(
        run_with_env(vec![("A".into(), "1".into())]),
        1,
        "one forwarded env var ⇒ guest sees exactly 1"
    );
    assert_eq!(
        run_with_env(vec![("A".into(), "1".into()), ("B".into(), "2".into())]),
        2,
        "two forwarded env vars ⇒ guest sees exactly 2"
    );
}
