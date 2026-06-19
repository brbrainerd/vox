//! Two concurrent build invocations in the same worktree must both go through
//! the fair queue (serializing on the run lock) and each must emit a metric.

use std::fs;
use std::process::Command;
use std::thread;

fn shim_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_cargo"))
}

fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn two_builds_serialize_and_emit_metrics() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt");
    fs::create_dir_all(wt.join(".cargo")).unwrap();
    fs::create_dir_all(wt.join("src")).unwrap();
    fs::write(wt.join(".cargo/config.toml"), "[env]\n").unwrap();
    fs::write(
        wt.join("Cargo.toml"),
        "[package]\nname = \"qtest\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(wt.join("src/lib.rs"), "").unwrap();

    let spawn = |wt: std::path::PathBuf| {
        thread::spawn(move || {
            Command::new(shim_bin())
                .args(["check"])
                .current_dir(&wt)
                .status()
                .unwrap()
                .success()
        })
    };

    let h1 = spawn(wt.clone());
    let h2 = spawn(wt.clone());
    let ok1 = h1.join().unwrap();
    let ok2 = h2.join().unwrap();
    assert!(ok1 && ok2, "both checks should succeed");

    let mut found = false;
    for entry in walk(&wt.join(".vox/build-queue")) {
        if entry.file_name().map(|n| n == "metrics.jsonl").unwrap_or(false) {
            found = true;
            let lines = fs::read_to_string(&entry).unwrap();
            assert_eq!(lines.lines().count(), 2, "expected 2 metric lines, got: {lines}");
        }
    }
    assert!(found, "metrics.jsonl not written under .vox/build-queue");
}
