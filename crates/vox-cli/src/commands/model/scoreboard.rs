use clap::Parser;
use comfy_table::Table;
use owo_colors::OwoColorize;
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::models::{
    MIN_CALLS_FOR_CONFIDENT_RANK, ModelScore, ParetoPoint, is_observed, pareto_frontier,
    pareto_point_for,
};

/// Show the model scoreboard.
#[derive(Parser)]
pub struct ScoreboardArgs {
    /// Time window in days (default: 7).
    #[arg(long, default_value_t = 7)]
    pub window: i64,
    /// Output format (default: table).
    #[arg(long, default_value = "table")]
    pub format: String,
    /// Spend ceiling in USD per success. When set, names the most reliable Pareto-optimal row
    /// costing no more than this. Advisory only — routing does not read it.
    #[arg(long)]
    pub budget: Option<f64>,
}

/// Task M3: below this many successes, `cost_per_success` is too noisy to trust — a single
/// expensive fallback call among 1-2 successes can swing the ratio by an order of magnitude.
const COST_PER_SUCCESS_MIN_SUCCESSES: i64 = 10;

/// Task M3: renders `cost_per_success` as "insufficient data" rather than a confident-looking
/// number when `success_count` hasn't crossed [`COST_PER_SUCCESS_MIN_SUCCESSES`].
fn cost_per_success_display(cost_per_success_usd: Option<f64>, success_count: i64) -> String {
    if success_count < COST_PER_SUCCESS_MIN_SUCCESSES {
        return "insufficient data".to_string();
    }
    cost_per_success_usd
        .map(|v| format!("${v:.4}"))
        .unwrap_or_default()
}

/// Task M3: colored success-rate percentage plus a `(low-N)` marker below
/// [`vox_orchestrator::models::MIN_CALLS_FOR_CONFIDENT_RANK`] calls. The full Wilson credible
/// interval isn't crammed into this table cell (the "Calls" column already lets a reader judge
/// confidence, and `--format json` exposes `n_calls`/`success_count` for anything that wants to
/// compute its own interval) — `vox model explain` renders the full interval per-candidate.
fn success_rate_cell(success_rate: f64, n_calls: i64) -> String {
    let pct = format!("{:.1}%", success_rate * 100.0);
    let colored = if success_rate > 0.95 {
        pct.green().to_string()
    } else if success_rate > 0.8 {
        pct.yellow().to_string()
    } else {
        pct.red().to_string()
    };
    if n_calls < MIN_CALLS_FOR_CONFIDENT_RANK {
        format!("{colored} (low-N)")
    } else {
        colored
    }
}

/// Task M3: `" *"` when this row is on the Pareto frontier *and* has enough observations to be
/// worth the claim. The low-N gate is not redundant with the frontier's own observation filter:
/// a row with 1 call is "observed", so it can sit on the frontier while `success_rate_cell`
/// prints "(low-N)" beside it — "untrustworthy" and "unbeaten" in the same row.
fn frontier_marker(frontier: &[usize], position: usize, n_calls: i64) -> &'static str {
    if n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK && frontier.contains(&position) {
        " *"
    } else {
        ""
    }
}

/// Task M3: frontier positions the budget line is allowed to name.
///
/// Both gates are load-bearing and neither implies the other:
/// - `n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK`, because [`frontier_marker`] withholds `*` from a
///   sub-threshold row. Without this gate the budget line calls a row "Pareto-optimal" in prose
///   directly beneath a table that pointedly does not mark it.
/// - `success_count >= COST_PER_SUCCESS_MIN_SUCCESSES`, because [`cost_per_success_display`]
///   prints "insufficient data" below it. Without this gate the recommendation matches on a
///   cost the same table refuses to show.
fn recommendable_positions(scores: &[ModelScore], frontier: &[usize]) -> Vec<usize> {
    frontier
        .iter()
        .copied()
        .filter(|&i| {
            scores.get(i).is_some_and(|s| {
                s.n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK
                    && s.success_count >= COST_PER_SUCCESS_MIN_SUCCESSES
            })
        })
        .collect()
}

