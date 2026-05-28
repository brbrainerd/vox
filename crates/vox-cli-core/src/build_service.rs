//! Build orchestration service (BL057).
//!
//! Centralizes Cargo spawn logic with lock telemetry. CLI commands delegate
//! to this module instead of calling `Command::new("cargo")` directly.
#![allow(dead_code)] // Slim `vox` binary wires only `run_cargo` / `CargoRequest::run` paths today.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Child, Command, Output};
use std::time::Instant;

/// Lightweight spawn telemetry (replaces the removed `vox-build-lock` crate for the slim CLI binary).
mod cargo_spawn_log {
    pub fn set_correlation_id(id: String) {
        tracing::debug!(target: "vox_cli::cargo", correlation_id = %id, "cargo correlation");
    }

    pub struct PeriodicWaitLogger;

    impl PeriodicWaitLogger {
        pub fn start() -> Self {
            Self
        }
    }

    pub fn log_spawn_start(
        command: &str,
        manifest: &str,
        target_dir: &str,
        build_dir: &str,
        workspace_root: &str,
    ) {
        tracing::debug!(
            target: "vox_cli::cargo",
            command = %command,
            manifest = %manifest,
            target_dir = %target_dir,
            build_dir = %build_dir,
            workspace_root = %workspace_root,
            "cargo spawn start",
        );
    }

    pub fn log_spawn_complete(
        command: &str,
        manifest: &str,
        target_dir: &str,
        build_dir: &str,
        workspace_root: &str,
        stderr_snippet: &str,
        wait_ms: Option<u64>,
    ) {
        tracing::debug!(
            target: "vox_cli::cargo",
            command = %command,
            manifest = %manifest,
            target_dir = %target_dir,
            build_dir = %build_dir,
            workspace_root = %workspace_root,
            stderr_len = stderr_snippet.len(),
            wait_ms = ?wait_ms,
            "cargo spawn complete",
        );
    }
}

use cargo_spawn_log::{
    PeriodicWaitLogger, log_spawn_complete, log_spawn_start, set_correlation_id,
};

/// Resolve cargo binary path. Precedence: VOX_CARGO_BIN, CARGO, "cargo".
fn cargo_binary() -> String {
    std::env::var("VOX_CARGO_BIN")
        .or_else(|_| std::env::var("CARGO"))
        .unwrap_or_else(|_| "cargo".to_string())
}

/// Request for a Cargo invocation (BL064).
#[derive(Debug, Clone)]
pub struct CargoRequest {
    /// The cargo subcommand to run (e.g., "build", "run", "test").
    pub command: String,
    /// Arguments passed to the cargo command.
    pub args: Vec<String>,
    /// Current working directory for the cargo process.
    pub cwd: PathBuf,
    /// Optional target directory override (CARGO_TARGET_DIR).
    pub target_dir: Option<PathBuf>,
    /// Optional build directory override.
    pub build_dir: Option<PathBuf>,
    /// Extra env vars (e.g. VOX_PORT for dev).
    pub env: Vec<(String, String)>,
}

impl CargoRequest {
    /// Create a build request.
    pub fn build(cwd: PathBuf, target_dir: Option<PathBuf>, args: impl Into<Vec<String>>) -> Self {
        let build_dir = target_dir.as_ref().map(|t| t.join("build"));
        Self {
            command: "build".to_string(),
            args: args.into(),
            cwd,
            target_dir,
            build_dir,
            env: vec![],
        }
    }

    /// Create a check request.
    pub fn check(cwd: PathBuf, target_dir: Option<PathBuf>) -> Self {
        let build_dir = target_dir.as_ref().map(|t| t.join("build"));
        Self {
            command: "check".to_string(),
            args: vec![],
            cwd,
            target_dir,
            build_dir,
            env: vec![],
        }
    }

