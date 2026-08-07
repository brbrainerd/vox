use crate::orchestrator::task_dispatch::complete::success::gates::GateOutcome;
use crate::orchestrator::{Orchestrator, OrchestratorError};
use crate::types::{AgentId, AgentTask, CompletionAttestation, TaskId, TaskStatus};
use tracing;

/// Compute the effective `(grounding_enforce, socrates_enforce)` pair for one
/// task -- extracted from `check_socrates_gate` so it's unit-testable without
/// a live `Orchestrator`/completion pipeline. Task 2.4: the effective risk
/// used here is the FULL precedence chain (explicit hint > this task's
/// category policy > this task's trigger-source policy), matching
/// `resolve_task_cost_policy` in `runtime.rs` -- previously this only
/// consulted `task.risk_posture`, so a category/source risk policy had zero
/// effect unless a caller also set an explicit hint. When nothing at all
/// applies, the global config flags are preserved unchanged.
#[must_use]
fn resolve_grounding_socrates_enforce(
    task: &AgentTask,
    overrides: &crate::config::TaskPolicyOverrides,
    global_grounding_enforce: bool,
    global_socrates_enforce: bool,
) -> (bool, bool) {
    let resolved = crate::mode::effective_risk_for_task(task, overrides).map(|r| r.resolve());
    let grounding_enforce = resolved
        .map(|r| r.grounding_enforce)
        .unwrap_or(global_grounding_enforce);
    let socrates_enforce = resolved
        .map(|r| r.socrates_enforce)
        .unwrap_or(global_socrates_enforce);
    (grounding_enforce, socrates_enforce)
}

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
            return Ok(GateOutcome {
                requeue: None,
                needs_review_approval: false,
            });
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
            let (grounding_enforce, socrates_enforce) = resolve_grounding_socrates_enforce(
                task,
                &config.task_policy,
                config.completion_grounding_enforce,
                config.socrates_gate_enforce,
            );
            (
                config.completion_grounding_shadow,
                grounding_enforce,
                config.socrates_gate_shadow,
                socrates_enforce,
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
                        needs_review_approval: false,
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
                needs_review_approval: false,
            })
        } else {
            Ok(GateOutcome {
                requeue: None,
                needs_review_approval: false,
            })
        }
    }
}

#[cfg(test)]
mod resolve_grounding_socrates_enforce_tests {
    use super::resolve_grounding_socrates_enforce;
    use crate::config::{TaskPolicyEntry, TaskPolicyOverrides};
    use crate::types::{AgentTask, TaskCategory, TaskId, TaskPriority};
    use std::collections::HashMap;

    // Task 2.4 regression test: a task with NO explicit risk_posture, but a
    // configured category-policy risk override, must still enable
    // grounding/socrates enforcement -- reproduces the same
    // "policy has zero effect without an explicit hint" bug fixed in
    // runtime.rs for cost/model routing.
    #[test]
    fn category_risk_policy_enforces_without_explicit_risk_posture() {
        let mut category = HashMap::new();
        category.insert(
            format!("{:?}", TaskCategory::CodeGen),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("low".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category,
            source: HashMap::new(),
        };

        let mut task = AgentTask::new(TaskId(1), "generate code", TaskPriority::Normal, vec![]);
        task.task_category = TaskCategory::CodeGen;
        assert_eq!(task.risk_posture, None, "no explicit risk hint set");

        // Global defaults are both OFF -- if the category policy had zero
        // effect, both enforce flags would stay false.
        let (grounding_enforce, socrates_enforce) =
            resolve_grounding_socrates_enforce(&task, &overrides, false, false);

        assert!(
            grounding_enforce,
            "Low-risk category policy must enable grounding enforcement even with no explicit risk_posture hint"
        );
        assert!(
            socrates_enforce,
            "Low-risk category policy must enable socrates enforcement even with no explicit risk_posture hint"
        );
    }

    #[test]
    fn source_risk_policy_applies_when_no_category_policy() {
        let mut source = HashMap::new();
        source.insert(
            format!("{:?}", crate::mode::TriggerSource::Automated),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("low".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category: HashMap::new(),
            source,
        };

        let mut task = AgentTask::new(TaskId(2), "generate code", TaskPriority::Normal, vec![]);
        task.trigger_source = Some(crate::mode::TriggerSource::Automated);

        let (grounding_enforce, socrates_enforce) =
            resolve_grounding_socrates_enforce(&task, &overrides, false, false);

        assert!(grounding_enforce);
        assert!(socrates_enforce);
    }

    #[test]
    fn nothing_configured_preserves_global_defaults() {
        let overrides = TaskPolicyOverrides {
            category: HashMap::new(),
            source: HashMap::new(),
        };
        let task = AgentTask::new(TaskId(3), "generate code", TaskPriority::Normal, vec![]);

        assert_eq!(
            resolve_grounding_socrates_enforce(&task, &overrides, false, false),
            (false, false)
        );
        assert_eq!(
            resolve_grounding_socrates_enforce(&task, &overrides, true, true),
            (true, true)
        );
    }

    #[test]
    fn explicit_risk_posture_still_wins_over_category_policy() {
        let mut category = HashMap::new();
        category.insert(
            format!("{:?}", TaskCategory::CodeGen),
            TaskPolicyEntry {
                clutch: None,
                risk: Some("low".to_string()),
            },
        );
        let overrides = TaskPolicyOverrides {
            category,
            source: HashMap::new(),
        };

        let mut task = AgentTask::new(TaskId(4), "generate code", TaskPriority::Normal, vec![]);
        task.task_category = TaskCategory::CodeGen;
        task.risk_posture = Some(crate::mode::RiskPosture::High);

        // High risk resolves both enforce flags to false, overriding the
        // category's Low-risk policy.
        let (grounding_enforce, socrates_enforce) =
            resolve_grounding_socrates_enforce(&task, &overrides, true, true);
        assert!(!grounding_enforce);
        assert!(!socrates_enforce);
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
