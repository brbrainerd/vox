//! MENS training-run signal producer.
//!
//! Reads `populi_training_run` rows and emits `algorithmic_improvement`
//! candidates when a run reaches `complete` status with checkpoint evidence.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use vox_research_events::ResearchEvent;

use super::heuristics::date_slug;
use super::producer::{Producer, ProducerContext};

const PRODUCER_NAME: &str = "mens_training_run";

/// Snapshot of one training run — mirrors [`vox_db::TrainingRunRecord`] fields
/// used by the detector (also deserialized from fixture JSON in tests).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MensTrainingRunSnapshot {
    pub run_id: String,
    pub status: String,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub global_step: u32,
    #[serde(default)]
    pub planned_steps: Option<u32>,
    #[serde(default)]
    pub last_loss: Option<f32>,
    #[serde(default)]
    pub last_checkpoint_path: Option<String>,
}

/// Map completed training runs to finding candidates (pure, testable).
#[must_use]
pub fn candidates_from_training_run_snapshots(
    runs: &[MensTrainingRunSnapshot],
    ctx: &ProducerContext,
) -> Vec<ResearchEvent> {
    let slug = date_slug(ctx.now_ms);
    let mut out = Vec::new();
    for run in runs {
        if run.status != "complete" {
            continue;
        }
        if run
            .last_checkpoint_path
            .as_deref()
            .is_none_or(str::is_empty)
        {
            continue;
        }
        let progress = match run.planned_steps.filter(|p| *p > 0) {
            Some(planned) => (f64::from(run.global_step) / f64::from(planned)).clamp(0.0, 1.0),
            None => 1.0,
        };
        let loss_bonus = run
            .last_loss
            .map(|l| (1.0 - f64::from(l)).clamp(0.0, 0.25))
            .unwrap_or(0.0);
        let worthiness_score = (0.65 * progress + loss_bonus).clamp(0.0, 1.0);

        let mut h = Sha3_256::new();
        h.update(PRODUCER_NAME.as_bytes());
        h.update(b"::");
        h.update(run.run_id.as_bytes());
        let digest = h.finalize();
        let sha7: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
        let finding_id = format!("mens-{slug}-train-{sha7}");
        out.push(ResearchEvent::FindingCandidateProposed {
            finding_id,
            claim_ids: vec![],
            worthiness_score,
            session_id: ctx.session_id.clone(),
            finding_candidate: Some(serde_json::json!({
                "candidate_class": "algorithmic_improvement",
                "producer": PRODUCER_NAME,
                "run_id": run.run_id,
                "model_name": run.model_name,
                "global_step": run.global_step,
                "planned_steps": run.planned_steps,
                "last_loss": run.last_loss,
            })),
        });
    }
    out
}

pub struct MensTrainingRunProducer {
    codex: vox_db::VoxDb,
}

impl MensTrainingRunProducer {
    pub fn new(codex: vox_db::VoxDb) -> Self {
        Self { codex }
    }
}

#[async_trait]
impl Producer for MensTrainingRunProducer {
    fn name(&self) -> &'static str {
        PRODUCER_NAME
    }

    async fn observe(&self, ctx: &ProducerContext) -> Vec<ResearchEvent> {
        const LIMIT: u32 = 32;
        let rows = match self.codex.list_training_runs(LIMIT).await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(error = %e, "mens_training_run: list_training_runs failed");
                return Vec::new();
            }
        };
        let snapshots: Vec<MensTrainingRunSnapshot> = rows
            .into_iter()
            .map(|r| MensTrainingRunSnapshot {
                run_id: r.run_id,
                status: r.status,
                model_name: r.model_name,
                global_step: r.global_step,
                planned_steps: r.planned_steps,
                last_loss: r.last_loss,
                last_checkpoint_path: r.last_checkpoint_path,
            })
            .collect();
        candidates_from_training_run_snapshots(&snapshots, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_fixture() -> Vec<MensTrainingRunSnapshot> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mens_training_run_complete.json");
        let raw = std::fs::read_to_string(&path).expect("fixture readable");
        serde_json::from_str(&raw).expect("fixture parses")
    }

    #[test]
    fn fixture_complete_run_emits_algorithmic_improvement_candidate() {
        let runs = load_fixture();
        let ctx = ProducerContext::for_test();
        let events = candidates_from_training_run_snapshots(&runs, &ctx);
        assert_eq!(events.len(), 1, "expected one candidate from fixture");
        match &events[0] {
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                worthiness_score,
                finding_candidate,
                ..
            } => {
                assert!(finding_id.starts_with("mens-"));
                assert!(*worthiness_score > 0.0);
                let fc = finding_candidate
                    .as_ref()
                    .expect("finding_candidate payload");
                assert_eq!(fc["candidate_class"], "algorithmic_improvement");
                assert_eq!(fc["run_id"], "run-fixture-001");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn running_status_is_skipped() {
        let runs = vec![MensTrainingRunSnapshot {
            run_id: "run-active".into(),
            status: "running".into(),
            model_name: None,
            global_step: 10,
            planned_steps: Some(100),
            last_loss: Some(1.2),
            last_checkpoint_path: Some("/tmp/ckpt.safetensors".into()),
        }];
        let ctx = ProducerContext::for_test();
        assert!(candidates_from_training_run_snapshots(&runs, &ctx).is_empty());
    }
}
