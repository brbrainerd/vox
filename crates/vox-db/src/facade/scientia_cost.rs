//! DB queries that back `vox scientia cost` / `CostInputs`.
//!
//! # What we can source from the live schema
//!
//! The scientia DB schema has two cost-bearing tables:
//!
//! * **`agent_telemetry_flat`** (`event_kind = 'cost'`) — rows recorded by
//!   `vox-actor-runtime` whenever an LLM call completes.  Each row carries:
//!   `provider TEXT`, `cost_usd REAL`, `recorded_at_ms INTEGER`.
//!   These rows are the most direct source of real spend.
//!
//! * **`model_pricing_catalog`** — a pricing SSOT (input/output $/1k tokens +
//!   observed blended rate).  Used by orchestrator for *budget forecasting*, not
//!   actuals.  We do NOT use it here because it does not hold per-run totals.
//!
//! # What we CANNOT source yet
//!
//! The schema does not tag individual telemetry rows with a Scientia pipeline
//! phase (extraction / critic / novelty-retrieval / scholarly-submission).
//! Therefore `extraction_usd`, `critic_usd`, `novelty_retrieval_usd`, and
//! `scholarly_submission_usd` are all set to **0.0** in this implementation.
//! They will be populated once `agent_telemetry_flat` gains a `pipeline_phase`
//! column (tracked in the Phase 0d roadmap).
//!
//! # What this implementation *does* provide
//!
//! * `by_provider` — real per-provider cost totals for the current calendar
//!   quarter sourced from `agent_telemetry_flat` `cost` rows.
//! * `findings_published_this_quarter` — count of publication manifests whose
//!   `state = 'published'` and `updated_at_ms` falls in the current quarter.

use crate::{StoreError, VoxDb};
use turso::params;

/// Raw per-provider cost row returned from the DB.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCostRow {
    pub provider: String,
    pub total_usd: f64,
}

impl VoxDb {
    /// Fetch per-provider cost totals from `agent_telemetry_flat` for the
    /// current calendar quarter (`recorded_at_ms` in `[quarter_start_ms,
    /// quarter_end_ms)`).  Only `event_kind = 'cost'` rows are included.
    pub async fn scientia_cost_by_provider(
        &self,
        quarter_start_ms: i64,
        quarter_end_ms: i64,
    ) -> Result<Vec<ProviderCostRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT provider, SUM(COALESCE(cost_usd, 0.0)) AS total_usd \
                 FROM agent_telemetry_flat \
                 WHERE event_kind = 'cost' \
                   AND recorded_at_ms >= ?1 \
                   AND recorded_at_ms < ?2 \
                   AND provider IS NOT NULL \
                 GROUP BY provider \
                 ORDER BY total_usd DESC",
                params![quarter_start_ms, quarter_end_ms],
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
        {
            let provider: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let total_usd: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(ProviderCostRow { provider, total_usd });
        }
        Ok(out)
    }

    /// Count publication manifests with `state = 'published'` whose
    /// `updated_at_ms` falls in `[quarter_start_ms, quarter_end_ms)`.
    pub async fn scientia_published_findings_count(
        &self,
        quarter_start_ms: i64,
        quarter_end_ms: i64,
    ) -> Result<u64, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM publication_manifests \
                 WHERE state = 'published' \
                   AND updated_at_ms >= ?1 \
                   AND updated_at_ms < ?2",
                params![quarter_start_ms, quarter_end_ms],
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        if let Some(row) = rows
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
        {
            let n: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            Ok(n.max(0) as u64)
        } else {
            Ok(0)
        }
    }

    /// Assemble the raw cost data for the current calendar quarter.
    ///
    /// Returns `(by_provider, findings_count)` where:
    /// * `by_provider` — per-provider `(name, total_usd)` pairs sourced from
    ///   `agent_telemetry_flat` `cost` rows.
    /// * `findings_count` — count of `publication_manifests` with
    ///   `state='published'` whose `updated_at_ms` is in the quarter window.
    ///
    /// The caller (the CLI handler) assembles these into a `CostInputs` and
    /// sets the four category lines to 0.0 (see module doc for the reason).
    pub async fn scientia_cost_raw_this_quarter(
        &self,
    ) -> Result<(Vec<ProviderCostRow>, u64), StoreError> {
        let (quarter_start_ms, quarter_end_ms) = current_quarter_window_ms();
        let provider_rows = self
            .scientia_cost_by_provider(quarter_start_ms, quarter_end_ms)
            .await?;
        let findings = self
            .scientia_published_findings_count(quarter_start_ms, quarter_end_ms)
            .await?;
        Ok((provider_rows, findings))
    }
}

/// Return `(quarter_start_ms, quarter_end_ms)` for the calendar quarter that
/// contains `now` (UTC).  Quarters are Jan–Mar / Apr–Jun / Jul–Sep / Oct–Dec.
pub fn current_quarter_window_ms() -> (i64, i64) {
    use chrono::{Datelike, TimeZone, Utc};
    let now = Utc::now();
    let year = now.year();
    let quarter = (now.month0() / 3) as i32; // 0-based quarter index
    let start_month = (quarter * 3 + 1) as u32;
    let end_month = start_month + 3;
    let start = Utc
        .with_ymd_and_hms(year, start_month, 1, 0, 0, 0)
        .single()
        .expect("valid quarter start");
    let end = if end_month > 12 {
        Utc.with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0)
            .single()
            .expect("valid next year start")
    } else {
        Utc.with_ymd_and_hms(year, end_month, 1, 0, 0, 0)
            .single()
            .expect("valid quarter end")
    };
    (start.timestamp_millis(), end.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the quarter window helper returns a non-empty, ordered range.
    #[test]
    fn quarter_window_is_ordered_and_nonzero() {
        let (start, end) = current_quarter_window_ms();
        assert!(start < end, "quarter start must precede end");
        // A quarter is roughly 90 days; verify it's at least 80 days and at
        // most 95 days wide.
        let days = (end - start) / (1000 * 60 * 60 * 24);
        assert!(days >= 80 && days <= 95, "unexpected quarter width: {days} days");
    }

    /// Mapping `ProviderCostRow` → `(String, f64)` tuple preserves values.
    #[test]
    fn provider_cost_row_maps_to_tuple() {
        let rows = vec![
            ProviderCostRow {
                provider: "anthropic".into(),
                total_usd: 3.50,
            },
            ProviderCostRow {
                provider: "openai".into(),
                total_usd: 1.25,
            },
        ];
        let tuples: Vec<(String, f64)> =
            rows.into_iter().map(|r| (r.provider, r.total_usd)).collect();
        assert_eq!(tuples[0], ("anthropic".into(), 3.50));
        assert_eq!(tuples[1], ("openai".into(), 1.25));
    }
}
