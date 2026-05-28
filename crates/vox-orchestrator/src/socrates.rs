//! Socrates task gate: evidence-weighted confidence against shared [`vox_orchestrator_types::socrates_policy`] thresholds.

use serde::{Deserialize, Serialize};
use vox_orchestrator_types::socrates_policy::{
    ConfidencePolicy, RiskBand, RiskDecision, SocratesResearchDecision,
};

/// Context-store key prefix for canonical context envelope JSON persisted per session.
#[must_use]
pub fn session_context_envelope_key(session_id: &str) -> String {
    format!("context_envelope:{session_id}")
}

/// Retrieval telemetry shape persisted by MCP (extra fields ignored on deserialize).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRetrievalEnvelope {
    #[serde(default)]
    pub retrieval_tier: String,
    #[serde(default)]
    pub memory_hit_count: usize,
    #[serde(default)]
    pub knowledge_hit_count: usize,
    /// Ingested `search_document_chunks` rows (RAG corpus) surfaced by MCP retrieval.
    #[serde(default)]
    pub chunk_hit_count: usize,
    #[serde(default)]
    pub repo_hit_count: usize,
    /// RRF fused excerpt count (matches MCP `RetrievalEvidenceEnvelope::rrf_fused_hit_count` JSON).
    #[serde(default)]
    pub rrf_fused_hit_count: usize,
    #[serde(default)]
    pub used_vector: bool,
    #[serde(default)]
    pub used_bm25: bool,
    #[serde(default)]
    pub used_lexical_fallback: bool,
    #[serde(default)]
    pub contradiction_count: usize,
    #[serde(default)]
    pub source_diversity: usize,
    #[serde(default)]
    pub evidence_quality: f64,
    #[serde(default)]
    pub citation_coverage: f64,
    #[serde(default)]
    pub verification_performed: bool,
    #[serde(default)]
    pub verification_reason: Option<String>,
    #[serde(default)]
    pub recommended_next_action: Option<String>,
}

impl SessionRetrievalEnvelope {
    /// Parse retrieval envelope projection from a canonical context envelope payload.
    #[must_use]
    pub fn from_context_envelope(env: &crate::ContextEnvelope) -> Option<Self> {
        if !matches!(
            env.envelope_type,
            crate::ContextEnvelopeType::RetrievalEvidence
        ) {
            return None;
        }
        env.content
            .structured_payload
            .as_ref()
            .and_then(|v| serde_json::from_value::<Self>(v.clone()).ok())
    }

    /// Refreshes evidence fields from a new retrieval pass while preserving budget hints
    /// (`risk_budget`, `factual_mode`) from a prior task context.
    #[must_use]
    pub fn merge_into(self, prev: Option<SocratesTaskContext>) -> SocratesTaskContext {
        let fresh = self.to_task_context();
        let Some(mut base) = prev else {
            return fresh;
        };
        base.retrieval_tier = fresh.retrieval_tier;
        base.retrieval_used_vector = fresh.retrieval_used_vector;
        base.retrieval_used_lexical_fallback = fresh.retrieval_used_lexical_fallback;
        base.required_citations = fresh.required_citations;
        base.evidence_count = fresh.evidence_count;
        base.contradiction_hints = fresh.contradiction_hints;
        base.source_diversity = fresh.source_diversity;
        base.evidence_quality = fresh.evidence_quality;
        base.citation_coverage = fresh.citation_coverage;
        base.verification_performed = fresh.verification_performed;
        base.verification_reason = fresh.verification_reason;
        base.recommended_next_action = fresh.recommended_next_action;
        if fresh.retrieval_diagnosis.is_some() {
            base.retrieval_diagnosis = fresh.retrieval_diagnosis;
        }
        base
    }

