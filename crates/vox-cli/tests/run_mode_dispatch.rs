//! `vox run --mode script` smoke (needs `script-execution` in the `vox` binary).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn vox_bin() -> PathBuf {
    let candidates = [
        "target/debug/vox",
        "target/debug/vox.exe",
        "target/release/vox",
        "target/release/vox.exe",
    ];
    let root = workspace_root();
    for c in &candidates {
        let p = root.join(c);
        if p.exists() {
            return p;
        }
    }
    panic!("build vox-cli first: cargo build -p vox-cli --features script-execution");
}

fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = fs::read_to_string(&candidate).unwrap_or_default();
            if content.contains("[workspace]") {
                return dir;
            }
        }
        assert!(dir.pop(), "workspace root not found");
    }
}

#[test]
fn run_mode_script_executes_minimal_main() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("smoke_run_mode.vox");
    fs::write(&vox_file, "fn main() {\n    print(str(\"run_mode_ok\"))\n").expect("write vox");

    let repo = workspace_root();
    let st = Command::new(vox_bin())
        .current_dir(&repo)
        .args([
            "run",
            "--mode",
            "script",
            vox_file.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("spawn vox run");
    assert!(
        st.success(),
        "vox run --mode script should succeed for fn main()"
    );
}

#[test]
fn run_mode_auto_matches_script_for_script_shaped_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("smoke_auto.vox");
    fs::write(&vox_file, "fn main() {\n    print(str(\"auto_lane\"))\n").expect("write vox");

    let repo = workspace_root();
    let st = Command::new(vox_bin())
        .current_dir(&repo)
        .args([
            "run",
            "--mode",
            "auto",
            vox_file.to_str().expect("utf8 path"),
        ])
        .status()
        .expect("spawn vox run");
    assert!(
        st.success(),
        "vox run --mode auto should route script-shaped files to the script lane"
    );
}

#[test]
fn run_mode_script_passes_trailing_args_to_std_args() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let vox_file = tmp.path().join("smoke_argv.vox");
    fs::write(
        &vox_file,
        "fn main() {\n    for a in std.args:\n        print(a)\n",
    )
    .expect("write vox");

    let repo = workspace_root();
    let out = Command::new(vox_bin())
        .current_dir(&repo)
        .args([
            "run",
            "--mode",
            "script",
            vox_file.to_str().expect("utf8 path"),
            "hello",
            "world",
        ])
        .output()
        .expect("spawn vox run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("hello") && stdout.contains("world"),
        "expected argv pass-through on stdout, got {stdout:?}"
    );
}

/// Full app lane compiles the generated server — too slow for default CI.
#[test]
#[ignore = "run locally: cargo test -p vox-cli --features script-execution --test run_mode_dispatch -- --ignored — owner: vox-cli sunset: 2026-12-31"]
fn run_mode_app_builds_examples_chatbot() {
    let repo = workspace_root();
    let chatbot = repo.join("examples/chatbot.vox");
    assert!(chatbot.is_file(), "missing {}", chatbot.display());
    let st = Command::new(vox_bin())
        .current_dir(&repo)
        .args(["run", "--mode", "app", chatbot.to_str().expect("utf8")])
        .status()
        .expect("spawn vox run");
    assert!(
        st.success(),
        "vox run --mode app on a UI example should complete (may take several minutes)"
    );
}
