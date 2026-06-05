//! CR-F1 — Behavioral golden gate.
//!
//! Where `golden_vox_test_runner.rs` runs `@test` fns and `golden_typecheck_gate.rs`
//! typechecks, THIS gate verifies that goldens which produce output actually
//! **execute and print the expected bytes** under `vox run --mode interp` — the
//! real user-facing path. It closes the "green means it parsed, not that it ran"
//! gap identified in
//! `docs/src/architecture/v1-foundation-criteria-advisory-2026-06-05.md`.
//!
//! Convention: a golden declares its expected stdout with one or more
//! `// EXPECT:` comment lines, in order. Example:
//!
//! ```text
//! // EXPECT: Decimal verification successful
//! pub fn main() { print("Decimal verification successful") }
//! ```
//!
//! The gate runs the file, normalizes line endings + trailing whitespace, and
//! asserts the captured stdout equals the concatenated EXPECT block.
//!
//! Binary discovery: uses `$VOX_BIN` if set, else `target/<profile>/vox[.exe]`.
//! If the binary is absent the gate fails loudly with a build hint (it must not
//! silently pass — a behavioral gate that no-ops is the thing we are replacing).

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn vox_binary() -> PathBuf {
    if let Ok(p) = std::env::var("VOX_BIN") {
        return PathBuf::from(p);
    }
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    // Prefer debug (test default); fall back to release.
    let root = repo_root();
    let debug = root.join("target").join("debug").join(exe);
    if debug.exists() {
        return debug;
    }
    root.join("target").join("release").join(exe)
}

fn collect_golden_vox(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_vox_recursive(&root.join("examples").join("golden"), &mut files);
    files.sort();
    files
}

fn collect_vox_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_vox_recursive(&p, out);
            } else if p.extension().is_some_and(|e| e == "vox") {
                out.push(p);
            }
        }
    }
}

/// Collect `// EXPECT:` lines (order-preserving) into an expected-stdout block.
/// Returns `None` if the file declares no EXPECT lines.
fn parse_expect(src: &str) -> Option<String> {
    let lines: Vec<String> = src
        .lines()
        .filter_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("// EXPECT:")
                .map(|rest| rest.strip_prefix(' ').unwrap_or(rest).to_string())
        })
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

/// CRLF→LF; strip trailing whitespace/newlines so the comparison is
/// platform- and trailing-newline-insensitive.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim_end().to_string()
}

fn run_interp(vox: &Path, file: &Path) -> Result<String, String> {
    let out = Command::new(vox)
        .args(["run", "--mode", "interp"])
        .arg(file)
        .output()
        .map_err(|e| format!("spawn `{}` failed: {e}", vox.display()))?;
    if !out.status.success() {
        return Err(format!(
            "non-zero exit {:?}\n--- stderr ---\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The gate. Every golden with an `// EXPECT:` block must execute and match.
#[test]
fn golden_expect_blocks_match_interp_stdout() {
    let root = repo_root();
    let vox = vox_binary();
    assert!(
        vox.exists(),
        "vox binary not found at {} — build it with `cargo build -p vox-cli --bin vox` \
         or set VOX_BIN. A behavioral gate must never silently pass.",
        vox.display()
    );

    let files = collect_golden_vox(&root);
    let mut checked = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let Some(expected) = parse_expect(&src) else {
            continue;
        };
        checked += 1;
        let label = f
            .strip_prefix(&root)
            .unwrap_or(f)
            .to_string_lossy()
            .into_owned();
        match run_interp(&vox, f) {
            Ok(stdout) => {
                let got = normalize(&stdout);
                let want = normalize(&expected);
                // Exact-match on the EXPECT block. Programs are authored to print
                // exactly their EXPECT content; if a runtime banner is ever added
                // to stdout this assertion will catch it (by design).
                if got != want {
                    failures.push(format!(
                        "  MISMATCH {label}\n    expected: {want:?}\n    got:      {got:?}"
                    ));
                }
            }
            Err(e) => failures.push(format!("  RUN-FAIL {label}\n    {e}")),
        }
    }

    assert!(
        checked > 0,
        "no goldens with `// EXPECT:` blocks found — the behavioral corpus is empty"
    );

    assert!(
        failures.is_empty(),
        "{} behavioral golden(s) failed (CR-F1):\n{}",
        failures.len(),
        failures.join("\n")
    );

    println!("[golden_behavioral_gate] {checked} EXPECT golden(s) executed and matched ✓");
}

/// Informational census: prints behavioral coverage so the path to CR-F1's
/// 1.0 coverage target is visible. Asserts only that the EXPECT corpus is
/// non-empty (the hard coverage ratchet is added once main()-goldens are
/// proven runnable under interp).
#[test]
fn golden_behavioral_coverage_census() {
    let root = repo_root();
    let files = collect_golden_vox(&root);
    let (mut has_expect, mut has_test, mut has_main, mut bare) = (0usize, 0usize, 0usize, 0usize);
    let mut uncovered: Vec<String> = Vec::new();

    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let expect = parse_expect(&src).is_some();
        let test = src.contains("@test");
        let main = src.contains("fn main");
        if expect {
            has_expect += 1;
        }
        if test {
            has_test += 1;
        }
        if main {
            has_main += 1;
        }
        if !expect && !test {
            bare += 1;
            uncovered.push(
                f.strip_prefix(&root)
                    .unwrap_or(f)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    println!(
        "[golden_behavioral_gate] census over {} goldens: EXPECT={has_expect} @test={has_test} \
         fn-main={has_main} | behaviorally-uncovered (no EXPECT, no @test)={bare}",
        files.len()
    );
    if !uncovered.is_empty() {
        println!("[golden_behavioral_gate] uncovered goldens (CR-F1 coverage backlog):");
        for u in &uncovered {
            println!("    - {u}");
        }
    }

    assert!(
        has_expect > 0,
        "CR-F1 requires at least one golden carrying an `// EXPECT:` block"
    );
}

#[cfg(test)]
mod self_tests {
    use super::*;

    /// Proves the gate is not a no-op: the EXPECT parser and comparator must
    /// accept a real match and reject a mismatch.
    #[test]
    fn expect_parsing_and_comparison_are_real() {
        let src = "// EXPECT: hello world\n// EXPECT: line two\npub fn main() {}\n";
        let parsed = parse_expect(src).expect("should parse EXPECT lines");
        assert_eq!(parsed, "hello world\nline two");

        // No EXPECT lines → None.
        assert_eq!(parse_expect("pub fn main() {}"), None);

        // Normalization: CRLF and trailing newline insensitivity.
        assert_eq!(
            normalize("hello world\r\nline two\r\n"),
            "hello world\nline two"
        );

        // A mismatch must NOT compare equal (the property the gate relies on).
        assert_ne!(normalize("hello world"), normalize("goodbye world"));
    }
}
