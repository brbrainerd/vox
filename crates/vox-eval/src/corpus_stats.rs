//! Paired significance testing for benchmark comparisons.
//!
//! Every model attempts the SAME fixtures, so comparisons are paired binary
//! outcomes and the correct test is McNemar's — exact, since discordant pairs
//! are few at this corpus size. An earlier design declared significance iff
//! two Wilson intervals failed to overlap, which fires at an effective alpha
//! near 0.005 (Cumming 2009) and has roughly half the efficiency of the
//! correct test (Schenker & Gentleman 2001), i.e. systematic false negatives.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §C4, §C5.

use serde::{Deserialize, Serialize};

pub const Z_95: f64 = 1.959_963_984_540_054;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    pub point: f64,
    pub low: f64,
    pub high: f64,
}

/// Result of a paired comparison between two systems on the same problems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedResult {
    /// Problems where A passed and B failed.
    pub b_only: usize,
    /// Problems where B passed and A failed.
    pub c_only: usize,
    pub p_value: f64,
    /// A's rate minus B's rate.
    pub difference: f64,
}

fn log_comb(n: usize, k: usize) -> f64 {
    (0..k)
        .map(|i| ((n - i) as f64).ln() - ((i + 1) as f64).ln())
        .sum()
}

/// Two-sided exact McNemar p-value over discordant counts.
///
/// Exact rather than chi-square because discordant pairs are few at this
/// corpus size (the chi-square approximation needs b + c >= 25).
#[must_use]
pub fn mcnemar_exact_p(b: usize, c: usize) -> f64 {
    let nd = b + c;
    if nd == 0 {
        return 1.0;
    }
    let lo = b.min(c);
    let tail: f64 = (0..=lo)
        .map(|i| (log_comb(nd, i) - (nd as f64) * 2f64.ln()).exp())
        .sum();
    (2.0 * tail).min(1.0)
}

/// Compare two systems on identical problems (paired binary outcomes).
#[must_use]
pub fn paired_compare(a: &[bool], b: &[bool]) -> PairedResult {
    assert_eq!(
        a.len(),
        b.len(),
        "paired comparison requires identical problem sets"
    );
    let b_only = a.iter().zip(b).filter(|(x, y)| **x && !**y).count();
    let c_only = a.iter().zip(b).filter(|(x, y)| !**x && **y).count();
    let rate = |v: &[bool]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().filter(|x| **x).count() as f64 / v.len() as f64
        }
    };
    PairedResult {
        b_only,
        c_only,
        p_value: mcnemar_exact_p(b_only, c_only),
        difference: rate(a) - rate(b),
    }
}

/// Holm-Bonferroni step-down. Returns per-input rejection flags in input order.
///
/// Controls family-wise error across the m(m-1)/2 pairwise claims published in
/// one run; uniformly more powerful than plain Bonferroni.
#[must_use]
pub fn holm_reject(p_values: &[f64], alpha: f64) -> Vec<bool> {
    let m = p_values.len();
    let mut idx: Vec<usize> = (0..m).collect();
    idx.sort_by(|&i, &j| {
        p_values[i]
            .partial_cmp(&p_values[j])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = vec![false; m];
    for (rank, &i) in idx.iter().enumerate() {
        if p_values[i] <= alpha / (m - rank) as f64 {
            out[i] = true;
        } else {
            break;
        }
    }
    out
}

/// Percentile bootstrap CI for a mean over problems (cluster-resampled).
///
/// Correct for pass@k, which is a mean of per-problem estimates rather than a
/// binomial proportion — Wilson does not apply there. `seed` is a simple
/// xorshift state, not cryptographic; deterministic across runs for a given
/// seed, which is all reproducibility here requires.
#[must_use]
pub fn bootstrap_ci(per_problem: &[f64], reps: usize, seed: u64) -> (f64, f64) {
    if per_problem.is_empty() {
        return (0.0, 1.0);
    }
    let n = per_problem.len();
    let mut state = seed | 1;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut means: Vec<f64> = (0..reps)
        .map(|_| {
            (0..n)
                .map(|_| per_problem[(next() % n as u64) as usize])
                .sum::<f64>()
                / n as f64
        })
        .collect();
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = ((0.025 * reps as f64) as usize).min(reps - 1);
    let hi = ((0.975 * reps as f64) as usize).min(reps - 1);
    (means[lo], means[hi])
}

/// Wilson score interval. Valid ONLY for single-attempt pass@1 (a genuine
/// binomial proportion). Use `bootstrap_ci` for pass@k.
#[must_use]
pub fn wilson_interval(successes: usize, trials: usize, z: f64) -> ConfidenceInterval {
    if trials == 0 {
        return ConfidenceInterval {
            point: 0.0,
            low: 0.0,
            high: 1.0,
        };
    }
    let (n, p) = (trials as f64, successes as f64 / trials as f64);
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let centre = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n) + (z2 / (4.0 * n * n))).sqrt();
    ConfidenceInterval {
        point: p,
        low: ((centre - margin) / denom).max(0.0),
        high: ((centre + margin) / denom).min(1.0),
    }
}