    /// Maps envelope fields into task-level Socrates evidence (same contract as MCP bridging).
    #[must_use]
    pub fn to_task_context(&self) -> SocratesTaskContext {
        let doc_graph_hits = self
            .knowledge_hit_count
            .saturating_add(self.chunk_hit_count)
            .saturating_add(self.repo_hit_count);
        let required_citations =
            if self.memory_hit_count == 0 && doc_graph_hits == 0 && self.rrf_fused_hit_count == 0 {
                1_u8
            } else {
                0_u8
            };
        let base_total = self.memory_hit_count.saturating_add(doc_graph_hits);
        let evidence_total = (if base_total == 0 && self.rrf_fused_hit_count > 0 {
            1usize
        } else {
            base_total
        })
        .min(u8::MAX as usize) as u8;
        SocratesTaskContext {
            risk_budget: "normal".to_string(),
            factual_mode: true,
            required_citations,
            evidence_count: evidence_total,
            contradiction_hints: self.contradiction_count.min(u8::MAX as usize) as u8,
            retrieval_tier: Some(self.retrieval_tier.clone()),
            retrieval_used_vector: self.used_vector,
            retrieval_used_lexical_fallback: self.used_lexical_fallback,
            source_diversity: self.source_diversity.min(u8::MAX as usize) as u8,
            evidence_quality: self.evidence_quality.clamp(0.0, 1.0),
            citation_coverage: self.citation_coverage.clamp(0.0, 1.0),
            verification_performed: self.verification_performed,
            verification_reason: self.verification_reason.clone(),
            recommended_next_action: self.recommended_next_action.clone(),
            retrieval_diagnosis: None,
            fatigue_active: false,
            orient_report: None,
            answered_questions: vec![],
            research_model_enabled: false,
            fabricated_tool_claims: None,
        }
    }
}

/// Corpus-level retrieval outcome for critique loops (orchestrator / MCP parity).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrievalDiagnosis {
    /// Corpora that returned at least one hit.
    #[serde(default)]
    pub corpora_with_hits: Vec<String>,
    /// Corpora in the planner set that returned no hits.
    #[serde(default)]
    pub corpora_empty: Vec<String>,
    /// Effective [`vox_search::SearchPolicy::version`].
    #[serde(default)]
    pub policy_version: u32,
    /// `Debug` lowercased intent label from [`vox_db::SearchPlan`].
    #[serde(default)]
    pub planner_intent: String,
    /// High-level shape of remaining risk for refiners.
    #[serde(default)]
    pub evidence_shape: String,
}

use crate::types::TaskCategory;

/// The result of the Orient phase evaluating evidence gap, risk, and planning complexity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OrientReport {
    pub evidence_gap: f64,
    pub risk_band: RiskBand,
    pub planning_complexity: f64,
    pub category: Option<TaskCategory>,
}

/// Structured evidence / risk hints attached to an [`crate::types::AgentTask`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SocratesTaskContext {
    /// Operational risk tier label (logged to traces only).
    #[serde(default)]
    pub risk_budget: String,
    /// When true, `required_citations` constrains completion confidence.
    #[serde(default)]
    pub factual_mode: bool,
    /// Minimum grounded citations expected before claiming completion.
    #[serde(default)]
    pub required_citations: u8,
    /// Citations the agent reports having satisfied.
    #[serde(default)]
    pub evidence_count: u8,
    /// Unresolved contradictions the agent is aware of.
    #[serde(default)]
    pub contradiction_hints: u8,
    /// Retrieval tier label (`hybrid`, `bm25`, `lexical_fallback`, ...), when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_tier: Option<String>,
    /// True when vector evidence contributed to the retrieval context.
    #[serde(default)]
    pub retrieval_used_vector: bool,
    /// True when retrieval had to fall back to lexical substring matching.
    #[serde(default)]
    pub retrieval_used_lexical_fallback: bool,
    /// Number of distinct corpora that returned evidence.
    #[serde(default)]
    pub source_diversity: u8,
    /// Retrieval-side evidence quality proxy in `[0, 1]`.
    #[serde(default)]
    pub evidence_quality: f64,
    /// Retrieval-side citation coverage proxy in `[0, 1]`.
    #[serde(default)]
    pub citation_coverage: f64,
    /// Whether a verification pass was already attempted before completion.
    #[serde(default)]
    pub verification_performed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_next_action: Option<String>,
    /// Optional orchestrator-owned retrieval diagnosis (native search path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_diagnosis: Option<RetrievalDiagnosis>,
    /// Phase 2B: Indicates if human is working while fatigued. Prompts Socratic Verification Lockout.
    #[serde(default)]
    pub fatigue_active: bool,
    /// Result of the Orient phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orient_report: Option<OrientReport>,
    /// Questions answered by the user/system to clear evidence gaps.
    #[serde(default)]
    pub answered_questions: Vec<String>,
    /// When true, upstream routing may prefer the dedicated research adapter (Lane G) when wired.
    #[serde(default)]
    pub research_model_enabled: bool,
    /// Receipt IDs claimed by the agent that were not found or verified in the ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fabricated_tool_claims: Option<Vec<String>>,
}

