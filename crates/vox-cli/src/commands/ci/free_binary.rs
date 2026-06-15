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

/// Decide whether the process with executable `exe`, pid `pid`, and command
/// line `cmdline`, should be reaped so a build can relink binaries under
/// `target_dir`. `current_pid` is the reaper's own pid (never reap self).
///
/// Safety guards (in order):
/// 1. Never reap self.
/// 2. Never reap a running `vox serve` — killing the server would be
///    catastrophic and is never the intent of this command.
/// 3. The exe must live inside `target_dir/` — this is the primary scope
///    narrowing that prevents touching any process not owned by this worktree.
/// 4. Only `vox` / `vox.exe` / `vox-*` managed siblings are eligible;
///    build scripts (`*-build`) are excluded.
fn should_reap(
    exe: &Path,
    cmdline: &[String],
    target_dir: &Path,
    pid: u32,
    current_pid: u32,
) -> bool {
    // Guard 1: never reap self.
    if pid == current_pid {
        return false;
    }

    // Guard 2: never reap a live `vox serve` process.  The cmdline is checked
    // rather than the exe name alone because the binary is always `vox` but
    // the subcommand is what distinguishes a long-lived server from a build
    // tool invocation.
    let is_serve = cmdline.iter().any(|arg| arg == "serve");
    if is_serve {
        return false;
    }

    // Guard 3: exe must reside under the target dir of THIS worktree.
    // Normalize both sides to lowercase string compare — robust on Windows
    // (case-insensitive FS, mixed separators) without canonicalize() (which
    // fails on a locked/transient path).
    let norm = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
    let exe_s = norm(exe);
    let target_s = norm(target_dir);
    if !exe_s.starts_with(&format!("{target_s}/")) {
        return false;
    }

    // Guard 4: only `vox` and its managed siblings (vox-orchestrator-d, ...)
    // are reapable; build scripts are excluded.
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
        // Collect the command-line args for the serve-guard check.
        let cmdline: Vec<String> = proc_
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        if should_reap(exe, &cmdline, target_dir, pid_u, current) {
            out.push(LockingProc {
                pid: pid_u,
                exe: exe.to_path_buf(),
            });
        }
    }
    out
}

/// Kill the given pid, but only after re-verifying that the process still
/// exists AND its exe path still matches `expected_exe`.  This closes the
/// TOCTOU window between scanning PIDs and sending SIGKILL: if the original
/// process died and a new, unrelated process recycled the PID, the exe path
/// will not match and we skip silently.
fn kill_pid(pid: u32, expected_exe: &Path) -> bool {
    let sys = System::new_all();
    let Some(proc_) = sys.process(sysinfo::Pid::from_u32(pid)) else {
        // Process already gone -- nothing to kill.
        return false;
    };

    // TOCTOU guard: re-verify the exe before sending the signal.
    let norm = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
    match proc_.exe() {
        Some(live_exe) if norm(live_exe) == norm(expected_exe) => proc_.kill(),
        _ => {
            // exe changed or unavailable -- PID was recycled or process vanished.
            false
        }
    }
}

/// `vox ci free-binary` entry. `target` defaults to `<root>/target`.
/// Dry-run by default; `apply` kills the stale lockers.
pub fn run(root: &Path, target: Option<PathBuf>, apply: bool) -> Result<()> {
    let target_dir = target.unwrap_or_else(|| root.join("target"));
    let locking = scan_locking_pids(&target_dir);
    if locking.is_empty() {
        println!(
            "free-binary: no stale vox processes hold {}",
            target_dir.display()
        );
        return Ok(());
    }
    for lp in &locking {
        if apply {
            let ok = kill_pid(lp.pid, &lp.exe);
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

    /// Empty cmdline -- helper for tests that do not exercise the serve-guard.
    fn no_cmd() -> Vec<String> {
        vec![]
    }

    #[test]
    fn reaps_vox_exe_under_target() {
        let target = p("/repo/target");
        assert!(should_reap(
            &p("/repo/target/debug/vox.exe"),
            &no_cmd(),
            &target,
            100,
            1
        ));
        assert!(should_reap(
            &p("/repo/target/debug/vox-orchestrator-d"),
            &no_cmd(),
            &target,
            100,
            1
        ));
        assert!(should_reap(
            &p("/repo/target/release/vox"),
            &no_cmd(),
            &target,
            100,
            1
        ));
    }

    #[test]
    fn never_reaps_self() {
        let target = p("/repo/target");
        assert!(!should_reap(
            &p("/repo/target/debug/vox.exe"),
            &no_cmd(),
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
            &no_cmd(),
            &target,
            100,
            1
        ));
        assert!(!should_reap(
            &p("/home/u/.vox/bin/vox"),
            &no_cmd(),
            &target,
            100,
            1
        ));
    }

    #[test]
    fn ignores_non_vox_binaries() {
        let target = p("/repo/target");
        assert!(!should_reap(
            &p("/repo/target/debug/build-script-build"),
            &no_cmd(),
            &target,
            100,
            1
        ));
        assert!(!should_reap(
            &p("/repo/target/debug/some-test-bin"),
            &no_cmd(),
            &target,
            100,
            1
        ));
    }

    #[test]
    fn never_reaps_vox_serve() {
        let target = p("/repo/target");
        // A vox process whose cmdline contains "serve" must never be reaped.
        let serve_cmd = vec!["vox".to_string(), "serve".to_string()];
        assert!(!should_reap(
            &p("/repo/target/debug/vox"),
            &serve_cmd,
            &target,
            100,
            1
        ));
        // Variant: serve with extra flags must also be excluded.
        let serve_cmd2 = vec![
            "vox".to_string(),
            "--quiet".to_string(),
            "serve".to_string(),
            "--port".to_string(),
            "8080".to_string(),
        ];
        assert!(!should_reap(
            &p("/repo/target/debug/vox"),
            &serve_cmd2,
            &target,
            100,
            1
        ));
        // A non-serve invocation with the same binary is still reapable.
        let build_cmd = vec!["vox".to_string(), "build".to_string()];
        assert!(should_reap(
            &p("/repo/target/debug/vox"),
            &build_cmd,
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
            &no_cmd(),
            &target,
            100,
            1
        ));
        assert!(!should_reap(
            &p("C:\\other\\target\\debug\\vox.exe"),
            &no_cmd(),
            &target,
            100,
            1
        ));
    }
}
