//! CR-F2a — **logic parity** gate: codegen-rust (`--mode script`) must produce
//! the same stdout as the interpreter for every executable logic golden.
//!
//! The interpreter arm is already gated (`golden_behavioral_gate.rs`, 10/10) and
//! the golden's `// EXPECT:` block IS the interpreter's verified output, so this
//! gate compares **codegen-rust output to `// EXPECT:`** directly — no second
//! interpreter run needed.
//!
//! Robust by construction (per the Phase 1 re-audit findings):
//! - generates the Rust script crate and builds it in a **dedicated, stable
//!   `CARGO_TARGET_DIR` under the OS temp dir** — NOT `~/.vox/script-target`, so
//!   one golden's build failure cannot poison the others, and dep compiles cache
//!   across runs;
//! - runs the **built binary directly**, so stdout is clean (the `INFO
//!   vox.script:` tracing line is printed by the `vox` CLI dispatch, not the
//!   program);
//! - ratchets against a committed allowlist of currently-diverging goldens
//!   (`contracts/eval/arm-parity-allowlist-script.txt`); a NEW divergence fails.
//!
//! `#[ignore]` — compiles generated crates (cold first build is minutes). Run:
//!   cargo test -p vox-integration-tests --test golden_arm_parity_test -- --ignored --nocapture
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use std::process::Command;

use vox_codegen::codegen_rust::generate_script;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

fn golden_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden")
}

fn runtime_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vox-actor-runtime")
}

fn allowlist_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/eval/arm-parity-allowlist-script.txt")
}

/// `examples/golden/**/*.vox` with both `fn main(` and a `// EXPECT:` line.
fn collect_logic_goldens() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![golden_root()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "vox") {
                if let Ok(src) = std::fs::read_to_string(&p) {
                    if src.contains("fn main(") && src.contains("// EXPECT:") {
                        out.push(p);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

/// Concatenated `// EXPECT:` payload (source order), trailing-trimmed.
fn parse_expect(src: &str) -> String {
    src.lines()
        .filter_map(|l| l.trim_start().strip_prefix("// EXPECT:"))
        .map(|r| r.trim())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

/// Golden path relative to `examples/golden/`, forward-slashed (allowlist key).
fn rel_key(p: &Path) -> String {
    p.strip_prefix(golden_root())
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Generate → build → run the codegen-rust script crate for `src`. Returns the
/// program's trimmed stdout, or a one-line error (parse / codegen / build / run).
fn build_and_run_script(src: &str) -> Result<String, String> {
    let module = parse_script(lex(src)).map_err(|e| format!("parse: {e:?}"))?;
    let hir = lower_module(&module);
    let output = generate_script(&hir, "vox-script", Some(&runtime_path()))
        .map_err(|e| format!("codegen: {e}"))?;

    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    output
        .write_to_dir(dir.path())
        .map_err(|e| format!("write_to_dir: {e}"))?;

    // Dedicated, stable target dir — isolated from ~/.vox/script-target and
    // shared across goldens so deps compile once.
    let target_dir = std::env::temp_dir().join("vox-f2a-logic-parity-target");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let build = Command::new(&cargo)
        .current_dir(dir.path())
        .arg("build")
        .env("CARGO_TARGET_DIR", &target_dir)
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        // Collect ALL real rustc diagnostic lines (`error[E0308]: ...` /
        // `error: ...`), not cargo progress like "Compiling thiserror", so the
        // full chain is visible per golden for batch diagnosis.
        let errors: Vec<&str> = stderr
            .lines()
            .map(str::trim_start)
            .filter(|l| l.starts_with("error[") || l.starts_with("error:"))
            .collect();
        let first = if errors.is_empty() {
            "build failed".to_string()
        } else {
            errors.join(" | ")
        };
        return Err(format!("build: {first}"));
    }

    let exe_name = if cfg!(windows) {
        "vox-script.exe"
    } else {
        "vox-script"
    };
    let exe = target_dir.join("debug").join(exe_name);
    let run = Command::new(&exe)
        .output()
        .map_err(|e| format!("spawn {}: {e}", exe.display()))?;
    if !run.status.success() {
        let stderr = String::from_utf8_lossy(&run.stderr);
        return Err(format!(
            "run exit {}: {}",
            run.status.code().unwrap_or(-1),
            stderr.lines().next().unwrap_or("")
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

/// Read the committed allowlist of currently-diverging goldens (one rel-key per
/// non-comment, non-blank line). Missing file ⇒ empty (seeding mode).
fn read_allowlist() -> Vec<String> {
    std::fs::read_to_string(allowlist_path())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
#[ignore = "compiles generated crates (cold build is minutes); run with --ignored — owner: integration-tests"]
fn codegen_rust_logic_parity_ratchet() {
    let goldens = collect_logic_goldens();
    assert!(!goldens.is_empty(), "no logic goldens found");
    let allowlist = read_allowlist();

    let mut diverged: Vec<(String, String)> = Vec::new();
    let mut passed = 0usize;

    println!("\n=== CR-F2a logic parity: codegen-rust vs `// EXPECT:` ===");
    for g in &goldens {
        let key = rel_key(g);
        let src = std::fs::read_to_string(g).expect("read golden");
        let expect = parse_expect(&src);
        match build_and_run_script(&src) {
            Ok(got) if got == expect => {
                passed += 1;
                println!("  {key:<30} OK");
            }
            Ok(got) => {
                println!("  {key:<30} DIVERGE  got={got:?} expect={expect:?}");
                diverged.push((key, format!("output {got:?} != {expect:?}")));
            }
            Err(e) => {
                println!("  {key:<30} FAIL     {e}");
                diverged.push((key, e));
            }
        }
    }
    println!(
        "=== codegen-rust logic parity: {}/{} pass; {} diverged (allowlist {}) ===\n",
        passed,
        goldens.len(),
        diverged.len(),
        allowlist.len()
    );

    // Ratchet: no divergence outside the committed allowlist, and the count must
    // not exceed the committed baseline.
    let new: Vec<&(String, String)> = diverged
        .iter()
        .filter(|(k, _)| !allowlist.contains(k))
        .collect();
    assert!(
        new.is_empty(),
        "NEW codegen-rust divergence(s) not in {}:\n{}",
        allowlist_path().display(),
        new.iter()
            .map(|(k, e)| format!("  {k}: {e}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        diverged.len() <= allowlist.len(),
        "divergence count {} exceeds allowlist baseline {}",
        diverged.len(),
        allowlist.len()
    );
}
