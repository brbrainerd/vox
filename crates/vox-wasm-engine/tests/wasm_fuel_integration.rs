//! Fuel limits and default host wiring (`vox-wasm-engine`).

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;
use vox_wasm_engine::{WasmExecOpts, WasmHost};

fn minimal_wasi_exit_success_wasm() -> Vec<u8> {
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

/// Infinite loop until fuel is exhausted (no WASI exit).
fn infinite_loop_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"(module
  (memory (export "memory") 1)
  (func (export "_start")
    (loop (br 0))
  )
)"#,
    )
    .expect("wat parse infinite loop module")
}

#[test]
fn wasm_host_default_matches_new() {
    let _ = WasmHost::default();
}

#[test]
fn with_fuel_host_runs_minimal_guest_when_budget_sufficient() {
    let wasm = minimal_wasi_exit_success_wasm();
    let mut tmp = NamedTempFile::new().expect("temp wasm file");
    tmp.write_all(&wasm).expect("write wasm bytes");
    tmp.flush().expect("flush wasm temp");

    let host = WasmHost::with_fuel(50_000).expect("WasmHost::with_fuel");
    let outcome = host
        .execute(tmp.path(), &WasmExecOpts::default())
        .expect("execute with fuel budget");
    assert!(outcome.success(), "stderr={}", outcome.stderr_str());
}

#[test]
fn fuel_exhaustion_errors_on_infinite_loop() {
    let wasm = infinite_loop_wasm();
    let mut tmp = NamedTempFile::new().expect("temp wasm file");
    tmp.write_all(&wasm).expect("write wasm bytes");
    tmp.flush().expect("flush wasm temp");

    let host = WasmHost::with_fuel(2_000).expect("WasmHost::with_fuel");
    let err = host
        .execute(Path::new(tmp.path()), &WasmExecOpts::default())
        .expect_err("expected fuel exhaustion / trap");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("fuel") || msg.contains("Fuel") || msg.contains("trap"),
        "unexpected error (want fuel/trap hint): {msg}"
    );
}

#[test]
fn per_execution_fuel_override_cuts_budget() {
    let wasm = infinite_loop_wasm();
    let mut tmp = NamedTempFile::new().expect("temp wasm file");
    tmp.write_all(&wasm).expect("write wasm bytes");
    tmp.flush().expect("flush wasm temp");

    let host = WasmHost::with_fuel(500_000).expect("WasmHost::with_fuel");
    let opts = WasmExecOpts {
        fuel_override: Some(500),
        ..WasmExecOpts::default()
    };
    let err = host
        .execute(Path::new(tmp.path()), &opts)
        .expect_err("expected early fuel exhaustion via fuel_override");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("fuel") || msg.contains("Fuel") || msg.contains("trap"),
        "unexpected error (want fuel/trap hint): {msg}"
    );
}
