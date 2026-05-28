//! Socrates-telemetry signal producer.
//!
//! Reads `agent_telemetry_flat` rows of `event_kind = 'trust_obs'` and emits
//! a `telemetry_trust` candidate when the trailing trust-score window shows
//! a sustained improvement (mean trust delta ≥ [`MIN_TRUST_IMPROVEMENT`])
//! with both windows containing at least [`MIN_SAMPLES_PER_WINDOW`] samples.
//!
//! The producer is dormant until there's enough trust-observation traffic.
//! Production wiring of trust events lives in the orchestrator and pre-dates
//! this phase.

use async_trait::async_trait;
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "socrates_telemetry";
/// Minimum trust observations per window.
pub const MIN_SAMPLES_PER_WINDOW: usize = 30;
/// Minimum mean trust-score improvement (`trailing - prior`) to emit.
pub const MIN_TRUST_IMPROVEMENT: f64 = 0.05;

pub struct SocratesTelemetryProducer {
    codex: vox_db::VoxDb,
}

impl SocratesTelemetryProducer {
    pub fn new(codex: vox_db::VoxDb) -> Self {
        Self { codex }
    }
}

#[async_trait]
impl Producer for SocratesTelemetryProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        let scores = match list_trust_scores(&self.codex).await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(error = %e, "socrates_telemetry: query failed");
                return Vec::new();
            }
        };
        if scores.len() < 2 * MIN_SAMPLES_PER_WINDOW {
            return Vec::new();
        }
        let mid = scores.len() / 2;
        let prior = &scores[..mid];
        let trailing = &scores[mid..];
        if prior.len() < MIN_SAMPLES_PER_WINDOW || trailing.len() < MIN_SAMPLES_PER_WINDOW {
            return Vec::new();
        }
        let prior_mean: f64 = prior.iter().sum::<f64>() / prior.len() as f64;
        let trailing_mean: f64 = trailing.iter().sum::<f64>() / trailing.len() as f64;
        let delta = trailing_mean - prior_mean;
        if delta < MIN_TRUST_IMPROVEMENT {
            return Vec::new();
        }

        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::trust::");
        h.update(scores.len().to_le_bytes());
        let digest = h.finalize();
        let sha7: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let slug = date_slug(ctx.now_ms);
        vec![ResearchEvent::FindingCandidateProposed {
            finding_id: format!("teltr-{slug}-trust-{sha7}"),
            claim_ids: vec![],
            worthiness_score: delta.clamp(0.0, 1.0),
            session_id: ctx.session_id.clone(),
        }]
    }
}

async fn list_trust_scores(codex: &vox_db::VoxDb) -> Result<Vec<f64>, vox_db::StoreError> {
    let mut rows = codex
        .connection()
        .query(
            "SELECT trust_score FROM agent_telemetry_flat \
             WHERE event_kind = 'trust_obs' AND trust_score IS NOT NULL \
             ORDER BY recorded_at_ms ASC",
            (),
        )
        .await
        .map_err(vox_db::StoreError::Turso)?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await.map_err(vox_db::StoreError::Turso)? {
        let v: Option<f64> = r.get(0).map_err(vox_db::StoreError::Turso)?;
        if let Some(v) = v {
            out.push(v);
        }
    }
    Ok(out)
}
