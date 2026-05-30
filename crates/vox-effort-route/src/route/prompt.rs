//! Prompt construction for decide + refute passes.

use crate::cluster::Cluster;
use crate::route::ArtifactForm;
use vox_actor_runtime::llm::LlmChatMessage;

pub fn allowed_forms(vox_capable: bool) -> Vec<ArtifactForm> {
    let mut v = vec![
        ArtifactForm::AgentsMdRule,
        ArtifactForm::CodeAuditDetector,
        ArtifactForm::ArchRule,
        ArtifactForm::CiGate,
        ArtifactForm::CorpusNegativeExample,
        ArtifactForm::None,
    ];
    if vox_capable {
        v.push(ArtifactForm::VoxScript);
    }
    v
}

pub fn build_decide_messages(
    cluster: &Cluster,
    diffs: &[(String, String)],
    vox_capable: bool,
) -> Vec<LlmChatMessage> {
    let system = include_str!("decide_system.md");
    let allowed: Vec<String> = allowed_forms(vox_capable)
        .iter()
        .map(|f| format!("{f:?}"))
        .collect();
    let members: String = cluster
        .bucket
        .members
        .iter()
        .map(|m| {
            let f = m.row.finding.as_ref().unwrap();
            format!(
                "- {} [{}] {}",
                m.row.commit_sha, f.waste_score, f.rationale_one_line
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let diff_block: String = diffs
        .iter()
        .map(|(sha, d)| format!("### {sha}\n```\n{d}\n```"))
        .collect::<Vec<_>>()
        .join("\n");
    let user = format!(
        "CLUSTER: {cat} / {kind} / {crate_}
ALLOWED artifact_form values: {allowed:?}

MEMBER COMMITS:
{members}

REPRESENTATIVE DIFFS:
{diff_block}

Decide one remediation and draft its artifact. Return the JSON object.",
        cat = cluster.bucket.key.waste_category,
        kind = cluster.bucket.key.remediation_kind,
        crate_ = cluster.bucket.key.primary_crate,
    );
    vec![
        LlmChatMessage {
            role: "system".into(),
            content: system.into(),
        },
        LlmChatMessage {
            role: "user".into(),
            content: user,
        },
    ]
}

pub fn build_refute_messages(
    cluster: &Cluster,
    form: ArtifactForm,
    body: &str,
) -> Vec<LlmChatMessage> {
    let system = include_str!("refute_system.md");
    let user = format!(
        "CLUSTER: {cat} / {kind} / {crate_} ({n} commits)
PROPOSED form: {form:?}
PROPOSED body:
```
{body}
```
Try to refute. Return the JSON object.",
        cat = cluster.bucket.key.waste_category,
        kind = cluster.bucket.key.remediation_kind,
        crate_ = cluster.bucket.key.primary_crate,
        n = cluster.bucket.members.len(),
    );
    vec![
        LlmChatMessage {
            role: "system".into(),
            content: system.into(),
        },
        LlmChatMessage {
            role: "user".into(),
            content: user,
        },
    ]
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

    fn member(sha: &str, rationale: &str) -> LoadedFinding {
        let row = FindingRow {
            schema_version: "1.0".into(),
            commit_sha: sha.into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            author_email_sha256: "0".repeat(64),
            branch_hint: "main".into(),
            message_first_line: "test".into(),
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
            cost: MeasuredCost::Unavailable,
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
                rationale_one_line: rationale.into(),
                evidence_pointers: vec!["crates/vox-config/src/x.rs:1".into()],
            }),
        };
        LoadedFinding { row }
    }

    fn cluster_2() -> Cluster {
        Cluster {
            key_suffix: String::new(),
            bucket: Bucket {
                key: BucketKey {
                    waste_category: "MechanicalSweep".into(),
                    remediation_kind: "ScriptAutomation".into(),
                    primary_crate: "vox-config".into(),
                },
                members: vec![
                    member("aaa1111", "rename a sweep"),
                    member("bbb2222", "another sweep"),
                ],
            },
        }
    }

    #[test]
    fn allowed_forms_gate_vox() {
        assert!(!allowed_forms(false).contains(&ArtifactForm::VoxScript));
        assert!(allowed_forms(true).contains(&ArtifactForm::VoxScript));
    }

    #[test]
    fn decide_prompt_includes_allowed_and_members() {
        let cluster = cluster_2();
        let msgs = build_decide_messages(&cluster, &[], false);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "system");
        let user = &msgs[1].content;
        assert!(user.contains("ALLOWED artifact_form"));
        assert!(user.contains("vox-config"));
        assert!(user.contains("aaa1111"));
        assert!(user.contains("bbb2222"));
        // VoxScript excluded when not vox-capable.
        assert!(!user.contains("VoxScript"));
    }

    #[test]
    fn refute_prompt_includes_form_and_body() {
        let cluster = cluster_2();
        let msgs = build_refute_messages(&cluster, ArtifactForm::CiGate, "name: lint");
        assert_eq!(msgs.len(), 2);
        let user = &msgs[1].content;
        assert!(user.contains("CiGate"));
        assert!(user.contains("name: lint"));
        assert!(user.contains("2 commits"));
    }
}
