use crate::orchestrator::task_dispatch::complete::success::gates::GateOutcome;
use crate::orchestrator::{Orchestrator, OrchestratorError};
use crate::types::{AgentId, AgentTask, CompletionAttestation, TaskId, TaskStatus};
use tracing;

impl Orchestrator {
    pub async fn check_socrates_gate(
        &self,
        task_id: TaskId,
        agent_id: AgentId,
        task: &AgentTask,
        attestation: Option<&CompletionAttestation>,
        max_socrates_debug_iterations: u8,
        trust_relax_gates: bool,
    ) -> Result<GateOutcome, OrchestratorError> {
        let Some(ref ctx) = task.socrates else {
            return Ok(GateOutcome { requeue: None });
        };

        let envelope_raw = task.session_id.as_ref().and_then(|sid| {
            let key = crate::socrates::session_context_envelope_key(sid);
            crate::sync_lock::rw_read(&*self.context_store).get(&key)
        });

        let (
            grounding_shadow,
            grounding_enforce,
            socrates_shadow,
            socrates_enforce,
            socrates_policy,
            bypass_blocked,
            force_research,
        ) = {
            let config = crate::sync_lock::rw_read(&*self.config);
            let (bb, fr) = if let Some(q_lock) = self.agent_queue(agent_id) {
                let q = crate::sync_lock::rw_read(&*q_lock);
                (
                    q.capabilities.is_low_confidence_bypass_blocked,
                    q.capabilities.force_socrates_research,
                )
            } else {
                (false, false)
            };
            (
                config.completion_grounding_shadow,
                config.completion_grounding_enforce,
                config.socrates_gate_shadow,
                config.socrates_gate_enforce,
                config.effective_socrates_policy(),
                bb,
                fr,
            )
        };

        if grounding_shadow || grounding_enforce {
            let declared = crate::grounding::declared_evidence_citations(attestation);
            let grounding_msg = if !declared.is_empty() {
                crate::grounding::grounding_violation_declared_not_in_envelope(
                    attestation,
                    envelope_raw.as_deref(),
                )
            } else {
                crate::grounding::grounding_violation_factual_mode_without_declarations(
                    attestation,
                    ctx,
                )
            };

            if let Some(msg) = grounding_msg {
                let violation_kind = if !declared.is_empty() {
                    "declared_not_in_envelope"
                } else {
                    "factual_without_declarations"
                };
                if grounding_shadow {
                    tracing::info!(
                        target: "vox_orchestrator::grounding",
                        task_id = task_id.0,
                        agent_id = agent_id.0,
                        violation_kind,
                        "{msg}"
                    );
                }
                if grounding_enforce
                    && !trust_relax_gates
                    && task.debug_iterations < max_socrates_debug_iterations
                {
                    tracing::warn!(
                        target: "vox_orchestrator::grounding",
                        task_id = task_id.0,
                        agent_id = agent_id.0,
                        violation_kind,
                        requeued = true,
                        "completion grounding enforce: task re-queued for more evidence",
                    );
                    let mut t = task.clone();
                    t.debug_iterations += 1;
                    t.description
                        .push_str(&format!("\n\n[GROUNDING GATE]\n{msg}\n",));
                    t.status = TaskStatus::Queued;
                    return Ok(GateOutcome {
                        requeue: Some((t, "grounding gate policy violation".into(), 1, 0)),
                    });
                }
            }
        }

        let mut augmented = crate::grounding::merge_attestation_into_socrates_context(
            (*ctx).clone(),
            attestation,
            envelope_raw.as_deref(),
        );

        if crate::sync_lock::rw_read(&*self.budget_manager).is_fatigued() {
            augmented.fatigue_active = true;
        }

        let mut outcome = crate::socrates::evaluate_socrates_gate(
            &augmented,
            &socrates_policy,
            task.description.as_str(),
        );

        if force_research && !outcome.research_decision.should_research {
            outcome.research_decision.should_research = true;
            outcome.research_decision.trigger =
                "Policy: force_socrates_research enabled".to_string();
        }

        if socrates_shadow {
            tracing::info!(
                target: "vox_orchestrator::socrates",
                task_id = task_id.0,
                agent_id = agent_id.0,
                decision = ?outcome.decision,
                confidence = outcome.confidence,
                contradiction = outcome.contradiction_ratio,
                "socrates gate (shadow)"
            );
        }

        let is_low_confidence = outcome.confidence < 0.7 || outcome.contradiction_ratio > 0.3;
        let bypass_disallowed = bypass_blocked && is_low_confidence;

        // `evaluate_socrates_gate` may recommend web/autonomous research even when no gate is
        // enabled to *consume* those results (shadow/off configs). Running `perform_autonomous_research`
        // unconditionally dominated integration tests and stress drains — skip unless a policy
        // surface actually needs retrieval telemetry or enforcement inputs.
        let mut research_results = Vec::new();
        if outcome.research_decision.should_research {
            let autonomous_research_needed = force_research
                || socrates_enforce
                || socrates_shadow
                || grounding_enforce
                || grounding_shadow
                || bypass_disallowed;
            if autonomous_research_needed {
                let queries = outcome
                    .research_decision
                    .suggested_query
                    .clone()
                    .map(|q| vec![q])
                    .unwrap_or_else(|| vec![task.description.clone()]);
                let trigger = outcome.research_decision.trigger.clone();

                let results = self
                    .perform_autonomous_research(Some(agent_id), Some(task_id), queries, &trigger)
                    .await
                    .unwrap_or_default();
                research_results = results;
            }
        }

        if (socrates_enforce || bypass_disallowed)
            && !trust_relax_gates
            && (outcome.decision != vox_orchestrator_types::socrates_policy::RiskDecision::Answer
                || !research_results.is_empty()
                || bypass_disallowed)
            && task.debug_iterations < max_socrates_debug_iterations
        {
            let mut t = task.clone();
            if let Some(ref sid) = t.session_id {
                let context_key = crate::socrates::session_context_envelope_key(sid);
                let store = crate::sync_lock::rw_read(&*self.context_store);
                let context_raw = store.get(&context_key);
                drop(store);
                let parsed = context_raw.as_ref().and_then(|raw| {
                    serde_json::from_str::<crate::ContextEnvelope>(raw)
                        .ok()
                        .and_then(|env| {
                            crate::socrates::SessionRetrievalEnvelope::from_context_envelope(&env)
                        })
                });
                if let Some(env) = parsed {
                    t.socrates = Some(env.merge_into(t.socrates.clone()));
                }
            }

            if !research_results.is_empty() {
                let mut s_ctx = t.socrates.clone().unwrap_or(augmented.clone());
                let old_quality = s_ctx.evidence_quality;
                self.inject_research_results(&mut s_ctx, research_results);
                t.socrates = Some(s_ctx.clone());

                tracing::info!(
                    target: "vox_orchestrator::socrates",
                    task_id = task_id.0,
                    agent_id = agent_id.0,
                    quality_improvement = s_ctx.evidence_quality - old_quality,
                    "autonomous research injected; evidence quality boosted"
                );
            }

            t.debug_iterations += 1;
            let next_action = t
                .socrates
                .as_ref()
                .and_then(|ctx| ctx.recommended_next_action.as_deref())
                .unwrap_or("gather_more_grounding");
            let mut reason = format!(
                "Risk decision {:?} (confidence {:.2}, contradiction {:.2}). Improve grounding (citations, evidence) or resolve contradictions before completing.",
                outcome.decision, outcome.confidence, outcome.contradiction_ratio
            );
            if bypass_disallowed {
                reason.push_str(" Bypass blocked by security policy due to low confidence.");
            }
            reason.push_str(&format!(" Suggested next action: {}.", next_action));
            t.description
                .push_str(&format!("\n\n[SOCRATES GATE]\n{}\n", reason));
            t.status = TaskStatus::Queued;
            Ok(GateOutcome {
                requeue: Some((t, "Socrates risk gate blocked completion".into(), 1, 0)),
            })
        } else {
            Ok(GateOutcome { requeue: None })
        }
    }
}

