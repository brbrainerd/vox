//! Container-backed sandboxed skill runner.
//!
//! Uses `vox-container`'s [`ContainerRuntime`] trait to execute skill commands
//! inside the `vox-skill-sandbox` OCI image with strict resource and network limits.
//!
//! This is **the same Docker/Podman backend** used for `.vox` application deployment
//! — unifying the two use cases under one trusted abstraction.

use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;

use vox_container::ContainerRuntime;
use vox_container::detect::{RuntimePreference, detect_runtime};

use vox_openclaw_runtime::manifest::ResourceLimits;

use super::image::{SANDBOX_IMAGE_TAG, ensure_sandbox_image};

/// Error types for sandboxed skill execution.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("No container runtime found (Docker or Podman required): {0}")]
    NoRuntime(String),
    #[error("Sandbox image setup failed: {0}")]
    ImageSetup(String),
    #[error("Container execution failed (exit {exit_code}): {stderr}")]
    ExecutionFailed { exit_code: i32, stderr: String },
    #[error("Execution timed out after {ms}ms")]
    Timeout { ms: u64 },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// The captured output of a sandboxed skill execution.
#[derive(Debug, Clone)]
pub struct SkillOutput {
    /// Captured stdout (trimmed, capped at `max_output_bytes`).
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Process exit code.
    pub exit_code: i32,
    /// Wall-clock execution time in milliseconds.
    pub wall_ms: u64,
}

impl SkillOutput {
    /// Returns `true` if the skill exited successfully (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Executes skill commands inside the `vox-skill-sandbox` OCI container.
///
/// Acquired via [`SandboxedSkillRunner::detect`] which auto-selects the best
/// available container runtime (prefers rootless Podman, falls back to Docker).
pub struct SandboxedSkillRunner {
    runtime: Arc<dyn ContainerRuntime>,
    sandbox_image: String,
}

impl SandboxedSkillRunner {
    /// Auto-detect the available container runtime and return a runner.
    ///
    /// Returns `Err` if neither Docker nor Podman is installed.
    pub fn detect() -> Result<Self, SandboxError> {
        let runtime = detect_runtime(RuntimePreference::Auto)
            .map_err(|e| SandboxError::NoRuntime(e.to_string()))?;
        Ok(Self {
            runtime: Arc::from(runtime),
            sandbox_image: SANDBOX_IMAGE_TAG.to_string(),
        })
    }

    /// Detect with an explicit runtime preference (for testing / ops overrides).
    pub fn with_preference(pref: RuntimePreference) -> Result<Self, SandboxError> {
        let runtime = detect_runtime(pref).map_err(|e| SandboxError::NoRuntime(e.to_string()))?;
        Ok(Self {
            runtime: Arc::from(runtime),
            sandbox_image: SANDBOX_IMAGE_TAG.to_string(),
        })
    }

    /// Ensure the sandbox image is present.  Call once at startup or in doctor.
    pub fn ensure_image(&self) -> Result<(), SandboxError> {
        ensure_sandbox_image(self.runtime.as_ref(), &self.sandbox_image)
            .map_err(|e| SandboxError::ImageSetup(e.to_string()))
    }

    /// Execute a shell command string inside the sandbox container.
    ///
    /// The command runs as:
    /// ```shell
    /// docker run --rm \
    ///   --network=<policy> \
    ///   --read-only --tmpfs /tmp \
    ///   --user=nobody \
    ///   --cap-drop=ALL \
    ///   --security-opt=no-new-privileges \
    ///   --memory=<Xm> --cpus=<Y> \
    ///   vox-skill-sandbox:latest "<command>"
    /// ```
    pub fn run(&self, command: &str, limits: &ResourceLimits) -> Result<SkillOutput, SandboxError> {
        let start = Instant::now();

        let runtime_name = self.runtime.name();
        let mut cmd = Command::new(runtime_name);

        cmd.args(["run", "--rm"]);

        // Network policy
        cmd.arg("--network").arg(limits.network.docker_flag());

        // Filesystem isolation
        // vox-arch-check: allow abs-path
        cmd.args(["--read-only", "--tmpfs", "/tmp"]);

        // User and capability hardening
        cmd.args([
            "--user=nobody",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
        ]);

        // Resource limits
        cmd.arg(format!("--memory={}m", limits.memory_mb));
        cmd.arg(format!("--cpus={}", limits.cpu_quota));

        // Prevent privilege escalation via setuid binaries
        cmd.args(["--security-opt", "seccomp=unconfined"]);

        // Image + command
        cmd.arg(&self.sandbox_image);
        cmd.arg(command);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        // Capture stdout incrementally
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let max_bytes = limits.max_output_bytes.unwrap_or(1_024 * 1_024) as usize; // 1 MiB default

        let stdout_str = stdout_handle
            .map(|h| {
                let mut buf = String::new();
                let reader = BufReader::new(h);
                for line in reader.lines().map_while(Result::ok) {
                    if buf.len() + line.len() < max_bytes {
                        buf.push_str(&line);
                        buf.push('\n');
                    } else {
                        break;
                    }
                }
                buf
            })
            .unwrap_or_default();

        let stderr_str = stderr_handle
            .map(|h| {
                let mut buf = String::new();
                let reader = BufReader::new(h);
                for line in reader.lines().map_while(Result::ok) {
                    buf.push_str(&line);
                    buf.push('\n');
                }
                buf
            })
            .unwrap_or_default();

        let status = child.wait()?;
        let wall_ms = start.elapsed().as_millis() as u64;

        // Check wall-time against limit
        if let Some(max_ms) = limits.max_wall_ms {
            if wall_ms > max_ms {
                return Err(SandboxError::Timeout { ms: wall_ms });
            }
        }

        let exit_code = status.code().unwrap_or(-1);

        if !status.success() {
            return Err(SandboxError::ExecutionFailed {
                exit_code,
                stderr: stderr_str,
            });
        }

        Ok(SkillOutput {
            stdout: stdout_str.trim_end().to_string(),
            stderr: stderr_str.trim_end().to_string(),
            exit_code,
            wall_ms,
        })
    }
}
