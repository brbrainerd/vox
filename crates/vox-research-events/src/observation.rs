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
