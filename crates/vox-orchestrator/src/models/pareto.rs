//! Task M3 ("Surface"): the non-dominated subset of a candidate set, for *reporting*.
//!
//! Deliberately beside [`crate::models::wilson_score_interval`] in `models/` rather than in
//! `routing/`: nothing here feeds model selection. `vox model scoreboard` and
//! `vox model explain` use it to stop presenting a single composite rank over axes that trade
//! against each other. See `docs/src/adr/046-pareto-frontier-reporting.md`.

use super::{ModelScore, wilson_score_interval};

/// One candidate's position in objective space. `quality` is higher-is-better; `cost_usd` and
/// `latency_ms` are lower-is-better.
///
/// `None` on a lower-is-better axis means **no measurement**, and is treated as
/// *incomparable*: it neither establishes superiority nor concedes it. Not "worst possible"
/// (that permanently dominates out every model without a `model_scoreboard` row) and not
/// "neutral" (that is non-transitive — see [`pareto_frontier`]).
///
/// Identity is the point's **position** in the slice passed to [`pareto_frontier`]; there is
/// deliberately no `index` field to drift out of sync with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParetoPoint {
    pub quality: f64,
    pub cost_usd: Option<f64>,
    pub latency_ms: Option<i64>,
}

/// `true` when `a` dominates `b`: at least as good on every comparable axis, strictly better
/// on at least one. Non-finite values compare false both ways, so a `NaN` point neither
/// dominates nor is dominated.
fn dominates(a: &ParetoPoint, b: &ParetoPoint) -> bool {
    /// An unknown reading is incomparable, not neutral. Neutral (`_ => true`) is not
    /// transitive — `Some(5) ≼ None ≼ Some(1)` while `Some(5) ⋠ Some(1)` — which admits
    /// domination cycles and an empty frontier for non-empty input.
    fn no_worse<T: PartialOrd>(a: Option<T>, b: Option<T>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => a <= b,
            (None, None) => true,
            _ => false,
        }
    }
    fn strictly_better<T: PartialOrd>(a: Option<T>, b: Option<T>) -> bool {
        matches!((a, b), (Some(a), Some(b)) if a < b)
    }

    if !(a.quality >= b.quality
        && no_worse(a.cost_usd, b.cost_usd)
        && no_worse(a.latency_ms, b.latency_ms))
    {
        return false;
    }
    a.quality > b.quality
        || strictly_better(a.cost_usd, b.cost_usd)
        || strictly_better(a.latency_ms, b.latency_ms)
}

/// Positions of every point that no other point dominates, in input order.
///
/// Never returns empty for non-empty input: [`dominates`] is irreflexive and transitive —
/// transitive *because* an unknown axis is incomparable rather than neutral, so no chain of
/// comparisons can cycle — and a strict partial order on a finite set always has a maximal
/// element. A `NaN` quality is incomparable in both directions, so such a point is always
/// maximal and never empties the frontier.
#[must_use]
pub fn pareto_frontier(points: &[ParetoPoint]) -> Vec<usize> {
    (0..points.len())
        .filter(|&i| !points.iter().any(|other| dominates(other, &points[i])))
        .collect()
}

/// The single mapping from a scoreboard row to objective space, shared by every surface so
/// two of them cannot mark different frontiers.
///
/// Quality is the 95% Wilson **lower** bound of the observed success rate. `success` means
/// "the provider returned a non-error response" (see this plan's Global Constraints), so this
/// axis is *reliability*, not answer quality. A row with no observations yields `0.0` — which
/// is why callers must gate on [`is_observed`] rather than putting such rows on a frontier.
///
/// A non-finite or negative cost is discarded to `None`: unknown, never an unbeatable minimum.
#[must_use]
pub fn pareto_point_for(score: Option<&ModelScore>) -> ParetoPoint {
    ParetoPoint {
        quality: score
            .and_then(|s| wilson_score_interval(s.success_count, s.n_calls))
            .map_or(0.0, |(lo, _hi)| lo),
        cost_usd: score
            .and_then(|s| s.cost_per_success_usd)
            .filter(|c| c.is_finite() && *c >= 0.0),
        latency_ms: score.and_then(|s| s.p50_latency_ms),
    }
}

