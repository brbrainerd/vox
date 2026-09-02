//! Unbiased pass@k over per-problem (n, c), per Chen et al. 2021 (arXiv 2107.03374).
//!
//! An earlier design computed "any attempt passed" with k derived from the
//! data. That is degenerate: at n=k it returns 1.000 for any problem with one
//! success, and a strong model reported k=1 while a weak one reported k=5 —
//! then both were sorted into one column. `k` is a CONFIG input here, never
//! derived from the samples.
//!
//! See `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md` §C2, §C3.

use serde::{Deserialize, Serialize};

/// One generation attempt. `compiled`/`tests_passed` are exit-code facts, never
/// an LLM judgment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptOutcome {
    pub compiled: bool,
    pub tests_passed: bool,
    /// The candidate neutralized the scoring oracle (see `vox_corpus::humaneval_runner::canary`).
    pub cheated: bool,
    pub total_tokens: u32,
    pub latency_ms: i64,
    pub cost_usd: Option<f64>,
}

/// All attempts at one fixture. `n` samples drawn, `c` correct — both required
/// by the unbiased estimator, so attempts are NEVER stopped early on a pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureOutcome {
    pub fixture_id: String,
    pub n: usize,
    pub c: usize,
    pub attempts: Vec<AttemptOutcome>,
}

/// Unbiased pass@k for one problem (Chen et al. 2021).
///
/// Product form: the closed form `1 - C(n-c,k)/C(n,k)` loses precision and
/// overflows at literature scales (n=200, k=100); this is the numerically
/// stable equivalent used by the reference `openai/human-eval` implementation.
#[must_use]
pub fn pass_at_k(n: usize, c: usize, k: usize) -> f64 {
    assert!(n >= k, "pass@k requires n >= k; got n={n}, k={k}");
    if n - c < k {
        return 1.0;
    }
    let mut prod = 1.0f64;
    for j in (n - c + 1)..=n {
        prod *= 1.0 - (k as f64) / (j as f64);
    }
    1.0 - prod
}

/// Corpus pass@k: the mean of per-problem unbiased estimates.
#[must_use]
pub fn corpus_pass_at_k(outcomes: &[FixtureOutcome], k: usize) -> f64 {
    if outcomes.is_empty() {
        return 0.0;
    }
    outcomes.iter().map(|f| pass_at_k(f.n, f.c, k)).sum::<f64>() / outcomes.len() as f64
}

/// Published axes for one (model, harness, condition) row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusScore {
    pub n_fixtures: usize,
    pub k: usize,
    /// The unbiased pass@k ESTIMATOR at k=1: the mean, over problems, of
    /// `c/n` (correct samples over all samples drawn). NOT "did the first
    /// sequential attempt pass" — see `per_problem_pass_at_1` for that. The
    /// two agree only when every fixture's `n == 1`.
    pub pass_at_1: f64,
    pub pass_at_k: f64,
    pub compile_rate: f64,
    pub n_cheated: usize,
    pub n_infra_errors: usize,
    /// `None` when this row's solutions were ingested rather than generated —
    /// never 0, which would publish a false "used no tokens" claim.
    pub total_tokens: Option<u64>,
    pub tokens_per_pass: Option<f64>,
    pub p50_ms: Option<i64>,
    pub cumulative_cost_usd: Option<f64>,
    pub cost_per_success_usd: Option<f64>,
    /// Per-problem FIRST-attempt pass/fail, one bool per fixture — the fixed
    /// single outcome McNemar/Holm pairing in `corpus_stats` requires. Distinct
    /// from `pass_at_1` above (the aggregate estimator over all samples): at
    /// n=1 (a greedy headline run) the two notions coincide, but at n>1 this
    /// field intentionally ignores samples after the first.
    pub per_problem_pass_at_1: Vec<bool>,
}

