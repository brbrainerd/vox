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
///   2. Otherwise map [`CostPreference`]: `Economy` → [`Free`](RoutingProfile::Free)
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
