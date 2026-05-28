use serde::{Deserialize, Serialize};
use vox_db::store::types::{ObservationReport, VictoryVerdict};

/// The VictoryEvaluator aggregates signals from multiple verification tiers
/// to determine if an agent's task implementation is "victorious" (complete and correct).
pub struct VictoryEvaluator {
    pub config: VictoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoryConfig {
    pub require_tests: bool,
    pub strict_toestub: bool,
    pub min_socrates_score: f32,
}

impl Default for VictoryConfig {
    fn default() -> Self {
        Self {
            require_tests: true,
            strict_toestub: true,
            min_socrates_score: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoryReport {
    pub is_victorious: bool,
    pub score: f32,
    pub tiers: VictoryTiers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VictoryTiers {
    pub syntactic: bool,     // Lints, Compiler checks
    pub structural: bool,    // TOESTUB (placeholder detection)
    pub behavioral: bool,    // Test failures or behavioral gates
    pub hallucination: bool, // Socrates grounding check
}

impl VictoryEvaluator {
    pub fn new(config: VictoryConfig) -> Self {
        Self { config }
    }

    /// Aggregate all verification signals into a single verdict consistent with the persistent store.
    pub fn evaluate(
        &self,
        task_id: &str,
        validation: &crate::validation::ValidationResult,
        observation: Option<&ObservationReport>,
        socrates_score: Option<f32>,
    ) -> VictoryVerdict {
        let syntactic = validation.passed && validation.error_count == 0;
        let structural = if self.config.strict_toestub {
            validation.passed && validation.warning_count == 0
        } else {
            validation.passed
        };

        // Behavioral check: use the recommended action from the observer.
        // If it's Continue or RequestMoreEvidence, we consider behavior "okay enough" for the tier.
        let behavioral = observation.is_none_or(|o| {
            matches!(
                o.recommended_action,
                vox_db::store::types::ObserverAction::Continue
                    | vox_db::store::types::ObserverAction::RequestMoreEvidence
            )
        });

        let grounding = socrates_score.is_none_or(|s| s >= self.config.min_socrates_score);

        let passed = syntactic && structural && behavioral && grounding;

        let mut tiers_run = vec!["Syntactic".to_string(), "Structural".to_string()];
        if observation.is_some() {
            tiers_run.push("Behavioral".to_string());
        }
        if socrates_score.is_some() {
            tiers_run.push("Hallucination".to_string());
        }

        let first_failure = if !syntactic {
            Some("Syntactic".to_string())
        } else if !structural {
            Some("Structural".to_string())
        } else if !behavioral {
            Some("Behavioral".to_string())
        } else if !grounding {
            Some("Hallucination".to_string())
        } else {
            None
        };

        VictoryVerdict {
            task_id: vox_db::DbTaskId::new(task_id),
            passed,
            tiers_run,
            first_failure,
            report: validation.report.clone(),
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }
}