/// True when two intervals overlap. NOT a significance test on its own (see
/// module doc) — kept only as a display hint for "visually close", never as
/// input to a publish/no-publish decision.
#[must_use]
pub fn intervals_overlap(a: &ConfidenceInterval, b: &ConfidenceInterval) -> bool {
    a.low <= b.high && b.low <= a.high
}

/// Smallest difference resolvable at ~80% power with `n` problems.
///
/// From exact McNemar enumeration (2026-09-01): n=31 detects a 10-point
/// difference only ~9% of the time and needs ~25-30 points for 80% power;
/// n=164 resolves ~10 points. Published on the leaderboard so readers can see
/// which rows are genuinely tied.
#[must_use]
pub fn min_detectable_difference(n: usize) -> f64 {
    match n {
        0..=40 => 0.25,
        41..=80 => 0.18,
        81..=130 => 0.13,
        131..=250 => 0.10,
        251..=450 => 0.07,
        _ => 0.05,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcnemar_is_symmetric_and_bounded() {
        assert!(
            (mcnemar_exact_p(5, 5) - 1.0).abs() < 1e-9,
            "no asymmetry -> p=1"
        );
        assert_eq!(mcnemar_exact_p(0, 0), 1.0, "no discordant pairs -> p=1");
        assert!(
            (mcnemar_exact_p(8, 1) - mcnemar_exact_p(1, 8)).abs() < 1e-12,
            "symmetric"
        );
        for (b, c) in [(0, 10), (3, 7), (10, 0)] {
            let p = mcnemar_exact_p(b, c);
            assert!(
                (0.0..=1.0).contains(&p),
                "p out of range for b={b} c={c}: {p}"
            );
        }
    }

    #[test]
    fn mcnemar_detects_a_lopsided_difference_that_ci_overlap_would_miss() {
        assert!(mcnemar_exact_p(10, 0) < 0.005);
        assert!(
            mcnemar_exact_p(0, 2) > 0.05,
            "2 discordant pairs cannot resolve anything"
        );
    }

    #[test]
    fn paired_compare_counts_discordant_pairs_correctly() {
        let a = [true, true, false, false];
        let b = [true, false, true, false];
        let r = paired_compare(&a, &b);
        assert_eq!(r.b_only, 1, "a passed, b failed");
        assert_eq!(r.c_only, 1, "b passed, a failed");
        assert!((r.p_value - 1.0).abs() < 1e-9, "1 vs 1 is a tie");
    }

    #[test]
    #[should_panic(expected = "identical problem sets")]
    fn paired_compare_rejects_mismatched_lengths() {
        let _ = paired_compare(&[true], &[true, false]);
    }

    #[test]
    fn holm_is_more_conservative_than_raw_but_less_than_bonferroni() {
        let ps = [0.001, 0.02, 0.04];
        let rejected = holm_reject(&ps, 0.05);
        assert!(rejected[0], "smallest p survives Holm at alpha/3");
        assert_eq!(rejected.len(), 3);
    }

    #[test]
    fn bootstrap_ci_brackets_the_mean_and_narrows_with_more_problems() {
        let small: Vec<f64> = (0..20).map(|i| if i < 16 { 1.0 } else { 0.0 }).collect();
        let large: Vec<f64> = (0..400).map(|i| if i < 320 { 1.0 } else { 0.0 }).collect();
        let (sl, sh) = bootstrap_ci(&small, 2000, 42);
        let (ll, lh) = bootstrap_ci(&large, 2000, 42);
        assert!(sl < 0.8 && 0.8 < sh, "brackets the 0.8 mean");
        assert!((lh - ll) < (sh - sl), "more problems -> tighter interval");
    }

    #[test]
    fn wilson_interval_stays_inside_zero_one_at_the_extremes() {
        let all_fail = wilson_interval(0, 10, Z_95);
        assert!(all_fail.low >= 0.0 && all_fail.high <= 1.0);
        let all_pass = wilson_interval(10, 10, Z_95);
        assert!(all_pass.low >= 0.0 && all_pass.high <= 1.0);
    }

    #[test]
    fn min_detectable_difference_reports_the_resolution_floor() {
        // Measured by exact enumeration 2026-09-01: 31 fixtures cannot resolve
        // a 10-point difference (power ~0.09). This must be published, not hidden.
        assert!(
            min_detectable_difference(31) >= 0.20,
            "31 fixtures is a ~25-30pt floor"
        );
        assert!(min_detectable_difference(164) < min_detectable_difference(31));
    }
}
