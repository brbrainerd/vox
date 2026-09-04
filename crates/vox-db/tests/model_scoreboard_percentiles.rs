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
    record_full(db, latency_ms, None, None).await;
}

async fn record_full(db: &VoxDb, latency_ms: i64, ttft_ms: Option<i64>, tpot_ms: Option<f64>) {
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
        ttft_ms,
        tpot_ms,
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

#[tokio::test]
async fn rollup_writes_p95_ttft_tpot_and_goodput() {
    let dir = tempdir().unwrap();
    let db = VoxDb::open(dir.path().join("pct2.db").to_str().unwrap())
        .await
        .unwrap();

    // 18 calls with ttft/tpot recorded, one pathological outlier -- nearest-rank p95 of 19
    // values is ceil(0.95*19) = 19th (1-based, i.e. the last / the outlier itself).
    for _ in 0..18 {
        record_full(&db, 100, Some(20), Some(5.0)).await;
    }
    record_full(&db, 100, Some(900), Some(90.0)).await;
    // One call with no ttft/tpot recorded (most callers as of this column's introduction) --
    // must not break the aggregate or count toward the percentile.
    record(&db, 100).await;

    let n = db.rollup_model_scoreboard(7).await.unwrap();
    assert!(n > 0, "rollup wrote no rows");

    let row = db
        .get_model_scoreboard(7)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.model_id == "test/model")
        .expect("scoreboard row for test/model");

    assert_eq!(
        row.p95_ttft_ms,
        Some(900),
        "p95 must be the outlier, not an average"
    );
    assert_eq!(row.p95_tpot_ms, Some(90.0));
    // Every recorded call: output_tokens=1, latency_ms=100 -> 1 / 0.1 = 10.0 tokens/sec.
    assert_eq!(row.goodput_tokens_per_sec, Some(10.0));
}