    /// Create a test request.
    pub fn test(cwd: PathBuf, target_dir: Option<PathBuf>, args: impl Into<Vec<String>>) -> Self {
        let build_dir = target_dir.as_ref().map(|t| t.join("build"));
        Self {
            command: "test".to_string(),
            args: args.into(),
            cwd,
            target_dir,
            build_dir,
            env: vec![],
        }
    }

    /// Create a run request.
    pub fn run(
        cwd: PathBuf,
        target_dir: Option<PathBuf>,
        args: impl Into<Vec<String>>,
        env: Vec<(String, String)>,
    ) -> Self {
        let build_dir = target_dir.as_ref().map(|t| t.join("build"));
        Self {
            command: "run".to_string(),
            args: args.into(),
            cwd,
            target_dir,
            build_dir,
            env,
        }
    }
}

/// Run a Cargo command with telemetry and return output (BL060).
pub fn run_cargo(req: &CargoRequest) -> Result<Output> {
    set_correlation_id(uuid::Uuid::new_v4().to_string());
    let workspace_root = req.cwd.canonicalize().unwrap_or_else(|_| req.cwd.clone());
    let target_dir = req
        .target_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "default".to_string());
    let build_dir = req
        .build_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| target_dir.clone());
    let manifest = req.cwd.join("Cargo.toml");

    log_spawn_start(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
    );
    let _guard = PeriodicWaitLogger::start();
    let t0 = Instant::now();

    let mut cmd = Command::new(cargo_binary());
    cmd.arg(&req.command).args(&req.args).current_dir(&req.cwd);
    if let Some(ref td) = req.target_dir {
        if !crate::artifact_policy::is_allowed_artifact_path(td, &workspace_root) {
            tracing::warn!("Blocked invalid CARGO_TARGET_DIR: {}", td.display());
            anyhow::bail!(
                "Disallowed target directory: {}. Target sprawl outside policy is forbidden.",
                td.display()
            );
        }
        cmd.env("CARGO_TARGET_DIR", td);
    }
    if let Some(ref bd) = req.build_dir {
        cmd.env("CARGO_BUILD_BUILD_DIR", bd);
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }

    let output = cmd.output().context("Failed to run cargo")?;
    let wait_ms = t0.elapsed().as_millis() as u64;
    drop(_guard);

    let stderr = String::from_utf8_lossy(&output.stderr);
    log_spawn_complete(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
        &stderr,
        Some(wait_ms),
    );

    Ok(output)
}

/// Run cargo and return status (for fire-and-forget or status-only callers).
pub fn run_cargo_status(req: &CargoRequest) -> Result<std::process::ExitStatus> {
    Ok(run_cargo(req)?.status)
}

/// Run cargo with inherited stdout/stderr (for bundle UX). No stderr capture for telemetry.
pub fn run_cargo_inherit(req: &CargoRequest) -> Result<std::process::ExitStatus> {
    set_correlation_id(uuid::Uuid::new_v4().to_string());
    let workspace_root = req.cwd.canonicalize().unwrap_or_else(|_| req.cwd.clone());
    let target_dir = req
        .target_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "default".to_string());
    let build_dir = req
        .build_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| target_dir.clone());
    let manifest = req.cwd.join("Cargo.toml");

    log_spawn_start(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
    );
    let _guard = PeriodicWaitLogger::start();
    let t0 = Instant::now();

    let mut cmd = Command::new(cargo_binary());
    cmd.arg(&req.command).args(&req.args).current_dir(&req.cwd);
    if let Some(ref td) = req.target_dir {
        if !crate::artifact_policy::is_allowed_artifact_path(td, &workspace_root) {
            tracing::warn!("Blocked invalid CARGO_TARGET_DIR: {}", td.display());
            anyhow::bail!(
                "Disallowed target directory: {}. Target sprawl outside policy is forbidden.",
                td.display()
            );
        }
        cmd.env("CARGO_TARGET_DIR", td);
    }
    if let Some(ref bd) = req.build_dir {
        cmd.env("CARGO_BUILD_BUILD_DIR", bd);
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let status = cmd.status().context("Failed to run cargo")?;
    let wait_ms = t0.elapsed().as_millis() as u64;
    drop(_guard);
    log_spawn_complete(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
        "",
        Some(wait_ms),
    );
    Ok(status)
}

