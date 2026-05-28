//! Three-tier model cascade for autonomous model-routing (D1).
//!
//! Maps task complexity, circuit-breaker alarm level, and budget state to a
//! `RoutingTier` (Economy / Standard / Strong), which callers translate to
//! [`crate::config::CostPreference`] or a direct [`crate::models::ModelSpec`] lookup.
//!
//! All logic is pure: no async, no I/O, no allocations on the hot path.

use serde::{Deserialize, Serialize};

use crate::circuit_breaker::AlarmTier;

/// Coarse three-level routing tier (independent of provider-specific ModelTier variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoutingTier {
    /// Low-complexity tasks; cheapest capable model. Complexity 0–3 when no pressure signals.
    Economy,
    /// Mid-complexity tasks; balanced quality/cost. Complexity 4–7.
    Standard,
    /// High-complexity or stressed tasks; strongest available model. Complexity 8–10.
    Strong,
}

impl std::fmt::Display for RoutingTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Economy => write!(f, "economy"),
            Self::Standard => write!(f, "standard"),
            Self::Strong => write!(f, "strong"),
        }
    }
}

/// Alarm level inferred from the circuit breaker; adapts tier upward when the loop is stressed.
/// Mirrors [`AlarmTier`] so callers that don't use circuit_breaker can still pass a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlarmLevel {
    None,
    Caution,
    Warning,
}

impl From<AlarmTier> for AlarmLevel {
    fn from(t: AlarmTier) -> Self {
        match t {
            AlarmTier::None => Self::None,
            AlarmTier::Caution => Self::Caution,
            AlarmTier::Warning => Self::Warning,
        }
    }
}

impl std::fmt::Display for AlarmLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Caution => write!(f, "caution"),
            Self::Warning => write!(f, "warning"),
        }
    }
}

/// Input signals consumed by the cascade router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeSignal {
    /// Task complexity 0–10 (10 = highest).
    pub complexity: u8,
    /// Current alarm level from the circuit breaker.
    pub alarm_level: AlarmLevel,
    /// Composite confidence score from [`crate::confidence_fusion`] in `[0, 1]`.
    pub confidence: f64,
    /// True when the session budget is exhausted; forces Economy.
    pub budget_exhausted: bool,
}

impl Default for CompositeSignal {
    fn default() -> Self {
        Self {
            complexity: 5,
            alarm_level: AlarmLevel::None,
            confidence: 0.75,
            budget_exhausted: false,
        }
    }
}

/// Thresholds loaded from contract YAML. Defaults mirror contract defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierCascadeConfig {
    /// Complexity ≤ this → Economy (if no override).
    pub economy_max_complexity: u8,
    /// Complexity ≤ this → Standard (if no override).
    pub standard_max_complexity: u8,
    /// Confidence below this triggers upgrade by one tier.
    pub low_confidence_threshold: f64,
}

impl Default for TierCascadeConfig {
    fn default() -> Self {
        Self {
            economy_max_complexity: 3,
            standard_max_complexity: 7,
            low_confidence_threshold: 0.55,
        }
    }
}

/// Pure tier cascade router.
pub struct TierCascadeRouter {
    config: TierCascadeConfig,
}

impl TierCascadeRouter {
    pub fn new(config: TierCascadeConfig) -> Self {
        Self { config }
    }

    /// Select the routing tier for a given [`CompositeSignal`].
    ///
    /// Resolution order:
    /// 1. `budget_exhausted` → Economy (hard cap).
    /// 2. Base tier from `complexity`.
    /// 3. Upgrade one tier if `confidence < low_confidence_threshold`.
    /// 4. Upgrade one tier if `alarm_level >= Warning`.
    /// 5. Cap at `Strong`.
    #[must_use]
    #[inline]
    pub fn select(&self, signal: &CompositeSignal) -> RoutingTier {
        if signal.budget_exhausted {
            return RoutingTier::Economy;
        }

        let base = self.base_tier(signal.complexity);

        let after_confidence = if signal.confidence < self.config.low_confidence_threshold {
            self.upgrade(base)
        } else {
            base
        };

        if signal.alarm_level >= AlarmLevel::Warning {
            self.upgrade(after_confidence)
        } else {
            after_confidence
        }
    }

