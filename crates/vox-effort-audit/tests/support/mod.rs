//! Test fixture helpers.
//!
//! Re-used from `walk` and other test modules via `#[path = "..."]`.

use std::path::PathBuf;
use std::process::Command;

#[allow(dead_code)]
pub fn make_smoke_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let run = |args: &[&str]| {
        let s = Command::new("git")
            .current_dir(&path)
            .args(args)
            .status()
            .unwrap();
        assert!(s.success(), "git {:?}", args);
    };
    run(&["init", "--quiet", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "commit.gpgsign", "false"]);
    for i in 0..5 {
        std::fs::write(path.join(format!("f{i}.txt")), format!("hello {i}\n")).unwrap();
        run(&["add", "."]);
        run(&["commit", "--quiet", "-m", &format!("commit {i}")]);
    }
    (dir, path)
}
