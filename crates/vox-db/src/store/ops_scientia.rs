//! Telemetry and Scoreboard operations for [`crate::VoxDb`] (Arca / Turso).

use crate::store::types::{ModelScoreboardRow, StoreError};
use turso::params;

impl crate::VoxDb {
    /// Retrieve the current model scoreboard for a specific window.
    pub async fn get_model_scoreboard(
        &self,
        window_days: i64,
    ) -> Result<Vec<ModelScoreboardRow>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    let mut rows = conn
                        .query(
                            "SELECT
                                model_id, task_category, strength_tag, window_days,
                                n_calls, success_rate, p50_latency_ms, p99_latency_ms,
                                cost_per_success_usd, quality_score, updated_at_ms,
                                success_count, cumulative_cost_usd,
                                p95_ttft_ms, p95_tpot_ms, goodput_tokens_per_sec
                             FROM model_scoreboard
                             WHERE window_days = ?1",
                            params![window_days],
                        )
                        .await?;

                    let mut out = Vec::new();
                    while let Some(row) = rows.next().await? {
                        out.push(ModelScoreboardRow {
                            model_id: row.get(0)?,
                            task_category: row.get(1)?,
                            strength_tag: row.get(2)?,
                            window_days: row.get(3)?,
                            n_calls: row.get(4)?,
                            success_rate: row.get(5)?,
                            p50_latency_ms: row.get(6)?,
                            p99_latency_ms: row.get(7)?,
                            cost_per_success_usd: row.get(8)?,
                            quality_score: row.get(9)?,
                            updated_at_ms: row.get(10)?,
                            success_count: row.get(11)?,
                            cumulative_cost_usd: row.get(12)?,
                            p95_ttft_ms: row.get(13)?,
                            p95_tpot_ms: row.get(14)?,
                            goodput_tokens_per_sec: row.get(15)?,
                        });
                    }
                    Ok::<_, StoreError>(out)
                }
            })
            .await
    }

    /// Aggregate Thompson arm stats `(successes, failures)` per `model_id` for a window.
    pub async fn list_model_arm_stats(
        &self,
        window_days: i64,
    ) -> Result<std::collections::HashMap<String, (u32, u32)>, StoreError> {
        let rows = self.get_model_scoreboard(window_days).await?;
        let mut map = std::collections::HashMap::<String, (u32, u32)>::new();
        for r in rows {
            let n = r.n_calls.max(0) as u64;
            if n == 0 {
                continue;
            }
            let sr = r.success_rate.clamp(0.0, 1.0);
            let successes_u64 = (sr * n as f64).round() as u64;
            let successes = successes_u64.min(n).min(u32::MAX as u64) as u32;
            let failures = (n.min(u32::MAX as u64) as u32).saturating_sub(successes);
            let e = map.entry(r.model_id).or_insert((0, 0));
            e.0 = e.0.saturating_add(successes);
            e.1 = e.1.saturating_add(failures);
        }
        Ok(map)
    }

    /// Upsert a model scoreboard entry.
    pub async fn upsert_model_scoreboard(&self, row: ModelScoreboardRow) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    conn.execute(
                        "INSERT INTO model_scoreboard (
                            model_id, task_category, strength_tag, window_days,
                            n_calls, success_rate, p50_latency_ms, p99_latency_ms,
                            cost_per_success_usd, quality_score, updated_at_ms,
                            success_count, cumulative_cost_usd,
                            p95_ttft_ms, p95_tpot_ms, goodput_tokens_per_sec
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                         ON CONFLICT(model_id, task_category, strength_tag, window_days) DO UPDATE SET
                            n_calls = excluded.n_calls,
                            success_rate = excluded.success_rate,
                            p50_latency_ms = excluded.p50_latency_ms,
                            p99_latency_ms = excluded.p99_latency_ms,
                            cost_per_success_usd = excluded.cost_per_success_usd,
                            quality_score = excluded.quality_score,
                            updated_at_ms = excluded.updated_at_ms,
                            success_count = excluded.success_count,
                            cumulative_cost_usd = excluded.cumulative_cost_usd,
                            p95_ttft_ms = excluded.p95_ttft_ms,
                            p95_tpot_ms = excluded.p95_tpot_ms,
                            goodput_tokens_per_sec = excluded.goodput_tokens_per_sec",
                        params![
                            row.model_id.as_str(),
                            row.task_category.as_str(),
                            row.strength_tag.as_str(),
                            row.window_days,
                            row.n_calls,
                            row.success_rate,
                            row.p50_latency_ms,
                            row.p99_latency_ms,
                            row.cost_per_success_usd,
                            row.quality_score,
                            row.updated_at_ms,
                            row.success_count,
                            row.cumulative_cost_usd,
                            row.p95_ttft_ms,
                            row.p95_tpot_ms,
                            row.goodput_tokens_per_sec,
                        ],
                    )
                    .await?;
                    Ok(())
                }
            })
            .await
    }

    /// Perform a batch rollup of telemetry into the scoreboard for a specific window.
    pub async fn rollup_model_scoreboard(&self, window_days: i64) -> Result<usize, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    // This query aggregates interactions and joins feedback (averaging ratings if present)
                    // rating is 0..1 (binary) or 0..5 (thumbs/stars), we normalize it to quality.
                    //
                    // p50/p99 used to be `AVG(latency_ms)` and `MAX(latency_ms)` — neither is a
                    // percentile, and the REAL that `AVG` wrote into the INTEGER
                    // `p50_latency_ms` column also made `get_model_scoreboard` panic inside
                    // turso on read-back. They are inserted NULL here and filled by the
                    // nearest-rank pass below; turso 0.6 rejects the correlated subquery that
                    // would compute them in this statement ("no such table: s").
                    //
                    // WARNING (Task M2): `quality_score` below is NOT a quality signal. It is
                    // `AVG(llm_feedback.rating)/5.0` with a `COALESCE(..., 1.0)` default, and
                    // `llm_feedback` has zero rows in practice — so every model scores a flat
                    // 1.0. The other writer (`record_llm_outcome`) defaults it to 1.0 too, and
                    // the only caller that sets it passed `success ? 1.0 : 0.0`. Do not rank,
                    // render, or reward on this column until M2 replaces its definition.
                    let sql = format!(
                        "INSERT INTO model_scoreboard (
                            model_id, task_category, strength_tag, window_days,
                            n_calls, success_rate, p50_latency_ms, p99_latency_ms,
                            cost_per_success_usd, quality_score, updated_at_ms,
                            success_count, cumulative_cost_usd,
                            p95_ttft_ms, p95_tpot_ms, goodput_tokens_per_sec
                        )
                        WITH interaction_stats AS (
                            SELECT
                                id,
                                model_version,
                                task_category,
                                strength_tag,
                                success,
                                latency_ms,
                                cost_usd,
                                output_tokens
                            FROM llm_interactions
                            WHERE created_at >= datetime('now', '-{} days')
                        ),
                        feedback_agg AS (
                            SELECT interaction_id, AVG(rating) as rating
                            FROM llm_feedback
                            GROUP BY interaction_id
                        )
                        SELECT
                            s.model_version,
                            s.task_category,
                            s.strength_tag,
                            ?1,
                            COUNT(*),
                            AVG(CAST(s.success AS REAL)),
                            NULL,
                            NULL,
                            SUM(s.cost_usd) / NULLIF(SUM(s.success), 0) as cost_per_success_usd,
                            COALESCE(AVG(CAST(f.rating AS REAL) / 5.0), 1.0),
                            ?2,
                            SUM(s.success),
                            COALESCE(SUM(s.cost_usd), 0.0),
                            NULL,
                            NULL,
                            -- Task M3: mean successful-call throughput. Guards latency_ms > 0
                            -- (avoid div-by-zero) and output_tokens > 0 (a call with no output
                            -- tokens has no meaningful tokens/sec, not a real 0).
                            AVG(CASE
                                WHEN s.success = 1 AND s.latency_ms > 0 AND s.output_tokens > 0
                                THEN CAST(s.output_tokens AS REAL) / (s.latency_ms / 1000.0)
                                ELSE NULL
                            END)
                        FROM interaction_stats s
                        LEFT JOIN feedback_agg f ON s.id = f.interaction_id
                        GROUP BY s.model_version, s.task_category, s.strength_tag
                        ON CONFLICT(model_id, task_category, strength_tag, window_days) DO UPDATE SET
                            n_calls = excluded.n_calls,
                            success_rate = excluded.success_rate,
                            p50_latency_ms = excluded.p50_latency_ms,
                            p99_latency_ms = excluded.p99_latency_ms,
                            cost_per_success_usd = excluded.cost_per_success_usd,
                            quality_score = excluded.quality_score,
                            updated_at_ms = excluded.updated_at_ms,
                            success_count = excluded.success_count,
                            cumulative_cost_usd = excluded.cumulative_cost_usd,
                            p95_ttft_ms = excluded.p95_ttft_ms,
                            p95_tpot_ms = excluded.p95_tpot_ms,
                            goodput_tokens_per_sec = excluded.goodput_tokens_per_sec",
                        window_days
                    );

                    let affected = conn.execute(&sql, params![window_days, now_ms]).await?;

                    // Nearest-rank percentile pass. Streams the window's latencies grouped and
                    // sorted by the SQL engine, so only one group is held in memory at a time.
                    // ponytail: one UPDATE per group; batch it if group counts ever get large.
                    let mut rows = conn
                        .query(
                            &format!(
                                "SELECT model_version, task_category, strength_tag, latency_ms
                                   FROM llm_interactions
                                  WHERE created_at >= datetime('now', '-{window_days} days')
                                    AND latency_ms IS NOT NULL
                                  ORDER BY model_version, task_category, strength_tag, latency_ms"
                            ),
                            (),
                        )
                        .await?;

                    let mut group: Option<(String, String, String)> = None;
                    let mut latencies: Vec<i64> = Vec::new();
                    let mut pending: Vec<((String, String, String), i64, i64)> = Vec::new();
                    while let Some(row) = rows.next().await? {
                        let key = (row.get(0)?, row.get(1)?, row.get(2)?);
                        let latency: i64 = row.get(3)?;
                        if group.as_ref() != Some(&key) {
                            if let (Some(k), Some((p50, p99))) =
                                (group.take(), percentiles_p50_p99(&latencies))
                            {
                                pending.push((k, p50, p99));
                            }
                            latencies.clear();
                            group = Some(key);
                        }
                        latencies.push(latency);
                    }
                    if let (Some(k), Some((p50, p99))) =
                        (group.take(), percentiles_p50_p99(&latencies))
                    {
                        pending.push((k, p50, p99));
                    }

                    for ((model_id, task_category, strength_tag), p50, p99) in pending {
                        conn.execute(
                            "UPDATE model_scoreboard
                                SET p50_latency_ms = ?1, p99_latency_ms = ?2
                              WHERE model_id = ?3 AND task_category = ?4
                                AND strength_tag = ?5 AND window_days = ?6",
                            params![
                                p50,
                                p99,
                                model_id.as_str(),
                                task_category.as_str(),
                                strength_tag.as_str(),
                                window_days
                            ],
                        )
                        .await?;
                    }

                    // Task M3: same nearest-rank streaming pass, for ttft_ms -- only rows
                    // where it was actually recorded (most callers don't measure it yet, see
                    // ModelOutcome's doc comment), so most groups here are smaller than the
                    // latency pass above.
                    let mut rows = conn
                        .query(
                            &format!(
                                "SELECT model_version, task_category, strength_tag, ttft_ms
                                   FROM llm_interactions
                                  WHERE created_at >= datetime('now', '-{window_days} days')
                                    AND ttft_ms IS NOT NULL
                                  ORDER BY model_version, task_category, strength_tag, ttft_ms"
                            ),
                            (),
                        )
                        .await?;
                    let mut group: Option<(String, String, String)> = None;
                    let mut values: Vec<i64> = Vec::new();
                    let mut pending: Vec<((String, String, String), i64)> = Vec::new();
                    while let Some(row) = rows.next().await? {
                        let key = (row.get(0)?, row.get(1)?, row.get(2)?);
                        let v: i64 = row.get(3)?;
                        if group.as_ref() != Some(&key) {
                            if let (Some(k), Some(p95)) =
                                (group.take(), percentile_p95_i64(&values))
                            {
                                pending.push((k, p95));
                            }
                            values.clear();
                            group = Some(key);
                        }
                        values.push(v);
                    }
                    if let (Some(k), Some(p95)) = (group.take(), percentile_p95_i64(&values)) {
                        pending.push((k, p95));
                    }
                    for ((model_id, task_category, strength_tag), p95) in pending {
                        conn.execute(
                            "UPDATE model_scoreboard
                                SET p95_ttft_ms = ?1
                              WHERE model_id = ?2 AND task_category = ?3
                                AND strength_tag = ?4 AND window_days = ?5",
                            params![
                                p95,
                                model_id.as_str(),
                                task_category.as_str(),
                                strength_tag.as_str(),
                                window_days
                            ],
                        )
                        .await?;
                    }

                    // Task M3: same pass again, for tpot_ms (REAL, not INTEGER).
                    let mut rows = conn
                        .query(
                            &format!(
                                "SELECT model_version, task_category, strength_tag, tpot_ms
                                   FROM llm_interactions
                                  WHERE created_at >= datetime('now', '-{window_days} days')
                                    AND tpot_ms IS NOT NULL
                                  ORDER BY model_version, task_category, strength_tag, tpot_ms"
                            ),
                            (),
                        )
                        .await?;
                    let mut group: Option<(String, String, String)> = None;
                    let mut values: Vec<f64> = Vec::new();
                    let mut pending: Vec<((String, String, String), f64)> = Vec::new();
                    while let Some(row) = rows.next().await? {
                        let key = (row.get(0)?, row.get(1)?, row.get(2)?);
                        let v: f64 = row.get(3)?;
                        if group.as_ref() != Some(&key) {
                            if let (Some(k), Some(p95)) =
                                (group.take(), percentile_p95_f64(&values))
                            {
                                pending.push((k, p95));
                            }
                            values.clear();
                            group = Some(key);
                        }
                        values.push(v);
                    }
                    if let (Some(k), Some(p95)) = (group.take(), percentile_p95_f64(&values)) {
                        pending.push((k, p95));
                    }
                    for ((model_id, task_category, strength_tag), p95) in pending {
                        conn.execute(
                            "UPDATE model_scoreboard
                                SET p95_tpot_ms = ?1
                              WHERE model_id = ?2 AND task_category = ?3
                                AND strength_tag = ?4 AND window_days = ?5",
                            params![
                                p95,
                                model_id.as_str(),
                                task_category.as_str(),
                                strength_tag.as_str(),
                                window_days
                            ],
                        )
                        .await?;
                    }

                    Ok(affected as usize)
                }
            })
            .await
    }

    /// Retrieve the most recent trace_id for a given task category.
    pub async fn get_last_interaction_trace_id(
        &self,
        task_category: &str,
    ) -> Result<Option<String>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let category = task_category.to_string();

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    let mut rows = conn
                        .query(
                            "SELECT trace_id
                             FROM llm_interactions
                             WHERE task_category = ?1 AND trace_id IS NOT NULL
                             ORDER BY created_at DESC
                             LIMIT 1",
                            params![category],
                        )
                        .await?;

                    if let Some(row) = rows.next().await? {
                        let tid: Option<String> = row.get(0)?;
                        Ok::<_, StoreError>(tid)
                    } else {
                        Ok::<_, StoreError>(None)
                    }
                }
            })
            .await
    }

    /// Retrieve the current model pricing catalog (confident rows).
    pub async fn get_pricing_catalog(
        &self,
    ) -> Result<Vec<crate::store::types::ModelPricingCatalogRow>, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    let mut rows = conn
                        .query(
                            "SELECT 
                                model_id, provider, observed_blended_per_1k, observed_input_per_1k, 
                                observed_output_per_1k, catalog_input_per_1k, catalog_output_per_1k, 
                                n_provider_reported, n_estimated, n_free, confidence, 
                                last_observed_at_ms, updated_at_ms
                             FROM model_pricing_catalog",
                            (),
                        )
                        .await?;

                    let mut out = Vec::new();
                    while let Some(row) = rows.next().await? {
                        out.push(crate::store::types::ModelPricingCatalogRow {
                            model_id: row.get(0)?,
                            provider: row.get(1)?,
                            observed_blended_per_1k: row.get(2)?,
                            observed_input_per_1k: row.get(3)?,
                            observed_output_per_1k: row.get(4)?,
                            catalog_input_per_1k: row.get(5)?,
                            catalog_output_per_1k: row.get(6)?,
                            n_provider_reported: row.get(7)?,
                            n_estimated: row.get(8)?,
                            n_free: row.get(9)?,
                            confidence: row.get(10)?,
                            last_observed_at_ms: row.get(11)?,
                            updated_at_ms: row.get(12)?,
                        });
                    }
                    Ok::<_, StoreError>(out)
                }
            })
            .await
    }

    /// Perform a batch rollup of telemetry into the pricing catalog.
    pub async fn rollup_pricing_catalog(&self) -> Result<usize, StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        // Ensure collection table exists before we try to select from it.
        if let Err(e) = self.collection("provider_usage").ensure_table().await {
            tracing::warn!(error = %e, "Failed to ensure provider_usage collection table exists");
        }

        breaker
            .call(move || {
                let conn = conn.clone();
                async move {
                    let sql = r#"
                        INSERT INTO model_pricing_catalog (
                            model_id, provider, observed_blended_per_1k, 
                            catalog_input_per_1k, catalog_output_per_1k,
                            n_provider_reported, n_estimated, n_free, confidence, 
                            last_observed_at_ms, updated_at_ms
                        )
                        WITH raw_usage AS (
                            SELECT 
                                json_extract(_data, '$.model') as model_id,
                                json_extract(_data, '$.provider') as provider,
                                CAST(json_extract(_data, '$.input_tokens') AS INTEGER) as input_tokens,
                                CAST(json_extract(_data, '$.output_tokens') AS INTEGER) as output_tokens,
                                CAST(json_extract(_data, '$.cost_usd') AS REAL) as cost_usd,
                                json_extract(_data, '$.cost_source') as cost_source,
                                CAST(json_extract(_data, '$.timestamp_ms') AS INTEGER) as timestamp_ms
                            FROM provider_usage
                        ),
                        agg_usage AS (
                            SELECT 
                                model_id,
                                provider,
                                SUM(CASE WHEN cost_source = 'provider_reported' AND cost_usd > 0.0 THEN cost_usd ELSE 0 END) as sum_reported_cost,
                                SUM(CASE WHEN cost_source = 'provider_reported' AND cost_usd > 0.0 THEN input_tokens + output_tokens ELSE 0 END) as sum_reported_tokens,
                                SUM(CASE WHEN cost_source = 'provider_reported' AND cost_usd > 0.0 THEN 1 ELSE 0 END) as n_provider_reported,
                                SUM(CASE WHEN cost_source = 'estimated' THEN 1 ELSE 0 END) as n_estimated,
                                SUM(CASE WHEN cost_source = 'provider_reported' AND cost_usd = 0.0 THEN 1 ELSE 0 END) as n_free,
                                MAX(timestamp_ms) as last_observed_at_ms
                            FROM raw_usage
                            WHERE model_id IS NOT NULL AND provider IS NOT NULL
                            GROUP BY model_id, provider
                        )
                        SELECT 
                            model_id,
                            provider,
                            CASE WHEN sum_reported_tokens > 0 THEN (sum_reported_cost / sum_reported_tokens) * 1000.0 ELSE NULL END as observed_blended_per_1k,
                            0.0 as catalog_input_per_1k,
                            0.0 as catalog_output_per_1k,
                            n_provider_reported,
                            n_estimated,
                            n_free,
                            CASE 
                                WHEN n_provider_reported >= 100 THEN 'high'
                                WHEN n_provider_reported >= 20 THEN 'medium'
                                ELSE 'low'
                            END as confidence,
                            last_observed_at_ms,
                            ?1 as updated_at_ms
                        FROM agg_usage
                        ON CONFLICT(model_id, provider) DO UPDATE SET
                            observed_blended_per_1k = excluded.observed_blended_per_1k,
                            n_provider_reported = excluded.n_provider_reported,
                            n_estimated = excluded.n_estimated,
                            n_free = excluded.n_free,
                            confidence = excluded.confidence,
                            last_observed_at_ms = excluded.last_observed_at_ms,
                            updated_at_ms = excluded.updated_at_ms
                    "#;

                    let changes = conn.execute(sql, turso::params![now_ms]).await?;
                    Ok::<_, StoreError>(changes as usize)
                }
            })
            .await
    }
}

