//! `vox ci free-binary` — reap stale `vox*` processes that lock the current
//! worktree's `target/` build output, so a subsequent relink (cargo run/build)
//! can overwrite `vox.exe` on Windows instead of failing with os error 5.
//!
//! Scoped on purpose: a process is reaped ONLY when its executable lives under
//! the target dir we are about to relink AND it is not the caller. Because each
//! worktree has its own `target/`, this never touches a sibling agent's procs.

use std::path::{Path, PathBuf};

use anyhow::Result;
use sysinfo::System;

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
    file == "vox" || file == "vox.exe" || (file.starts_with("vox-") && !file.ends_with("-build"))
}

/// One stale process holding `target/`.
struct LockingProc {
    pid: u32,
    exe: PathBuf,
}

/// Scan all processes and return those whose executable lives under `target_dir`
/// and is reapable (`should_reap`), excluding the current process.
fn scan_locking_pids(target_dir: &Path) -> Vec<LockingProc> {
    let current = std::process::id();
    let sys = System::new_all();
    let mut out = Vec::new();
    for (pid, proc_) in sys.processes() {
        let Some(exe) = proc_.exe() else { continue };
        let pid_u = pid.as_u32();
        if should_reap(exe, target_dir, pid_u, current) {
            out.push(LockingProc {
                pid: pid_u,
                exe: exe.to_path_buf(),
            });
        }
    }
    out
}

/// Kill the given pid. Best-effort: a process may already be gone.
fn kill_pid(pid: u32) -> bool {
    let sys = System::new_all();
    sys.process(sysinfo::Pid::from_u32(pid))
        .map(|p| p.kill())
        .unwrap_or(false)
}

/// `vox ci free-binary` entry. `target` defaults to `<root>/target`.
/// Dry-run by default; `apply` kills the stale lockers.
pub fn run(root: &Path, target: Option<PathBuf>, apply: bool) -> Result<()> {
    let target_dir = target.unwrap_or_else(|| root.join("target"));
    let locking = scan_locking_pids(&target_dir);
    if locking.is_empty() {
        println!("free-binary: no stale vox processes hold {}", target_dir.display());
        return Ok(());
    }
    for lp in &locking {
        if apply {
            let ok = kill_pid(lp.pid);
            println!(
                "free-binary: {} pid={} exe={}",
                if ok { "killed" } else { "could-not-kill" },
                lp.pid,
                lp.exe.display()
            );
        } else {
            println!(
                "free-binary: [dry-run] would kill pid={} exe={}",
                lp.pid,
                lp.exe.display()
            );
        }
    }
    Ok(())
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
        assert!(should_reap(
            &p("/repo/target/debug/vox.exe"),
            &target,
            100,
            1
        ));
        assert!(should_reap(
            &p("/repo/target/debug/vox-orchestrator-d"),
            &target,
            100,
            1
        ));
        assert!(should_reap(&p("/repo/target/release/vox"), &target, 100, 1));
    }

    #[test]
    fn never_reaps_self() {
        let target = p("/repo/target");
        assert!(!should_reap(
            &p("/repo/target/debug/vox.exe"),
            &target,
            1,
            1
        ));
    }

    #[test]
    fn ignores_procs_outside_target() {
        let target = p("/repo/target");
        assert!(!should_reap(
            &p("/other-wt/target/debug/vox.exe"),
            &target,
            100,
            1
        ));
        assert!(!should_reap(&p("/home/u/.vox/bin/vox"), &target, 100, 1));
    }

    #[test]
    fn ignores_non_vox_binaries() {
        let target = p("/repo/target");
        assert!(!should_reap(
            &p("/repo/target/debug/build-script-build"),
            &target,
            100,
            1
        ));
        assert!(!should_reap(
            &p("/repo/target/debug/some-test-bin"),
            &target,
            100,
            1
        ));
    }

    #[test]
    fn scan_returns_empty_for_nonexistent_target() {
        // A target dir that cannot contain live procs yields no reap candidates.
        let report = scan_locking_pids(&p("/definitely/not/a/real/target/xyz123abc"));
        assert!(report.is_empty());
    }

    #[test]
    fn handles_windows_paths() {
        let target = p("C:\\repo\\target");
        assert!(should_reap(
            &p("C:\\repo\\target\\debug\\vox.exe"),
            &target,
            100,
            1
        ));
        assert!(!should_reap(
            &p("C:\\other\\target\\debug\\vox.exe"),
            &target,
            100,
            1
        ));
    }
}
