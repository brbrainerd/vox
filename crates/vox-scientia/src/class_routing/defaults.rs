//! Per-class defaults file shape + loader + built-in defaults.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Closed enum of candidate classes the micro-track config knows about.
/// Mirrors the values in `FindingCandidateClass` in
/// `vox-research-events` and the schema in
/// `contracts/scientia/finding-candidate.v1.schema.json`, plus the
/// Atlas-specific classes from Finalization Phase 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingClass {
    AlgorithmicImprovement,
    ReproducibilityInfra,
    PolicyGovernance,
    TelemetryTrust,
    Other,
    /// Atlas track — provider behavior epidemiology.
    ModelCapabilityAtlas,
    /// Atlas track — provider reliability epidemiology.
    ProviderReliabilityAtlas,
}

impl FindingClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AlgorithmicImprovement => "algorithmic_improvement",
            Self::ReproducibilityInfra => "reproducibility_infra",
            Self::PolicyGovernance => "policy_governance",
            Self::TelemetryTrust => "telemetry_trust",
            Self::Other => "other",
            Self::ModelCapabilityAtlas => "model_capability_atlas",
            Self::ProviderReliabilityAtlas => "provider_reliability_atlas",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "algorithmic_improvement" => Some(Self::AlgorithmicImprovement),
            "reproducibility_infra" => Some(Self::ReproducibilityInfra),
            "policy_governance" => Some(Self::PolicyGovernance),
            "telemetry_trust" => Some(Self::TelemetryTrust),
            "other" => Some(Self::Other),
            "model_capability_atlas" => Some(Self::ModelCapabilityAtlas),
            "provider_reliability_atlas" => Some(Self::ProviderReliabilityAtlas),
            _ => None,
        }
    }

    /// Atlas-class predicate; drives [`crate::routing::atlas_gate_applies_to`].
    pub fn is_atlas(&self) -> bool {
        matches!(self, Self::ModelCapabilityAtlas | Self::ProviderReliabilityAtlas)
    }
}

/// One row of `finding-class-defaults.v1.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassPolicy {
    /// How many days the right-of-reply window stays open before the
    /// manifest can ship live.
    pub reply_window_days: u32,
    /// Minimum number of null-result publications required per N positive
    /// findings in the same quarterly window. `0` means the negative-result
    /// mandate (Finalization Phase 10) does NOT apply to this class.
    pub negative_result_quota: u32,
    /// Whether the venue catalog defaults to allowing an audited LLM critic
    /// to substitute for the second human approver (Phase D). Per-venue
    /// rows in `venue-catalog.v1.yaml` can override.
    pub critic_allowed: bool,
    /// Ordered list of venues to recommend for this class, primary first.
    /// Empty → no recommendation (caller falls back to a manual choice).
    #[serde(default)]
    pub recommended_venues: Vec<String>,
}

/// Top-level shape of `finding-class-defaults.v1.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDefaults {
    #[serde(flatten)]
    pub by_class: HashMap<String, ClassPolicy>,
}

impl ClassDefaults {
    pub fn policy_for(&self, class: FindingClass) -> Option<&ClassPolicy> {
        self.by_class.get(class.as_str())
    }
}

#[derive(Debug, Error)]
pub enum ClassRoutingError {
    #[error("yaml parse: {0}")]
    Yaml(String),
}

/// Load class defaults from YAML matching the shape of
/// `contracts/scientia/finding-class-defaults.v1.yaml`.
pub fn load_class_defaults_from_yaml(yaml: &str) -> Result<ClassDefaults, ClassRoutingError> {
    let by_class: HashMap<String, ClassPolicy> =
        serde_yaml::from_str(yaml).map_err(|e| ClassRoutingError::Yaml(e.to_string()))?;
    Ok(ClassDefaults { by_class })
}

