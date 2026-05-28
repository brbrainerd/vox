//! Rolling aggregates from `research_metrics` → neutral [`vox_search::SearchPolicyFeedback`].

use vox_db::Codex;
use vox_search::SearchPolicyFeedback;

const DEFAULT_ROLLUP_SAMPLE: i64 = 24;

fn avg_metric_values(rows: &[(String, Option<f64>, Option<String>)]) -> Option<f64> {
    let vals: Vec<f64> = rows.iter().filter_map(|(_, mv, _)| *mv).collect();
    if vals.is_empty() {
        None
    } else {
        Some(vals.iter().sum::<f64>() / vals.len() as f64)
    }
}

/// Load a small rolling window from durable research metrics (best-effort).
///
/// Returns [`None`] when there is no usable history so callers keep env-default policy only.
pub async fn load_rolling_search_policy_feedback(db: &Codex) -> Option<SearchPolicyFeedback> {
    let lim = DEFAULT_ROLLUP_SAMPLE;
    let cit = db
        .list_research_metrics_by_type("citation_precision", "", lim)
        .await
        .ok()?;
    let ver = db
        .list_research_metrics_by_type("self_verification_reliability", "", lim)
        .await
        .ok()?;
    let hit = db
        .list_research_metrics_by_type("retrieval_hit_rate", "", lim)
        .await
        .ok()?;

    if cit.is_empty() && ver.is_empty() && hit.is_empty() {
        return None;
    }

    let citation_precision = avg_metric_values(&cit).unwrap_or(0.75);
    let model_reliability = avg_metric_values(&ver).unwrap_or(0.75);
    let source_hit_rate = avg_metric_values(&hit).unwrap_or(0.75);

    Some(SearchPolicyFeedback {
        citation_precision,
        model_reliability,
        source_hit_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_db::DbConfig;
    use vox_db::VoxDb;

    #[tokio::test]
    async fn rollup_derives_from_prior_metrics() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory db");
        db.record_research_metric(999, "citation_precision", 1.0, Some("{}"))
            .await
            .expect("row");
        db.record_research_metric(999, "retrieval_hit_rate", 0.25, Some("{}"))
            .await
            .expect("row2");

        let fb = load_rolling_search_policy_feedback(&db)
            .await
            .expect("feedback");
        assert!((fb.citation_precision - 1.0).abs() < f64::EPSILON);
        assert!((fb.source_hit_rate - 0.25).abs() < 1e-9);
    }
}