#[cfg(test)]
mod autonomous_research_short_circuit_tests {
    use super::*;
    use crate::config::OrchestratorConfig;
    use crate::sync_lock;
    use crate::types::{FileAffinity, TaskPriority};
    use std::time::Duration;

    #[tokio::test]
    async fn completion_does_not_call_autonomous_research_when_gates_disabled() {
        let mut cfg = OrchestratorConfig::for_testing();
        cfg.socrates_gate_enforce = false;
        cfg.socrates_gate_shadow = false;
        cfg.completion_grounding_enforce = false;
        cfg.completion_grounding_shadow = false;
        cfg.max_agents = 4;

        let orch = Orchestrator::new(cfg);
        orch.submit_task(
            "explore factual claims about rust async runtimes",
            vec![FileAffinity::write("src/a.rs")],
            Some(TaskPriority::Normal),
            None,
            None,
        )
        .await
        .unwrap();

        let aid = orch.agent_ids()[0];
        let task_id = {
            let q = orch.get_agent_queue_mut(aid).unwrap();
            let mut w = sync_lock::rw_write(&*q);
            w.dequeue().map(|t| t.id).expect("task dequeued")
        };

        tokio::time::timeout(
            Duration::from_secs(8),
            orch.complete_task_with_attestation(task_id, Some(CompletionAttestation::default())),
        )
        .await
        .expect("completion must not block on autonomous research when gates are off")
        .expect("complete ok");
    }
}
