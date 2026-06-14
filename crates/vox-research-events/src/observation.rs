use serde::{Deserialize, Serialize};

/// A learned model behavior profile row (Mesh §5.5 / Phase 0d scientia_model_profile_learning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedProfileRow {
    pub provider: String,
    pub model_id: String,
    pub profile_key: String,
    pub profile_value: f64,
    pub sample_count: u64,
    pub last_updated_ms: i64,
}

/// Classification result from the ScientiaObservationClassifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObservationClass {
    ProviderObservation,
    ModelCapabilityEvidence,
    Other,
}

/// Trait for classifying a raw telemetry observation into a SCIENTIA signal class.
pub trait ScientiaObservationClassifier: Send + Sync {
    fn classify(&self, observation_text: &str, metadata: &serde_json::Value) -> ObservationClass;
}

/// Heuristic keyword-based classifier (default implementation).
#[derive(Debug, Default, Clone)]
pub struct KeywordObservationClassifier;

impl ScientiaObservationClassifier for KeywordObservationClassifier {
    fn classify(&self, observation_text: &str, _metadata: &serde_json::Value) -> ObservationClass {
        let lower = observation_text.to_ascii_lowercase();
        if lower.contains("latency")
            || lower.contains("reliability")
            || lower.contains("uptime")
            || lower.contains("refusal")
        {
            ObservationClass::ProviderObservation
        } else if lower.contains("capability")
            || lower.contains("benchmark")
            || lower.contains("accuracy")
            || lower.contains("eval")
        {
            ObservationClass::ModelCapabilityEvidence
        } else {
            ObservationClass::Other
        }
    }
}

/// Extended scoring weights with SCIENTIA signal bonus (Mesh §5.4).
/// Behind feature flag — only affects routing when `scientia_weights_enabled = true`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScientiaWeightExtension {
    pub provider_observation_bonus: f64, // added to fusion score for ProviderObservation signals
    pub capability_evidence_bonus: f64,
    pub scientia_weights_enabled: bool, // default false
}

impl Default for ScientiaWeightExtension {
    fn default() -> Self {
        Self {
            provider_observation_bonus: 0.05,
            capability_evidence_bonus: 0.03,
            scientia_weights_enabled: false, // OFF by default
        }
    }
}

/// A penalty record with context (Mesh §5.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PenaltyRecord {
    pub provider: String,
    pub model_id: String,
    pub penalty_score: f64,
    pub context: String,
    pub recorded_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_identifies_latency_as_provider_observation() {
        let c = KeywordObservationClassifier;
        assert_eq!(
            c.classify("p95 latency increased by 15ms", &serde_json::Value::Null),
            ObservationClass::ProviderObservation
        );
    }

    #[test]
    fn classifier_identifies_eval_as_capability_evidence() {
        let c = KeywordObservationClassifier;
        assert_eq!(
            c.classify("MMLU accuracy improved to 92%", &serde_json::Value::Null),
            ObservationClass::ModelCapabilityEvidence
        );
    }

    #[test]
    fn scoring_weights_default_off() {
        let w = ScientiaWeightExtension::default();
        assert!(!w.scientia_weights_enabled);
    }

    #[test]
    fn learned_profile_row_round_trips() {
        let row = LearnedProfileRow {
            provider: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            profile_key: "p95_latency_ms_mean".to_string(),
            profile_value: 312.5,
            sample_count: 1000,
            last_updated_ms: 1_715_299_200_000,
        };
        let json = serde_json::to_string(&row).unwrap();
        let back: LearnedProfileRow = serde_json::from_str(&json).unwrap();
        assert_eq!(back.provider, "openai");
        assert_eq!(back.sample_count, 1000);
    }
}

