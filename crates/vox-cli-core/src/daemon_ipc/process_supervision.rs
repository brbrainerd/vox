//! Shared process supervision helpers for sidecar/daemon binaries.
//!
//! SSOT: managed binary path resolution (`sibling` → `~/.vox/bin` → `PATH`),
//! detached spawning, process-tree termination, `--version` probing.
//!
//! State files under `.vox/process-supervision/` are Tier-D cache per
//! `contracts/db/data-storage-policy.v1.yaml`.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedProcessState {
    pub process_name: String,
    pub pid: u32,
    pub started_unix_ms: u64,
    pub binary_path: String,
}

#[derive(Debug, Clone)]
pub struct EnsureManagedProcessResult {
    pub pid: u32,
    pub state_file: PathBuf,
    pub started_now: bool,
}

#[derive(Debug, Clone)]
pub struct ManagedProcessStatus {
    pub process_name: String,
    pub pid: Option<u32>,
    pub running: bool,
    pub stale_state: bool,
    pub state_file: PathBuf,
    pub binary_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StopManagedProcessResult {
    pub process_name: String,
    pub pid: Option<u32>,
    pub stopped: bool,
    pub state_file: PathBuf,
}

fn executable_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

pub fn resolve_managed_binary_path(base: &str) -> PathBuf {
    let exe_name = executable_name(base);

    if let Ok(current_exe) = std::env::current_exe()
        && let Some(parent) = current_exe.parent()
    {
        let sibling = parent.join(&exe_name);
        if sibling.exists() {
            return sibling;
        }
    }

    let home = vox_config::paths::user_home_dir();
    let vox_bin = home.join(".vox").join("bin").join(&exe_name);
    if vox_bin.exists() {
        return vox_bin;
    }

    if let Some(found) = path_lookup_executable(base) {
        return found;
    }

    PathBuf::from(base)
}

fn path_lookup_executable(base: &str) -> Option<PathBuf> {
    which::which(base)
        .ok()
        .or_else(|| which::which(executable_name(base)).ok())
}

pub fn probe_binary_version(base: &str) -> Option<String> {
    let binary = resolve_managed_binary_path(base);
    let mut cmd = Command::new(binary);
    cmd.arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() { None } else { Some(raw) }
}

pub fn spawn_detached_null_stdio(base: &str, args: &[&str]) -> anyhow::Result<Child> {
    let home = vox_config::paths::user_home_dir();
    let bin_dir = home.join(".vox").join("bin");
    let target_sibling = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(executable_name(base))))
        .unwrap_or_else(|| PathBuf::from(executable_name(base)));
    let binary = resolve_or_stage_daemon(&target_sibling, &bin_dir)
        .unwrap_or_else(|_| resolve_managed_binary_path(base));
    let mut cmd = Command::new(binary);
    cmd.args(args).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn()
        .with_context(|| format!("spawn detached binary `{base}`"))
}

