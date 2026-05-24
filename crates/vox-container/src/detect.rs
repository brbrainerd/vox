//! Auto-detection of available container runtimes.
//!
//! Prefers Podman (rootless, daemonless) when available, falling back to
//! Docker. Returns a boxed [`ContainerRuntime`] trait object.
//!
//! The [`RuntimePreference`] type lives in `vox-container-types` and is
//! re-exported here for backwards compatibility.

use crate::docker::DockerRuntime;
use crate::podman::PodmanRuntime;
use vox_container_types::ContainerRuntime;

pub use vox_container_types::RuntimePreference;

/// Detect and return the best available container runtime.
///
/// With [`RuntimePreference::Auto`], Podman is preferred because it runs
/// rootless without a daemon. If neither runtime is available, returns an error.
pub fn detect_runtime(preference: RuntimePreference) -> anyhow::Result<Box<dyn ContainerRuntime>> {
    match preference {
        RuntimePreference::Docker => {
            let rt = DockerRuntime::new();
            if rt.available() {
                Ok(Box::new(rt))
            } else {
                anyhow::bail!(
                    "Docker was requested but is not installed or not running.\n\
                     Install from https://docs.docker.com/get-docker/"
                )
            }
        }
        RuntimePreference::Podman => {
            let rt = PodmanRuntime::new();
            if rt.available() {
                Ok(Box::new(rt))
            } else {
                anyhow::bail!(
                    "Podman was requested but is not installed.\n\
                     Install from https://podman.io/getting-started/installation"
                )
            }
        }
        RuntimePreference::Auto => {
            let podman = PodmanRuntime::new();
            if podman.available() {
                tracing::info!("Auto-detected Podman (rootless)");
                return Ok(Box::new(podman));
            }

            let docker = DockerRuntime::new();
            if docker.available() {
                tracing::info!("Auto-detected Docker");
                return Ok(Box::new(docker));
            }

            anyhow::bail!(
                "No container runtime found.\n\
                 Install Podman (recommended): https://podman.io/getting-started/installation\n\
                 Or install Docker: https://docs.docker.com/get-docker/"
            )
        }
    }
}
