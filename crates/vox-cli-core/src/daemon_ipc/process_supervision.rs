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
use std::path::PathBuf;
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
    let output = Command::new(binary)
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() { None } else { Some(raw) }
}

pub fn spawn_detached_null_stdio(base: &str, args: &[&str]) -> anyhow::Result<Child> {
    let binary = resolve_managed_binary_path(base);
    Command::new(binary)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
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

pub fn terminate_process_tree(pid: u32) -> anyhow::Result<()> {
    if cfg!(windows) {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .context("run taskkill")?;
        if !status.success() {
            bail!("taskkill failed for pid {pid}");
        }
        return Ok(());
    }

    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .context("run kill")?;
    if !status.success() {
        bail!("kill failed for pid {pid}");
    }
    Ok(())
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
