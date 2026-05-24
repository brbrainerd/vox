//! Routing functions over a [`ClassDefaults`] map.

use super::defaults::{ClassDefaults, FindingClass};

/// Return the recommended-venues list for `class`, or an empty slice if no
/// policy is configured.
pub fn recommended_venues_for<'a>(
    defaults: &'a ClassDefaults,
    class: FindingClass,
) -> &'a [String] {
    defaults
        .policy_for(class)
        .map(|p| p.recommended_venues.as_slice())
        .unwrap_or(&[])
}

/// Return the reply-window length (in days) for `class`. Falls back to 14
/// when no policy is configured (the rubric default).
pub fn reply_window_days_for(defaults: &ClassDefaults, class: FindingClass) -> u32 {
    defaults
        .policy_for(class)
        .map(|p| p.reply_window_days)
        .unwrap_or(14)
}

/// Return the negative-result quota for `class`. Falls back to 0
/// (micro-track default).
pub fn negative_result_quota_for(defaults: &ClassDefaults, class: FindingClass) -> u32 {
    defaults
        .policy_for(class)
        .map(|p| p.negative_result_quota)
        .unwrap_or(0)
}

/// Whether an LLM critic may substitute for the second human approver for
/// `class`. Falls back to `false` (safer default).
pub fn critic_allowed_for(defaults: &ClassDefaults, class: FindingClass) -> bool {
    defaults
        .policy_for(class)
        .map(|p| p.critic_allowed)
        .unwrap_or(false)
}

/// Phase E §4: the Atlas-specific submission gate (negative-result quota,
/// 14-day window) applies to a class iff the class is an Atlas class.
pub fn atlas_gate_applies_to(class: FindingClass) -> bool {
    class.is_atlas()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::defaults::builtin_class_defaults;

    #[test]
    fn algorithmic_improvement_routes_to_swe_venues() {
        let d = builtin_class_defaults();
        let v = recommended_venues_for(&d, FindingClass::AlgorithmicImprovement);
        assert!(v.contains(&"ICSE".to_string()));
        assert!(v.contains(&"FSE".to_string()));
        assert!(!v.iter().any(|n| n.contains("IMC")), "SWE class must not route to IMC");
    }

    #[test]
    fn atlas_classes_route_to_imc_or_mlsys() {
        let d = builtin_class_defaults();
        for atlas in [
            FindingClass::ModelCapabilityAtlas,
            FindingClass::ProviderReliabilityAtlas,
        ] {
            let v = recommended_venues_for(&d, atlas);
            assert!(v.iter().any(|n| n == "IMC" || n == "MLSys"));
        }
    }

    #[test]
    fn atlas_gate_predicate_separates_atlas_from_micro() {
        assert!(atlas_gate_applies_to(FindingClass::ModelCapabilityAtlas));
        assert!(atlas_gate_applies_to(FindingClass::ProviderReliabilityAtlas));
        for c in [
            FindingClass::AlgorithmicImprovement,
            FindingClass::ReproducibilityInfra,
            FindingClass::PolicyGovernance,
            FindingClass::TelemetryTrust,
            FindingClass::Other,
        ] {
            assert!(
                !atlas_gate_applies_to(c),
                "Atlas gate must not apply to {:?}",
                c
            );
        }
    }

    #[test]
    fn reply_window_for_swe_is_shorter_than_for_atlas() {
        let d = builtin_class_defaults();
        let swe = reply_window_days_for(&d, FindingClass::AlgorithmicImprovement);
        let atlas = reply_window_days_for(&d, FindingClass::ModelCapabilityAtlas);
        assert!(swe < atlas, "swe={swe}, atlas={atlas}");
    }

    #[test]
    fn unknown_class_in_user_yaml_falls_back_to_defaults() {
        // Empty defaults map — every lookup returns the safe fallback.
        let empty = super::defaults::ClassDefaults {
            by_class: Default::default(),
        };
        assert_eq!(
            reply_window_days_for(&empty, FindingClass::AlgorithmicImprovement),
            14
        );
        assert_eq!(
            negative_result_quota_for(&empty, FindingClass::AlgorithmicImprovement),
            0
        );
        assert!(!critic_allowed_for(&empty, FindingClass::AlgorithmicImprovement));
        assert!(recommended_venues_for(&empty, FindingClass::AlgorithmicImprovement).is_empty());
    }
}