#[cfg(test)]
mod semcov_wave6_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: classifier returns ProviderObservation for a text that only matches
    // a capability keyword because the provider branch is checked first — or vice
    // versa.  If the branch order is swapped the wrong class is returned.
    #[test]
    fn classifier_provider_keywords_take_precedence_over_capability_when_both_present() {
        let c = KeywordObservationClassifier;
        // "latency" is a provider keyword; "benchmark" is capability — provider wins
        let result = c.classify("benchmark showed high latency", &serde_json::Value::Null);
        assert_eq!(
            result,
            ObservationClass::ProviderObservation,
            "latency (provider keyword) must dominate benchmark (capability keyword)"
        );
    }

    // Catches: case-folding bug — classifier compares lowercase text against
    // lowercase literals, so an ALL-CAPS keyword should still match.  A bug
    // where `to_ascii_lowercase()` is missing would return Other here.
    #[test]
    fn classifier_matches_keywords_case_insensitively() {
        let c = KeywordObservationClassifier;
        assert_eq!(
            c.classify("UPTIME was degraded", &serde_json::Value::Null),
            ObservationClass::ProviderObservation,
            "UPTIME in upper-case must still match the provider branch"
        );
        assert_eq!(
            c.classify("ACCURACY improved", &serde_json::Value::Null),
            ObservationClass::ModelCapabilityEvidence,
            "ACCURACY in upper-case must still match the capability branch"
        );
    }

    // Catches: classifier incorrectly returning ProviderObservation or
    // ModelCapabilityEvidence for text with no matching keyword (e.g., a
    // prefix/suffix off-by-one in contains() or a wildcard match).
    #[test]
    fn classifier_returns_other_for_unrelated_text() {
        let c = KeywordObservationClassifier;
        assert_eq!(
            c.classify("the weather today is sunny", &serde_json::Value::Null),
            ObservationClass::Other,
            "text with no keyword must return Other"
        );
    }

    // Catches: empty-string edge case — a bug where `contains("")` always
    // returns true (it does in Rust) would make an empty string match the
    // first branch; the actual keywords are non-empty so empty must be Other.
    #[test]
    fn classifier_empty_observation_is_other() {
        let c = KeywordObservationClassifier;
        assert_eq!(
            c.classify("", &serde_json::Value::Null),
            ObservationClass::Other,
            "empty observation must classify as Other, not match a keyword"
        );
    }

    // Catches: "eval" substring accidentally matching unrelated words such as
    // "evaluate" or "medieval" — confirms the substring match is intentional
    // and that "eval" inside a longer word still triggers the capability branch
    // (documents current behavior so a future regex change doesn't silently drop it).
    #[test]
    fn classifier_eval_as_substring_triggers_capability_evidence() {
        let c = KeywordObservationClassifier;
        // "evaluate" contains "eval" — current impl uses contains() so this matches
        let result = c.classify("evaluate the model", &serde_json::Value::Null);
        assert_eq!(
            result,
            ObservationClass::ModelCapabilityEvidence,
            "substring 'eval' inside 'evaluate' must trigger ModelCapabilityEvidence (contains semantics)"
        );
    }

    // Catches: ScientiaWeightExtension default having wrong bonus values — e.g.,
    // accidentally swapping provider_observation_bonus and capability_evidence_bonus.
    #[test]
    fn scientia_weight_extension_default_bonus_ordering() {
        let w = ScientiaWeightExtension::default();
        assert!(
            w.provider_observation_bonus > w.capability_evidence_bonus,
            "provider_observation_bonus ({}) should exceed capability_evidence_bonus ({})",
            w.provider_observation_bonus,
            w.capability_evidence_bonus
        );
        // Exact values — catches accidental changes to the constants.
        assert!(
            (w.provider_observation_bonus - 0.05).abs() < 1e-9,
            "provider_observation_bonus must be 0.05"
        );
        assert!(
            (w.capability_evidence_bonus - 0.03).abs() < 1e-9,
            "capability_evidence_bonus must be 0.03"
        );
    }

    // Catches: LearnedProfileRow with NaN profile_value appearing to serialize
    // successfully (no panic) but producing broken JSON that cannot round-trip —
    // serde_json encodes NaN as `null`, which then fails deserialization back to
    // `f64` (null is not a float). This documents the one-way corruption trap so
    // callers know they must validate profile_value before construction.
    #[test]
    fn learned_profile_row_nan_serializes_as_null_and_fails_deserialization() {
        let row = LearnedProfileRow {
            provider: "test".into(),
            model_id: "m".into(),
            profile_key: "k".into(),
            profile_value: f64::NAN,
            sample_count: 5,
            last_updated_ms: 0,
        };
        // serde_json serializes NaN as JSON `null` — no error at this step.
        let json = serde_json::to_string(&row)
            .expect("serde_json must not error on NaN (it encodes as null)");
        assert!(
            json.contains("null"),
            "NaN profile_value must be encoded as null in JSON; got: {json}"
        );
        // Deserialization from `null` to `f64` MUST fail because the field is
        // non-optional — this is the observable bug: silent lossy serialization
        // followed by a hard error on the read path.
        let result: Result<LearnedProfileRow, _> = serde_json::from_str(&json);
        assert!(
            result.is_err(),
            "deserializing null back to f64 must fail; round-trip is broken for NaN values"
        );
    }
}
