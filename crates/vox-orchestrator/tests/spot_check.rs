// P5-T5: spot-check sampler tests.
use vox_orchestrator::spot_check::SpotCheckSampler;

/// Generate a unique task_id string for use in sampling tests.
fn task_id(n: usize) -> String {
    format!("task-{n:08x}")
}

#[test]
fn prob_zero_never_checks() {
    let sampler = SpotCheckSampler::new(0.0);
    for i in 0..1000 {
        assert!(
            !sampler.should_check(&task_id(i)),
            "prob=0 must never check"
        );
    }
}

#[test]
fn prob_one_always_checks() {
    let sampler = SpotCheckSampler::new(1.0);
    for i in 0..1000 {
        assert!(
            sampler.should_check(&task_id(i)),
            "prob=1 must always check"
        );
    }
}

#[test]
fn prob_five_pct_within_tolerance_over_ten_thousand_samples() {
    let sampler = SpotCheckSampler::new(0.05);
    let n = 10_000;
    let checked = (0..n)
        .filter(|i| sampler.should_check(&task_id(*i)))
        .count();
    let ratio = checked as f64 / n as f64;
    // Expect 5% ± 2%
    assert!(
        (0.03..=0.07).contains(&ratio),
        "5% sampler produced {:.2}% over {n} samples (expected 3-7%)",
        ratio * 100.0
    );
}

#[test]
fn sampling_is_deterministic() {
    let sampler = SpotCheckSampler::new(0.1);
    let id = "deterministic-test-task";
    let first = sampler.should_check(id);
    for _ in 0..100 {
        assert_eq!(
            sampler.should_check(id),
            first,
            "same input must always yield same decision"
        );
    }
}

#[test]
fn prob_half_approximately_balanced() {
    let sampler = SpotCheckSampler::new(0.5);
    let n = 10_000;
    let checked = (0..n)
        .filter(|i| sampler.should_check(&task_id(*i)))
        .count();
    let ratio = checked as f64 / n as f64;
    // Expect 50% ± 5%
    assert!(
        (0.45..=0.55).contains(&ratio),
        "50% sampler produced {:.2}% over {n} samples",
        ratio * 100.0
    );
}
