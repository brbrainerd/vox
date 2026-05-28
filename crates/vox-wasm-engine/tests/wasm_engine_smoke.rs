//! Integration smoke for `vox-wasm-engine` public surface (no bundled `.wasm`).

use std::path::PathBuf;

use vox_wasm_engine::{Preopen, PreopenMode, WasmExecOpts, WasmHost, WasmRunOutcome};

#[test]
fn wasm_host_new_succeeds() {
    let host = WasmHost::new().expect("default WasmHost");
    drop(host);
}

#[test]
fn wasm_run_outcome_helpers() {
    let o = WasmRunOutcome {
        exit_code: 0,
        stdout: b"ok".to_vec(),
        stderr: Vec::new(),
        wall_ms: 1,
    };
    assert!(o.success());
    assert_eq!(o.stdout_str(), "ok");

    let fail = WasmRunOutcome {
        exit_code: 7,
        stdout: Vec::new(),
        stderr: vec![0xff, 0xfe],
        wall_ms: 2,
    };
    assert!(!fail.success());
    assert!(!fail.stderr_str().is_empty());
}

#[test]
fn preopen_builders_and_modes() {
    let ro = Preopen::read_only(PathBuf::from("/tmp"), "/guest");
    assert_eq!(ro.mode, PreopenMode::ReadOnly);
    assert_eq!(ro.guest, "/guest");

    let rw = Preopen::read_write("/var/data", ".");
    assert_eq!(rw.mode, PreopenMode::ReadWrite);
}

#[test]
fn wasm_exec_opts_with_args() {
    let opts = WasmExecOpts::with_args(["--help", "more"]);
    assert_eq!(opts.args, vec!["--help".to_string(), "more".to_string()]);
    assert!(opts.preopens.is_empty());
}
