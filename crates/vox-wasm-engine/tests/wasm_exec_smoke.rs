//! Integration smoke for WASI module execution (no network).

use std::io::Write;
use std::path::Path;

use tempfile::NamedTempFile;
use vox_wasm_engine::{WasmExecOpts, WasmHost};

mod common;
use common::minimal_wasi_exit_success_wasm;

#[test]
fn wasm_host_executes_minimal_wasi_module() {
    let wasm = minimal_wasi_exit_success_wasm();
    let mut tmp = NamedTempFile::new().expect("temp wasm file");
    tmp.write_all(&wasm).expect("write wasm bytes");
    tmp.flush().expect("flush wasm temp");

    let path = tmp.path().to_path_buf();
    assert!(path.exists(), "temp wasm path missing");

    let host = WasmHost::new().expect("WasmHost::new");
    let outcome = host
        .execute(Path::new(&path), &WasmExecOpts::default())
        .expect("execute minimal WASI guest");

    assert!(outcome.success(), "stderr={}", outcome.stderr_str());
    assert_eq!(outcome.exit_code, 0);
}
