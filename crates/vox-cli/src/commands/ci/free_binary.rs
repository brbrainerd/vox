//! `vox ci free-binary` — reap stale `vox*` processes that lock the current
//! worktree's `target/` build output, so a subsequent relink (cargo run/build)
//! can overwrite `vox.exe` on Windows instead of failing with os error 5.
//!
//! Scoped on purpose: a process is reaped ONLY when its executable lives under
//! the target dir we are about to relink AND it is not the caller. Because each
//! worktree has its own `target/`, this never touches a sibling agent's procs.

use std::path::Path;

/// Decide whether the process with executable `exe`, pid `pid`, should be
/// reaped so a build can relink binaries under `target_dir`. `current_pid` is
/// the reaper's own pid (never reap self).
fn should_reap(exe: &Path, target_dir: &Path, pid: u32, current_pid: u32) -> bool {
    if pid == current_pid {
        return false;
    }
    // Normalize both sides to lowercase string compare — robust on Windows
    // (case-insensitive FS, mixed separators) without canonicalize() (which
    // fails on a locked/transient path).
    let norm = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
    let exe_s = norm(exe);
    let target_s = norm(target_dir);
    if !exe_s.starts_with(&format!("{target_s}/")) {
        return false;
    }
    // Only `vox` and its managed siblings (vox-orchestrator-d, …) are reapable.
    let file = exe
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    file == "vox"
        || file == "vox.exe"
        || file.starts_with("vox-") && !file.ends_with("-build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn reaps_vox_exe_under_target() {
        let target = p("/repo/target");
        assert!(should_reap(&p("/repo/target/debug/vox.exe"), &target, 100, 1));
        assert!(should_reap(&p("/repo/target/debug/vox-orchestrator-d"), &target, 100, 1));
        assert!(should_reap(&p("/repo/target/release/vox"), &target, 100, 1));
    }

    #[test]
    fn never_reaps_self() {
        let target = p("/repo/target");
        assert!(!should_reap(&p("/repo/target/debug/vox.exe"), &target, 1, 1));
    }

    #[test]
    fn ignores_procs_outside_target() {
        let target = p("/repo/target");
        assert!(!should_reap(&p("/other-wt/target/debug/vox.exe"), &target, 100, 1));
        assert!(!should_reap(&p("/home/u/.vox/bin/vox"), &target, 100, 1));
    }

    #[test]
    fn ignores_non_vox_binaries() {
        let target = p("/repo/target");
        assert!(!should_reap(&p("/repo/target/debug/build-script-build"), &target, 100, 1));
        assert!(!should_reap(&p("/repo/target/debug/some-test-bin"), &target, 100, 1));
    }
}
