//! Cluster re-judge and adversarial verification.

pub mod decide;
pub mod verify;
pub mod prompt;

use crate::cluster::Cluster;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ArtifactForm {
    AgentsMdRule,
    CodeAuditDetector,
    ArchRule,
    CiGate,
    VoxScript,
    CorpusNegativeExample,
    None,
}

impl ArtifactForm {
    /// Staging-file extension for this form (always ends in `.proposed`).
    pub fn staging_extension(self) -> &'static str {
        match self {
            ArtifactForm::AgentsMdRule => "agents-rule.md.proposed",
            ArtifactForm::CodeAuditDetector => "detector.md.proposed",
            ArtifactForm::ArchRule => "arch-rule.toml.proposed",
            ArtifactForm::CiGate => "ci.yaml.proposed",
            ArtifactForm::VoxScript => "vox.proposed",
            ArtifactForm::CorpusNegativeExample => "corpus.jsonl.proposed",
            ArtifactForm::None => "",
        }
    }
    /// Forms allowed when the authoring model is not Vox-capable.
    pub fn vox_required(self) -> bool {
        matches!(self, ArtifactForm::VoxScript)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftedArtifact {
    pub form: ArtifactForm,
    pub staging_path: String,
    pub body: String,
    pub form_rationale: String,
    pub authoring_model_vox_capable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationDecision {
    pub cluster_id: String,
    pub member_commit_shas: Vec<String>,
    pub member_count: usize,
    pub total_member_tokens: u64,
    pub artifact_form: ArtifactForm,
    pub confidence: f32,
    pub synthesized_fix_summary: String,
    pub drafted_artifact: Option<DraftedArtifact>,
    pub verified: bool,
    pub refutation_note: String,
}

/// Whether the selected judge model can author Vox source. Passed in by the CLI
/// so this crate need not depend on vox-orchestrator's model registry.
#[derive(Debug, Clone, Copy)]
pub struct ModelVoxCapability(pub bool);

#[async_trait]
pub trait Router: Send + Sync {
    /// Re-judge one cluster into a decision (decide + verify happen inside).
    async fn route(
        &self,
        cluster: &Cluster,
        cluster_id: &str,
        vox_capable: ModelVoxCapability,
    ) -> RemediationDecision;
}

/// Deterministic in-memory router for tests.
pub struct MockRouter {
    pub confidence: f32,
}

#[async_trait]
impl Router for MockRouter {
    async fn route(
        &self,
        cluster: &Cluster,
        cluster_id: &str,
        vox_capable: ModelVoxCapability,
    ) -> RemediationDecision {
        // Pick a form from the bucket's remediation_kind, respecting the vox gate.
        let kind = &cluster.bucket.key.remediation_kind;
        let mut form = match kind.as_str() {
            "ScriptAutomation" => ArtifactForm::VoxScript,
            "AgentsMdRule" => ArtifactForm::AgentsMdRule,
            "LinterRule" => ArtifactForm::CodeAuditDetector,
            "CorpusNegativeExample" => ArtifactForm::CorpusNegativeExample,
            _ => ArtifactForm::None,
        };
        if form.vox_required() && !vox_capable.0 {
            form = ArtifactForm::CiGate; // fallback when not vox-capable
        }
        let shas: Vec<String> = cluster
            .bucket
            .members
            .iter()
            .map(|m| m.row.commit_sha.clone())
            .collect();
        let tokens = cluster
            .bucket
            .members
            .iter()
            .map(|m| token_sum(&m.row.cost))
            .sum();
        let artifact = if matches!(form, ArtifactForm::None) {
            None
        } else {
            Some(DraftedArtifact {
                form,
                staging_path: format!("{cluster_id}.{}", form.staging_extension()),
                body: format!("# proposed fix for {} members", shas.len()),
                form_rationale: "mock".into(),
                authoring_model_vox_capable: vox_capable.0,
            })
        };
        RemediationDecision {
            cluster_id: cluster_id.to_string(),
            member_count: shas.len(),
            member_commit_shas: shas,
            total_member_tokens: tokens,
            artifact_form: form,
            confidence: self.confidence,
            synthesized_fix_summary: "mock synthesis".into(),
            drafted_artifact: artifact,
            verified: self.confidence >= 0.5,
            refutation_note: "mock".into(),
        }
    }
}

/// Sum input+output tokens from a MeasuredCost (0 for Unavailable/Ambiguous).
pub fn token_sum(cost: &vox_effort_audit::hybrid::MeasuredCost) -> u64 {
    use vox_effort_audit::hybrid::MeasuredCost::*;
    match cost {
        Measured {
            input_tokens,
            output_tokens,
            ..
        } => input_tokens + output_tokens,
        Estimated {
            input_tokens,
            output_tokens,
        } => input_tokens + output_tokens,
        Ambiguous | Unavailable => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bucket::{Bucket, BucketKey};
    use crate::load::LoadedFinding;
    use std::collections::HashMap;
    use vox_effort_audit::hybrid::MeasuredCost;
    use vox_effort_audit::judge::schema::{JudgeFinding, RemediationKind, WasteCategory};
    use vox_effort_audit::output::{FindingRow, JudgeMeta};
    use vox_effort_audit::shape::{CommitKind, ShapeFeatures};

    /// Build a single-member cluster with a chosen remediation kind.
    fn cluster_with_kind(kind: &str) -> Cluster {
        let row = FindingRow {
            schema_version: "1.0".into(),
            commit_sha: "deadbeef".into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(),
            message_first_line: "test commit".into(),
            shape: ShapeFeatures {
                additions: 1,
                deletions: 0,
                files_changed: 1,
                file_extension_histogram: HashMap::new(),
                mechanical_sweep_score: 0.0,
                is_lockfile_only: false,
                is_generated_only: false,
                is_doc_only: false,
                commit_kind_from_message: CommitKind::Other,
            },
            cost: MeasuredCost::Estimated {
                input_tokens: 100,
                output_tokens: 50,
            },
            judge: JudgeMeta {
                model_id: "mock".into(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Judged".into(),
            },
            finding: Some(JudgeFinding {
                waste_score: 8,
                waste_category: WasteCategory::MechanicalSweep,
                suggested_remediation_kind: RemediationKind::ScriptAutomation,
                rationale_one_line: "r".into(),
                evidence_pointers: vec!["crates/vox-config/src/x.rs:1".into()],
            }),
        };
        Cluster {
            key_suffix: String::new(),
            bucket: Bucket {
                key: BucketKey {
                    waste_category: "MechanicalSweep".into(),
                    remediation_kind: kind.into(),
                    primary_crate: "vox-config".into(),
                },
                members: vec![LoadedFinding { row }],
            },
        }
    }

    #[test]
    fn vox_form_falls_back_when_not_capable() {
        assert!(ArtifactForm::VoxScript.vox_required());
        assert!(!ArtifactForm::CiGate.vox_required());
    }

    #[test]
    fn staging_extensions_all_end_in_proposed() {
        for f in [
            ArtifactForm::AgentsMdRule,
            ArtifactForm::CodeAuditDetector,
            ArtifactForm::ArchRule,
            ArtifactForm::CiGate,
            ArtifactForm::VoxScript,
            ArtifactForm::CorpusNegativeExample,
        ] {
            assert!(f.staging_extension().ends_with(".proposed"), "{:?}", f);
        }
        assert_eq!(ArtifactForm::None.staging_extension(), "");
    }

    #[test]
    fn token_sum_handles_all_variants() {
        assert_eq!(
            token_sum(&MeasuredCost::Estimated {
                input_tokens: 3,
                output_tokens: 4
            }),
            7
        );
        assert_eq!(token_sum(&MeasuredCost::Unavailable), 0);
        assert_eq!(token_sum(&MeasuredCost::Ambiguous), 0);
    }

    #[tokio::test]
    async fn mock_router_respects_vox_gate() {
        let router = MockRouter { confidence: 0.9 };
        let cluster = cluster_with_kind("ScriptAutomation");

        let incapable = router
            .route(&cluster, "c1", ModelVoxCapability(false))
            .await;
        assert_eq!(incapable.artifact_form, ArtifactForm::CiGate);
        assert_eq!(incapable.member_count, 1);
        assert_eq!(incapable.total_member_tokens, 150);
        assert!(incapable.verified);

        let capable = router
            .route(&cluster, "c1", ModelVoxCapability(true))
            .await;
        assert_eq!(capable.artifact_form, ArtifactForm::VoxScript);
        assert!(
            capable
                .drafted_artifact
                .unwrap()
                .staging_path
                .ends_with("vox.proposed")
        );
    }
}
