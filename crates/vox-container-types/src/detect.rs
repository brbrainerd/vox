//! Runtime preference type for container backend selection.

/// Preferred runtime selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimePreference {
    /// Try Podman first, fall back to Docker.
    #[default]
    Auto,
    /// Use Docker only.
    Docker,
    /// Use Podman only.
    Podman,
}

impl std::str::FromStr for RuntimePreference {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "docker" => Ok(Self::Docker),
            "podman" => Ok(Self::Podman),
            other => {
                anyhow::bail!("Unknown runtime preference: {other:?}. Use auto, docker, or podman.")
            }
        }
    }
}
