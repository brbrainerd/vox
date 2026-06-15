use serde::{Deserialize, Serialize};

/// Strategy for handling queue overflow when max tasks is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowStrategy {
    /// Block the request until space is available.
    Block,
    /// Drop the lowest-priority task to make room.
    DropLowest,
    /// Spawn a new agent to handle overflow.
    SpawnNewAgent,
}

/// Preference for balancing model quality vs operational cost.
///
/// Default is [`Economy`](CostPreference::Economy) — free-by-default product directive.
/// Callers that genuinely need the best model available should pass `Performance` explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostPreference {
    /// Prioritize model performance/quality over cost.
    Performance,
    /// Prioritize lower-cost models; zero-cost and free-tier models are first-class choices.
    #[default]
    Economy,
}

/// User-governable scaling profile: when to scale up and how aggressively to scale down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScalingProfile {
    /// Scale up only when load is high; retire idle agents quickly.
    Conservative,
    /// Default balance of scale-up threshold and retirement time.
    #[default]
    Balanced,
    /// Scale up earlier; keep idle agents longer.
    Aggressive,
}

impl ScalingProfile {
    /// Multiplier for scaling_threshold (higher = scale up later).
    pub fn threshold_multiplier(self) -> f64 {
        match self {
            ScalingProfile::Conservative => 1.5,
            ScalingProfile::Balanced => 1.0,
            ScalingProfile::Aggressive => 0.7,
        }
    }

    /// Multiplier for idle_retirement_ms (higher = retire later).
    pub fn retirement_multiplier(self) -> f64 {
        match self {
            ScalingProfile::Conservative => 0.6,
            ScalingProfile::Balanced => 1.0,
            ScalingProfile::Aggressive => 1.5,
        }
    }
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    // Catches: a swapped Conservative/Aggressive arm in threshold_multiplier that
    // would make the Aggressive profile scale up LATER than Conservative (it must
    // scale up earlier, i.e. a lower multiplier).
    #[test]
    fn threshold_multiplier_orders_aggressive_below_conservative() {
        assert_eq!(ScalingProfile::Conservative.threshold_multiplier(), 1.5);
        assert_eq!(ScalingProfile::Balanced.threshold_multiplier(), 1.0);
        assert_eq!(ScalingProfile::Aggressive.threshold_multiplier(), 0.7);
        assert!(
            ScalingProfile::Aggressive.threshold_multiplier()
                < ScalingProfile::Conservative.threshold_multiplier()
        );
    }

    // Catches: a retirement_multiplier regression where Aggressive retires idle
    // agents SOONER than Conservative (Aggressive must keep idle agents longer →
    // larger multiplier).
    #[test]
    fn retirement_multiplier_keeps_aggressive_idle_agents_longest() {
        assert_eq!(ScalingProfile::Conservative.retirement_multiplier(), 0.6);
        assert_eq!(ScalingProfile::Balanced.retirement_multiplier(), 1.0);
        assert_eq!(ScalingProfile::Aggressive.retirement_multiplier(), 1.5);
        assert!(
            ScalingProfile::Aggressive.retirement_multiplier()
                > ScalingProfile::Conservative.retirement_multiplier()
        );
    }
}
