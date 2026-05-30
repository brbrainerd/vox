//! Shared helpers for the CLI integration test harness (Phase 0.3 of
//! [mobile-rn-expo-implementation-spec-2026.md](../../docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md) §1).
//!
//! The harness runs `cargo run -p vox-cli -- build <fixture>.vox -o <tempdir>` as a subprocess
//! and asserts the emitted output is real, not stubbed. Each fixture sits in
//! `tests/fixtures/<name>/` with a `main.vox` source and an `expected_files.toml` listing
//! filenames that MUST appear in the output directory.
//!
//! The harness checks five things per fixture:
//!
//! 1. **Exit code 0.** The CLI must succeed.
//! 2. **No `panicked at` in stderr.** Catches the §0.1 class of bugs (library-works,
//!    CLI-panics) that snapshot tests miss.
//! 3. **Expected files exist** in the output directory.
//! 4. **TypeScript compiles** — every emitted `*.tsx`/`*.ts` passes `npx tsc --noEmit`
//!    against a shared `tests/tsconfig.json`.
//! 5. **Rust compiles** (when the build target produces `target/generated/Cargo.toml`) —
//!    `cargo check --manifest-path target/generated/Cargo.toml` succeeds.
//!
//! Tsc + cargo-check require network/disk access; they are gated behind environment
//! variables (`VOX_CLI_TESTS_SKIP_TSC=1`, `VOX_CLI_TESTS_SKIP_CARGO=1`) so CI can opt
//! out per-step when the toolchain isn't available.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Shape of `expected_files.toml` next to each fixture's `main.vox`.
#[derive(Debug, Deserialize)]
pub struct ExpectedFiles {
    /// Files that MUST appear in the `--out-dir` after `vox build`.
    #[serde(default)]
    pub required: Vec<String>,
    /// Filenames forbidden from appearing (typically stale artifacts that prior builds
    /// would have left behind).
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// Whether this fixture is expected to produce a `target/generated/` Rust project.
    /// When true, the harness runs `cargo check` against the emitted Cargo.toml.
    #[serde(default)]
    pub emits_rust_backend: bool,
    /// Optional CLI flags forwarded to `vox build` (e.g. `--target client`).
    #[serde(default)]
    pub extra_args: Vec<String>,
}

/// One end-to-end run of `vox build` against a fixture.
pub struct BuildRun {
    pub fixture_name: String,
    pub out_dir: tempfile::TempDir,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub expected: ExpectedFiles,
}

