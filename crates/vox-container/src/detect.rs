//! Auto-detection of available container runtimes.
//!
//! Prefers Podman (rootless, daemonless) when available, falling back to
//! Docker. Returns a boxed [`ContainerRuntime`] trait object.

use crate::docker::DockerRuntime;
use crate::podman::PodmanRuntime;
use crate::runtime::ContainerRuntime;

/// Preferred runtime selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePreference {
    /// Try Podman first, fall back to Docker.
    Auto,
    /// Use Docker only.
    Docker,
    /// Use Podman only.
    Podman,
}

impl Default for RuntimePreference {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::str::FromStr for RuntimePreference {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "docker" => Ok(Self::Docker),
            "podman" => Ok(Self::Podman),
            other => anyhow::bail!("Unknown runtime preference: {other:?}. Use auto, docker, or podman."),
        }
    }
}

/// Detect and return the best available container runtime.
///
/// With [`RuntimePreference::Auto`], Podman is preferred because it runs
/// rootless without a daemon. If neither runtime is available, returns an error.
///
/// # Examples
///
/// ```no_run
/// let runtime = vox_container::detect_runtime(vox_container::detect::RuntimePreference::Auto)?;
/// println!("Using container runtime: {}", runtime.name());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn detect_runtime(
    preference: RuntimePreference,
) -> anyhow::Result<Box<dyn ContainerRuntime>> {
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
            // Prefer Podman (rootless, daemonless)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_preference_from_str() {
        assert_eq!(
            "auto".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Auto
        );
        assert_eq!(
            "docker".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Docker
        );
        assert_eq!(
            "podman".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Podman
        );
        assert!("invalid".parse::<RuntimePreference>().is_err());
    }

    #[test]
    fn docker_runtime_name() {
        let rt = DockerRuntime::new();
        assert_eq!(rt.name(), "docker");
    }

    #[test]
    fn podman_runtime_name() {
        let rt = PodmanRuntime::new();
        assert_eq!(rt.name(), "podman");
    }
}
