//! Benchmark-history signal producer.
//!
//! Reads `agent_telemetry_flat` rows of `event_kind = 'exec'`, groups by
//! `tool_name`, and emits an `algorithmic_improvement` candidate when the
//! trailing window's p95 latency drops by ≥ [`MIN_IMPROVEMENT`] vs the prior
//! window, with each window containing at least [`MIN_SAMPLES_PER_WINDOW`]
//! observations.
//!
//! Determinism: candidates are seeded by `(tool_name, window_count_pair)` so
//! a stable benchmark history produces stable ids across runs.

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::{date_slug, fractional_improvement, p95};
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "bench_history";
/// Minimum samples in each window for the comparison to be considered
/// statistically meaningful.
pub const MIN_SAMPLES_PER_WINDOW: usize = 30;
/// Minimum fractional p95 improvement (≥ 20%) to emit a candidate.
pub const MIN_IMPROVEMENT: f64 = 0.20;

pub struct BenchHistoryProducer {
    codex: vox_db::VoxDb,
}

impl BenchHistoryProducer {
    pub fn new(codex: vox_db::VoxDb) -> Self {
        Self { codex }
    }
}

#[async_trait]
impl Producer for BenchHistoryProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        let tools = match list_exec_tool_names(&self.codex).await {
            Ok(t) => t,
            Err(e) => {
                tracing::debug!(error = %e, "bench_history: list_exec_tool_names failed");
                return Vec::new();
            }
        };
        let mut out = Vec::new();
        let slug = date_slug(ctx.now_ms);
        for tool in tools {
            let durations = match list_exec_durations_for_tool(&self.codex, &tool).await {
                Ok(d) => d,
                Err(e) => {
                    tracing::debug!(tool=%tool, error=%e, "bench_history: query failed");
                    continue;
                }
            };
            // Need at least 2 windows worth of samples.
            if durations.len() < 2 * MIN_SAMPLES_PER_WINDOW {
                continue;
            }
            // Split into two halves, prior then trailing.
            let mid = durations.len() / 2;
            let prior = &durations[..mid];
            let trailing = &durations[mid..];
            if prior.len() < MIN_SAMPLES_PER_WINDOW || trailing.len() < MIN_SAMPLES_PER_WINDOW {
                continue;
            }
            let (Some(prior_p95), Some(trailing_p95)) = (p95(prior), p95(trailing)) else {
                continue;
            };
            let improvement = fractional_improvement(prior_p95 as f64, trailing_p95 as f64);
            if improvement < MIN_IMPROVEMENT {
                continue;
            }
            let mut h = Sha3_256::new();
            h.update(PRODUCER_NAME.as_bytes());
            h.update(b"::");
            h.update(tool.as_bytes());
            let digest = h.finalize();
            let sha7: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
            let finding_id = format!("algimp-{slug}-bench-{sha7}");
            // Cap worthiness_score within [0, 1].
            let worthiness_score = improvement.clamp(0.0, 1.0);
            out.push(ResearchEvent::FindingCandidateProposed {
                finding_id,
                claim_ids: vec![],
                worthiness_score,
                session_id: ctx.session_id.clone(),
            });
        }
        out
    }
}

async fn list_exec_tool_names(codex: &vox_db::VoxDb) -> Result<Vec<String>, vox_db::StoreError> {
    let mut rows = codex
        .connection()
        .query(
            "SELECT DISTINCT tool_name FROM agent_telemetry_flat \
             WHERE event_kind = 'exec' AND tool_name IS NOT NULL",
            (),
        )
        .await
        .map_err(vox_db::StoreError::Turso)?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.map_err(vox_db::StoreError::Turso)? {
        let name: Option<String> = r.get(0).map_err(vox_db::StoreError::Turso)?;
        if let Some(n) = name {
            out.push(n);
        }
    }
    Ok(out)
}

async fn list_exec_durations_for_tool(
    codex: &vox_db::VoxDb,
    tool: &str,
) -> Result<Vec<u64>, vox_db::StoreError> {
    let mut rows = codex
        .connection()
        .query(
            "SELECT duration_ms FROM agent_telemetry_flat \
             WHERE event_kind = 'exec' AND tool_name = ?1 AND duration_ms IS NOT NULL \
             ORDER BY recorded_at_ms ASC",
            turso::params![tool.to_string()],
        )
        .await
        .map_err(vox_db::StoreError::Turso)?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.map_err(vox_db::StoreError::Turso)? {
        let d: Option<i64> = r.get(0).map_err(vox_db::StoreError::Turso)?;
        if let Some(d) = d {
            if d >= 0 {
                out.push(d as u64);
            }
        }
    }
    Ok(out)
}
