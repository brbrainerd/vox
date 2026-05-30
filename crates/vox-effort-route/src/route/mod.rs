//! Cluster re-judge and adversarial verification.
//!
//! # Deferred follow-ups
//!
//! These are intentional S2 scope boundaries, not bugs — recorded here so they
//! are discoverable rather than silent:
//!
//! - **`git show` subprocess has no timeout.** Re-reading member diffs shells out
//!   to `git show <sha>` with no wall-clock bound. This is the same known-minor
//!   carried over from S1's `walk.rs`; acceptable until a runaway repo surfaces it.
//! - **The embedder reuses the chat model id rather than a dedicated embed model.**
//!   Fine for current usage; it only matters when a bucket exceeds
//!   `max_bucket_size` while the resolved model is chat-only (no embedding head).
//! - **Telemetry events are defined but not yet wired into the pipeline.** The
//!   `audit.route.*` event types exist (parity with S1) but the pipeline does not
//!   emit them yet; wiring is deferred to a later slice.

pub mod decide;
pub mod verify;
pub mod prompt;

use crate::cluster::Cluster;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use self::decide::DecideResponse;
use self::verify::RefuteResponse;

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
    /// Whether this form requires the authoring model to be Vox-capable
    /// (true only for `VoxScript`). Such forms are gated out on non-Vox-capable runs.
    pub fn vox_required(self) -> bool {
        matches!(self, ArtifactForm::VoxScript)
    }
}

/// Strip an optional ` ```json `/` ``` ` markdown code-fence wrapper from an LLM
/// response so the inner JSON can be parsed. Returns a trimmed slice; a no-op for
/// already-bare JSON. Shared by the decide and verify parse fns.
pub(crate) fn strip_json_fence(s: &str) -> &str {
    let trimmed = s.trim();
    let inner = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    inner.strip_suffix("```").unwrap_or(inner).trim()
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
    /// Judge tokens (decide + verify prompt+completion) this cluster cost.
    /// 0 for the mock router and for budget-skipped clusters.
    #[serde(default)]
    pub judge_tokens_used: u64,
}

impl RemediationDecision {
    /// A cluster that was not routed because the judge token budget was already
    /// exhausted. Emitted as a `None`-form, unverified row so the run is honest
    /// about what it skipped rather than silently truncating.
    pub fn budget_skipped(cluster: &Cluster, cluster_id: &str) -> RemediationDecision {
        let shas: Vec<String> = cluster
            .bucket
            .members
            .iter()
            .map(|m| m.row.commit_sha.clone())
            .collect();
        let total_member_tokens = cluster.bucket.members.iter().map(|m| token_sum(&m.row.cost)).sum();
        RemediationDecision {
            cluster_id: cluster_id.to_string(),
            member_count: shas.len(),
            member_commit_shas: shas,
            total_member_tokens,
            artifact_form: ArtifactForm::None,
            confidence: 0.0,
            synthesized_fix_summary: String::new(),
            drafted_artifact: None,
            verified: false,
            refutation_note: "[budget] skipped: judge token budget exhausted".into(),
            judge_tokens_used: 0,
        }
    }
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
            judge_tokens_used: 0,
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

/// Real router wired through `vox_actor_runtime::llm::infer_with_retry`.
///
/// All LLM I/O goes through the model-agnostic facade. The model id is resolved
/// upstream (orchestrator model registry) and passed in as `resolved_model`;
/// no provider hostnames or SDKs leak in here. See AGENTS.md
/// §Model-Agnostic LLM Boundary.
pub struct LlmRouter {
    pub resolved_model: String,
    pub timeout: Duration,
    /// Worktree root used to re-read member diffs via `git show`.
    pub repo_root: std::path::PathBuf,
    /// Upper bound on member diffs read into the decide prompt.
    pub max_context_commits: usize,
    /// When false, the refute pass is skipped and decisions are unverified.
    pub verify: bool,
}

impl LlmRouter {
    fn llm_config(&self, response_format: serde_json::Value) -> vox_actor_runtime::llm::LlmConfig {
        // `provider: "auto"` defers vendor selection to the facade / model
        // registry — no vendor is named here.
        vox_actor_runtime::llm::LlmConfig {
            provider: "auto".into(),
            model: self.resolved_model.clone(),
            cost_per_1k: None,
            base_url: None,
            api_key: None,
            temperature: Some(0.0),
            top_p: None,
            max_tokens: Some(2048),
            response_format: Some(response_format),
            timeout_ms: Some(self.timeout.as_millis() as u64),
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: Some("CodeEffortJudge".into()),
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: Some(1),
            telemetry_skip_interaction: false,
        }
    }