/// Task M3: position of the most reliable recommendable row costing no more than `budget`.
///
/// `recommendable` must be the output of [`recommendable_positions`] — frontier positions that
/// cleared both confidence gates. The parameter is named for that precondition rather than
/// taking a raw frontier, so a caller cannot silently reintroduce an unmarked, "insufficient
/// data" row into a line that calls it Pareto-optimal.
///
/// Only frontier positions are candidates — recommending a dominated row would contradict the
/// `*` marks in the same table. A row with unknown cost is never recommended: a recommendation
/// commits the reader to a spend, and "we don't know what this costs" is not affordability.
/// Ties break on a **total** order — reliability, then cheapest, then fastest, then lowest
/// position — because `get_model_scoreboard` has no `ORDER BY` and row order is not reproducible
/// across runs. All four keys are load-bearing: two rows can tie on reliability *and* cost while
/// differing on latency (an unknown latency is incomparable, so both stay on the frontier), and
/// without the latency key the winner would be whichever the database happened to return first.
/// An unknown latency sorts last rather than first — it is not evidence of being fast.
fn budget_recommendation(
    points: &[ParetoPoint],
    recommendable: &[usize],
    budget: f64,
) -> Option<usize> {
    if !budget.is_finite() || budget <= 0.0 {
        return None;
    }
    recommendable
        .iter()
        .copied()
        .filter_map(|i| {
            let point = points.get(i)?;
            let cost = point.cost_usd.filter(|&c| c <= budget)?;
            Some((i, point.quality, cost, point.latency_ms.unwrap_or(i64::MAX)))
        })
        .reduce(|best, cur| {
            // Position is a real fourth key, not just "whichever the fold saw first". Today
            // `pareto_frontier` yields ascending indices, so the two coincide — but that is the
            // caller's invariant, and a tie-break whose stability depends on its caller is the
            // same defect one level up.
            let better = cur.1 > best.1
                || (cur.1 == best.1
                    && (cur.2 < best.2
                        || (cur.2 == best.2
                            && (cur.3 < best.3 || (cur.3 == best.3 && cur.0 < best.0)))));
            if better { cur } else { best }
        })
        .map(|(i, ..)| i)
}

/// Task M3: the advisory line printed under the table when `--budget` is set.
///
/// `recommendable` carries [`budget_recommendation`]'s precondition: frontier positions that
/// cleared both confidence gates.
fn render_budget_line(
    labels: &[String],
    points: &[ParetoPoint],
    recommendable: &[usize],
    budget: f64,
) -> String {
    match budget_recommendation(points, recommendable, budget).and_then(|i| labels.get(i)) {
        Some(label) => format!(
            "Within ${budget:.4}/success: {label} — highest Wilson lower bound on success rate \
             among Pareto-optimal rows."
        ),
        None => format!("Within ${budget:.4}/success: no Pareto-optimal row qualifies."),
    }
}

/// Task M3: the legend for the `*` marker.
///
/// States the strict-domination clause explicitly. "At least as good on all axes" alone would be
/// wrong — two identical rows are each at least as good as the other, yet neither is dominated,
/// so both are correctly marked.
///
/// The last two sentences are not decoration. "Success counts non-error provider responses" is
/// the sentence that stops a reader reading `*` as *answer* quality, and naming the Wilson lower
/// bound stops them trying to reproduce the ranking from the raw `Success %` column beside it.
pub(super) fn pareto_legend() -> &'static str {
    "* Pareto-optimal: no other row is at least as good on success rate, cost and latency \
     while being strictly better on at least one. Ranked on the Wilson lower bound of the \
     success rate, not the raw percentage shown. Success counts non-error provider responses, \
     not answer correctness. Rows below the observation threshold are never marked."
}

