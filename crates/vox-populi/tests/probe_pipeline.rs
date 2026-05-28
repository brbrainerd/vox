// Integration tests for the hardware probe pipeline.
// These run against the real ProbePipeline public API (no mock access).
// On CI machines without GPU hardware, all platform probes return NoDevice and
// the pipeline falls back to "Host CPU".

use std::time::Duration;
use vox_populi::mens::hardware::pipeline::ProbePipeline;
use vox_populi::mens::hardware::probe::ProbeOutcome;
use vox_populi::mens::hardware::registry::HardwareRegistryV2;

#[tokio::test]
async fn default_pipeline_returns_non_empty_model_name() {
    let report = ProbePipeline::default_for_platform().run().await;
    assert!(
        !report.summary.model_name.is_empty(),
        "model_name must not be empty; got an empty string"
    );
}

#[tokio::test]
async fn default_pipeline_attempts_vector_is_non_empty() {
    let report = ProbePipeline::default_for_platform().run().await;
    assert!(
        !report.attempts.is_empty(),
        "expected at least one probe attempt in the default pipeline"
    );
}

#[tokio::test]
async fn default_pipeline_no_panics_on_multiple_runs() {
    for _ in 0..3 {
        let report = ProbePipeline::default_for_platform().run().await;
        assert!(!report.summary.model_name.is_empty());
    }
}

#[tokio::test]
async fn all_attempts_have_probe_names() {
    let report = ProbePipeline::default_for_platform().run().await;
    for attempt in &report.attempts {
        assert!(
            !attempt.probe_name.is_empty(),
            "every attempt must have a non-empty probe_name"
        );
    }
}

#[tokio::test]
async fn summary_probe_failures_not_empty_vec() {
    let report = ProbePipeline::default_for_platform().run().await;
    if let Some(ref failures) = report.summary.probe_failures {
        assert!(
            !failures.is_empty(),
            "probe_failures must be None or a non-empty vec; got Some([])"
        );
    }
}

#[tokio::test]
async fn empty_pipeline_produces_cpu_fallback() {
    let report = ProbePipeline::empty().run().await;
    assert_eq!(report.summary.model_name, "Host CPU");
    assert_eq!(report.attempts.len(), 0);
    assert!(report.summary.probe_failures.is_none());
}

#[tokio::test]
async fn registry_probe_returns_non_empty_model_name() {
    let registry = HardwareRegistryV2::new(Duration::from_secs(300));
    let summary = registry.probe().await;
    assert!(!summary.model_name.is_empty());
}

#[tokio::test]
async fn hardware_registry_probe_compat_wrapper_works() {
    let summary = vox_populi::mens::hardware::probe().await;
    assert!(!summary.model_name.is_empty());
}

#[tokio::test]
async fn probe_with_report_compat_wrapper_works() {
    let report = vox_populi::mens::hardware::probe_with_report().await;
    assert!(!report.summary.model_name.is_empty());
}

#[tokio::test]
async fn reorder_is_stable_on_default_pipeline() {
    let report = ProbePipeline::default_for_platform()
        .reorder(&[])
        .run()
        .await;
    assert!(!report.summary.model_name.is_empty());
}

/// Unknown probe names in an operator-supplied order list must be rejected.
#[test]
fn operator_override_rejects_unknown_probe_name() {
    let pipeline = ProbePipeline::default_for_platform();
    let known_names = pipeline.probe_names();
    // All known names should pass validation.
    let known_refs: Vec<&str> = known_names.to_vec();
    assert_eq!(pipeline.validate_probe_names(&known_refs), Ok(()));

    // Inject a typo — validation must reject it.
    let bad = pipeline.validate_probe_names(&["nvvml_typo"]);
    assert!(
        bad.is_err(),
        "expected Err for unknown probe name 'nvvml_typo'"
    );
    assert_eq!(bad.unwrap_err(), vec!["nvvml_typo".to_string()]);
}

/// Verify that `NotApplicable` attempts have `duration_ms == 0`.
#[tokio::test]
async fn not_applicable_attempts_have_zero_duration() {
    let report = ProbePipeline::default_for_platform().run().await;
    for attempt in &report.attempts {
        if matches!(attempt.outcome, ProbeOutcome::NotApplicable) {
            assert_eq!(
                attempt.duration_ms, 0,
                "NotApplicable probe '{}' should have duration_ms == 0",
                attempt.probe_name
            );
        }
    }
}