/// Sync wrapper that logs spawn completion when the child exits (Wave 3).
pub struct MonitoredCargoChildSync {
    /// The underlying child process.
    child: Option<Child>,
    /// The command string for logging.
    command: String,
    /// The manifest path for logging.
    manifest: String,
    /// The target directory for logging.
    target_dir: String,
    /// The build directory for logging.
    build_dir: String,
    /// The workspace root for logging.
    workspace_root: String,
    /// Start time for latency measurement.
    t0: Instant,
}

impl MonitoredCargoChildSync {
    /// Wait for the child and log completion.
    pub fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let status = self.child.as_mut().map(|c| c.wait()).transpose()?.unwrap();
        self.child = None;
        let wait_ms = self.t0.elapsed().as_millis() as u64;
        log_spawn_complete(
            &self.command,
            &self.manifest,
            &self.target_dir,
            &self.build_dir,
            &self.workspace_root,
            "",
            Some(wait_ms),
        );
        Ok(status)
    }
}

impl Drop for MonitoredCargoChildSync {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let command = self.command.clone();
            let manifest = self.manifest.clone();
            let target_dir = self.target_dir.clone();
            let build_dir = self.build_dir.clone();
            let workspace_root = self.workspace_root.clone();
            let wait_ms = self.t0.elapsed().as_millis() as u64;
            std::thread::spawn(move || {
                let _ = child.wait();
                log_spawn_complete(
                    &command,
                    &manifest,
                    &target_dir,
                    &build_dir,
                    &workspace_root,
                    "",
                    Some(wait_ms),
                );
            });
        }
    }
}

/// Spawn cargo and return monitored std child (sync). Logs completion on exit (Wave 3).
pub fn run_cargo_spawn(req: &CargoRequest) -> Result<MonitoredCargoChildSync> {
    set_correlation_id(uuid::Uuid::new_v4().to_string());
    let workspace_root = req.cwd.canonicalize().unwrap_or_else(|_| req.cwd.clone());
    let target_dir = req
        .target_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "default".to_string());
    let build_dir = req
        .build_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| target_dir.clone());
    let manifest = req.cwd.join("Cargo.toml");

    log_spawn_start(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
    );

    let mut cmd = Command::new(cargo_binary());
    cmd.arg(&req.command).args(&req.args).current_dir(&req.cwd);
    if let Some(ref td) = req.target_dir {
        if !crate::artifact_policy::is_allowed_artifact_path(td, &workspace_root) {
            tracing::warn!("Blocked invalid CARGO_TARGET_DIR: {}", td.display());
            anyhow::bail!(
                "Disallowed target directory: {}. Target sprawl outside policy is forbidden.",
                td.display()
            );
        }
        cmd.env("CARGO_TARGET_DIR", td);
    }
    if let Some(ref bd) = req.build_dir {
        cmd.env("CARGO_BUILD_BUILD_DIR", bd);
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let child = cmd.spawn().context("Failed to spawn cargo")?;
    let t0 = Instant::now();
    Ok(MonitoredCargoChildSync {
        child: Some(child),
        command: req.command.clone(),
        manifest: manifest.display().to_string(),
        target_dir,
        build_dir,
        workspace_root: workspace_root.display().to_string(),
        t0,
    })
}

/// Wrapper that logs spawn completion when the child exits (Wave 3).
pub struct MonitoredCargoChild {
    /// The underlying tokio child process.
    child: Option<tokio::process::Child>,
    /// The command string for logging.
    command: String,
    /// The manifest path for logging.
    manifest: String,
    /// The target directory for logging.
    target_dir: String,
    /// The build directory for logging.
    build_dir: String,
    /// The workspace root for logging.
    workspace_root: String,
    /// Start time for latency measurement.
    t0: std::time::Instant,
}

