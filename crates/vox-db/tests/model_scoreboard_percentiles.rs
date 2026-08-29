//! Regression: `rollup_model_scoreboard` must write real percentiles.
//!
//! Before Task M0, `p50_latency_ms` was `AVG(latency_ms)` and `p99_latency_ms`
//! was `MAX(latency_ms)` — neither is a percentile. With the skewed sample
//! below the old code reported p50 = 64.8 (mean) and p99 = 5000 (max) where the
//! true nearest-rank values are 10 and 500.

use tempfile::tempdir;
use vox_db::VoxDb;
use vox_db::store::types::ModelOutcome;

async fn record(db: &VoxDb, latency_ms: i64) {
    db.record_llm_outcome(ModelOutcome {
        session_id: "sess_pct",
        user_id: None,
        tenant_id: None,
        prompt: "p",
        response: "r",
        model_id: "test/model",
        provider: "test",
        task_category: "general",
        strength_tag: "generalist",
        latency_ms: Some(latency_ms),
        input_tokens: Some(1),
        output_tokens: Some(1),
        cache_read_tokens: None,
        trace_id: None,
        context_utilization_pct: None,
        success: true,
        cost_usd: Some(0.01),
        quality_score: None,
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn rollup_writes_nearest_rank_percentiles_not_avg_and_max() {
    let dir = tempdir().unwrap();
    let db = VoxDb::open(dir.path().join("pct.db").to_str().unwrap())
        .await
        .unwrap();

    // 98 fast calls, one slow, one pathological.
    for _ in 0..98 {
        record(&db, 10).await;
    }
    record(&db, 500).await;
    record(&db, 5000).await;

    let n = db.rollup_model_scoreboard(7).await.unwrap();
    assert!(n > 0, "rollup wrote no rows");

    let row = db
        .get_model_scoreboard(7)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.model_id == "test/model")
        .expect("scoreboard row for test/model");

    assert_eq!(row.n_calls, 100);
    // Mean is 64.8, max is 5000 — both wrong.
    assert_eq!(
        row.p50_latency_ms,
        Some(10),
        "p50 must be the median, not AVG"
    );
    assert_eq!(
        row.p99_latency_ms,
        Some(500),
        "p99 must be the 99th percentile, not MAX"
    );
}
