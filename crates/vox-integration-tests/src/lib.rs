//! Integration test harness crate (no public API).
//!
//! Tests live under `crates/vox-integration-tests/tests/`. This library target exists so
//! integration tests can share helpers if needed without publishing a surface area.
//!
//! The `ts_emit_*` test files all gate on the same emitted-TypeScript `tsc --noEmit`
//! contract, so their shared plumbing (path resolution, fixture collection, the strict
//! tsconfig, and the node-direct `tsc` invocation) lives here once instead of being
//! copy-pasted per file — that copy-paste previously let each file's tsconfig drift
//! independently, which is exactly the risk the negative-control test exists to catch.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

/// Strip the Windows `\\?\` UNC prefix that `canonicalize()` adds on Windows.
/// `cmd.exe` and many CLI tools cannot handle the extended-length path prefix.
pub fn strip_unc_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p
    }
}

/// Absolute path to the `ts-noemit-scratch` dir that contains `node_modules` and the
/// base `tsconfig.json` used by every TS-emit-typecheck gate test.
pub fn ts_scratch_dir() -> PathBuf {
    strip_unc_prefix(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ts-noemit-scratch")
            .canonicalize()
            .expect("ts-noemit-scratch directory must exist"),
    )
}

/// Collect all `.vox` files directly under `dir` (non-recursive), sorted for determinism.
pub fn collect_vox_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "vox") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

/// The canonical strict `tsconfig.json` (`compilerOptions` + `include`) shared by every
/// TS-emit-typecheck gate test. The negative-control test's entire premise is proving
/// that THIS SAME config rejects bad TypeScript — keep it in exactly one place so that
/// invariant can't silently drift between the positive gate and the negative control.
pub fn strict_tsconfig_json() -> serde_json::Value {
    serde_json::json!({
        "compilerOptions": {
            "target": "ES2022",
            "module": "ESNext",
            "moduleResolution": "bundler",
            "strict": true,
            "noEmit": true,
            "jsx": "react-jsx",
            "skipLibCheck": true,
            "esModuleInterop": true,
            "isolatedModules": true,
            "lib": ["ES2022", "DOM", "DOM.Iterable"]
        },
        "include": ["./**/*.ts", "./**/*.tsx"]
    })
}

/// Resolve `tsc`'s CLI entrypoint under `scratch/node_modules/typescript/bin/tsc` and run
/// `tsc --noEmit --project <tsconfig>` by invoking `node` directly. The
/// `node_modules/.bin/tsc(.cmd)` shims rely on PATH resolution that fails under nextest
/// on Windows ('node' is not recognized); this is portable and shim-free.
pub fn run_tsc_noemit(scratch: &Path, tsconfig_path: &Path) -> std::process::Output {
    let tsc_js = scratch
        .join("node_modules")
        .join("typescript")
        .join("bin")
        .join("tsc");
    assert!(
        tsc_js.exists(),
        "TypeScript CLI missing at {}. Run: pnpm install --frozen-lockfile (from ts-noemit-scratch/)",
        tsc_js.display()
    );
    std::process::Command::new("node")
        .arg(&tsc_js)
        .arg("--noEmit")
        .arg("--project")
        .arg(tsconfig_path)
        .current_dir(scratch)
        .output()
        .expect("Failed to spawn `node` — is Node.js installed and on PATH?")
}