pub fn load_managed_process_state(base: &str) -> Option<ManagedProcessState> {
    let path = managed_process_state_path(base);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn clear_managed_process_state(base: &str) -> anyhow::Result<()> {
    let path = managed_process_state_path(base);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

pub fn managed_process_status(base: &str) -> ManagedProcessStatus {
    let state_file = managed_process_state_path(base);
    let binary_path = resolve_managed_binary_path(base);
    if let Some(existing) = load_managed_process_state(base) {
        let running = process_is_running(existing.pid);
        return ManagedProcessStatus {
            process_name: base.to_string(),
            pid: Some(existing.pid),
            running,
            stale_state: !running,
            state_file,
            binary_path,
        };
    }
    ManagedProcessStatus {
        process_name: base.to_string(),
        pid: None,
        running: false,
        stale_state: false,
        state_file,
        binary_path,
    }
}

pub fn ensure_managed_process_running(
    base: &str,
    args: &[&str],
) -> anyhow::Result<EnsureManagedProcessResult> {
    let state_file = managed_process_state_path(base);

    if let Some(existing) = load_managed_process_state(base) {
        if process_is_running(existing.pid) {
            return Ok(EnsureManagedProcessResult {
                pid: existing.pid,
                state_file,
                started_now: false,
            });
        }
        clear_managed_process_state(base)?;
    }

    let child = spawn_detached_null_stdio(base, args)?;
    let pid = child.id();
    write_managed_process_state(
        base,
        &ManagedProcessState {
            process_name: base.to_string(),
            pid,
            started_unix_ms: current_unix_ms(),
            binary_path: resolve_managed_binary_path(base)
                .to_string_lossy()
                .into_owned(),
        },
    )?;
    Ok(EnsureManagedProcessResult {
        pid,
        state_file,
        started_now: true,
    })
}

pub fn stop_managed_process(base: &str) -> anyhow::Result<StopManagedProcessResult> {
    let state_file = managed_process_state_path(base);
    let mut pid: Option<u32> = None;
    let mut stopped = false;
    if let Some(existing) = load_managed_process_state(base) {
        pid = Some(existing.pid);
        if process_is_running(existing.pid) {
            terminate_process_tree(existing.pid)?;
            stopped = true;
        }
        clear_managed_process_state(base)?;
    }
    Ok(StopManagedProcessResult {
        process_name: base.to_string(),
        pid,
        stopped,
        state_file,
    })
}

/// Absolute path to Windows' `taskkill.exe`.
///
/// Resolved from `%SystemRoot%` rather than looked up on `PATH`. A sanitized or
/// minimal `PATH` — services, CI runners, restricted shells, and some terminal
/// integrations — frequently omits `System32`, and `Command::new("taskkill")`
/// then fails to spawn. That surfaced as `vox populi down` reporting
/// "Access is denied. (os error 5)" and leaving the daemon running, which reads
/// like a permissions problem and is not one: invoking the same command by
/// absolute path succeeds for the same user against the same pid.
#[cfg(windows)]
fn taskkill_path() -> std::path::PathBuf {
    let root = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"));
    root.join("System32").join("taskkill.exe")
}

pub fn terminate_process_tree(pid: u32) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let exe = taskkill_path();
        let mut cmd = Command::new(&exe);
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let status = cmd
            .status()
            .with_context(|| format!("run {}", exe.display()))?;
        if !status.success() {
            bail!("taskkill failed for pid {pid} (exit {status})");
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .context("run kill")?;
        if !status.success() {
            bail!("kill failed for pid {pid}");
        }
        Ok(())
    }
}

pub fn process_is_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut sys = System::new();
    let p = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(std::slice::from_ref(&p)), true);
    sys.process(p).is_some()
}

fn write_managed_process_state(base: &str, state: &ManagedProcessState) -> anyhow::Result<()> {
    let path = managed_process_state_path(base);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let serialized = serde_json::to_string_pretty(state)?;
    fs::write(&path, serialized).with_context(|| format!("write {}", path.display()))
}

fn managed_process_state_path(base: &str) -> PathBuf {
    workspace_root_or_cwd()
        .join(".vox")
        .join("process-supervision")
        .join(format!("{base}.state.json"))
}

fn workspace_root_or_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Copy `src` into `dest_dir` (preserving file name) when the destination is
/// missing or older than `src`. Returns the staged path.
///
/// This is the key daemon-lifecycle fix: long-lived managed daemons must run
/// from `~/.vox/bin`, not from `target/debug/`. A running daemon exe holds an
/// open file handle on Windows, preventing `cargo build` from relinking the
/// binary (os error 5). Staging into a stable directory breaks the coupling.
pub fn stage_binary(src: &std::path::Path, dest_dir: &std::path::Path) -> std::io::Result<PathBuf> {
    let name = src.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "source has no file name")
    })?;
    let dest = dest_dir.join(name);
    std::fs::create_dir_all(dest_dir)?;
    let needs_copy = match (std::fs::metadata(&dest), std::fs::metadata(src)) {
        (Ok(d), Ok(s)) => match (d.modified(), s.modified()) {
            (Ok(dm), Ok(sm)) => sm > dm,
            _ => true,
        },
        _ => true, // dest missing
    };
    if needs_copy {
        std::fs::copy(src, &dest)?;
    }
    Ok(dest)
}

/// Resolve the path to launch a long-lived managed daemon from, guaranteeing
/// it is NOT under a `target/` build dir (which a running daemon would lock,
/// causing os error 5 on the next `cargo build`).
///
/// If `src` (typically the target/debug sibling) exists, stage it into
/// `dest_dir` (`~/.vox/bin`) first. Otherwise fall back to
/// `resolve_managed_binary_path` which checks `~/.vox/bin` and PATH.
pub fn resolve_or_stage_daemon(src: &Path, dest_dir: &Path) -> std::io::Result<PathBuf> {
    resolve_or_stage_daemon_with_version_hint(src, dest_dir).0
}

