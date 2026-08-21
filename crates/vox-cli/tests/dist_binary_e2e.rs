//! Black-box verification of the shipped `dist`-profile binary.
//!
//! Runs the binary as a subprocess, so nothing here needs a test harness linked
//! into it. `cargo test --profile dist` cannot serve this purpose: cargo ignores
//! the `panic` setting for test targets, and a full fat-LTO test lane would
//! fat-LTO-link each of the 80+ integration targets against a 1656-package graph
//! and OOM the 14 GB runner. See spec finding F8.

use std::path::PathBuf;
use std::process::Command;

/// Locate the dist binary.
///
/// CI sets `VOX_DIST_BIN` to the exact artifact under verification. When it is
/// set, a missing binary is a HARD FAILURE — silently skipping would make the
/// whole verification lane a no-op that reports green.
fn dist_binary() -> Option<PathBuf> {
    // An exported-but-empty value is a misconfigured workflow, not a missing
    // artifact — say so, rather than asserting on a blank path.
    if let Ok(raw) = std::env::var("VOX_DIST_BIN") {
        assert!(
            !raw.trim().is_empty(),
            "VOX_DIST_BIN is set but empty — the workflow that exported it did \
             not resolve a path"
        );
        let p = PathBuf::from(raw);
        assert!(
            p.is_file(),
            "VOX_DIST_BIN={} does not exist — the verification lane would \
             otherwise silently pass without testing anything",
            p.display()
        );
        return Some(p);
    }
    let exe = if cfg!(windows) { "vox.exe" } else { "vox" };
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/dist")
        .join(exe);
    p.exists().then_some(p)
}

fn run_dist(args: &[&str]) -> Option<(String, String, i32)> {
    let bin = dist_binary()?;
    let out = Command::new(&bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {} {:?}: {e}", bin.display(), args));
    Some((
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    ))
}

/// Proves the binary was actually built at [profile.dist]. Every other test here
/// would pass identically against a debug build; this one would not.
#[cfg(target_os = "linux")]
#[test]
fn dist_binary_is_stripped_of_symbols() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    let bytes = std::fs::read(&bin).expect("read dist binary");
    assert!(
        !bytes.windows(7).any(|w| w == b".symtab"),
        "dist binary retains a symbol table — it was not built at [profile.dist] \
         (strip = \"symbols\")"
    );
}

#[test]
fn dist_binary_version_matches_the_crate() {
    let Some((stdout, _, code)) = run_dist(&["--version"]) else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    assert_eq!(code, 0, "`vox --version` must exit 0");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "`vox --version` must report {}, got {stdout:?}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn dist_binary_rejects_unknown_subcommand_cleanly() {
    let Some((_, _, code)) = run_dist(&["definitely-not-a-real-subcommand"]) else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    // clap's parse-error path exits 2. A process killed by SIGABRT yields no
    // exit code at all, which `run_dist` surfaces as -1.
    assert!(
        code > 0 && code < 100,
        "unknown subcommand must exit with a normal error code; got {code} \
         (-1 means killed by a signal, i.e. an abort)"
    );
}

#[test]
fn dist_binary_compiles_and_runs_a_golden_program() {
    let Some(bin) = dist_binary() else {
        eprintln!("SKIP: dist binary unavailable");
        return;
    };
    // A unique dir per run: two concurrent fleet jobs would otherwise race on
    // the same hello.vox, and a fixed path is never cleaned up.
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("hello.vox");
    std::fs::write(&src, "fn main() {\n    print(\"dist-ok\")\n}\n").expect("write hello.vox");

    let out = Command::new(&bin)
        .args(["run", "--interp"])
        .arg(&src)
        .output()
        .expect("spawn vox run");

    assert!(
        out.status.success(),
        "`vox run --interp hello.vox` failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("dist-ok"),
        "golden program output missing"
    );
}
