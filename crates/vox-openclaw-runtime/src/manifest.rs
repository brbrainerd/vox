//! ARS-facing manifest shapes (execution limits, trust classification, skill kind).

use serde::{Deserialize, Serialize};

/// Advisory resource envelope for sandboxed task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Max wall-clock milliseconds (advisory; executor may ignore if unset).
    pub max_wall_ms: Option<u64>,
    /// Max captured output bytes (advisory; executor may ignore if unset).
    pub max_output_bytes: Option<u64>,
    /// Memory limit in MiB for container sandbox. Default: 256 MiB.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u64,
    /// CPU quota (fractional cores) for container sandbox. Default: 0.5.
    #[serde(default = "default_cpu_quota")]
    pub cpu_quota: f32,
    /// Network access policy inside the container sandbox.
    #[serde(default)]
    pub network: NetworkPolicy,
}

fn default_memory_mb() -> u64 {
    256
}

fn default_cpu_quota() -> f32 {
    0.5
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_wall_ms: None,
            max_output_bytes: None,
            memory_mb: default_memory_mb(),
            cpu_quota: default_cpu_quota(),
            network: NetworkPolicy::None,
        }
    }
}

/// Network access policy for sandboxed skill execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// No network access (default for community skills).
    #[default]
    None,
    /// Loopback only (`127.0.0.1`).
    Loopback,
    /// Unrestricted — only for operator-pinned trusted skills.
    Unrestricted,
}

impl NetworkPolicy {
    /// Return the `--network` flag value for Docker/Podman.
    pub fn docker_flag(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Loopback => "host",
            Self::Unrestricted => "bridge",
        }
    }
}

/// High-level skill classification for the runtime harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillKind {
    /// Document-style skill (markdown instructions).
    #[default]
    Document,
    /// Executable / tool-backed skill (calls Vox MCP tools).
    Tool,
    /// Shell-execution skill — always requires `Container` isolation tier.
    Shell,
}

/// Trust classification for a skill.
///
/// Determines the minimum required isolation tier and whether an explicit
/// operator approval is needed before the skill may execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Internal Vox builtins — runs with `Permissive` isolation.
    Trusted,
    /// Community skills (namespace `openclaw` or unverified) — requires
    /// explicit operator approval AND `Container` isolation.
    #[default]
    Community,
    /// Imported but not yet reviewed — execution blocked until promoted to `Community`
    /// via `vox openclaw approve`.
    Untrusted,
}

impl TrustLevel {
    /// Returns `true` if this trust level requires a pre-execution approval check.
    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::Community | Self::Untrusted)
    }

    /// Returns the minimum required isolation tier for this trust level.
    pub fn minimum_isolation(&self) -> &'static str {
        match self {
            Self::Trusted => "permissive",
            Self::Community | Self::Untrusted => "container",
        }
    }
}

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: Trusted requiring approval (security regression — trusted builtins must not block)
    #[test]
    fn trusted_does_not_require_approval() {
        assert!(
            !TrustLevel::Trusted.requires_approval(),
            "Trusted must never require approval"
        );
    }

    // Catches: Community or Untrusted being silently allowed without approval
    #[test]
    fn community_and_untrusted_require_approval() {
        assert!(
            TrustLevel::Community.requires_approval(),
            "Community must require approval"
        );
        assert!(
            TrustLevel::Untrusted.requires_approval(),
            "Untrusted must require approval"
        );
    }

    // Catches: Trusted using container isolation (too restrictive for internal builtins)
    #[test]
    fn trusted_isolation_is_permissive_not_container() {
        assert_eq!(
            TrustLevel::Trusted.minimum_isolation(),
            "permissive",
            "Trusted must have permissive isolation"
        );
    }

    // Catches: Community or Untrusted getting permissive isolation (security hole)
    #[test]
    fn community_and_untrusted_isolation_is_container() {
        assert_eq!(
            TrustLevel::Community.minimum_isolation(),
            "container",
            "Community must require container isolation"
        );
        assert_eq!(
            TrustLevel::Untrusted.minimum_isolation(),
            "container",
            "Untrusted must require container isolation"
        );
    }

    // Catches: NetworkPolicy::None docker flag being wrong (would unintentionally grant network)
    #[test]
    fn network_policy_none_maps_to_docker_none() {
        assert_eq!(NetworkPolicy::None.docker_flag(), "none");
    }

    // Catches: Unrestricted being mapped to "none" or "host" instead of "bridge"
    #[test]
    fn network_policy_unrestricted_maps_to_bridge() {
        assert_eq!(
            NetworkPolicy::Unrestricted.docker_flag(),
            "bridge",
            "Unrestricted network must use 'bridge' (full access), not 'none' or 'host'"
        );
    }

    // Catches: default ResourceLimits having memory_mb = 0, which would prevent execution
    #[test]
    fn resource_limits_default_memory_is_nonzero() {
        let limits = ResourceLimits::default();
        assert!(
            limits.memory_mb > 0,
            "default ResourceLimits memory_mb must be > 0, got {}",
            limits.memory_mb
        );
    }

    // Catches: default cpu_quota being 0.0 (would starve the container)
    #[test]
    fn resource_limits_default_cpu_is_positive() {
        let limits = ResourceLimits::default();
        assert!(
            limits.cpu_quota > 0.0,
            "default cpu_quota must be positive, got {}",
            limits.cpu_quota
        );
    }

    // Catches: default NetworkPolicy not being None (community skills must default to no network)
    #[test]
    fn resource_limits_default_network_is_none() {
        let limits = ResourceLimits::default();
        assert_eq!(
            limits.network,
            NetworkPolicy::None,
            "default network policy must be None for community-skill safety"
        );
    }

    // Catches: TrustLevel default being Trusted instead of Community
    #[test]
    fn trust_level_default_is_community_not_trusted() {
        let t = TrustLevel::default();
        assert_eq!(
            t,
            TrustLevel::Community,
            "TrustLevel default must be Community, not Trusted (escalation risk)"
        );
    }
}