    /// Read up to `max_context_commits` member diffs via `git show`. Failures
    /// are tolerated (the diff is simply omitted) so a missing object never
    /// aborts routing.
    fn read_diffs(&self, cluster: &Cluster) -> Vec<(String, String)> {
        cluster
            .bucket
            .members
            .iter()
            .take(self.max_context_commits)
            .map(|m| {
                let sha = m.row.commit_sha.clone();
                let diff = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&self.repo_root)
                    .args(["show", "--no-color", "--format=", &sha])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                    .unwrap_or_default();
                (sha, diff)
            })
            .collect()
    }

    /// Single facade call returning `(response_text, judge_tokens_used)`, or an
    /// error string. `judge_tokens_used` is `prompt_tokens + completion_tokens`.
    async fn infer(
        &self,
        messages: Vec<vox_actor_runtime::llm::LlmChatMessage>,
        response_format: serde_json::Value,
    ) -> Result<(String, u64), String> {
        let activity_options =
            vox_actor_runtime::ActivityOptions::default().with_timeout(self.timeout);
        let config = self.llm_config(response_format);
        let infer_result =
            vox_actor_runtime::llm::infer_with_retry(&activity_options, messages, vec![config]).await;
        match infer_result {
            vox_actor_runtime::ActivityResult::Ok(Ok((resp, _cfg))) => {
                let tokens = u64::from(resp.prompt_tokens) + u64::from(resp.completion_tokens);
                Ok((resp.content, tokens))
            }
            vox_actor_runtime::ActivityResult::Ok(Err(api_err)) => Err(format!("llm error: {api_err}")),
            vox_actor_runtime::ActivityResult::Failed(activity_err) => {
                Err(format!("activity error: {activity_err:?}"))
            }
            vox_actor_runtime::ActivityResult::Cancelled => Err("activity cancelled".into()),
        }
    }
}

#[async_trait]
impl Router for LlmRouter {
    async fn route(
        &self,
        cluster: &Cluster,
        cluster_id: &str,
        vox_capable: ModelVoxCapability,
    ) -> RemediationDecision {
        let mut judge_tokens = 0u64;
        let diffs = self.read_diffs(cluster);
        let decide_messages = prompt::build_decide_messages(cluster, &diffs, vox_capable.0);
        let decide_raw = match self
            .infer(decide_messages, decide::decide_json_schema(vox_capable.0))
            .await
        {
            Ok((raw, toks)) => {
                judge_tokens += toks;
                raw
            }
            Err(e) => return failed_decision(cluster, cluster_id, &e, judge_tokens),
        };
        let decide = match decide::parse(&decide_raw) {
            Ok(d) => d,
            Err(e) => {
                return failed_decision(cluster, cluster_id, &format!("decide parse: {e}"), judge_tokens)
            }
        };

        // Verify pass (adversarial refutation).
        let refute = if self.verify {
            let refute_messages =
                prompt::build_refute_messages(cluster, decide.artifact_form, &decide.drafted_body);
            match self
                .infer(refute_messages, verify::refute_json_schema())
                .await
            {
                Ok((raw, toks)) => {
                    judge_tokens += toks;
                    verify::parse(&raw).ok()
                }
                Err(_) => None,
            }
        } else {
            None
        };

        let mut decision = assemble_decision(decide, refute, cluster, cluster_id, vox_capable.0);
        decision.judge_tokens_used = judge_tokens;
        decision
    }
}