/// Built-in defaults — what the system uses when no YAML is supplied.
/// These match the design-doc recommendations.
pub fn builtin_class_defaults() -> ClassDefaults {
    let mut by_class = HashMap::new();
    by_class.insert(
        FindingClass::AlgorithmicImprovement.as_str().to_string(),
        ClassPolicy {
            reply_window_days: 7,
            negative_result_quota: 0,
            critic_allowed: true,
            recommended_venues: vec![
                "ICSE".into(),
                "FSE".into(),
                "OOPSLA".into(),
                "PLDI".into(),
            ],
        },
    );
    by_class.insert(
        FindingClass::ReproducibilityInfra.as_str().to_string(),
        ClassPolicy {
            reply_window_days: 7,
            negative_result_quota: 0,
            critic_allowed: true,
            recommended_venues: vec!["REP".into(), "MSR".into(), "ICSE-SEIP".into()],
        },
    );
    by_class.insert(
        FindingClass::TelemetryTrust.as_str().to_string(),
        ClassPolicy {
            reply_window_days: 14,
            negative_result_quota: 0,
            // Provider-behavior implications → human approver required.
            critic_allowed: false,
            recommended_venues: vec![
                "MLSys-workshop".into(),
                "SOSP-workshop".into(),
                "USENIX-ATC".into(),
            ],
        },
    );
    by_class.insert(
        FindingClass::PolicyGovernance.as_str().to_string(),
        ClassPolicy {
            reply_window_days: 14,
            negative_result_quota: 0,
            critic_allowed: false,
            recommended_venues: vec!["AIES".into(), "FAccT".into()],
        },
    );
    by_class.insert(
        FindingClass::Other.as_str().to_string(),
        ClassPolicy {
            reply_window_days: 14,
            negative_result_quota: 0,
            critic_allowed: false,
            recommended_venues: vec![],
        },
    );
    // Atlas classes: unchanged Atlas behavior (14-day window, 3:1 negative
    // quota, critic forbidden).
    for atlas in [
        FindingClass::ModelCapabilityAtlas,
        FindingClass::ProviderReliabilityAtlas,
    ] {
        by_class.insert(
            atlas.as_str().to_string(),
            ClassPolicy {
                reply_window_days: 14,
                negative_result_quota: 3,
                critic_allowed: false,
                recommended_venues: vec!["IMC".into(), "MLSys".into()],
            },
        );
    }
    ClassDefaults { by_class }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finding_class_round_trips_through_string() {
        for c in [
            FindingClass::AlgorithmicImprovement,
            FindingClass::ReproducibilityInfra,
            FindingClass::PolicyGovernance,
            FindingClass::TelemetryTrust,
            FindingClass::Other,
            FindingClass::ModelCapabilityAtlas,
            FindingClass::ProviderReliabilityAtlas,
        ] {
            assert_eq!(FindingClass::from_str(c.as_str()), Some(c));
        }
        assert_eq!(FindingClass::from_str("nope"), None);
    }

    #[test]
    fn atlas_predicate_matches_only_atlas_classes() {
        assert!(FindingClass::ModelCapabilityAtlas.is_atlas());
        assert!(FindingClass::ProviderReliabilityAtlas.is_atlas());
        assert!(!FindingClass::AlgorithmicImprovement.is_atlas());
        assert!(!FindingClass::TelemetryTrust.is_atlas());
    }

    #[test]
    fn builtin_defaults_cover_all_classes() {
        let d = builtin_class_defaults();
        for c in [
            FindingClass::AlgorithmicImprovement,
            FindingClass::ReproducibilityInfra,
            FindingClass::PolicyGovernance,
            FindingClass::TelemetryTrust,
            FindingClass::Other,
            FindingClass::ModelCapabilityAtlas,
            FindingClass::ProviderReliabilityAtlas,
        ] {
            assert!(d.policy_for(c).is_some(), "missing policy for {:?}", c);
        }
    }

    #[test]
    fn builtin_micro_classes_have_zero_negative_result_quota() {
        let d = builtin_class_defaults();
        for c in [
            FindingClass::AlgorithmicImprovement,
            FindingClass::ReproducibilityInfra,
            FindingClass::PolicyGovernance,
            FindingClass::TelemetryTrust,
        ] {
            assert_eq!(d.policy_for(c).unwrap().negative_result_quota, 0);
        }
    }

    #[test]
    fn builtin_atlas_classes_keep_3to1_negative_quota() {
        let d = builtin_class_defaults();
        for atlas in [
            FindingClass::ModelCapabilityAtlas,
            FindingClass::ProviderReliabilityAtlas,
        ] {
            assert_eq!(d.policy_for(atlas).unwrap().negative_result_quota, 3);
        }
    }

    #[test]
    fn yaml_loader_round_trip() {
        let y = r#"
algorithmic_improvement:
  reply_window_days: 7
  negative_result_quota: 0
  critic_allowed: true
  recommended_venues: [ICSE, FSE]
telemetry_trust:
  reply_window_days: 14
  negative_result_quota: 0
  critic_allowed: false
  recommended_venues: [MLSys-workshop]
"#;
        let d = load_class_defaults_from_yaml(y).unwrap();
        let algimp = d.policy_for(FindingClass::AlgorithmicImprovement).unwrap();
        assert_eq!(algimp.reply_window_days, 7);
        assert!(algimp.critic_allowed);
        assert_eq!(algimp.recommended_venues, vec!["ICSE", "FSE"]);
        let teltr = d.policy_for(FindingClass::TelemetryTrust).unwrap();
        assert!(!teltr.critic_allowed);
    }

    #[test]
    fn malformed_yaml_yields_error() {
        let res = load_class_defaults_from_yaml("not: valid: yaml: at: all");
        assert!(matches!(res, Err(ClassRoutingError::Yaml(_))));
    }

    #[test]
    fn micro_track_default_critic_allowed_matrix() {
        let d = builtin_class_defaults();
        // SWE / repro: critic allowed.
        assert!(d.policy_for(FindingClass::AlgorithmicImprovement).unwrap().critic_allowed);
        assert!(d.policy_for(FindingClass::ReproducibilityInfra).unwrap().critic_allowed);
        // Provider-implication classes: critic NOT allowed by default.
        assert!(!d.policy_for(FindingClass::TelemetryTrust).unwrap().critic_allowed);
        assert!(!d.policy_for(FindingClass::PolicyGovernance).unwrap().critic_allowed);
    }
}
