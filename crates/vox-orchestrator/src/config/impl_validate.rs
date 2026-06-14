use super::errors::ConfigValidationError;
use super::orchestrator_fields::OrchestratorConfig;

impl OrchestratorConfig {
    /// Validates the configuration against required invariants.
    pub fn validate(&self) -> Result<(), Vec<ConfigValidationError>> {
        let mut errors = Vec::new();

        if self.max_agents < 1 {
            errors.push(ConfigValidationError::InvalidMaxAgents(self.max_agents));
        }
        if self.lock_timeout_ms < 100 {
            errors.push(ConfigValidationError::InvalidLockTimeout(
                self.lock_timeout_ms,
            ));
        }
        if self.bulletin_capacity < 1 {
            errors.push(ConfigValidationError::InvalidBulletinCapacity(
                self.bulletin_capacity,
            ));
        }
        if self.min_agents > self.max_agents {
            errors.push(ConfigValidationError::InvalidScalingLimits(
                self.min_agents,
                self.max_agents,
            ));
        }
        if self.planning_router_enabled && !self.planning_enabled {
            errors.push(ConfigValidationError::PlanningInvalid(
                "planning_router_enabled requires planning_enabled".to_string(),
            ));
        }
        if self.planning_replan_enabled && !self.planning_enabled {
            errors.push(ConfigValidationError::PlanningInvalid(
                "planning_replan_enabled requires planning_enabled".to_string(),
            ));
        }
        if self.planning_workflow_handoff_enabled && !self.planning_enabled {
            errors.push(ConfigValidationError::PlanningInvalid(
                "planning_workflow_handoff_enabled requires planning_enabled".to_string(),
            ));
        }
        if self.planning_rollout_percent > 100 {
            errors.push(ConfigValidationError::PlanningInvalid(
                "planning_rollout_percent must be <= 100".to_string(),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod semcov_wave14_tests {
    use super::super::errors::ConfigValidationError;
    use super::super::orchestrator_fields::OrchestratorConfig;

    #[test]
    fn lock_timeout_exactly_at_minimum_is_valid() {
        // Catches: off-by-one where `< 100` is replaced with `<= 100`, accepting 99 ms.
        let cfg = OrchestratorConfig {
            lock_timeout_ms: 100,
            ..OrchestratorConfig::for_testing()
        };
        assert!(
            cfg.validate().is_ok(),
            "lock_timeout_ms=100 must be accepted (minimum boundary)"
        );
    }

    #[test]
    fn lock_timeout_one_below_minimum_is_rejected_with_exact_variant() {
        // Catches: validator checking wrong field or swallowing the error.
        let cfg = OrchestratorConfig {
            lock_timeout_ms: 99,
            ..OrchestratorConfig::for_testing()
        };
        let errs = cfg.validate().unwrap_err();
        assert!(
            errs.contains(&ConfigValidationError::InvalidLockTimeout(99)),
            "expected InvalidLockTimeout(99), got {:?}",
            errs
        );
    }

    #[test]
    fn max_agents_zero_rejected_min_agents_inversion_also_fires() {
        // Catches: validator only checking one of the two related fields, missing the
        // compound error — e.g. max_agents=0 but min_agents=1 triggers BOTH
        // InvalidMaxAgents AND InvalidScalingLimits in a single call.
        let cfg = OrchestratorConfig {
            max_agents: 0,
            min_agents: 1,
            ..OrchestratorConfig::for_testing()
        };
        let errs = cfg.validate().unwrap_err();
        assert!(
            errs.contains(&ConfigValidationError::InvalidMaxAgents(0)),
            "expected InvalidMaxAgents(0)"
        );
        assert!(
            errs.contains(&ConfigValidationError::InvalidScalingLimits(1, 0)),
            "expected InvalidScalingLimits(1, 0)"
        );
    }

    #[test]
    fn min_agents_equal_to_max_agents_is_valid() {
        // Catches: `>=` used instead of `>` in the scaling-limits check.
        let cfg = OrchestratorConfig {
            min_agents: 4,
            max_agents: 4,
            ..OrchestratorConfig::for_testing()
        };
        assert!(
            cfg.validate().is_ok(),
            "min_agents == max_agents must be accepted"
        );
    }

    #[test]
    fn planning_router_without_planning_enabled_is_rejected() {
        // Catches: dependency check omitted for planning_router_enabled when
        // the author only wired planning_replan_enabled.
        let cfg = OrchestratorConfig {
            planning_enabled: false,
            planning_router_enabled: true,
            ..OrchestratorConfig::for_testing()
        };
        let errs = cfg.validate().unwrap_err();
        assert!(
            errs.iter().any(|e| matches!(e, ConfigValidationError::PlanningInvalid(msg) if msg.contains("planning_router_enabled"))),
            "expected PlanningInvalid mentioning planning_router_enabled, got {:?}",
            errs
        );
    }

    #[test]
    fn all_three_planning_dependents_off_with_planning_disabled_is_valid() {
        // Catches: validate returning Err when all planning flags are consistently off.
        let cfg = OrchestratorConfig {
            planning_enabled: false,
            planning_router_enabled: false,
            planning_replan_enabled: false,
            planning_workflow_handoff_enabled: false,
            ..OrchestratorConfig::for_testing()
        };
        assert!(
            cfg.validate().is_ok(),
            "all planning flags off must not produce errors"
        );
    }

    #[test]
    fn planning_rollout_percent_101_is_rejected() {
        // Catches: `> 100` check silently dropped, allowing 101 → silent bad behavior.
        let cfg = OrchestratorConfig {
            planning_enabled: true,
            planning_rollout_percent: 101,
            ..OrchestratorConfig::for_testing()
        };
        let errs = cfg.validate().unwrap_err();
        assert!(
            errs.iter().any(|e| matches!(e, ConfigValidationError::PlanningInvalid(msg) if msg.contains("planning_rollout_percent"))),
            "expected PlanningInvalid mentioning planning_rollout_percent, got {:?}",
            errs
        );
    }

    #[test]
    fn planning_rollout_percent_100_is_valid() {
        // Catches: off-by-one — `> 100` coded as `>= 100`, rejecting 100%.
        let cfg = OrchestratorConfig {
            planning_enabled: true,
            planning_rollout_percent: 100,
            ..OrchestratorConfig::for_testing()
        };
        assert!(
            cfg.validate().is_ok(),
            "planning_rollout_percent=100 must be accepted"
        );
    }
}
