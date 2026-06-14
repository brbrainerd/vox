//! Free-tier routing profile (per `free-by-default-and-residual-work-plan-2026.md`
//! §F-F-6).
//!
//! This is the cost/tier-oriented routing profile: it answers "which *tier* of
//! models may this selection draw from" (free-only, mixed, paid, local). It is
//! distinct from the capability-oriented [`super::RoutingProfile`] (which
//! answers "does this task need vision / strict-JSON / web-search").
//!
//! Exported from [`crate::types`] as `FreeRoutingProfile` to avoid colliding
//! with the pre-existing capability `RoutingProfile`. The enum *type name* is
//! `RoutingProfile` to match the design doc verbatim.

use serde::{Deserialize, Serialize};

/// Cost/tier-oriented routing profile. [`Default`] is [`Free`](Self::Free) —
/// the free-by-default product directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingProfile {
    /// Free-tier models only — no API keys required.
    #[default]
    Free,
    /// Mix free + paid; prefer free, fall back to paid.
    Mixed,
    /// Prioritize quality; paid models freely chosen (current paid path).
    Performance,
    /// Local-only (Mens, Ollama); no external calls.
    Local,
}

impl RoutingProfile {
    /// Canonical string key for telemetry / persistence.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Mixed => "mixed",
            Self::Performance => "performance",
            Self::Local => "local",
        }
    }

    /// True when this profile should restrict selection to free-tier models
    /// (no paid fallback). [`Mixed`](Self::Mixed) prefers free but permits a
    /// paid fallback, so it is *not* free-only.
    #[must_use]
    pub fn is_free_only(self) -> bool {
        matches!(self, Self::Free)
    }

    /// True when this profile prefers free models but allows a paid fallback.
    #[must_use]
    pub fn prefers_free(self) -> bool {
        matches!(self, Self::Free | Self::Mixed)
    }

    /// True when this profile restricts selection to local providers.
    #[must_use]
    pub fn is_local_only(self) -> bool {
        matches!(self, Self::Local)
    }
}

impl std::fmt::Display for RoutingProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for RoutingProfile {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "free" => Ok(Self::Free),
            "mixed" => Ok(Self::Mixed),
            "performance" | "perf" => Ok(Self::Performance),
            "local" => Ok(Self::Local),
            _ => Err(()),
        }
    }
}

/// Derive a [`RoutingProfile`] from orchestrator config + the `VoxRoutingProfile`
/// secret overlay.
///
/// Resolution order:
///   1. The `VoxRoutingProfile` secret, if set and parseable, wins.
///   2. Otherwise map `CostPreference`: `Economy` → [`Free`](RoutingProfile::Free)
///      (free-by-default), `Performance` → [`Performance`](RoutingProfile::Performance).
#[must_use]
pub fn config_to_routing_profile(cfg: &crate::config::OrchestratorConfig) -> RoutingProfile {
    if let Some(raw) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxRoutingProfile)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        && let Ok(profile) = raw.parse::<RoutingProfile>()
    {
        return profile;
    }
    match cfg.cost_preference {
        crate::config::CostPreference::Economy => RoutingProfile::Free,
        crate::config::CostPreference::Performance => RoutingProfile::Performance,
    }
}

#[cfg(test)]
mod tests {
    // The env-mutating test removes/restores VOX_ROUTING_PROFILE; it is `#[serial]`
    // so no other env-mutating test runs concurrently, and it restores the prior value.
    use serial_test::serial;

    use super::*;
    use std::str::FromStr;

    #[test]
    fn is_free_only_per_variant() {
        assert!(RoutingProfile::Free.is_free_only());
        assert!(!RoutingProfile::Mixed.is_free_only());
        assert!(!RoutingProfile::Performance.is_free_only());
        assert!(!RoutingProfile::Local.is_free_only());
    }

    #[test]
    fn prefers_free_per_variant() {
        assert!(RoutingProfile::Free.prefers_free());
        assert!(RoutingProfile::Mixed.prefers_free());
        assert!(!RoutingProfile::Performance.prefers_free());
        assert!(!RoutingProfile::Local.prefers_free());
    }

    #[test]
    fn is_local_only_per_variant() {
        assert!(!RoutingProfile::Free.is_local_only());
        assert!(!RoutingProfile::Mixed.is_local_only());
        assert!(!RoutingProfile::Performance.is_local_only());
        assert!(RoutingProfile::Local.is_local_only());
    }

    #[test]
    fn default_is_free() {
        assert_eq!(RoutingProfile::default(), RoutingProfile::Free);
    }

    #[test]
    fn from_str_round_trips_canonical_keys() {
        for p in [
            RoutingProfile::Free,
            RoutingProfile::Mixed,
            RoutingProfile::Performance,
            RoutingProfile::Local,
        ] {
            assert_eq!(RoutingProfile::from_str(p.as_str()), Ok(p));
        }
    }

    #[test]
    fn from_str_accepts_perf_alias_and_is_case_insensitive() {
        assert_eq!(
            RoutingProfile::from_str("perf"),
            Ok(RoutingProfile::Performance)
        );
        assert_eq!(
            RoutingProfile::from_str("  PERFORMANCE  "),
            Ok(RoutingProfile::Performance)
        );
    }

    #[test]
    fn from_str_rejects_unknown() {
        assert_eq!(RoutingProfile::from_str("nonsense"), Err(()));
    }

    #[test]
    #[serial]
    #[allow(unsafe_code)]
    fn config_maps_cost_preference_when_secret_unset() {
        // The secret overlay reads VOX_ROUTING_PROFILE; remove it so the
        // CostPreference branch is exercised deterministically.
        let prior = std::env::var("VOX_ROUTING_PROFILE").ok();
        unsafe { std::env::remove_var("VOX_ROUTING_PROFILE") };

        let mut cfg = crate::config::OrchestratorConfig {
            cost_preference: crate::config::CostPreference::Economy,
            ..Default::default()
        };
        assert_eq!(config_to_routing_profile(&cfg), RoutingProfile::Free);
        cfg.cost_preference = crate::config::CostPreference::Performance;
        assert_eq!(config_to_routing_profile(&cfg), RoutingProfile::Performance);

        unsafe {
            if let Some(v) = prior {
                std::env::set_var("VOX_ROUTING_PROFILE", v);
            }
        }
    }
}