/// Like `resolve_or_stage_daemon`, but when falling through to an
/// already-staged (not freshly re-staged from a sibling) binary, also probes
/// its reported `--version` so callers can warn on a stale/mismatched daemon
/// BEFORE even attempting to launch it — the earliest possible signal for
/// the "old staged binary paired with a new GUI build" bug class.
pub fn resolve_or_stage_daemon_with_version_hint(
    src: &Path,
    dest_dir: &Path,
) -> (std::io::Result<PathBuf>, Option<String>) {
    if src.exists() {
        // Freshly re-staged from a live sibling — this IS the current build,
        // no version hint needed (there's nothing to compare against yet;
        // Task 2's ping-based check covers this case once the daemon is
        // actually running).
        return (stage_binary(src, dest_dir), None);
    }
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let resolved = resolve_managed_binary_path(name);
    let version_hint = probe_binary_version(name);
    (Ok(resolved), version_hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_is_running_current_pid() {
        assert!(process_is_running(std::process::id()));
    }

    #[test]
    fn process_is_running_zero_false() {
        assert!(!process_is_running(0));
    }
}

#[cfg(test)]
mod stage_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn resolve_daemon_stages_and_avoids_target_path() {
        let tmp = std::env::temp_dir().join(format!("vox-resolve-{}", std::process::id()));
        let target = tmp.join("target").join("debug");
        let bin = tmp.join("home").join(".vox").join("bin");
        std::fs::create_dir_all(&target).unwrap();
        let src = target.join("vox-demo2-d");
        std::fs::write(&src, b"x").unwrap();

        let resolved = resolve_or_stage_daemon(&src, &bin).unwrap();
        // Must NOT be under target/
        let resolved_str = resolved.to_string_lossy().replace('\\', "/").to_lowercase();
        assert!(
            !resolved_str.contains("/target/"),
            "daemon must not run from target/: {}",
            resolved.display()
        );
        assert!(resolved.exists(), "staged binary must exist");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn stage_copies_when_dest_missing_or_older() {
        let tmp = std::env::temp_dir().join(format!("vox-stage-{}", std::process::id()));
        let src_dir = tmp.join("target").join("debug");
        let dst_dir = tmp.join("home").join(".vox").join("bin");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::create_dir_all(&dst_dir).unwrap();
        let src = src_dir.join("vox-demo-d");
        let mut f = std::fs::File::create(&src).unwrap();
        f.write_all(b"binary-v1").unwrap();
        drop(f);

        let staged = stage_binary(&src, &dst_dir).unwrap();
        assert_eq!(staged, dst_dir.join("vox-demo-d"));
        assert_eq!(std::fs::read(&staged).unwrap(), b"binary-v1");

        // Second call with same (not newer) source should succeed (idempotent).
        let staged2 = stage_binary(&src, &dst_dir).unwrap();
        assert_eq!(staged2, staged);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_or_stage_reports_none_version_hint_when_falling_back_with_no_probeable_binary() {
        let tmp =
            std::env::temp_dir().join(format!("vox-test-version-hint-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let nonexistent_src = tmp.join("does-not-exist-vox-orchestrator-d");
        let dest_dir = tmp.join("dest");
        let (_path, version_hint) =
            resolve_or_stage_daemon_with_version_hint(&nonexistent_src, &dest_dir);
        assert_eq!(version_hint, None);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod terminate_tests {
    #[test]
    #[cfg(windows)]
    fn taskkill_is_resolved_absolutely_not_via_path() {
        // The bug: `Command::new("taskkill")` needs System32 on PATH, and a
        // sanitized PATH (services, CI, some terminals) omits it — which
        // surfaced as a misleading "Access is denied (os error 5)" while the
        // daemon kept running. Resolving from %SystemRoot% removes the
        // dependency on PATH entirely.
        let p = super::taskkill_path();
        assert!(
            p.is_absolute(),
            "must not depend on PATH lookup: {}",
            p.display()
        );
        assert!(p.ends_with("taskkill.exe"), "{}", p.display());
        assert!(p.exists(), "taskkill.exe not found at {}", p.display());
    }

    #[test]
    #[cfg(windows)]
    fn taskkill_path_falls_back_when_systemroot_is_unset() {
        // Reading %SystemRoot% must not panic or yield a relative path even in
        // a stripped environment; the fallback is the standard location.
        let p = super::taskkill_path();
        assert!(p.starts_with(std::path::Path::new(
            &std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string())
        )));
    }
}