    #[inline]
    fn base_tier(&self, complexity: u8) -> RoutingTier {
        if complexity <= self.config.economy_max_complexity {
            RoutingTier::Economy
        } else if complexity <= self.config.standard_max_complexity {
            RoutingTier::Standard
        } else {
            RoutingTier::Strong
        }
    }

    #[inline]
    fn upgrade(&self, tier: RoutingTier) -> RoutingTier {
        match tier {
            RoutingTier::Economy => RoutingTier::Standard,
            RoutingTier::Standard | RoutingTier::Strong => RoutingTier::Strong,
        }
    }
}

/// Metric payload emitted when a tier routing decision is made.
/// Serialize-only — see `TripEvent` for rationale on the missing `Deserialize`.
#[derive(Debug, Clone, Serialize)]
pub struct TierRouteEvent {
    pub metric_type: &'static str,
    pub tier: String,
    pub complexity: u8,
    pub confidence: f64,
    pub alarm_level: String,
    pub budget_exhausted: bool,
    pub session_id: Option<String>,
}

impl TierRouteEvent {
    pub fn new(tier: RoutingTier, signal: &CompositeSignal, session_id: Option<String>) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_MODEL_TIER_ROUTE,
            tier: tier.to_string(),
            complexity: signal.complexity,
            confidence: signal.confidence,
            alarm_level: signal.alarm_level.to_string(),
            budget_exhausted: signal.budget_exhausted,
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn router() -> TierCascadeRouter {
        TierCascadeRouter::new(TierCascadeConfig::default())
    }

    #[test]
    fn low_complexity_gives_economy() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 2,
            alarm_level: AlarmLevel::None,
            confidence: 0.80,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Economy);
    }

    #[test]
    fn mid_complexity_gives_standard() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 5,
            alarm_level: AlarmLevel::None,
            confidence: 0.80,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Standard);
    }

    #[test]
    fn high_complexity_gives_strong() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 9,
            alarm_level: AlarmLevel::None,
            confidence: 0.80,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Strong);
    }

    #[test]
    fn budget_exhausted_forces_economy() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 9,
            alarm_level: AlarmLevel::Warning,
            confidence: 0.30,
            budget_exhausted: true,
        };
        assert_eq!(r.select(&sig), RoutingTier::Economy);
    }

    #[test]
    fn low_confidence_upgrades_economy_to_standard() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 2,
            alarm_level: AlarmLevel::None,
            confidence: 0.40,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Standard);
    }

    #[test]
    fn low_confidence_upgrades_standard_to_strong() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 5,
            alarm_level: AlarmLevel::None,
            confidence: 0.40,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Strong);
    }

    #[test]
    fn warning_alarm_upgrades_economy_to_standard() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 2,
            alarm_level: AlarmLevel::Warning,
            confidence: 0.80,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Standard);
    }

    #[test]
    fn caution_alarm_does_not_upgrade() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 2,
            alarm_level: AlarmLevel::Caution,
            confidence: 0.80,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Economy);
    }

    #[test]
    fn both_low_confidence_and_warning_alarm_caps_at_strong() {
        let r = router();
        let sig = CompositeSignal {
            complexity: 5,
            alarm_level: AlarmLevel::Warning,
            confidence: 0.40,
            budget_exhausted: false,
        };
        assert_eq!(r.select(&sig), RoutingTier::Strong);
    }

    #[test]
    fn alarm_tier_from_circuit_breaker_converts() {
        assert_eq!(AlarmLevel::from(AlarmTier::None), AlarmLevel::None);
        assert_eq!(AlarmLevel::from(AlarmTier::Caution), AlarmLevel::Caution);
        assert_eq!(AlarmLevel::from(AlarmTier::Warning), AlarmLevel::Warning);
    }

    #[test]
    fn tier_route_event_has_correct_metric_type() {
        let sig = CompositeSignal::default();
        let ev = TierRouteEvent::new(RoutingTier::Standard, &sig, None);
        assert_eq!(ev.metric_type, "orch.routing.tier");
    }
}