/// Nearest-rank p50/p99 of an **ascending-sorted** latency slice.
///
/// Nearest rank: the `ceil(p * n)`-th element, 1-based. Returns `None` for an
/// empty slice.
fn percentiles_p50_p99(sorted: &[i64]) -> Option<(i64, i64)> {
    let n = sorted.len();
    if n == 0 {
        return None;
    }
    // Integer ceil(a/b) = (a + b - 1) / b.
    let p50_idx = n.div_ceil(2) - 1;
    let p99_idx = (99 * n).div_ceil(100) - 1;
    Some((sorted[p50_idx], sorted[p99_idx]))
}

/// Task M3: nearest-rank index shared by every percentile helper here — `ceil(pct * n /
/// 100)`-th element, 1-based, converted to a 0-based index.
fn nearest_rank_idx(n: usize, pct: usize) -> usize {
    (pct * n).div_ceil(100) - 1
}

/// Task M3: nearest-rank p95 of an **ascending-sorted** integer slice (used for
/// `p95_ttft_ms`). `None` for an empty slice.
fn percentile_p95_i64(sorted: &[i64]) -> Option<i64> {
    (!sorted.is_empty()).then(|| sorted[nearest_rank_idx(sorted.len(), 95)])
}

/// Task M3: nearest-rank p95 of an **ascending-sorted** float slice (used for
/// `p95_tpot_ms`). `None` for an empty slice.
fn percentile_p95_f64(sorted: &[f64]) -> Option<f64> {
    (!sorted.is_empty()).then(|| sorted[nearest_rank_idx(sorted.len(), 95)])
}

#[cfg(test)]
mod percentile_tests {
    use super::percentiles_p50_p99;

    #[test]
    fn empty_has_no_percentiles() {
        assert_eq!(percentiles_p50_p99(&[]), None);
    }

    #[test]
    fn single_sample_is_both_percentiles() {
        assert_eq!(percentiles_p50_p99(&[42]), Some((42, 42)));
    }

    #[test]
    fn skewed_sample_is_not_mean_or_max() {
        // 98 x 10ms, one 500ms, one 5000ms: mean 64.8, max 5000.
        let mut v = vec![10_i64; 98];
        v.push(500);
        v.push(5000);
        assert_eq!(percentiles_p50_p99(&v), Some((10, 500)));
    }

    #[test]
    fn uniform_1_to_100() {
        let v: Vec<i64> = (1..=100).collect();
        assert_eq!(percentiles_p50_p99(&v), Some((50, 99)));
    }
}
