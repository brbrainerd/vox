//! Podman CLI backend for [`ContainerRuntime`].
//!
//! Podman runs rootless by default, making it ideal for userspace containers
//! without requiring elevated privileges or a daemon process.

use crate::runtime::{BuildOpts, ContainerRuntime, RunOpts};
use std::process::Command;

/// Podman-backed container runtime.
///
/// Shells out to the `podman` CLI. Runs rootless by default — no daemon
/// required. Uses OCI image format for maximum compatibility with Docker
/// registries and tooling.
#[derive(Debug, Default)]
pub struct PodmanRuntime;

impl PodmanRuntime {
    /// Create a new Podman runtime handle.
    pub fn new() -> Self {
        Self
    }
}

impl ContainerRuntime for PodmanRuntime {
    fn name(&self) -> &str {
        "podman"
    }

    fn available(&self) -> bool {
        Command::new("podman")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn version(&self) -> anyhow::Result<String> {
        let output = Command::new("podman")
            .arg("--version")
            .output()
            .map_err(|e| anyhow::anyhow!("podman not found: {e}"))?;
        if !output.status.success() {
            anyhow::bail!("podman --version failed");
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn build(&self, opts: &BuildOpts) -> anyhow::Result<String> {
        let mut cmd = Command::new("podman");
        cmd.arg("build");
        // Force OCI format for maximum registry compatibility
        cmd.arg("--format").arg("oci");
        cmd.arg("-t").arg(&opts.tag);

        if let Some(ref df) = opts.dockerfile {
            cmd.arg("-f").arg(df);
        }

        for (key, val) in &opts.build_args {
            cmd.arg("--build-arg").arg(format!("{key}={val}"));
        }

        cmd.arg(&opts.context_dir);

        tracing::info!("Running: podman build --format oci -t {} ...", opts.tag);
        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run podman build: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("podman build failed:\n{stderr}");
        }

        Ok(opts.tag.clone())
    }

    fn run(&self, opts: &RunOpts) -> anyhow::Result<()> {
        let mut cmd = Command::new("podman");
        cmd.arg("run");

        if opts.detach {
            cmd.arg("-d");
        }
        if opts.rm {
            cmd.arg("--rm");
        }
        if let Some(ref name) = opts.name {
            cmd.arg("--name").arg(name);
        }
        for (host, container) in &opts.ports {
            cmd.arg("-p").arg(format!("{host}:{container}"));
        }
        for (key, val) in &opts.env {
            cmd.arg("-e").arg(format!("{key}={val}"));
        }
        for (host_path, container_path) in &opts.volumes {
            cmd.arg("-v").arg(format!("{host_path}:{container_path}"));
        }

        cmd.arg(&opts.image);

        tracing::info!("Running: podman run {} ...", opts.image);
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run podman run: {e}"))?;

        if !status.success() {
            anyhow::bail!("podman run failed with exit code: {:?}", status.code());
        }
        Ok(())
    }

    fn push(&self, tag: &str) -> anyhow::Result<()> {
        tracing::info!("Running: podman push {tag}");
        let status = Command::new("podman")
            .arg("push")
            .arg(tag)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run podman push: {e}"))?;

        if !status.success() {
            anyhow::bail!("podman push failed");
        }
        Ok(())
    }

    fn tag(&self, source: &str, target: &str) -> anyhow::Result<()> {
        tracing::info!("Running: podman tag {source} {target}");
        let status = Command::new("podman")
            .arg("tag")
            .arg(source)
            .arg(target)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run podman tag: {e}"))?;

        if !status.success() {
            anyhow::bail!("podman tag failed");
        }
        Ok(())
    }

    fn login(&self, registry: &str, username: &str, token: &str) -> anyhow::Result<()> {
        tracing::info!("Running: podman login {registry}");
        let mut cmd = Command::new("podman");
        cmd.arg("login");
        cmd.arg("-u").arg(username);
        cmd.arg("--password-stdin");
        cmd.arg(registry);

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn podman login: {e}"))?;

        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(token.as_bytes())?;
        stdin.flush()?;
        drop(stdin);

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("podman login failed for registry: {registry}");
        }
        Ok(())
    }
}
