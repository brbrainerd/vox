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
#![allow(unsafe_code)] // EnvVarGuard wraps set_var/remove_var, isolated behind a Mutex

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// Strip the Windows `\\?\` UNC prefix that `canonicalize()` adds on Windows.
/// `cmd.exe` and many CLI tools cannot handle the extended-length path prefix.
pub fn strip_unc_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    // A verbatim UNC path (`\\?\UNC\server\share\...`) needs the `\\?\UNC\` prefix
    // converted to `\\`, not stripped outright — stripping it bare would produce
    // `UNC\server\share\...`, a relative path, not the `\\server\share\...` UNC
    // path it actually names. Check this case before the plain `\\?\` case below.
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{stripped}"))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
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

/// Serializes process-global env-var mutations across the `ts_emit_*` tests that touch
/// `VOX_WEBIR_VALIDATE` / `VOX_EMIT_ADMIN` / `VOX_ADMIN_REGISTRY`. `cargo nextest` (used by
/// CI and this crate's documented `--run-ignored` invocations) isolates each test into its
/// own process, so these vars never actually collide there — but this file's own doc
/// comment also documents a plain `cargo test -p vox-integration-tests -- --ignored` entry
/// point, which runs every test in the binary in one process across multiple threads by
/// default. Under that path, two tests setting different values for the same var can race.
static ENV_VAR_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard: holds [`ENV_VAR_LOCK`], sets each `(name, value)` pair, and restores each
/// var to its prior value when dropped — including when dropped during a panic unwind
/// (e.g. a panic inside a `rayon` batch running under this guard), so a failing test never
/// leaves a mutated env var for the next test to observe.
pub struct EnvVarGuard {
    _lock: MutexGuard<'static, ()>,
    prior: Vec<(String, Option<String>)>,
}

impl EnvVarGuard {
    pub fn set(vars: &[(&str, &str)]) -> Self {
        let lock = ENV_VAR_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut prior = Vec::with_capacity(vars.len());
        for (name, value) in vars {
            prior.push(((*name).to_string(), std::env::var(name).ok()));
            unsafe { std::env::set_var(name, value) };
        }
        Self { _lock: lock, prior }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for (name, value) in self.prior.drain(..) {
            match value {
                Some(v) => unsafe { std::env::set_var(&name, v) },
                None => unsafe { std::env::remove_var(&name) },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_unc_prefix_plain_verbatim() {
        let p = strip_unc_prefix(PathBuf::from(r"\\?\C:\Users\Owner\vox"));
        assert_eq!(p, PathBuf::from(r"C:\Users\Owner\vox"));
    }

    #[test]
    fn strip_unc_prefix_verbatim_unc_share() {
        let p = strip_unc_prefix(PathBuf::from(r"\\?\UNC\server\share\vox"));
        assert_eq!(p, PathBuf::from(r"\\server\share\vox"));
    }

    #[test]
    fn strip_unc_prefix_no_prefix_unchanged() {
        let p = strip_unc_prefix(PathBuf::from(r"C:\Users\Owner\vox"));
        assert_eq!(p, PathBuf::from(r"C:\Users\Owner\vox"));
    }
}