/// Whether a row has any observations at all. A row with `n_calls == 0` has quality `0.0` by
/// construction, so including it in a frontier would either delete it (if compared) or hand it
/// an unearned mark (if all its axes are unknown and thus incomparable). Neither is honest:
/// "we measured this and it lost" and "we never tried it" are different facts.
#[must_use]
pub fn is_observed(score: Option<&ModelScore>) -> bool {
    score.is_some_and(|s| s.n_calls > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(quality: f64, cost: f64, latency: i64) -> ParetoPoint {
        ParetoPoint {
            quality,
            cost_usd: Some(cost),
            latency_ms: Some(latency),
        }
    }

    fn score(success_count: i64, n_calls: i64, cost: Option<f64>, p50: Option<i64>) -> ModelScore {
        ModelScore {
            success_count,
            n_calls,
            cost_per_success_usd: cost,
            p50_latency_ms: p50,
            ..ModelScore::default()
        }
    }

    #[test]
    fn empty_input_has_empty_frontier() {
        assert_eq!(pareto_frontier(&[]), Vec::<usize>::new());
    }

    #[test]
    fn single_point_is_its_own_frontier() {
        assert_eq!(pareto_frontier(&[p(0.9, 0.01, 200)]), vec![0]);
    }

    #[test]
    fn strictly_dominated_point_is_excluded() {
        assert_eq!(
            pareto_frontier(&[p(0.95, 0.01, 200), p(0.80, 0.01, 200)]),
            vec![0]
        );
    }

    #[test]
    fn tradeoff_points_are_both_on_the_frontier() {
        assert_eq!(
            pareto_frontier(&[p(0.95, 0.05, 400), p(0.80, 0.01, 400)]),
            vec![0, 1]
        );
    }

    #[test]
    fn identical_points_do_not_dominate_each_other() {
        // Irreflexivity. Without the strict-inequality clause each "dominates" the other and
        // the frontier is EMPTY for non-empty input.
        assert_eq!(
            pareto_frontier(&[p(0.9, 0.01, 200), p(0.9, 0.01, 200)]),
            vec![0, 1]
        );
    }

    #[test]
    fn frontier_preserves_input_order() {
        let a = p(0.95, 0.05, 400);
        let b = p(0.60, 0.09, 900); // dominated by a
        let c = p(0.70, 0.01, 150);
        assert_eq!(
            pareto_frontier(&[a, b, c]),
            vec![0, 2],
            "input order, not sorted"
        );
    }

    #[test]
    fn latency_alone_can_keep_a_point_on_the_frontier() {
        // b is worse on quality and TIED on cost; only latency saves it. A two-axis
        // (quality, cost) implementation returns [0] and fails -- and would otherwise pass
        // every other test in this file, because no other test's outcome depends on latency.
        assert_eq!(
            pareto_frontier(&[p(0.90, 0.02, 900), p(0.70, 0.02, 100)]),
            vec![0, 1]
        );
    }

    #[test]
    fn latency_alone_can_remove_a_point_from_the_frontier() {
        // Mirror: tied on quality AND cost, strictly slower. A latency-blind `dominates`
        // returns [0,1]; correct is [0]. Also kills `a.quality > b.quality` (instead of
        // `>=`), which never dominates on a quality tie.
        assert_eq!(
            pareto_frontier(&[p(0.80, 0.02, 100), p(0.80, 0.02, 900)]),
            vec![0]
        );
    }

    #[test]
    fn a_tie_on_quality_still_lets_cost_decide_domination() {
        // Two models 100%-reliable over the same n produce identical Wilson bounds, so
        // quality ties are routine here, not exotic.
        assert_eq!(
            pareto_frontier(&[p(0.90, 0.01, 200), p(0.90, 0.50, 200)]),
            vec![0]
        );
    }

    #[test]
    fn an_unknown_axis_is_incomparable_not_conceded() {
        // Equal quality, one row missing both measurements: neither dominates.
        let known = p(0.5, 0.02, 300);
        let unknown = ParetoPoint {
            quality: 0.5,
            cost_usd: None,
            latency_ms: None,
        };
        assert_eq!(pareto_frontier(&[known, unknown]), vec![0, 1]);
    }

    #[test]
    fn an_unknown_axis_does_not_rescue_a_worse_quality_point() {
        let good = ParetoPoint {
            quality: 0.9,
            cost_usd: None,
            latency_ms: None,
        };
        let bad = ParetoPoint {
            quality: 0.2,
            cost_usd: None,
            latency_ms: None,
        };
        assert_eq!(pareto_frontier(&[good, bad]), vec![0]);
    }

    #[test]
    fn unknown_on_one_axis_only_still_lets_the_known_axes_decide() {
        // Worse on quality AND cost, but with ONE unknown axis (latency). Under a strictly
        // incomparable `no_worse`, a mismatched Some/None pair on any single axis blocks
        // domination for that pair outright -- this is required for transitivity (see the
        // 4-cycle test) and cannot be relaxed per-pair without reintroducing it. So `worse`
        // survives alongside `good` here: an unmeasured axis is incomparable, not something
        // the other, fully-known axes can override.
        let good = ParetoPoint {
            quality: 0.9,
            cost_usd: Some(0.01),
            latency_ms: Some(100),
        };
        let worse = ParetoPoint {
            quality: 0.5,
            cost_usd: Some(0.20),
            latency_ms: None,
        };
        assert_eq!(pareto_frontier(&[good, worse]), vec![0, 1]);
    }

    #[test]
    fn a_mix_of_known_and_unknown_axes_cannot_form_a_domination_cycle() {
        // Under a *neutral* unknown these four form the cycle 0>1>2>3>0 and the frontier is
        // EMPTY for non-empty input. This is the test that pins incomparability.
        let pts = vec![
            ParetoPoint {
                quality: 0.5,
                cost_usd: Some(1.0),
                latency_ms: None,
            },
            ParetoPoint {
                quality: 0.5,
                cost_usd: Some(2.0),
                latency_ms: Some(1),
            },
            ParetoPoint {
                quality: 0.5,
                cost_usd: None,
                latency_ms: Some(2),
            },
            ParetoPoint {
                quality: 0.5,
                cost_usd: Some(0.5),
                latency_ms: Some(3),
            },
        ];
        assert_eq!(pareto_frontier(&pts), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_non_empty_input_never_produces_an_empty_frontier() {
        // The invariant the doc comment claims. Hostile mix: NaN, all-unknown, a duplicate,
        // and a strictly dominated row.
        let pts = vec![
            p(0.9, 0.01, 200),
            p(0.9, 0.01, 200),
            ParetoPoint {
                quality: f64::NAN,
                cost_usd: None,
                latency_ms: Some(5),
            },
            ParetoPoint {
                quality: 0.5,
                cost_usd: None,
                latency_ms: None,
            },
            p(0.1, 0.90, 9000),
        ];
        let frontier = pareto_frontier(&pts);
        assert!(!frontier.is_empty());
        assert!(
            !frontier.contains(&4),
            "the strictly dominated row must be excluded"
        );
    }

    #[test]
    fn a_non_finite_quality_neither_dominates_nor_is_dominated() {
        // NaN compares false both ways. Pin the exact answer -- asserting only
        // `contains(&0)` would pass for `vec![0]`, `vec![0,1]`, or return-everything.
        let sane = p(0.9, 0.01, 200);
        let nan = ParetoPoint {
            quality: f64::NAN,
            cost_usd: Some(0.01),
            latency_ms: Some(200),
        };
        assert_eq!(pareto_frontier(&[sane, nan]), vec![0, 1]);
    }

    #[test]
    fn a_nan_quality_point_cannot_dominate_out_a_real_one_even_when_cheapest() {
        // The dangerous direction: if quality comparison is ever "fixed" with
        // total_cmp/unwrap_or(Less), NaN sorts highest and deletes the real row.
        let real = p(0.95, 0.50, 900);
        let nan = ParetoPoint {
            quality: f64::NAN,
            cost_usd: Some(0.001),
            latency_ms: Some(10),
        };
        assert_eq!(pareto_frontier(&[real, nan]), vec![0, 1]);
    }

    #[test]
    fn appending_a_dominated_point_does_not_change_the_original_frontier() {
        let base = [p(0.95, 0.05, 400), p(0.70, 0.01, 150)];
        let with_extra = [base[0], base[1], p(0.60, 0.09, 900)];
        assert_eq!(pareto_frontier(&base), vec![0, 1]);
        assert_eq!(pareto_frontier(&with_extra), vec![0, 1]);
    }

    #[test]
    fn pareto_point_for_uses_the_wilson_lower_bound_not_the_raw_rate() {
        // 18/20: raw .900000, Wilson lo .698962, center .835548, upper .972134. The band
        // excludes all three wrong readings with ~.05 of margin on the right one. The
        // fixture leaves success_rate at its 0.0 default, so reading that field is excluded
        // too.
        let pt = pareto_point_for(Some(&score(18, 20, Some(0.03), Some(250))));
        assert!(
            pt.quality > 0.65 && pt.quality < 0.75,
            "expected ~0.699, got {}",
            pt.quality
        );
        assert_eq!(pt.cost_usd, Some(0.03));
        assert_eq!(pt.latency_ms, Some(250));
    }

    #[test]
    fn pareto_point_for_treats_an_absent_or_zero_call_row_as_unobserved() {
        // `wilson_score_interval` is None at n=0, so quality falls back to 0.0. Callers must
        // NOT put these on a frontier -- see `is_observed`.
        for row in [None, Some(&score(0, 0, None, None))] {
            let pt = pareto_point_for(row);
            assert_eq!(pt.quality, 0.0);
            assert_eq!(pt.cost_usd, None);
            assert_eq!(pt.latency_ms, None);
        }
    }

    #[test]
    fn pareto_point_for_discards_a_non_finite_or_negative_cost() {
        // NaN/inf from a divide-by-zero upstream, or a negative from bad data, must read as
        // "unknown" -- never as an unbeatable minimum on the cost axis.
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            let pt = pareto_point_for(Some(&score(9, 10, Some(bad), Some(100))));
            assert_eq!(
                pt.cost_usd, None,
                "cost {bad} must not sort as best-possible"
            );
        }
    }

    #[test]
    fn is_observed_is_false_for_rows_that_cannot_be_ranked() {
        assert!(!is_observed(None));
        assert!(!is_observed(Some(&score(0, 0, None, None))));
        assert!(is_observed(Some(&score(1, 1, None, None))));
    }
}
