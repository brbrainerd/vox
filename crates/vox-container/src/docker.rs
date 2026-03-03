//! Docker CLI backend for [`ContainerRuntime`].

use crate::runtime::{BuildOpts, ContainerRuntime, RunOpts};
use std::process::Command;

/// Docker-backed container runtime.
///
/// Shells out to the `docker` CLI. Requires Docker Desktop or Docker Engine
/// to be installed and the daemon to be running.
#[derive(Debug, Default)]
pub struct DockerRuntime;

impl DockerRuntime {
    /// Create a new Docker runtime handle.
    pub fn new() -> Self {
        Self
    }
}

impl ContainerRuntime for DockerRuntime {
    fn name(&self) -> &str {
        "docker"
    }

    fn available(&self) -> bool {
        Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn version(&self) -> anyhow::Result<String> {
        let output = Command::new("docker")
            .arg("--version")
            .output()
            .map_err(|e| anyhow::anyhow!("docker not found: {e}"))?;
        if !output.status.success() {
            anyhow::bail!("docker --version failed");
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn build(&self, opts: &BuildOpts) -> anyhow::Result<String> {
        let mut cmd = Command::new("docker");
        cmd.arg("build");
        cmd.arg("-t").arg(&opts.tag);

        if let Some(ref df) = opts.dockerfile {
            cmd.arg("-f").arg(df);
        }

        for (key, val) in &opts.build_args {
            cmd.arg("--build-arg").arg(format!("{key}={val}"));
        }

        cmd.arg(&opts.context_dir);

        tracing::info!("Running: docker build -t {} ...", opts.tag);
        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to run docker build: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker build failed:\n{stderr}");
        }

        Ok(opts.tag.clone())
    }

    fn run(&self, opts: &RunOpts) -> anyhow::Result<()> {
        let mut cmd = Command::new("docker");
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

        tracing::info!("Running: docker run {} ...", opts.image);
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run docker run: {e}"))?;

        if !status.success() {
            anyhow::bail!("docker run failed with exit code: {:?}", status.code());
        }
        Ok(())
    }

    fn push(&self, tag: &str) -> anyhow::Result<()> {
        tracing::info!("Running: docker push {tag}");
        let status = Command::new("docker")
            .arg("push")
            .arg(tag)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run docker push: {e}"))?;

        if !status.success() {
            anyhow::bail!("docker push failed");
        }
        Ok(())
    }

    fn tag(&self, source: &str, target: &str) -> anyhow::Result<()> {
        tracing::info!("Running: docker tag {source} {target}");
        let status = Command::new("docker")
            .arg("tag")
            .arg(source)
            .arg(target)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run docker tag: {e}"))?;

        if !status.success() {
            anyhow::bail!("docker tag failed");
        }
        Ok(())
    }

    fn login(&self, registry: &str, username: &str, token: &str) -> anyhow::Result<()> {
        tracing::info!("Running: docker login {registry}");
        let mut cmd = Command::new("docker");
        cmd.arg("login");
        cmd.arg("-u").arg(username);
        cmd.arg("-p-stdin"); // Use stdin for security
        cmd.arg(registry);

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn docker login: {e}"))?;

        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(token.as_bytes())?;
        stdin.flush()?;
        drop(stdin); // Close stdin to signal EOF

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("docker login failed for registry: {registry}");
        }
        Ok(())
    }
}
