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
//! # Per-phase cost split
//!
//! `agent_telemetry_flat` now carries a nullable `pipeline_phase` column
//! (baseline v70). Cost rows written via
//! [`VoxDb::insert_scientia_cost_telemetry`](crate::VoxDb::insert_scientia_cost_telemetry)
//! tag the phase (`'extraction'` | `'critic'` | `'novelty'` | `'scholarly'`);
//! [`scientia_cost_by_phase`](VoxDb::scientia_cost_by_phase) groups by it so the
//! four category lines in `vox scientia cost` reflect real, attributed spend.
//!
//! Honesty note: a phase only shows non-zero cost once its call sites actually
//! write phase-tagged rows. As of this change the **extraction** phase is wired
//! at the `vox scientia publication-extract-claims` handler; `critic`,
//! `novelty`, and `scholarly` legitimately stay 0.0 until their LLM/cost sites
//! emit phase-tagged rows. The mechanism is real for all four — only the
//! emit-side wiring differs.
//!
//! # What this implementation *does* provide
//!
//! * `by_provider` — real per-provider cost totals for the current calendar
//!   quarter sourced from `agent_telemetry_flat` `cost` rows.
//! * `by_phase` — real per-phase cost totals (GROUP BY `pipeline_phase`).
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

/// Raw per-pipeline-phase cost row returned from the DB.
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseCostRow {
    /// Scientia pipeline phase: `'extraction'` | `'critic'` | `'novelty'` | `'scholarly'`.
    pub phase: String,
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
            out.push(ProviderCostRow {
                provider,
                total_usd,
            });
        }
        Ok(out)
    }

    /// Whether `table` currently has `column` (via `PRAGMA table_info`). Used to
    /// stay compatible with DBs created before an additive column landed.
    async fn scientia_table_has_column(
        &self,
        table: &str,
        column: &str,
    ) -> Result<bool, StoreError> {
        // `table` is a fixed internal literal at all call sites (no user input).
        let mut rows = self
            .conn
            .query(&format!("PRAGMA table_info({table})"), ())
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
        {
            // PRAGMA table_info columns: cid, name, type, notnull, dflt_value, pk.
            let name: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            if name == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Fetch per-pipeline-phase cost totals from `agent_telemetry_flat` for the
    /// window `[start_ms, end_ms)`. Only `event_kind = 'cost'` rows with a
    /// non-NULL `pipeline_phase` are included (rows tagged by
    /// [`insert_scientia_cost_telemetry`](Self::insert_scientia_cost_telemetry)).
    pub async fn scientia_cost_by_phase(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<PhaseCostRow>, StoreError> {
        // Tolerate DBs that predate the `pipeline_phase` column (baseline v70).
        // The column is added to fresh DBs by the baseline and to existing DBs by
        // `AutoMigrator` (`vox db` migrate); until then there are simply no
        // phase-tagged rows, so report none rather than failing the whole rollup.
        if !self
            .scientia_table_has_column("agent_telemetry_flat", "pipeline_phase")
            .await?
        {
            return Ok(Vec::new());
        }
        let mut rows = self
            .conn
            .query(
                "SELECT pipeline_phase, SUM(COALESCE(cost_usd, 0.0)) AS total_usd \
                 FROM agent_telemetry_flat \
                 WHERE event_kind = 'cost' \
                   AND recorded_at_ms >= ?1 \
                   AND recorded_at_ms < ?2 \
                   AND pipeline_phase IS NOT NULL \
                 GROUP BY pipeline_phase \
                 ORDER BY total_usd DESC",
                params![start_ms, end_ms],
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
        {
            let phase: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let total_usd: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push(PhaseCostRow { phase, total_usd });
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
    /// Returns `(by_provider, by_phase, findings_count)` where:
    /// * `by_provider` — per-provider `(name, total_usd)` pairs sourced from
    ///   `agent_telemetry_flat` `cost` rows.
    /// * `by_phase` — per-pipeline-phase totals (only phases with tagged rows).
    /// * `findings_count` — count of `publication_manifests` with
    ///   `state='published'` whose `updated_at_ms` is in the quarter window.
    ///
    /// The caller (the CLI handler) maps `phase_rows` onto the four category
    /// lines of `CostInputs` (see [`PhaseCostRow`]); uninstrumented phases are
    /// simply absent from the result and stay 0.0.
    pub async fn scientia_cost_raw_this_quarter(
        &self,
    ) -> Result<(Vec<ProviderCostRow>, Vec<PhaseCostRow>, u64), StoreError> {
        let (quarter_start_ms, quarter_end_ms) = current_quarter_window_ms();
        let provider_rows = self
            .scientia_cost_by_provider(quarter_start_ms, quarter_end_ms)
            .await?;
        let phase_rows = self
            .scientia_cost_by_phase(quarter_start_ms, quarter_end_ms)
            .await?;
        let findings = self
            .scientia_published_findings_count(quarter_start_ms, quarter_end_ms)
            .await?;
        Ok((provider_rows, phase_rows, findings))
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
        assert!(
            days >= 80 && days <= 95,
            "unexpected quarter width: {days} days"
        );
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
        let tuples: Vec<(String, f64)> = rows
            .into_iter()
            .map(|r| (r.provider, r.total_usd))
            .collect();
        assert_eq!(tuples[0], ("anthropic".into(), 3.50));
        assert_eq!(tuples[1], ("openai".into(), 1.25));
    }
}
