//! Two concurrent builds must serialize under the machine-wide concurrency cap
//! and each emit a metric + log line to the (isolated) global broker root.
//!
//! `VOX_BROKER_HOME` points the broker at a temp dir so the test neither touches
//! the real `~/.vox/build-broker` nor competes with real machine builds.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

fn shim_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cargo"))
}

#[test]
fn two_builds_serialize_under_global_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let broker_home = tmp.path().join("broker");
    let wt = tmp.path().join("wt");
    fs::create_dir_all(wt.join("src")).unwrap();
    fs::write(
        wt.join("Cargo.toml"),
        "[package]\nname = \"qtest\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(wt.join("src/lib.rs"), "").unwrap();

    let spawn = |broker_home: PathBuf, wt: PathBuf| {
        thread::spawn(move || {
            Command::new(shim_bin())
                .args(["check"])
                .current_dir(&wt)
                .env("VOX_BROKER_HOME", &broker_home)
                .env("VOX_BROKER_MAX_CONCURRENT", "1") // force serialization
                .status()
                .unwrap()
                .success()
        })
    };

    let h1 = spawn(broker_home.clone(), wt.clone());
    let h2 = spawn(broker_home.clone(), wt.clone());
    assert!(
        h1.join().unwrap() && h2.join().unwrap(),
        "both checks succeed"
    );

    let log = fs::read_to_string(broker_home.join("broker.log")).unwrap_or_default();
    assert_eq!(
        log.lines().count(),
        2,
        "expected 2 broker.log lines, got:\n{log}"
    );
    let metrics = fs::read_to_string(broker_home.join("metrics.jsonl")).unwrap_or_default();
    assert_eq!(metrics.lines().count(), 2, "expected 2 metric lines");
}