/// Result of applying the completion gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocratesGateOutcome {
    /// Triaged answer / ask / abstain.
    pub decision: RiskDecision,
    /// Normalized confidence in `[0, 1]`.
    pub confidence: f64,
    /// Contradiction mass in `[0, 1]`.
    pub contradiction_ratio: f64,
    /// Discrete band for dashboards.
    pub band: RiskBand,
    /// Detailed research guidance ( queries, reason) returned by the policy.
    pub research_decision: SocratesResearchDecision,
}

/// Evaluate structured task metadata against `policy`.
#[must_use]
pub fn evaluate_socrates_gate(
    ctx: &SocratesTaskContext,
    policy: &ConfidencePolicy,
    query: &str,
) -> SocratesGateOutcome {
    let contradiction_ratio = match ctx.contradiction_hints {
        0 => 0.0,
        1 => 0.15,
        2 => 0.28,
        n => ((n as f64) * 0.22).min(1.0),
    };

    let coverage = if ctx.required_citations == 0 {
        1.0
    } else {
        (f64::from(ctx.evidence_count) / f64::from(ctx.required_citations)).clamp(0.0, 1.0)
    };

    let mut confidence = coverage.max(ctx.citation_coverage);
    if matches!(ctx.retrieval_tier.as_deref(), Some("hybrid") | Some("bm25")) {
        confidence = (confidence + 0.05).clamp(0.0, 1.0);
    }
    if ctx.retrieval_used_lexical_fallback {
        confidence = (confidence - 0.08).clamp(0.0, 1.0);
    }
    if ctx.factual_mode && ctx.required_citations > 0 && ctx.evidence_count < ctx.required_citations
    {
        confidence *= policy.abstain_threshold;
    }
    confidence = (confidence + (ctx.evidence_quality.clamp(0.0, 1.0) * 0.10)).clamp(0.0, 1.0);
    if ctx.source_diversity > 1 {
        confidence = (confidence + 0.04).clamp(0.0, 1.0);
    }
    if ctx.verification_performed && ctx.evidence_quality >= 0.60 {
        confidence = (confidence + 0.03).clamp(0.0, 1.0);
    }

    if let Some(ref diag) = ctx.retrieval_diagnosis {
        match diag.evidence_shape.as_str() {
            "contradictory" => {
                confidence = (confidence - 0.10).clamp(0.0, 1.0);
            }
            "empty" => {
                confidence = (confidence - 0.12).clamp(0.0, 1.0);
            }
            "narrow" => {
                confidence = (confidence - 0.03).clamp(0.0, 1.0);
            }
            _ => {}
        }
    }

    if ctx.fatigue_active {
        // High penalty for Socratic Verification Lockout (Phase 2B).
        // This forces either explicit manual override or strict compliance to prevent sloppy
        // commits when the human is compromised due to burnout/late hours.
        confidence = (confidence - 0.40).clamp(0.0, 1.0);
    }

    if let Some(ref fabricated) = ctx.fabricated_tool_claims {
        if !fabricated.is_empty() {
            // Hard block on fabricated tool calls.
            return SocratesGateOutcome {
                decision: RiskDecision::Abstain,
                confidence: 0.0,
                contradiction_ratio: 1.0,
                band: RiskBand::Low,
                research_decision: SocratesResearchDecision {
                    should_research: false,
                    trigger: format!("Fabricated tool receipts detected: {:?}", fabricated),
                    suggested_query: None,
                },
            };
        }
    }

    let band = policy.classify_risk(confidence, contradiction_ratio, ctx.citation_coverage);
    let decision =
        policy.evaluate_risk_decision(confidence, contradiction_ratio, ctx.citation_coverage);
    let research_decision = policy.evaluate_research_need(
        confidence,
        contradiction_ratio,
        ctx.citation_coverage,
        query,
    );

    SocratesGateOutcome {
        decision,
        confidence,
        contradiction_ratio,
        band,
        research_decision,
    }
}

