//! Embedded `contracts/orchestration/feature-flags.v1.yaml`.

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FeatureFlagsRoot {
    flags: std::collections::HashMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationFeatureFlags {
    pub circuit_breaker: bool,
    pub socrates_fusion: bool,
    pub tier_cascade: bool,
    pub plan_mode_trigger: bool,
    pub risk_matrix_hitl: bool,
    pub privacy_routing: bool,
    pub cache_aware_routing: bool,
    pub compaction_5layer: bool,
    pub tenant_budget: bool,
    pub calibration_loop: bool,
    pub drift_detector: bool,
    pub contextual_bandit: bool,
    pub subagent_dispatch: bool,
    pub chain_length_cap: bool,
    pub agentos_aci_envelope: bool,
    pub agentos_guardrail_kernel: bool,
    pub agentos_checkpoint_hints: bool,
}

impl OrchestrationFeatureFlags {
    #[must_use]
    pub fn from_embedded_contract() -> Self {
        Self::parse_yaml(include_str!(
            "../../../contracts/orchestration/feature-flags.v1.yaml"
        ))
        .unwrap_or_else(|_| Self::all_disabled())
    }

    fn parse_yaml(yaml: &str) -> Result<Self, String> {
        let root: FeatureFlagsRoot =
            serde_yaml::from_str(yaml).map_err(|e| format!("feature-flags YAML: {e}"))?;
        let g = |k: &str| *root.flags.get(k).unwrap_or(&false);
        Ok(Self {
            circuit_breaker: g("vox.orchestrator.circuit_breaker.enabled"),
            socrates_fusion: g("vox.orchestrator.socrates_fusion.enabled"),
            tier_cascade: g("vox.orchestrator.tier_cascade.enabled"),
            plan_mode_trigger: g("vox.orchestrator.plan_mode_trigger.enabled"),
            risk_matrix_hitl: g("vox.orchestrator.risk_matrix_hitl.enabled"),
            privacy_routing: g("vox.orchestrator.privacy_routing.enabled"),
            cache_aware_routing: g("vox.orchestrator.cache_aware_routing.enabled"),
            compaction_5layer: g("vox.orchestrator.compaction_5layer.enabled"),
            tenant_budget: g("vox.orchestrator.tenant_budget.enabled"),
            calibration_loop: g("vox.orchestrator.calibration_loop.enabled"),
            drift_detector: g("vox.orchestrator.drift_detector.enabled"),
            contextual_bandit: g("vox.orchestrator.contextual_bandit.enabled"),
            subagent_dispatch: g("vox.orchestrator.subagent_dispatch.enabled"),
            chain_length_cap: g("vox.orchestrator.chain_length_cap.enabled"),
            agentos_aci_envelope: g("vox.orchestrator.agentos.aci_envelope.enabled"),
            agentos_guardrail_kernel: g("vox.orchestrator.agentos.guardrail_kernel.enabled"),
            agentos_checkpoint_hints: g("vox.orchestrator.agentos.checkpoint_hints.enabled"),
        })
    }

    #[must_use]
    pub fn all_disabled() -> Self {
        Self {
            circuit_breaker: false,
            socrates_fusion: false,
            tier_cascade: false,
            plan_mode_trigger: false,
            risk_matrix_hitl: false,
            privacy_routing: false,
            cache_aware_routing: false,
            compaction_5layer: false,
            tenant_budget: false,
            calibration_loop: false,
            drift_detector: false,
            contextual_bandit: false,
            subagent_dispatch: false,
            chain_length_cap: false,
            agentos_aci_envelope: false,
            agentos_guardrail_kernel: false,
            agentos_checkpoint_hints: false,
        }
    }

    #[must_use]
    pub fn all_enabled_for_testing() -> Self {
        Self {
            circuit_breaker: true,
            socrates_fusion: true,
            tier_cascade: true,
            plan_mode_trigger: true,
            risk_matrix_hitl: true,
            privacy_routing: true,
            cache_aware_routing: true,
            compaction_5layer: true,
            tenant_budget: true,
            calibration_loop: true,
            drift_detector: true,
            contextual_bandit: true,
            subagent_dispatch: true,
            chain_length_cap: true,
            agentos_aci_envelope: true,
            agentos_guardrail_kernel: true,
            agentos_checkpoint_hints: true,
        }
    }

    #[must_use]
    #[inline]
    pub fn contextual_bandit_enabled(&self) -> bool {
        self.contextual_bandit
    }
}

#[must_use]
pub fn orchestration_feature_flags_cached() -> &'static OrchestrationFeatureFlags {
    static CELL: OnceLock<OrchestrationFeatureFlags> = OnceLock::new();
    CELL.get_or_init(OrchestrationFeatureFlags::from_embedded_contract)
}
