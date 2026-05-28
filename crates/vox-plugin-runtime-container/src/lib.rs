//! # vox-plugin-runtime-container
//!
//! Skill-runtime plugin providing Docker and Podman backends.
//!
//! Implements [`vox_skill_runtime::SkillRuntime`] for both Docker and Podman,
//! and also implements [`vox_container_types::ContainerRuntime`] (OCI build/push/tag/login)
//! for use by `vox deploy`.
//!
//! Registered as a Vox plugin; loaded by the plugin host on demand.
//! Install with: `vox plugin install runtime-container`

use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, std_types::*};
use std::io::{BufRead as _, BufReader};
use std::process::{Command, Stdio};
use std::time::Instant;
use vox_container_types::ContainerRuntime;
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;
use vox_skill_runtime::{
    BuildOpts as SkillBuildOpts, RunOpts as SkillRunOpts, RunOutcome, SkillRuntime,
};

pub mod docker;
pub mod podman;

// ─── Shared container-run helper ─────────────────────────────────────────────

/// Execute a skill OCI image using the given CLI tool name (`"docker"` or `"podman"`).
///
/// Applies the Vox sandbox hardening flags:
/// - `--read-only --tmpfs /tmp`
/// - `--user=nobody --cap-drop=ALL --security-opt=no-new-privileges`
/// - stdout/stderr capture with 1 MiB cap
fn run_sandboxed(cli: &str, opts: &SkillRunOpts) -> anyhow::Result<RunOutcome> {
    let start = Instant::now();

    // The artifact_path field doubles as the image tag for container runtimes.
    let image = opts
        .artifact_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("artifact_path is not valid UTF-8"))?;

    vox_container::log_exec_risk(image);

    let mut cmd = Command::new(cli);
    cmd.args(["run", "--rm"]);

    // Filesystem isolation + user hardening.
    cmd.args(["--read-only", "--tmpfs", "/tmp"]);
    cmd.args([
        "--user=nobody",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
    ]);

    if opts.detach {
        cmd.arg("-d");
    }
    if let Some(ref name) = opts.name {
        cmd.arg("--name").arg(name);
    }
    for (host_port, container_port) in &opts.ports {
        cmd.arg("-p").arg(format!("{host_port}:{container_port}"));
    }
    for (key, val) in &opts.env {
        cmd.arg("-e").arg(format!("{key}={val}"));
    }
    for (host_path, container_path) in &opts.volumes {
        cmd.arg("-v").arg(format!("{host_path}:{container_path}"));
    }

    cmd.arg(image);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    tracing::info!(target: "container-runtime", "{cli} run {image}");
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn {cli}: {e}"))?;

    const MAX_BYTES: usize = 1024 * 1024; // 1 MiB

    let stdout_str = child
        .stdout
        .take()
        .map(|h| {
            let mut buf = String::new();
            for line in BufReader::new(h).lines().map_while(Result::ok) {
                if buf.len() + line.len() < MAX_BYTES {
                    buf.push_str(&line);
                    buf.push('\n');
                } else {
                    break;
                }
            }
            buf
        })
        .unwrap_or_default();

    let stderr_str = child
        .stderr
        .take()
        .map(|h| {
            let mut buf = String::new();
            for line in BufReader::new(h).lines().map_while(Result::ok) {
                buf.push_str(&line);
                buf.push('\n');
            }
            buf
        })
        .unwrap_or_default();

    let status = child.wait()?;
    let wall_ms = start.elapsed().as_millis() as u64;
    let exit_code = status.code().unwrap_or(-1);

    Ok(RunOutcome {
        exit_code,
        stdout: stdout_str.trim_end().to_string(),
        stderr: stderr_str.trim_end().to_string(),
        wall_ms,
    })
}

// ─── SkillRuntime impls ──────────────────────────────────────────────────────

impl SkillRuntime for docker::DockerRuntime {
    fn name(&self) -> &str {
        "docker"
    }

    fn available(&self) -> bool {
        ContainerRuntime::available(self)
    }

    fn build(&self, opts: &SkillBuildOpts) -> anyhow::Result<()> {
        let build_opts = vox_container_types::BuildOpts {
            context_dir: opts.context_dir.clone(),
            dockerfile: opts.artifact_path.clone(),
            tag: opts.tag.clone(),
            build_args: opts.build_args.clone(),
        };
        ContainerRuntime::build(self, &build_opts)?;
        Ok(())
    }

    fn run(&self, opts: &SkillRunOpts) -> anyhow::Result<RunOutcome> {
        run_sandboxed("docker", opts)
    }
}

impl SkillRuntime for podman::PodmanRuntime {
    fn name(&self) -> &str {
        "podman"
    }

    fn available(&self) -> bool {
        ContainerRuntime::available(self)
    }

    fn build(&self, opts: &SkillBuildOpts) -> anyhow::Result<()> {
        let build_opts = vox_container_types::BuildOpts {
            context_dir: opts.context_dir.clone(),
            dockerfile: opts.artifact_path.clone(),
            tag: opts.tag.clone(),
            build_args: opts.build_args.clone(),
        };
        ContainerRuntime::build(self, &build_opts)?;
        Ok(())
    }

    fn run(&self, opts: &SkillRunOpts) -> anyhow::Result<RunOutcome> {
        run_sandboxed("podman", opts)
    }
}

// ─── Plugin scaffold ─────────────────────────────────────────────────────────

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"runtime-container","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = RuntimeContainerPlugin;
    let to = VoxPlugin_TO::from_value(plugin, abi_stable::erased_types::TD_Opaque);
    RResult::ROk(to)
}

struct RuntimeContainerPlugin;

impl VoxPlugin for RuntimeContainerPlugin {
    fn id(&self) -> RString {
        RString::from("runtime-container")
    }

    fn shutdown(&self) -> RResult<(), RBoxError> {
        RResult::ROk(())
    }
}

// Re-export detect_runtime and RuntimePreference for consumers of the plugin as an rlib.
pub use vox_container::{detect::RuntimePreference, detect_runtime};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_advertises_runtime_container_id() {
        assert!(manifest_json().as_str().contains("\"runtime-container\""));
        assert_eq!(RuntimeContainerPlugin.id().as_str(), "runtime-container");
    }
}