impl BuildRun {
    /// Locate the workspace root by walking up until we find a `Cargo.toml` with `[workspace]`.
    pub fn workspace_root() -> PathBuf {
        let mut current = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            let candidate = current.join("Cargo.toml");
            if candidate.is_file() {
                let text = std::fs::read_to_string(&candidate).unwrap_or_default();
                if text.contains("[workspace]") {
                    return current;
                }
            }
            if !current.pop() {
                panic!(
                    "workspace root not found from {}",
                    env!("CARGO_MANIFEST_DIR")
                );
            }
        }
    }

    /// Path to a fixture directory inside this crate's `tests/fixtures/`.
    pub fn fixture_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    /// Path to the `vox` binary cargo built before our tests started running.
    ///
    /// vox-cli is declared as a `[dev-dependencies]` entry in our Cargo.toml — cargo
    /// builds it during test setup, depositing the binary at
    /// `<workspace>/target/debug/vox{ .exe }`. We compute that path here rather than
    /// running `cargo build` from inside the test (which would race the outer
    /// `cargo test`'s target-directory file lock).
    pub fn vox_binary_path() -> &'static Path {
        static PATH: OnceLock<PathBuf> = OnceLock::new();
        PATH.get_or_init(|| {
            let ext = if cfg!(windows) { ".exe" } else { "" };
            let bin = Self::workspace_root()
                .join("target")
                .join("debug")
                .join(format!("vox{ext}"));
            assert!(
                bin.is_file(),
                "vox binary not found at {} — ensure vox-cli is declared as a dev-dep so cargo \
                 builds it during test setup",
                bin.display()
            );
            bin
        })
    }

    /// Run `vox build` against a fixture and capture the result.
    pub fn run(name: &str) -> Self {
        let fixture = Self::fixture_dir(name);
        assert!(
            fixture.is_dir(),
            "missing fixture dir: {}",
            fixture.display()
        );
        let source = fixture.join("main.vox");
        assert!(
            source.is_file(),
            "missing fixture source: {}",
            source.display()
        );

        let expected_path = fixture.join("expected_files.toml");
        let expected: ExpectedFiles = if expected_path.is_file() {
            let text = std::fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", expected_path.display()));
            toml::from_str(&text)
                .unwrap_or_else(|e| panic!("parse {}: {e}", expected_path.display()))
        } else {
            ExpectedFiles {
                required: vec![],
                forbidden: vec![],
                emits_rust_backend: false,
                extra_args: vec![],
            }
        };

        let out_dir = tempfile::tempdir().expect("create out tempdir");

        let bin = Self::vox_binary_path();
        let mut cmd = Command::new(bin);
        cmd.arg("build").arg(&source).arg("-o").arg(out_dir.path());
        for a in &expected.extra_args {
            cmd.arg(a);
        }
        cmd.current_dir(Self::workspace_root());

        let output = cmd.output().expect("spawn cargo run vox-cli build");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        Self {
            fixture_name: name.to_string(),
            out_dir,
            stdout,
            stderr,
            success,
            expected,
        }
    }

    /// (1) Exit code 0.
    pub fn assert_success(&self) {
        assert!(
            self.success,
            "vox build {} failed.\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.fixture_name, self.stdout, self.stderr
        );
    }

    /// (2) No `panicked at` in stderr. Regression gate for §0.1-class bugs.
    pub fn assert_no_panic(&self) {
        assert!(
            !self.stderr.contains("panicked at"),
            "vox build {} produced a panic in stderr:\n{}",
            self.fixture_name,
            self.stderr
        );
    }

    /// (3) Expected files present, forbidden files absent.
    pub fn assert_expected_files(&self) {
        for name in &self.expected.required {
            let p = self.out_dir.path().join(name);
            assert!(
                p.is_file(),
                "vox build {} missing required output file {:?}\n--- files in out dir ---\n{}",
                self.fixture_name,
                name,
                list_dir(self.out_dir.path()).join("\n")
            );
        }
        for name in &self.expected.forbidden {
            let p = self.out_dir.path().join(name);
            assert!(
                !p.exists(),
                "vox build {} produced forbidden file {:?}",
                self.fixture_name,
                name
            );
        }
    }

    /// (4) Every emitted `*.tsx`/`*.ts` compiles under `tsc --noEmit`.
    ///
    /// Skipped when `VOX_CLI_TESTS_SKIP_TSC=1` is set. Honors `tsc` if on PATH;
    /// otherwise falls back to `npx tsc` (assumes Node + npm + write permission on
    /// a temporary npx cache). When neither is available, prints a warning and
    /// returns without asserting.
    pub fn assert_tsc_compiles(&self) {
        if std::env::var_os("VOX_CLI_TESTS_SKIP_TSC").is_some() {
            return;
        }
        let ts_files: Vec<PathBuf> = walk_files(self.out_dir.path())
            .into_iter()
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e == "ts" || e == "tsx")
                    .unwrap_or(false)
            })
            .collect();
        if ts_files.is_empty() {
            return;
        }

        let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let tsconfig = tests_dir.join("tsconfig.json");
        if !tsconfig.is_file() {
            eprintln!(
                "warning: vox-cli-tests tsconfig missing at {}; skipping tsc check for {}",
                tsconfig.display(),
                self.fixture_name
            );
            return;
        }

        // Ensure node_modules exists next to the tsconfig so emitted imports of
        // `react`, `@tauri-apps/api`, `@tanstack/react-query`, etc. resolve.
        // Idempotent once-per-test-process via OnceLock; npm install is slow on
        // first run, but the cache makes subsequent runs free.
        ensure_node_modules_installed(&tests_dir);

        let local_tsc = which_executable("tsc");
        let (program, prefix_args): (String, Vec<String>) = match local_tsc {
            Some(p) => (p.to_string_lossy().into_owned(), vec![]),
            None => match which_executable("npx") {
                Some(p) => (
                    p.to_string_lossy().into_owned(),
                    // npx 7+ form: `--yes -p typescript@5 -- tsc <args>`
                    // (`-p` pins the package; the `--` separates npx flags from the
                    // executable name + its args.)
                    vec![
                        "--yes".to_string(),
                        "-p".to_string(),
                        "typescript@5".to_string(),
                        "--".to_string(),
                        "tsc".to_string(),
                    ],
                ),
                None => {
                    eprintln!(
                        "warning: neither `tsc` nor `npx` on PATH; skipping tsc check for {}",
                        self.fixture_name
                    );
                    return;
                }
            },
        };
        // Drop the harness tsconfig into the out dir so `tsc` auto-discovers it and
        // applies our relaxed `noImplicitAny: false` / `jsx: react-jsx` options to
        // the emitted source. (`--project` + explicit source files is a tsc error.)
        let tsconfig_dst = self.out_dir.path().join("tsconfig.json");
        std::fs::copy(&tsconfig, &tsconfig_dst).unwrap_or_else(|e| {
            panic!(
                "copy tsconfig {} → {}: {e}",
                tsconfig.display(),
                tsconfig_dst.display()
            )
        });

        // Point tsc's module resolution at the shared node_modules so emitted
        // `import { ... } from "react"` resolves without per-fixture npm install.
        let shared_node_modules = tests_dir.join("node_modules");
        let mut cmd = Command::new(&program);
        for a in &prefix_args {
            cmd.arg(a);
        }
        cmd.arg("--noEmit");
        cmd.arg("--project");
        cmd.arg(self.out_dir.path());
        cmd.arg("--baseUrl");
        cmd.arg(&shared_node_modules);
        cmd.env(
            "NODE_PATH",
            shared_node_modules
                .to_str()
                .expect("non-utf8 node_modules path"),
        );
        let output = cmd
            .output()
            .unwrap_or_else(|e| panic!("spawn tsc for {}: {e}", self.fixture_name));
        if !output.status.success() {
            panic!(
                "tsc --noEmit failed for {}\n--- tsc stdout ---\n{}\n--- tsc stderr ---\n{}",
                self.fixture_name,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// (5) Generated Rust backend compiles under `cargo check`.
    ///
    /// Skipped when `VOX_CLI_TESTS_SKIP_CARGO=1` is set or when the fixture's
    /// `emits_rust_backend = false`.
    pub fn assert_cargo_check(&self) {
        if std::env::var_os("VOX_CLI_TESTS_SKIP_CARGO").is_some() {
            return;
        }
        if !self.expected.emits_rust_backend {
            return;
        }
        let cargo_toml = Self::workspace_root()
            .join("target")
            .join("generated")
            .join("Cargo.toml");
        if !cargo_toml.is_file() {
            eprintln!(
                "warning: fixture {} declared emits_rust_backend=true but no Cargo.toml at {}",
                self.fixture_name,
                cargo_toml.display()
            );
            return;
        }
        let output = Command::new("cargo")
            .arg("check")
            .arg("--manifest-path")
            .arg(&cargo_toml)
            .output()
            .unwrap_or_else(|e| panic!("spawn cargo check for {}: {e}", self.fixture_name));
        if !output.status.success() {
            panic!(
                "cargo check failed for generated backend of fixture {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                self.fixture_name,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// Run all five assertions in order. Convenience for the standard fixture flow.
    pub fn assert_all(&self) {
        self.assert_success();
        self.assert_no_panic();
        self.assert_expected_files();
        self.assert_tsc_compiles();
        self.assert_cargo_check();
    }
}

fn list_dir(p: &Path) -> Vec<String> {
    std::fs::read_dir(p)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// Run `npm install` in `tests/` so emitted TypeScript can resolve `react`,
/// `@tauri-apps/api`, etc. Idempotent across the test process — first call
/// runs `npm install`; subsequent calls are no-ops.
fn ensure_node_modules_installed(tests_dir: &Path) {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let package_json = tests_dir.join("package.json");
        if !package_json.is_file() {
            eprintln!(
                "warning: vox-cli-tests package.json missing at {}; tsc imports will not resolve",
                package_json.display()
            );
            return;
        }
        let node_modules = tests_dir.join("node_modules");
        // Cheap heuristic: if `react/package.json` exists in node_modules, assume install is done.
        if node_modules.join("react").join("package.json").is_file() {
            return;
        }
        let npm = which_executable("npm").unwrap_or_else(|| {
            panic!(
                "npm not found on PATH; either install Node or set VOX_CLI_TESTS_SKIP_TSC=1 \
                 (was looking for {package_json})",
                package_json = package_json.display()
            )
        });
        let status = Command::new(&npm)
            .arg("install")
            .arg("--no-audit")
            .arg("--no-fund")
            .arg("--no-progress")
            .arg("--silent")
            .current_dir(tests_dir)
            .status()
            .expect("spawn npm install");
        assert!(
            status.success(),
            "npm install in {} failed",
            tests_dir.display()
        );
    });
}

fn which_executable(name: &str) -> Option<PathBuf> {
    // On Windows prefer the .cmd/.exe/.bat shims first. A bare-name file (no
    // extension) on Windows is usually a shell script that cannot exec as a
    // Win32 application — picking it instead of the .cmd shim raises
    // "os error 193" at spawn time.
    let exts: &[&str] = if cfg!(windows) {
        &[".cmd", ".exe", ".bat", ""]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