/// Build a failed/no-fix decision when the decide pass cannot complete.
/// `judge_tokens` is whatever was already spent before the failure.
fn failed_decision(
    cluster: &Cluster,
    cluster_id: &str,
    note: &str,
    judge_tokens: u64,
) -> RemediationDecision {
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
    RemediationDecision {
        cluster_id: cluster_id.to_string(),
        member_count: shas.len(),
        member_commit_shas: shas,
        total_member_tokens: tokens,
        artifact_form: ArtifactForm::None,
        confidence: 0.0,
        synthesized_fix_summary: String::new(),
        drafted_artifact: None,
        verified: false,
        refutation_note: note.to_string(),
        judge_tokens_used: judge_tokens,
    }
}

/// Pure assembly of a `RemediationDecision` from parsed decide + (optional)
/// refute responses. Extracted so the routing logic is unit-testable without a
/// network round-trip.
///
/// `verified` is true only when the refute pass ran AND did not refute. When the
/// verify pass is disabled (`refute == None`) the decision is conservatively
/// marked unverified.
pub(crate) fn assemble_decision(
    decide: DecideResponse,
    refute: Option<RefuteResponse>,
    cluster: &Cluster,
    cluster_id: &str,
    vox_capable: bool,
) -> RemediationDecision {
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

    let (verified, mut refutation_note) = match &refute {
        Some(r) => (!r.refuted, r.refutation_note.clone()),
        None => (false, String::new()),
    };

    // DEFENSIVE Vox-capability gate (spec §5). The decide JSON schema already
    // excludes VoxScript from the enum when !vox_capable, but a non-compliant
    // model (or a provider that doesn't enforce response_format) could still
    // return "VoxScript". We MUST NOT let a Vox artifact escape a non-Vox-capable
    // run — that is the exact inverse of "fixes must not be forced into Vox".
    // Coerce to CiGate (matching MockRouter's fallback) and record the override.
    let mut form = decide.artifact_form;
    let mut form_rationale = decide.form_rationale;
    if form.vox_required() && !vox_capable {
        form = ArtifactForm::CiGate;
        let note = "[gate] model returned VoxScript on a non-Vox-capable run; coerced to CiGate.";
        form_rationale = format!("{note} (original rationale: {form_rationale})");
        refutation_note = if refutation_note.is_empty() {
            note.to_string()
        } else {
            format!("{refutation_note} {note}")
        };
    }

    let drafted_artifact = if matches!(form, ArtifactForm::None) {
        None
    } else {
        Some(DraftedArtifact {
            form,
            staging_path: format!("{cluster_id}.{}", form.staging_extension()),
            body: decide.drafted_body,
            form_rationale,
            authoring_model_vox_capable: vox_capable,
        })
    };

    RemediationDecision {
        cluster_id: cluster_id.to_string(),
        member_count: shas.len(),
        member_commit_shas: shas,
        total_member_tokens: tokens,
        artifact_form: form,
        confidence: decide.confidence.clamp(0.0, 1.0),
        synthesized_fix_summary: decide.synthesized_fix_summary,
        drafted_artifact,
        verified,
        refutation_note,
        // Set by the LlmRouter after assembly; assemble itself is token-agnostic.
        judge_tokens_used: 0,
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

    fn decide_resp(form: ArtifactForm) -> DecideResponse {
        DecideResponse {
            artifact_form: form,
            confidence: 0.8,
            synthesized_fix_summary: "summary".into(),
            drafted_body: "body".into(),
            form_rationale: "rationale".into(),
        }
    }

    #[test]
    fn assemble_verified_when_not_refuted() {
        let cluster = cluster_with_kind("LinterRule");
        let refute = RefuteResponse {
            refuted: false,
            refutation_note: "looks solid".into(),
        };
        let d = assemble_decision(
            decide_resp(ArtifactForm::CodeAuditDetector),
            Some(refute),
            &cluster,
            "c7",
            false,
        );
        assert!(d.verified);
        assert_eq!(d.artifact_form, ArtifactForm::CodeAuditDetector);
        assert_eq!(d.confidence, 0.8);
        assert_eq!(d.total_member_tokens, 150);
        let art = d.drafted_artifact.unwrap();
        assert_eq!(art.body, "body");
        assert!(art.staging_path.starts_with("c7."));
        assert!(art.staging_path.ends_with("detector.md.proposed"));
        assert!(!art.authoring_model_vox_capable);
    }

    #[test]
    fn assemble_unverified_when_refuted() {
        let cluster = cluster_with_kind("LinterRule");
        let refute = RefuteResponse {
            refuted: true,
            refutation_note: "commit 2 slips through".into(),
        };
        let d = assemble_decision(
            decide_resp(ArtifactForm::CiGate),
            Some(refute),
            &cluster,
            "c8",
            false,
        );
        assert!(!d.verified);
        assert_eq!(d.refutation_note, "commit 2 slips through");
    }

    #[test]
    fn assemble_no_artifact_for_none_form() {
        let cluster = cluster_with_kind("Unknown");
        let d = assemble_decision(
            decide_resp(ArtifactForm::None),
            None,
            &cluster,
            "c9",
            true,
        );
        // No refute pass → conservatively unverified, no drafted artifact.
        assert!(!d.verified);
        assert!(d.drafted_artifact.is_none());
        assert_eq!(d.artifact_form, ArtifactForm::None);
    }

    #[test]
    fn vox_artifact_cannot_escape_non_vox_capable_run() {
        // A non-compliant model returns VoxScript despite the schema excluding it.
        // assemble_decision MUST coerce it away on a non-Vox-capable run so no
        // .vox artifact is ever drafted. (Spec §5; the inverse of the user's
        // "don't force fixes into Vox" correction is just as wrong.)
        let cluster = cluster_with_kind("ScriptAutomation");
        let refute = RefuteResponse { refuted: false, refutation_note: "ok".into() };
        let d = assemble_decision(
            decide_resp(ArtifactForm::VoxScript),
            Some(refute),
            &cluster,
            "cV",
            false, // NOT vox-capable
        );
        assert_ne!(d.artifact_form, ArtifactForm::VoxScript);
        assert_eq!(d.artifact_form, ArtifactForm::CiGate);
        let art = d.drafted_artifact.expect("non-None form drafts an artifact");
        assert!(!art.staging_path.ends_with("vox.proposed"), "no .vox artifact may escape");
        assert!(art.staging_path.ends_with("ci.yaml.proposed"));
        assert!(art.form_rationale.contains("[gate]"));
    }

    #[test]
    fn vox_artifact_allowed_when_vox_capable() {
        let cluster = cluster_with_kind("ScriptAutomation");
        let refute = RefuteResponse { refuted: false, refutation_note: "ok".into() };
        let d = assemble_decision(
            decide_resp(ArtifactForm::VoxScript),
            Some(refute),
            &cluster,
            "cV2",
            true, // vox-capable
        );
        assert_eq!(d.artifact_form, ArtifactForm::VoxScript);
        assert!(d.drafted_artifact.unwrap().staging_path.ends_with("vox.proposed"));
    }

    #[test]
    fn failed_decision_is_none_unverified() {
        let cluster = cluster_with_kind("ScriptAutomation");
        let d = failed_decision(&cluster, "cX", "boom", 1234);
        assert_eq!(d.artifact_form, ArtifactForm::None);
        assert!(!d.verified);
        assert_eq!(d.confidence, 0.0);
        assert_eq!(d.refutation_note, "boom");
        assert_eq!(d.judge_tokens_used, 1234); // tokens spent before the failure are reported
        assert_eq!(d.member_count, 1);
    }
}
