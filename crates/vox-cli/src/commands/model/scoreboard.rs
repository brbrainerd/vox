use clap::Parser;
use comfy_table::Table;
use owo_colors::OwoColorize;
use vox_db::{DbConfig, VoxDb};

/// Show the model scoreboard.
#[derive(Parser)]
pub struct ScoreboardArgs {
    /// Time window in days (default: 7).
    #[arg(long, default_value_t = 7)]
    pub window: i64,
    /// Output format (default: table).
    #[arg(long, default_value = "table")]
    pub format: String,
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
    if n_calls < vox_orchestrator::models::MIN_CALLS_FOR_CONFIDENT_RANK {
        format!("{colored} (low-N)")
    } else {
        colored
    }
}

pub async fn run(args: ScoreboardArgs) -> anyhow::Result<()> {
    let db_config = DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?;
    let db = VoxDb::connect(db_config).await?;

    let rows = db.get_model_scoreboard(args.window).await?;

    if args.format == "json" {
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

    for row in rows {
        let success_cell = success_rate_cell(row.success_rate, row.n_calls);

        table.add_row(vec![
            row.model_id,
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
}