pub async fn run(args: ScoreboardArgs) -> anyhow::Result<()> {
    let db_config = DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?;
    let db = VoxDb::connect(db_config).await?;

    let rows = db.get_model_scoreboard(args.window).await?;

    if let Some(budget) = args.budget {
        // Rejected here as well as inside `budget_recommendation`: the inner guard returns `None`,
        // which renders as "no row qualifies" — indistinguishable from a real negative result. A
        // reader who typed `--budget -5` deserves to be told the input was nonsense.
        anyhow::ensure!(
            budget.is_finite() && budget > 0.0,
            "--budget must be a finite positive number of USD per success (got {budget})"
        );
    }

    if args.format == "json" {
        if args.budget.is_some() {
            eprintln!(
                "note: --budget applies to the table output only; ignored with --format json"
            );
        }
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    let mut table = Table::new();
    table.set_header(vec![
        "Model ID",
        "Category",
        "Strength",
        "Calls",
        "Success %",
        "p50 ms",
        "p99 ms",
        "Cost/Succ",
        "p95 TTFT",
        "p95 TPOT",
        "Goodput tok/s",
    ]);

    // Objective-space view of every row, then the frontier over the *observed* subset mapped
    // back into row positions. Unobserved rows are excluded rather than scored: their quality is
    // `0.0` by construction and all their axes are unknown, so leaving them in would hand them an
    // unearned `*` (all-unknown axes are incomparable, hence never dominated).
    let scores: Vec<ModelScore> = rows.iter().cloned().map(ModelScore::from).collect();
    let points: Vec<ParetoPoint> = scores.iter().map(|s| pareto_point_for(Some(s))).collect();
    let observed: Vec<usize> = (0..scores.len())
        .filter(|&i| is_observed(Some(&scores[i])))
        .collect();
    let observed_points: Vec<ParetoPoint> = observed.iter().map(|&i| points[i]).collect();
    let frontier: Vec<usize> = pareto_frontier(&observed_points)
        .into_iter()
        .map(|j| observed[j])
        .collect();
    let labels: Vec<String> = rows
        .iter()
        .map(|r| format!("{} ({}/{})", r.model_id, r.task_category, r.strength_tag))
        .collect();

    for (i, row) in rows.into_iter().enumerate() {
        let success_cell = success_rate_cell(row.success_rate, row.n_calls);

        table.add_row(vec![
            format!(
                "{}{}",
                row.model_id,
                frontier_marker(&frontier, i, row.n_calls)
            ),
            row.task_category,
            row.strength_tag,
            row.n_calls.to_string(),
            success_cell,
            row.p50_latency_ms
                .map(|v| v.to_string())
                .unwrap_or_default(),
            row.p99_latency_ms
                .map(|v| v.to_string())
                .unwrap_or_default(),
            cost_per_success_display(row.cost_per_success_usd, row.success_count),
            // Deliberately not surfaced (Task M0/M2): `model_scoreboard.quality_score` is
            // `COALESCE(AVG(llm_feedback.rating)/5.0), 1.0)` over a table with zero rows in
            // practice, i.e. a constant 1.0 for every model — see the GUI's
            // `crates/vox-gui/src/commands/models.rs::list_model_cards`, which already omits
            // it for the same reason. Restore once a real quality gate defines it.
            row.p95_ttft_ms.map(|v| v.to_string()).unwrap_or_default(),
            row.p95_tpot_ms
                .map(|v| format!("{v:.1}"))
                .unwrap_or_default(),
            row.goodput_tokens_per_sec
                .map(|v| format!("{v:.1}"))
                .unwrap_or_default(),
        ]);
    }

    println!("{}", table);
    // Printed unconditionally: when nothing is marked, the legend's closing sentence is what
    // explains the absence.
    println!("\n{}", pareto_legend());
    if let Some(budget) = args.budget {
        let recommendable = recommendable_positions(&scores, &frontier);
        println!(
            "\n{}",
            render_budget_line(&labels, &points, &recommendable, budget)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_per_success_display_flags_insufficient_data_under_the_threshold() {
        assert_eq!(cost_per_success_display(Some(0.01), 9), "insufficient data");
        assert_eq!(cost_per_success_display(Some(0.01), 0), "insufficient data");
    }

    #[test]
    fn cost_per_success_display_shows_the_value_at_and_above_the_threshold() {
        assert_eq!(cost_per_success_display(Some(0.0123), 10), "$0.0123");
        assert_eq!(cost_per_success_display(Some(1.5), 50), "$1.5000");
    }

    #[test]
    fn cost_per_success_display_handles_missing_value_above_threshold() {
        assert_eq!(cost_per_success_display(None, 20), "");
    }

    #[test]
    fn success_rate_cell_flags_low_n_below_the_threshold() {
        let cell = success_rate_cell(1.0, 2);
        assert!(cell.contains("100.0%"), "{cell}");
        assert!(cell.contains("(low-N)"), "{cell}");
    }

    #[test]
    fn success_rate_cell_omits_marker_at_and_above_the_threshold() {
        let cell = success_rate_cell(0.9, 20);
        assert!(cell.contains("90.0%"), "{cell}");
        assert!(!cell.contains("low-N"), "{cell}");
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
    fn frontier_marker_annotates_only_observed_frontier_members() {
        // Frontier mixes parity (1, 2) so a `position % 2 == 0` implementation cannot pass by
        // accident, which an earlier [0, 2] fixture allowed.
        assert_eq!(frontier_marker(&[1, 2], 0, 40), "");
        assert_eq!(frontier_marker(&[1, 2], 1, 40), " *");
        assert_eq!(frontier_marker(&[1, 2], 2, 40), " *");
        assert_eq!(frontier_marker(&[1, 2], 3, 40), "");
        assert_eq!(
            frontier_marker(&[], 0, 40),
            "",
            "an empty frontier marks nothing"
        );
        assert_eq!(
            frontier_marker(&[0], 0, 40),
            " *",
            "a singleton frontier marks its member"
        );
    }

    #[test]
    fn frontier_marker_never_marks_a_low_n_row() {
        // `success_rate_cell` already prints "(low-N)" on these. A row reading
        // "100.0% (low-N) *" claims both "untrustworthy" and "unbeaten" at once.
        assert_eq!(
            frontier_marker(&[0], 0, MIN_CALLS_FOR_CONFIDENT_RANK - 1),
            ""
        );
        assert_eq!(frontier_marker(&[0], 0, MIN_CALLS_FOR_CONFIDENT_RANK), " *");
    }

    #[test]
    fn budget_recommendation_picks_the_best_row_the_reader_can_afford() {
        // wilson_lo(95,100)=0.888248 > wilson_lo(85,100)=0.767163, and neither dominates
        // (a wins quality, b wins cost), so frontier == [0,1].
        // Kills "return the cheapest affordable" (fails the 1.00 case) and "ignore the budget,
        // return highest quality" (fails the 0.05 case).
        let points = vec![
            pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400)))),
            pareto_point_for(Some(&score(85, 100, Some(0.02), Some(400)))),
        ];
        let frontier = pareto_frontier(&points);
        assert_eq!(
            frontier,
            vec![0, 1],
            "precondition: a quality/cost tradeoff"
        );
        assert_eq!(budget_recommendation(&points, &frontier, 0.05), Some(1));
        assert_eq!(budget_recommendation(&points, &frontier, 1.00), Some(0));
    }

    #[test]
    fn budget_recommendation_prefers_the_cheaper_row_on_a_quality_tie() {
        // Identical (success_count, n_calls) gives an exactly equal Wilson bound, so ties are
        // routine. `max_by` returns the LAST maximum, and `get_model_scoreboard` has no ORDER BY
        // (`ops_scientia.rs:21-29`), so leaving the tie to row order is both cost-blind and
        // irreproducible across runs.
        let points = vec![
            pareto_point_for(Some(&score(9, 10, Some(0.01), Some(900)))),
            pareto_point_for(Some(&score(9, 10, Some(0.90), Some(100)))),
        ];
        let frontier = pareto_frontier(&points);
        assert_eq!(budget_recommendation(&points, &frontier, 10.0), Some(0));
    }

    #[test]
    fn budget_recommendation_breaks_a_cost_tie_on_latency_regardless_of_row_order() {
        // Regression for a two-key (quality, cost) tie-break, which passed every other tie test
        // here while leaving the winner to database row order — the exact irreproducibility the
        // tie-break exists to prevent. An unknown latency is incomparable, so BOTH rows sit on
        // the frontier, and equal (success_count, n_calls) makes the Wilson bounds bit-identical.
        let fast = score(9, 10, Some(0.02), Some(100));
        let unknown_latency = score(9, 10, Some(0.02), None);

        let ab = [
            pareto_point_for(Some(&unknown_latency)),
            pareto_point_for(Some(&fast)),
        ];
        let ba = [
            pareto_point_for(Some(&fast)),
            pareto_point_for(Some(&unknown_latency)),
        ];
        assert_eq!(
            pareto_frontier(&ab),
            vec![0, 1],
            "precondition: neither dominates"
        );

        // The fast row wins from either order; an unknown latency is not evidence of speed.
        assert_eq!(budget_recommendation(&ab, &[0, 1], 1.0), Some(1));
        assert_eq!(budget_recommendation(&ba, &[0, 1], 1.0), Some(0));
    }

    #[test]
    fn budget_recommendation_is_stable_when_every_key_ties() {
        // Total tie on all three axes must resolve to the lowest position, not to row order.
        let points = vec![
            pareto_point_for(Some(&score(9, 10, Some(0.02), Some(100)))),
            pareto_point_for(Some(&score(9, 10, Some(0.02), Some(100)))),
        ];
        assert_eq!(budget_recommendation(&points, &[0, 1], 1.0), Some(0));
        assert_eq!(budget_recommendation(&points, &[1, 0], 1.0), Some(0));
    }

    #[test]
    fn pareto_legend_discloses_what_success_means_and_that_low_n_rows_are_unmarked() {
        // These sentences are the mitigation for the plan's headline constraint: without them a
        // reader can read `*` as answer quality, or try to reproduce the ranking from the raw
        // `Success %` column instead of the Wilson lower bound the mark is actually computed on.
        let legend = pareto_legend();
        assert!(legend.contains("not answer correctness"), "{legend}");
        assert!(legend.contains("Wilson lower bound"), "{legend}");
        assert!(legend.contains("observation threshold"), "{legend}");
    }

    #[test]
    fn budget_recommendation_never_recommends_an_off_frontier_row() {
        let points = vec![
            pareto_point_for(Some(&score(95, 100, Some(0.10), Some(100)))),
            pareto_point_for(Some(&score(85, 100, Some(0.02), Some(100)))),
            pareto_point_for(Some(&score(80, 100, Some(0.02), Some(900)))), // dominated by [1]
        ];
        let frontier = pareto_frontier(&points);
        assert_eq!(frontier, vec![0, 1], "precondition: row 2 is dominated");
        assert_eq!(budget_recommendation(&points, &frontier, 0.05), Some(1));
        // Kills an implementation that ignores `frontier` and scans all points.
        assert_eq!(
            budget_recommendation(&points, &[0], 0.05),
            None,
            "only frontier positions are candidates"
        );
    }

    #[test]
    fn budget_recommendation_is_none_when_nothing_is_affordable() {
        let points = vec![pareto_point_for(Some(&score(
            95,
            100,
            Some(0.10),
            Some(400),
        )))];
        let frontier = pareto_frontier(&points);
        assert_eq!(budget_recommendation(&points, &frontier, 0.001), None);
    }

    #[test]
    fn budget_recommendation_excludes_unknown_cost_rows() {
        // Unknown cost stays on the frontier (no evidence against it) but cannot be recommended:
        // a recommendation commits, and "we don't know what this costs" is not affordability.
        let points = vec![pareto_point_for(Some(&score(99, 100, None, Some(100))))];
        let frontier = pareto_frontier(&points);
        assert_eq!(frontier, vec![0]);
        assert_eq!(budget_recommendation(&points, &frontier, 100.0), None);
    }

    #[test]
    fn budget_recommendation_rejects_a_nonsense_budget() {
        let points = vec![pareto_point_for(Some(&score(
            95,
            100,
            Some(0.10),
            Some(400),
        )))];
        let frontier = pareto_frontier(&points);
        for bad in [f64::NAN, -1.0, 0.0] {
            assert_eq!(
                budget_recommendation(&points, &frontier, bad),
                None,
                "budget {bad}"
            );
        }
    }

    #[test]
    fn render_budget_line_names_a_row_that_is_both_marked_and_affordable() {
        let labels = vec![
            "a/pricey (codegen/pro)".to_string(),
            "b/cheap (codegen/pro)".to_string(),
        ];
        let points = vec![
            pareto_point_for(Some(&score(95, 100, Some(0.10), Some(400)))),
            pareto_point_for(Some(&score(85, 100, Some(0.02), Some(400)))),
        ];
        let frontier = pareto_frontier(&points);
        let line = render_budget_line(&labels, &points, &frontier, 0.05);
        assert!(line.contains("b/cheap"), "{line}");
        assert!(
            !line.contains("a/pricey"),
            "must not name the over-budget row: {line}"
        );
    }

    #[test]
    fn render_budget_line_says_nothing_qualifies_rather_than_naming_an_overbudget_row() {
        let labels = vec!["a/pricey (codegen/pro)".to_string()];
        let points = vec![pareto_point_for(Some(&score(
            95,
            100,
            Some(0.10),
            Some(400),
        )))];
        let frontier = pareto_frontier(&points);
        let line = render_budget_line(&labels, &points, &frontier, 0.001);
        assert!(
            !line.contains("a/pricey"),
            "must not name an unaffordable row: {line}"
        );
        assert!(line.contains("no Pareto-optimal row qualifies"), "{line}");
    }

    #[test]
    fn render_budget_line_disambiguates_rows_of_the_same_model() {
        // A scoreboard row is a (model, category, strength) triple; the same model appears more
        // than once. The recommendation must say which row it means.
        let labels = vec![
            "m/x (codegen/pro)".to_string(),
            "m/x (research/pro)".to_string(),
        ];
        let points = vec![
            pareto_point_for(Some(&score(50, 100, Some(0.90), Some(400)))),
            pareto_point_for(Some(&score(95, 100, Some(0.02), Some(100)))),
        ];
        let frontier = pareto_frontier(&points);
        let line = render_budget_line(&labels, &points, &frontier, 1.0);
        assert!(
            line.contains("research/pro"),
            "must name the winning row's category: {line}"
        );
    }

    #[test]
    fn pareto_legend_states_the_strictly_better_clause_and_avoids_the_word_quality() {
        // "at least as good on all axes" alone is wrong: identical rows are each at least as good
        // as the other, yet neither is dominated, so both are marked.
        let legend = pareto_legend();
        assert!(legend.contains("strictly better"), "{legend}");
        // Global Constraint, enforced mechanically rather than by prose: the axis is reliability.
        assert!(!legend.to_lowercase().contains("quality"), "{legend}");
    }

    #[test]
    fn recommendable_positions_excludes_rows_the_table_itself_refuses_to_vouch_for() {
        // 0: qualifies. 1: low-N — the table prints "(low-N)" and withholds `*`. 2: only 9
        // successes — the table prints "insufficient data" in the very cell the budget matches on.
        let scores = vec![
            score(20, 40, Some(0.01), Some(100)),
            score(20, MIN_CALLS_FOR_CONFIDENT_RANK - 1, Some(0.01), Some(100)),
            score(
                COST_PER_SUCCESS_MIN_SUCCESSES - 1,
                40,
                Some(0.01),
                Some(100),
            ),
        ];
        assert_eq!(recommendable_positions(&scores, &[0, 1, 2]), vec![0]);
        assert_eq!(
            recommendable_positions(&scores, &[1, 2]),
            Vec::<usize>::new(),
            "no frontier row clears both gates"
        );
    }

    #[test]
    fn budget_line_declines_when_only_sub_threshold_rows_are_affordable() {
        // The F1 defect: a cheap low-N row was named "Pareto-optimal" in prose beneath a table
        // that pointedly did not mark it.
        let labels = vec![
            "a/rich (codegen/pro)".to_string(),
            "b/low-n (codegen/pro)".to_string(),
        ];
        let scores = vec![
            score(20, 40, Some(0.90), Some(100)),
            score(2, 2, Some(0.01), Some(100)),
        ];
        let points: Vec<ParetoPoint> = scores.iter().map(|s| pareto_point_for(Some(s))).collect();
        let frontier = pareto_frontier(&points);
        assert!(frontier.contains(&1), "precondition: low-N row is on it");

        let recommendable = recommendable_positions(&scores, &frontier);
        let line = render_budget_line(&labels, &points, &recommendable, 0.05);
        assert!(!line.contains("b/low-n"), "{line}");
        assert!(line.contains("no Pareto-optimal row qualifies"), "{line}");
    }

    #[test]
    fn budget_flag_parses_and_defaults_to_none() {
        use clap::Parser as _;
        let with =
            ScoreboardArgs::try_parse_from(["scoreboard", "--budget", "0.05"]).expect("parses");
        assert_eq!(with.budget, Some(0.05));
        let without = ScoreboardArgs::try_parse_from(["scoreboard"]).expect("parses");
        assert_eq!(without.budget, None);
        assert!(ScoreboardArgs::try_parse_from(["scoreboard", "--budget", "abc"]).is_err());
    }
}