impl MonitoredCargoChild {
    /// Kill the child, wait for exit, and log completion.
    pub async fn kill(&mut self) -> std::io::Result<()> {
        if let Some(ref mut c) = self.child {
            c.start_kill()?;
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.wait().await;
            let wait_ms = self.t0.elapsed().as_millis() as u64;
            log_spawn_complete(
                &self.command,
                &self.manifest,
                &self.target_dir,
                &self.build_dir,
                &self.workspace_root,
                "",
                Some(wait_ms),
            );
        }
        Ok(())
    }

    /// Wait for the child and log completion.
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus> {
        let status = if let Some(ref mut c) = self.child {
            c.wait().await?
        } else {
            return Err(anyhow::anyhow!("child already consumed"));
        };
        self.child = None;
        let wait_ms = self.t0.elapsed().as_millis() as u64;
        log_spawn_complete(
            &self.command,
            &self.manifest,
            &self.target_dir,
            &self.build_dir,
            &self.workspace_root,
            "",
            Some(wait_ms),
        );
        Ok(status)
    }
}

impl Drop for MonitoredCargoChild {
    fn drop(&mut self) {
        if self.child.is_some() {
            let command = self.command.clone();
            let manifest = self.manifest.clone();
            let target_dir = self.target_dir.clone();
            let build_dir = self.build_dir.clone();
            let workspace_root = self.workspace_root.clone();
            let wait_ms = self.t0.elapsed().as_millis() as u64;
            let mut child = self.child.take().unwrap();
            tokio::spawn(async move {
                let _ = child.wait().await;
                log_spawn_complete(
                    &command,
                    &manifest,
                    &target_dir,
                    &build_dir,
                    &workspace_root,
                    "",
                    Some(wait_ms),
                );
            });
        }
    }
}

/// Spawn cargo and return monitored tokio child (async, for dev/watch). Logs completion on exit (Wave 3).
pub async fn run_cargo_spawn_async(req: &CargoRequest) -> Result<MonitoredCargoChild> {
    set_correlation_id(uuid::Uuid::new_v4().to_string());
    let workspace_root = req.cwd.canonicalize().unwrap_or_else(|_| req.cwd.clone());
    let target_dir = req
        .target_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "default".to_string());
    let build_dir = req
        .build_dir
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| target_dir.clone());
    let manifest = req.cwd.join("Cargo.toml");

    log_spawn_start(
        &req.command,
        &manifest.display().to_string(),
        &target_dir,
        &build_dir,
        &workspace_root.display().to_string(),
    );

    let mut cmd = tokio::process::Command::new(cargo_binary());
    cmd.arg(&req.command).args(&req.args).current_dir(&req.cwd);
    if let Some(ref td) = req.target_dir {
        if !crate::artifact_policy::is_allowed_artifact_path(td, &workspace_root) {
            tracing::warn!("Blocked invalid CARGO_TARGET_DIR: {}", td.display());
            anyhow::bail!(
                "Disallowed target directory: {}. Target sprawl outside policy is forbidden.",
                td.display()
            );
        }
        cmd.env("CARGO_TARGET_DIR", td);
    }
    if let Some(ref bd) = req.build_dir {
        cmd.env("CARGO_BUILD_BUILD_DIR", bd);
    }
    for (k, v) in &req.env {
        cmd.env(k, v);
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let child = cmd.spawn().context("Failed to spawn cargo")?;
    let t0 = std::time::Instant::now();
    Ok(MonitoredCargoChild {
        child: Some(child),
        command: req.command.clone(),
        manifest: manifest.display().to_string(),
        target_dir,
        build_dir,
        workspace_root: workspace_root.display().to_string(),
        t0,
    })
}
