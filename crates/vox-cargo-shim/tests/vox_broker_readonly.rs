//! `vox-broker` is a read-only viewer: on a machine (or temp dir) where the
//! broker has never run, every subcommand must print a clear message and
//! exit 0 -- and, crucially, must create nothing. A "viewer" that creates the
//! broker home just by being run would be a bug.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn broker_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_vox-broker"))
}

#[test]
fn never_run_subcommands_exit_zero_and_say_so() {
    // Match real-world usage: callers set VOX_BROKER_HOME to a `mktemp -d`
    // path, which creates the directory itself even though the broker has
    // never written into it -- the never-run message must not be fooled by
    // that pre-existing empty directory.
    let broker_home = tempfile::tempdir().unwrap();
    let broker_home = broker_home.path();

    for sub in ["stats", "log", "status"] {
        let out = Command::new(broker_bin())
            .arg(sub)
            .env("VOX_BROKER_HOME", broker_home)
            .output()
            .unwrap();
        assert!(out.status.success(), "{sub} did not exit 0: {out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.to_lowercase().contains("no data") || stdout.contains("never run"),
            "{sub} did not report the never-run state: {stdout}"
        );
    }

    // The whole point of a read-only viewer: it must not write anything into
    // the broker home just by being asked about it (the directory itself
    // pre-exists here, mirroring `mktemp -d` usage -- what must stay absent
    // is any file or subdirectory the broker would normally create).
    let entries: Vec<_> = fs::read_dir(broker_home).unwrap().collect();
    assert!(
        entries.is_empty(),
        "vox-broker must never write into the broker home: found {entries:?}"
    );
}

#[test]
fn unknown_subcommand_is_a_usage_error() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(broker_bin())
        .arg("bogus")
        .env("VOX_BROKER_HOME", tmp.path())
        .output()
        .unwrap();
    assert!(!out.status.success(), "unknown subcommand must fail");
}

#[test]
fn log_reads_last_n_lines_without_disturbing_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let broker_home = tmp.path().join("broker");
    fs::create_dir_all(&broker_home).unwrap();
    fs::write(broker_home.join("broker.log"), "l1\nl2\nl3\nl4\nl5\n").unwrap();

    let out = Command::new(broker_bin())
        .args(["log", "-n", "2"])
        .env("VOX_BROKER_HOME", &broker_home)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("l4"), "got: {stdout}");
    assert!(stdout.contains("l5"), "got: {stdout}");
    assert!(!stdout.contains("l1"), "got: {stdout}");

    // File is untouched.
    let after = fs::read_to_string(broker_home.join("broker.log")).unwrap();
    assert_eq!(after, "l1\nl2\nl3\nl4\nl5\n");
}