/// Fold outcomes at a caller-supplied `k`. `measured` says whether generation
/// metrics (tokens/latency/cost) exist for this row — false for `--from-dir`
/// ingested solutions.
#[must_use]
pub fn score_corpus(outcomes: &[FixtureOutcome], k: usize, measured: bool) -> CorpusScore {
    let n_fixtures = outcomes.len();
    let frac = |x: usize| {
        if n_fixtures == 0 {
            0.0
        } else {
            x as f64 / n_fixtures as f64
        }
    };
    fn first(f: &FixtureOutcome) -> Option<&AttemptOutcome> {
        f.attempts.first()
    }

    let n_compiled = outcomes
        .iter()
        .filter(|f| first(f).is_some_and(|a| a.compiled))
        .count();
    let n_cheated = outcomes
        .iter()
        .filter(|f| f.attempts.iter().any(|a| a.cheated))
        .count();
    let all: Vec<&AttemptOutcome> = outcomes.iter().flat_map(|f| f.attempts.iter()).collect();

    let (total_tokens, tokens_per_pass, p50_ms, cum_cost, cost_per_success) = if measured {
        let tt: u64 = all.iter().map(|a| a.total_tokens as u64).sum();
        let passes = outcomes.iter().filter(|f| f.c > 0).count().max(1);
        let mut lat: Vec<i64> = all.iter().map(|a| a.latency_ms).collect();
        lat.sort_unstable();
        let p50 = if lat.is_empty() {
            0
        } else {
            let idx = (0.5 * lat.len() as f64).ceil().max(1.0) as usize - 1;
            lat[idx.min(lat.len() - 1)]
        };
        let cost: f64 = all.iter().filter_map(|a| a.cost_usd).sum();
        let known = all.iter().any(|a| a.cost_usd.is_some());
        (
            Some(tt),
            Some(tt as f64 / passes as f64),
            Some(p50),
            known.then_some(cost),
            (known && passes > 0).then(|| cost / passes as f64),
        )
    } else {
        (None, None, None, None, None)
    };

    CorpusScore {
        n_fixtures,
        k,
        pass_at_1: corpus_pass_at_k(outcomes, 1),
        pass_at_k: corpus_pass_at_k(outcomes, k),
        compile_rate: frac(n_compiled),
        n_cheated,
        n_infra_errors: 0,
        total_tokens,
        tokens_per_pass,
        p50_ms,
        cumulative_cost_usd: cum_cost,
        cost_per_success_usd: cost_per_success,
        per_problem_pass_at_1: outcomes
            .iter()
            .map(|f| first(f).is_some_and(|a| a.tests_passed && !a.cheated))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_at_k_matches_the_closed_form_and_is_stable_at_scale() {
        fn closed(n: u64, c: u64, k: u64) -> f64 {
            fn comb(n: u64, k: u64) -> f64 {
                if k > n {
                    return 0.0;
                }
                (0..k).map(|i| (n - i) as f64 / (i + 1) as f64).product()
            }
            if n - c < k {
                1.0
            } else {
                1.0 - comb(n - c, k) / comb(n, k)
            }
        }
        for n in 1..=30u64 {
            for c in 0..=n {
                for k in 1..=n {
                    let (a, b) = (
                        closed(n, c, k),
                        pass_at_k(n as usize, c as usize, k as usize),
                    );
                    assert!((a - b).abs() < 1e-9, "n={n} c={c} k={k}: {a} vs {b}");
                }
            }
        }
        assert!(
            pass_at_k(200, 100, 100).is_finite(),
            "must not overflow at literature scale"
        );
    }

    #[test]
    fn pass_at_1_equals_the_empirical_rate() {
        assert!((pass_at_k(10, 5, 1) - 0.5).abs() < 1e-9);
        assert_eq!(pass_at_k(10, 0, 1), 0.0);
        assert_eq!(pass_at_k(10, 10, 1), 1.0);
    }

    #[test]
    fn n_equals_k_is_degenerate_which_is_why_k_must_be_config_driven() {
        // Pinned regression: at n=k any success scores 1.0.
        for n in [5, 10, 20] {
            assert_eq!(pass_at_k(n, 1, n), 1.0);
        }
        assert!((pass_at_k(20, 1, 1) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn corpus_pass_at_k_averages_over_problems() {
        let o = vec![
            FixtureOutcome {
                fixture_id: "a".into(),
                n: 10,
                c: 10,
                attempts: vec![],
            },
            FixtureOutcome {
                fixture_id: "b".into(),
                n: 10,
                c: 0,
                attempts: vec![],
            },
        ];
        assert!((corpus_pass_at_k(&o, 1) - 0.5).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "pass@k requires n >= k")]
    fn corpus_pass_at_k_panics_when_k_exceeds_samples() {
        let o = vec![FixtureOutcome {
            fixture_id: "a".into(),
            n: 1,
            c: 1,
            attempts: vec![],
        }];
        let _ = corpus_pass_at_k(&o, 5);
    }

    fn attempt(compiled: bool, passed: bool, tokens: u32, latency: i64) -> AttemptOutcome {
        AttemptOutcome {
            compiled,
            tests_passed: passed,
            cheated: false,
            total_tokens: tokens,
            latency_ms: latency,
            cost_usd: None,
        }
    }

    #[test]
    fn score_corpus_never_early_stops_and_reports_all_n_attempts() {
        let outcomes = vec![FixtureOutcome {
            fixture_id: "041".into(),
            n: 3,
            c: 1,
            attempts: vec![
                attempt(true, false, 10, 5),
                attempt(true, true, 10, 5),
                attempt(true, false, 10, 5),
            ],
        }];
        let score = score_corpus(&outcomes, 3, true);
        assert_eq!(score.n_fixtures, 1);
        // `pass_at_1` is the unbiased ESTIMATOR at k=1 (c/n over all samples),
        // not "did the first sequential attempt pass" — those are two
        // different statistics with the same expectation but different
        // variance, and conflating them was the exact mistake this test
        // originally made. c=1, n=3 -> 1/3, not 0.0.
        assert!((score.pass_at_1 - (1.0 / 3.0)).abs() < 1e-9);
        // The "first attempt passed" indicator lives on a separate field,
        // used for paired significance testing (see corpus_stats), where a
        // fixed single outcome per problem is what pairing requires.
        assert_eq!(score.per_problem_pass_at_1, vec![false]);
        // pass@3 uses (n=3, c=1) via the unbiased estimator, not "any passed".
        assert!((score.pass_at_k - pass_at_k(3, 1, 3)).abs() < 1e-9);
    }

    #[test]
    fn unmeasured_rows_never_report_a_false_zero_for_tokens_or_latency() {
        let outcomes = vec![FixtureOutcome {
            fixture_id: "a".into(),
            n: 1,
            c: 1,
            attempts: vec![attempt(true, true, 0, 0)],
        }];
        let score = score_corpus(&outcomes, 1, false);
        assert!(
            score.total_tokens.is_none(),
            "ingested row must be None, not 0"
        );
        assert!(score.tokens_per_pass.is_none());
        assert!(score.p50_ms.is_none());
    }

    #[test]
    fn cheating_marks_the_fixture_without_crashing_scoring() {
        let cheat_attempt = AttemptOutcome {
            compiled: true,
            tests_passed: false,
            cheated: true,
            total_tokens: 5,
            latency_ms: 5,
            cost_usd: None,
        };
        let outcomes = vec![FixtureOutcome {
            fixture_id: "a".into(),
            n: 1,
            c: 0,
            attempts: vec![cheat_attempt],
        }];
        let score = score_corpus(&outcomes, 1, true);
        assert_eq!(score.n_cheated, 1);
        assert!(
            (score.pass_at_1 - 0.0).abs() < 1e-9,
            "cheating never scores a pass"
        );
    }

    #[test]
    fn compile_rate_is_separate_from_test_pass_rate() {
        let outcomes = vec![
            FixtureOutcome {
                fixture_id: "a".into(),
                n: 1,
                c: 0,
                attempts: vec![attempt(true, false, 10, 5)],
            },
            FixtureOutcome {
                fixture_id: "b".into(),
                n: 1,
                c: 0,
                attempts: vec![attempt(false, false, 10, 5)],
            },
        ];
        let score = score_corpus(&outcomes, 1, true);
        assert!((score.compile_rate - 0.5).abs() < 1e-9, "1 of 2 compiled");
        assert!((score.pass_at_1 - 0.0).abs() < 1e-9, "neither passed tests");
    }
}
