//! Per-commit judge pipeline.

pub mod parse;
pub mod prompt;
pub mod schema;

use crate::config::JudgeConfig;
use crate::judge::schema::JudgeFinding;
use crate::shape::ShapeFeatures;
use crate::walk::CommitRecord;
use async_trait::async_trait;
use std::time::Duration;
use vox_config::timeouts::EFFORT_AUDIT_JUDGE_TIMEOUT;

#[derive(Debug, Clone)]
pub struct JudgeOutcome {
    pub finding: Option<JudgeFinding>,
    pub model_id: String,
    pub latency_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub status: JudgeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeStatus {
    Judged,
    Failed(String),
    Skipped(SkipReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    BudgetExhausted,
    DryRun,
}

#[async_trait]
pub trait Judge: Send + Sync {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome;
    fn model_id(&self) -> &str;
}

/// Deterministic in-memory judge for tests.
pub struct MockJudge {
    pub fixed_score: u8,
    pub model: String,
}

#[async_trait]
impl Judge for MockJudge {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome {
        use schema::{RemediationKind, WasteCategory};
        let kind = if shape.mechanical_sweep_score > 0.7 {
            RemediationKind::ScriptAutomation
        } else if shape.is_doc_only {
            RemediationKind::NoneNeeded
        } else {
            RemediationKind::Unknown
        };
        JudgeOutcome {
            finding: Some(JudgeFinding {
                waste_score: self.fixed_score,
                waste_category: if shape.is_doc_only {
                    WasteCategory::LegitDocs
                } else {
                    WasteCategory::Other
                },
                suggested_remediation_kind: kind,
                rationale_one_line: format!(
                    "mock judgement of {}",
                    &rec.sha[..7.min(rec.sha.len())]
                ),
                evidence_pointers: vec![],
            }),
            model_id: self.model.clone(),
            latency_ms: 0,
            input_tokens: 0,
            output_tokens: 0,
            status: JudgeStatus::Judged,
        }
    }
    fn model_id(&self) -> &str {
        &self.model
    }
}

/// Real judge wired through `vox_actor_runtime::llm` with the chosen model.
///
/// All LLM I/O goes through `vox_actor_runtime::llm::infer_with_retry`. The
/// model id is resolved upstream (orchestrator model registry) and passed in
/// as `resolved_model`. The judge surface is intentionally vendor-agnostic —
/// no provider hostnames or SDKs leak in here. See AGENTS.md §Model-Agnostic
/// LLM Boundary.
pub struct LlmJudge {
    pub config: JudgeConfig,
    pub resolved_model: String,
    pub timeout: Duration,
}

#[async_trait]
impl Judge for LlmJudge {
    async fn judge_one(&self, rec: &CommitRecord, shape: &ShapeFeatures) -> JudgeOutcome {
        let started = std::time::Instant::now();
        let mut messages = prompt::build_messages(rec, shape);

        // Build a single LlmConfig candidate. `provider: "auto"` defers vendor
        // selection to the facade / model registry — we do not name any vendor.
        let llm_config = vox_actor_runtime::llm::LlmConfig {
            provider: "auto".into(),
            model: self.resolved_model.clone(),
            cost_per_1k: None,
            base_url: None,
            api_key: None,
            temperature: Some(0.0),
            top_p: None,
            max_tokens: Some(512),
            response_format: Some(schema::judge_finding_json_schema()),
            tools: None,
            tool_choice: None,
            timeout_ms: Some(self.timeout.as_millis() as u64),
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: Some("CodeEffortJudge".into()),
            telemetry_strength_tag: None,
            telemetry_trace_id: None,
            telemetry_attempt_number: Some(1),
            telemetry_skip_interaction: false,
        };

        let activity_options =
            vox_actor_runtime::ActivityOptions::default().with_timeout(self.timeout);

        let mut attempts: u32 = 0;
        loop {
            attempts += 1;
            let infer_result = vox_actor_runtime::llm::infer_with_retry(
                &activity_options,
                messages.clone(),
                vec![llm_config.clone()],
            )
            .await;

            let response = match infer_result {
                vox_actor_runtime::ActivityResult::Ok(Ok((resp, _cfg))) => resp,
                vox_actor_runtime::ActivityResult::Ok(Err(api_err)) => {
                    return JudgeOutcome {
                        finding: None,
                        model_id: self.resolved_model.clone(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        status: JudgeStatus::Failed(format!("llm error: {api_err}")),
                    };
                }
                vox_actor_runtime::ActivityResult::Failed(activity_err) => {
                    return JudgeOutcome {
                        finding: None,
                        model_id: self.resolved_model.clone(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        status: JudgeStatus::Failed(format!("activity error: {activity_err:?}")),
                    };
                }
                vox_actor_runtime::ActivityResult::Cancelled => {
                    return JudgeOutcome {
                        finding: None,
                        model_id: self.resolved_model.clone(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: 0,
                        output_tokens: 0,
                        status: JudgeStatus::Failed("activity cancelled".into()),
                    };
                }
            };

            match parse::parse(&response.content) {
                Ok(finding) => {
                    return JudgeOutcome {
                        finding: Some(finding),
                        model_id: self.resolved_model.clone(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: response.prompt_tokens as u64,
                        output_tokens: response.completion_tokens as u64,
                        status: JudgeStatus::Judged,
                    };
                }
                Err(e) if attempts <= self.config.schema_retry_limit => {
                    // Append the model's bad output + a corrective user message,
                    // then loop. The original `messages` accumulates so the model
                    // can see what it produced and what we asked for instead.
                    messages.push(vox_actor_runtime::llm::LlmChatMessage {
                        role: "assistant".into(),
                        content: response.content.clone(),
                    });
                    messages.push(vox_actor_runtime::llm::LlmChatMessage {
                        role: "user".into(),
                        content: parse::retry_message(&e),
                    });
                    continue;
                }
                Err(e) => {
                    return JudgeOutcome {
                        finding: None,
                        model_id: self.resolved_model.clone(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        input_tokens: response.prompt_tokens as u64,
                        output_tokens: response.completion_tokens as u64,
                        status: JudgeStatus::Failed(format!(
                            "parse error after {attempts} attempts: {e}"
                        )),
                    };
                }
            }
        }
    }
    fn model_id(&self) -> &str {
        &self.resolved_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::{CommitKind, ShapeFeatures};
    use std::collections::HashMap;

    fn fake() -> (CommitRecord, ShapeFeatures) {
        let r = crate::walk::CommitRecord {
            sha: "deadbeef".into(),
            parent_sha: None,
            commit_ts: chrono::Utc::now(),
            message: "refactor: mass sweep".into(),
            author_email_sha256: "z".into(),
            files: vec![],
            additions: 100,
            deletions: 100,
            unified_diff_text: "".into(),
            diff_truncated: false,
        };
        let s = ShapeFeatures {
            additions: 100,
            deletions: 100,
            files_changed: 50,
            file_extension_histogram: HashMap::new(),
            mechanical_sweep_score: 0.9,
            is_lockfile_only: false,
            is_generated_only: false,
            is_doc_only: false,
            commit_kind_from_message: CommitKind::Refactor,
        };
        (r, s)
    }

    #[tokio::test]
    async fn mock_judge_routes_high_sweep_to_script() {
        let j = MockJudge {
            fixed_score: 8,
            model: "mock".into(),
        };
        let (r, s) = fake();
        let out = j.judge_one(&r, &s).await;
        assert_eq!(out.status, JudgeStatus::Judged);
        assert_eq!(
            out.finding.unwrap().suggested_remediation_kind,
            schema::RemediationKind::ScriptAutomation
        );
        assert_eq!(j.model_id(), "mock");
    }

    #[tokio::test]
    async fn mock_judge_routes_doc_only_to_none_needed() {
        let j = MockJudge {
            fixed_score: 1,
            model: "mock".into(),
        };
        let (r, mut s) = fake();
        s.mechanical_sweep_score = 0.0;
        s.is_doc_only = true;
        let out = j.judge_one(&r, &s).await;
        let f = out.finding.unwrap();
        assert_eq!(
            f.suggested_remediation_kind,
            schema::RemediationKind::NoneNeeded
        );
        assert_eq!(f.waste_category, schema::WasteCategory::LegitDocs);
    }

    #[tokio::test]
    async fn mock_judge_default_routes_to_unknown() {
        let j = MockJudge {
            fixed_score: 4,
            model: "mock".into(),
        };
        let (r, mut s) = fake();
        s.mechanical_sweep_score = 0.1;
        s.is_doc_only = false;
        let out = j.judge_one(&r, &s).await;
        assert_eq!(
            out.finding.unwrap().suggested_remediation_kind,
            schema::RemediationKind::Unknown
        );
    }

    #[test]
    fn llm_judge_model_id_returns_resolved_model() {
        let j = LlmJudge {
            config: JudgeConfig::default(),
            resolved_model: "fast".into(),
            timeout: EFFORT_AUDIT_JUDGE_TIMEOUT,
        };
        assert_eq!(j.model_id(), "fast");
    }
}