pub fn spawn_socrates_research_poller(orch: std::sync::Arc<crate::Orchestrator>) {
    tokio::spawn(async move {
        let mut rx = orch.bulletin().subscribe();
        loop {
            match rx.recv().await {
                Ok(crate::types::AgentMessage::A2A(msg)) => {
                    if let crate::types::A2AMessageType::SocratesResearchRequest = msg.msg_type {
                        let payload_json: serde_json::Value =
                            serde_json::from_str(&msg.payload).unwrap_or_default();
                        let target_agent_id = payload_json
                            .get("agent_id")
                            .and_then(|v| v.as_u64())
                            .unwrap_or_default();
                        let queue_depth = payload_json
                            .get("queue_depth")
                            .and_then(|v| v.as_u64())
                            .unwrap_or_default();
                        let reason = payload_json
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");

                        tracing::info!(
                            "Socrates intercepted Research Request for Agent {}. Queue depth: {}. Reason: {}",
                            target_agent_id,
                            queue_depth,
                            reason
                        );

                        // Capture structured AST/Compiler diagnostics to enforce rigorous integration tests
                        let mut ast_diagnostics = String::new();
                        if std::env::current_dir()
                            .map(|p| p.join("Cargo.toml").exists())
                            .unwrap_or(false)
                        {
                            let mut c = tokio::process::Command::new("cargo");
                            c.arg("check").arg("--message-format=json");
                            if let Ok(out) = c.output().await {
                                let lines: Vec<&str> = std::str::from_utf8(&out.stdout)
                                    .unwrap_or("")
                                    .lines()
                                    .collect();
                                let mut errs = Vec::new();
                                for l in lines {
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
                                        if v["reason"] == "compiler-message"
                                            && v["message"]["level"] == "error"
                                        {
                                            errs.push(
                                                v["message"]["rendered"]
                                                    .as_str()
                                                    .unwrap_or("")
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }
                                if !errs.is_empty() {
                                    ast_diagnostics = format!(
                                        "\n\nStructured AST Diagnostics:\n{}",
                                        errs.join("\n")
                                    );
                                } else {
                                    ast_diagnostics =
                                        "\n\nStructured AST Diagnostics: Clean check.".to_string();
                                }
                            }
                        }

                        let desc = format!(
                            "Analyze overloaded worker ID: {} pipeline. Queue depth is {}. Reason: {}. Evaluate dependency bottlenecks and propose load-shedding or parallel routing solutions to MCP.{}",
                            target_agent_id, queue_depth, reason, ast_diagnostics
                        );
                        let observer_model = orch.config.read().unwrap().observer_model.clone();
                        let hints = crate::types::TaskEnqueueHints {
                            task_category: Some(crate::types::TaskCategory::Research),
                            model_override: observer_model.clone(),
                            model_preference: observer_model,
                            ..Default::default()
                        };

                        let task_res = orch
                            .submit_task_with_agent(
                                desc,
                                vec![],
                                Some(crate::types::TaskPriority::Urgent),
                                Some(target_agent_id.to_string()),
                                None,
                                Some(hints),
                                None,
                                None,
                            )
                            .await;
                        if let Ok(task_id) = task_res {
                            let socrates_context = SocratesTaskContext {
                                required_citations: 1,
                                fabricated_tool_claims: None,
                                ..Default::default()
                            };
                            let _ = orch.attach_socrates_context(task_id, socrates_context);
                        }
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factual_under_cited_abstains() {
        let p = ConfidencePolicy::default();
        let ctx = SocratesTaskContext {
            factual_mode: true,
            required_citations: 3,
            evidence_count: 0,
            ..Default::default()
        };
        let o = evaluate_socrates_gate(&ctx, &p, "dummy query");
        assert_eq!(o.decision, RiskDecision::Abstain);
    }

    #[test]
    fn retrieval_diagnosis_contradictory_lowers_confidence() {
        let p = ConfidencePolicy::default();
        let base_ctx = SocratesTaskContext {
            factual_mode: false,
            required_citations: 0,
            evidence_count: 5,
            evidence_quality: 0.9,
            citation_coverage: 0.9,
            ..Default::default()
        };
        let mut with_diag = base_ctx.clone();
        with_diag.retrieval_diagnosis = Some(RetrievalDiagnosis {
            evidence_shape: "contradictory".into(),
            ..Default::default()
        });
        let with = evaluate_socrates_gate(&with_diag, &p, "dummy query");
        let without = evaluate_socrates_gate(&base_ctx, &p, "dummy query");
        assert!(with.confidence < without.confidence);
    }

    #[test]
    fn session_envelope_rrf_hits_satisfy_evidence_floor() {
        let env = SessionRetrievalEnvelope {
            retrieval_tier: "hybrid".to_string(),
            memory_hit_count: 0,
            knowledge_hit_count: 0,
            chunk_hit_count: 0,
            repo_hit_count: 0,
            rrf_fused_hit_count: 3,
            used_vector: true,
            used_bm25: false,
            used_lexical_fallback: false,
            contradiction_count: 0,
            source_diversity: 0,
            evidence_quality: 0.0,
            citation_coverage: 0.0,
            verification_performed: false,
            verification_reason: None,
            recommended_next_action: None,
        };
        let ctx = env.to_task_context();
        assert_eq!(ctx.required_citations, 0);
        assert_eq!(ctx.evidence_count, 1);
    }

    #[test]
    fn session_envelope_can_parse_from_context_envelope_projection() {
        let retrieval = SessionRetrievalEnvelope {
            retrieval_tier: "hybrid".to_string(),
            memory_hit_count: 1,
            knowledge_hit_count: 2,
            chunk_hit_count: 3,
            repo_hit_count: 4,
            rrf_fused_hit_count: 5,
            used_vector: true,
            used_bm25: true,
            used_lexical_fallback: false,
            contradiction_count: 1,
            source_diversity: 3,
            evidence_quality: 0.9,
            citation_coverage: 0.8,
            verification_performed: true,
            verification_reason: Some("contradiction_detected".to_string()),
            recommended_next_action: Some("retry_hybrid".to_string()),
        };
        let env = crate::ContextEnvelope::from_session_retrieval("repo-1", "sid-1", &retrieval);
        let parsed = SessionRetrievalEnvelope::from_context_envelope(&env).expect("parse");
        assert_eq!(parsed.retrieval_tier, "hybrid");
        assert_eq!(parsed.memory_hit_count, 1);
        assert_eq!(parsed.rrf_fused_hit_count, 5);
    }
}
