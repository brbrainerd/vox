//! Analysis-plan-deviation detector.
//!
//! Compares the metric name and statistical test kind declared in a signed
//! [`PreregistrationV1`] against what was actually used in the campaign run.
//! Any mismatch is collected into a [`DeviationReport`], which the publication
//! pipeline stamps onto the output artifact as `analysis_plan_deviation: true`.
//!
//! Per SCIENTIA plan §5.3: "Pre-register the **analysis tree**, not just the
//! hypothesis. The system records prereg signature + analysis-code commit hash;
//! any deviation surfaces as `analysis_plan_deviation: true`."

use vox_research_events::preregistration::{PreregistrationV1, StatisticalTest};

/// Report of deviations between a signed prereg and an actual run.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviationReport {
    /// True if the actual metric name matches `prereg.metric.name`.
    pub metric_matches: bool,
    /// True if the actual test kind matches `prereg.statistical_test.kind`.
    pub test_matches: bool,
    /// True if both `metric_matches` and `test_matches` are true (no deviations).
    pub is_clean: bool,
    /// Human-readable descriptions of each deviation found.
    pub deviations: Vec<String>,
}

/// Detects analysis-plan deviations between a signed prereg and an actual campaign run.
#[derive(Debug, Default, Clone)]
pub struct DeviationDetector;

impl DeviationDetector {
    pub fn new() -> Self {
        Self
    }

    /// Check `actual_metric` and `actual_test` against the values declared in `prereg`.
    ///
    /// Returns a [`DeviationReport`] with `is_clean = true` iff both match exactly.
    pub fn check(
        &self,
        prereg: &PreregistrationV1,
        actual_metric: &str,
        actual_test: &StatisticalTest,
    ) -> DeviationReport {
        let mut deviations = Vec::new();

        let metric_matches = prereg.metric.name == actual_metric;
        if !metric_matches {
            deviations.push(format!(
                "metric deviation: prereg declared '{}', actual run used '{}'",
                prereg.metric.name, actual_metric
            ));
        }

        let test_matches = test_kind_eq(&prereg.statistical_test.kind, actual_test);
        if !test_matches {
            deviations.push(format!(
                "test kind deviation: prereg declared '{:?}', actual run used '{:?}'",
                prereg.statistical_test.kind, actual_test
            ));
        }

        let is_clean = metric_matches && test_matches;
        DeviationReport {
            metric_matches,
            test_matches,
            is_clean,
            deviations,
        }
    }
}

/// Compare two [`StatisticalTest`] variants for equality by discriminant.
fn test_kind_eq(a: &StatisticalTest, b: &StatisticalTest) -> bool {
    matches!(
        (a, b),
        (StatisticalTest::Frequentist, StatisticalTest::Frequentist)
            | (StatisticalTest::Bayesian, StatisticalTest::Bayesian)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn prereg_with(metric_name: &str, test_kind: StatisticalTest) -> PreregistrationV1 {
        PreregistrationV1 {
            id: "RA_test".to_string(),
            hypothesis: "test hypothesis".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:000".to_string(),
                eval_set_swhid: "swh:1:dir:000".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: metric_name.to_string(),
                aggregation: "mean".to_string(),
                units: "ms".to_string(),
            },
            statistical_test: TestSpec {
                kind: test_kind,
                prior: None,
                threshold: None,
                alpha: Some(0.05),
            },
            stopping_rule: StopRule {
                max_n: 100,
                alpha: Some(0.05),
                threshold: None,
            },
            decision_rule: DecisionRule {
                description: "reject if p < alpha".to_string(),
            },
            cost_cap_usd: 10.0,
            signed_at: 1715299200,
            signing_key: "aa".repeat(32),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn clean_run_no_deviations() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(report.is_clean, "identical metric and test should be clean");
        assert!(report.deviations.is_empty());
        assert!(report.metric_matches);
        assert!(report.test_matches);
    }

    #[test]
    fn metric_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Frequentist);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p99_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("metric")));
    }

    #[test]
    fn test_kind_mismatch_detected() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "p95_latency_ms", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(report.metric_matches);
        assert!(!report.test_matches);
        assert!(report.deviations.iter().any(|d| d.contains("test")));
    }

    #[test]
    fn both_mismatches_reported() {
        let prereg = prereg_with("p95_latency_ms", StatisticalTest::Bayesian);
        let detector = DeviationDetector::new();
        let report = detector.check(&prereg, "refusal_rate_pct", &StatisticalTest::Frequentist);
        assert!(!report.is_clean);
        assert!(!report.metric_matches);
        assert!(!report.test_matches);
        assert_eq!(report.deviations.len(), 2);
    }
}
