use anyhow::Result;
use tracing;

use crate::ApprovalTier;
use crate::orchestrator::task_dispatch::complete::harness::{
    HarnessCompletionGuardMode, completion_attestation_satisfies_tier,
    harness_completion_guard_mode, harness_completion_issues,
};
use crate::types::{AgentTask, CompletionAttestation, TaskStatus};

pub struct GateOutcome {
    pub requeue: Option<(AgentTask, String, usize, usize)>,
    /// When `true`, the task requires human Review-tier approval before it can proceed.
    /// The caller should set `BlockedOnApproval` status and create a `HitlApprovalRow`.
    pub needs_review_approval: bool,
}

pub fn check_behavioral_gate(behavioral_failure: &mut Option<String>) -> Option<String> {
    behavioral_failure.take()
}

pub fn check_research_json_gate(
    task: &AgentTask,
    attestation: Option<&CompletionAttestation>,
    max_debug_iterations: u8,
) -> Result<GateOutcome, String> {
    if task.task_category != crate::types::TaskCategory::Research {
        return Ok(GateOutcome { requeue: None, needs_review_approval: false });
    }

    let is_json = attestation
        .and_then(|a| a.completion_summary.as_ref())
        .map(|s| {
            let mut text = s.trim();
            if text.starts_with("```json") {
                text = text
                    .trim_start_matches("```json")
                    .trim_end_matches("```")
                    .trim();
            }
            serde_json::from_str::<serde_json::Value>(text).is_ok()
        })
        .unwrap_or(false);

    if !is_json {
        if task.debug_iterations < max_debug_iterations {
            let mut task_clone = task.clone();
            task_clone.debug_iterations += 1;
            task_clone.description.push_str(
                "\n\n[SCHEMA COMPLIANCE GATE]\nResearch logic must output a valid JSON response in the completion attestation summary to prevent anomalous routing instructions."
            );
            task_clone.status = TaskStatus::Queued;
            Ok(GateOutcome {
                requeue: Some((
                    task_clone,
                    "JSON schema compliance failure on Research task".to_string(),
                    1,
                    0,
                )),
                needs_review_approval: false,
            })
        } else {
            Err("Research tasks require a valid JSON completion summary.".to_string())
        }
    } else {
        Ok(GateOutcome { requeue: None, needs_review_approval: false })
    }
}

pub fn check_approval_gate(
    task: &AgentTask,
    attestation: Option<&CompletionAttestation>,
    max_debug_iterations: u8,
) -> Result<GateOutcome, String> {
    let Some(tier) = task.approval_tier else {
        return Ok(GateOutcome { requeue: None, needs_review_approval: false });
    };

    let ok = completion_attestation_satisfies_tier(tier, attestation);
    if !ok {
        // Review tier: surface as a human approval request rather than re-queuing
        // autonomically. The caller will set BlockedOnApproval and create a HitlApprovalRow.
        if tier == ApprovalTier::Review {
            return Ok(GateOutcome {
                requeue: None,
                needs_review_approval: true,
            });
        }

        if task.debug_iterations < max_debug_iterations {
            let mut task_clone = task.clone();
            task_clone.debug_iterations += 1;
            let requirement = match tier {
                ApprovalTier::Confirm => {
                    "completion attestation with non-empty checks_passed[] (or declared_non_placeholder=true)"
                }
                ApprovalTier::Review => {
                    "checks_passed[] containing one of: human_review_approved, peer_review_approved, approval_reviewed"
                }
                ApprovalTier::Blocked => {
                    "task is approval-blocked and cannot be completed without policy override"
                }
                ApprovalTier::AutoApprove => "no extra attestation required",
            };
            task_clone.description.push_str(&format!(
                "\n\n[APPROVAL GATE]\nTask tier `{tier:?}` requires {requirement} before completion."
            ));
            task_clone.status = TaskStatus::Queued;
            Ok(GateOutcome {
                requeue: Some((
                    task_clone,
                    format!("approval gate blocked completion for tier {tier:?}"),
                    1,
                    0,
                )),
                needs_review_approval: false,
            })
        } else {
            Err(format!(
                "task {} failed approval attestation for tier {tier:?}",
                task.id
            ))
        }
    } else {
        Ok(GateOutcome { requeue: None, needs_review_approval: false })
    }
}

pub fn check_trust_gate(
    task: &AgentTask,
    attestation: Option<&CompletionAttestation>,
    trust_score: f64,
    trust_floor: f64,
    max_toestub_debug_iterations: u8,
    phase_label: &str,
) -> GateOutcome {
    let has_checks = attestation
        .map(|a| !a.checks_passed.is_empty())
        .unwrap_or(false);
    if trust_score < trust_floor
        && !has_checks
        && task.toestub_iterations < max_toestub_debug_iterations
    {
        let mut task_clone = task.clone();
        task_clone.toestub_iterations += 1;
        task_clone.description.push_str(&format!(
            "\n\n[TRUST GATE]\nAgent task-completion trust {:.2} is below floor {:.2} for phase `{}`. \
Provide completion attestation checks_passed[] before completing.",
            trust_score, trust_floor, phase_label
        ));
        task_clone.status = TaskStatus::Queued;
        GateOutcome {
            requeue: Some((
                task_clone,
                format!(
                    "trust gate blocked completion (score {:.2} < floor {:.2}, checks_passed missing)",
                    trust_score, trust_floor
                ),
                1,
                0,
            )),
            needs_review_approval: false,
        }
    } else {
        GateOutcome { requeue: None, needs_review_approval: false }
    }
}

pub fn check_harness_gate(
    task: &AgentTask,
    attestation: Option<&CompletionAttestation>,
    max_debug_iterations: u8,
) -> Result<GateOutcome, String> {
    let guard_mode = harness_completion_guard_mode();
    if guard_mode == HarnessCompletionGuardMode::Off {
        return Ok(GateOutcome { requeue: None, needs_review_approval: false });
    }

    match harness_completion_issues(task, attestation) {
        Ok((harness_id, issues)) if !issues.is_empty() => {
            let detail = format!(
                "{}{}",
                harness_id
                    .as_deref()
                    .map(|h| format!("harness_id={h}; "))
                    .unwrap_or_default(),
                issues.join("; ")
            );
            tracing::warn!(task_id = task.id.0, mode = ?guard_mode, "{}", detail);
            if guard_mode == HarnessCompletionGuardMode::Enforce {
                if task.debug_iterations < max_debug_iterations {
                    let mut task_clone = task.clone();
                    task_clone.debug_iterations += 1;
                    task_clone.description.push_str(&format!(
                        "\n\n[HARNESS COMPLETION GATE]\nCompletion blocked until harness requirements are satisfied:\n{}",
                        detail
                    ));
                    task_clone.status = TaskStatus::Queued;
                    Ok(GateOutcome {
                        requeue: Some((task_clone, detail.clone(), 1, 0)),
                        needs_review_approval: false,
                    })
                } else {
                    Err(format!(
                        "harness completion gate rejected completion for task {}: {detail}",
                        task.id
                    ))
                }
            } else {
                Ok(GateOutcome { requeue: None, needs_review_approval: false })
            }
        }
        Ok(_) => Ok(GateOutcome { requeue: None, needs_review_approval: false }),
        Err(err) => {
            tracing::warn!(task_id = task.id.0, mode = ?guard_mode, error = %err, "harness completion gate evaluation failed");
            if guard_mode == HarnessCompletionGuardMode::Enforce {
                Err(format!(
                    "harness completion gate failed to evaluate task {}: {err}",
                    task.id
                ))
            } else {
                Ok(GateOutcome { requeue: None, needs_review_approval: false })
            }
        }
    }
}

#[cfg(feature = "toestub-gate")]
pub fn check_toestub_gate(
    task: &AgentTask,
    attestation: Option<&CompletionAttestation>,
    max_toestub_debug_iterations: u8,
) -> Result<GateOutcome, String> {
    match crate::services::PolicyEngine::check_completion_before_complete(Some(task), attestation) {
        crate::services::PolicyCheckResult::Allowed => {}
        crate::services::PolicyCheckResult::ScopeDenied(reason) => {
            if task.debug_iterations < max_toestub_debug_iterations {
                let mut task_clone = task.clone();
                task_clone.debug_iterations += 1;
                task_clone.description.push_str(&format!(
                    "\n\n[AUTO-DEBUG ITERATION {}]\nCompletion policy blocked completion. Fix and retry:\n{}",
                    task_clone.debug_iterations, reason
                ));
                task_clone.status = TaskStatus::Queued;
                return Ok(GateOutcome {
                    requeue: Some((task_clone, reason, 1, 0)),
                    needs_review_approval: false,
                });
            } else {
                return Err(reason);
            }
        }
        crate::services::PolicyCheckResult::LockConflict(conflict) => {
            // This is a bit tricky, LockConflict usually returns an OrchestratorError.
            // But we'll handle it as a String error for now or wrap it.
            return Err(format!("Lock conflict: {conflict}"));
        }
    }

    let vr = crate::validation::post_task_validate(task, attestation);
    if !crate::validation::quality_gate(&vr) && task.debug_iterations < max_toestub_debug_iterations
    {
        let mut task_clone = task.clone();
        task_clone.debug_iterations += 1;
        task_clone.description.push_str(&format!(
            "\n\n[AUTO-DEBUG ITERATION {}]\nValidation failed with diagnostic issues. Please fix the following:\n{}",
            task_clone.debug_iterations, vr.report
        ));
        task_clone.status = TaskStatus::Queued;
        Ok(GateOutcome {
            requeue: Some((task_clone, vr.report, vr.error_count, vr.warning_count)),
            needs_review_approval: false,
        })
    } else {
        Ok(GateOutcome { requeue: None, needs_review_approval: false })
    }
}

pub fn check_doc_integrity_gate(
    task: &AgentTask,
    broken_links_report: &str,
    write_files: &[std::path::PathBuf],
    max_debug_iterations: u8,
) -> GateOutcome {
    let md_files: Vec<_> = write_files
        .iter()
        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
        .collect();

    if !md_files.is_empty() && !broken_links_report.is_empty() {
        let detail = format!(
            "Broken links detected. Please investigate if files need to be created or fix the links:\n{}",
            broken_links_report
        );
        if task.debug_iterations < max_debug_iterations {
            let mut task_clone = task.clone();
            task_clone.debug_iterations += 1;
            task_clone
                .description
                .push_str(&format!("\n\n[DOC INTEGRITY GATE]\n{}", detail));
            task_clone.status = TaskStatus::Queued;
            return GateOutcome {
                requeue: Some((task_clone, detail, 1, 0)),
                needs_review_approval: false,
            };
        }
    }
    GateOutcome { requeue: None, needs_review_approval: false }
}

#[cfg(test)]
mod review_approval_gate_tests {
    use super::*;
    use crate::types::{AgentTask, TaskId, TaskPriority, TaskStatus};

    fn make_task_with_tier(tier: crate::ApprovalTier) -> AgentTask {
        let mut t = AgentTask::new(
            TaskId(1),
            "Do something risky",
            TaskPriority::Normal,
            vec![],
        );
        t.approval_tier = Some(tier);
        t
    }

    #[test]
    fn review_risk_routes_completion_to_pending_approvals() {
        // Review-tier task with no attestation should signal needs_review_approval=true,
        // NOT requeue and NOT return an error.
        let task = make_task_with_tier(crate::ApprovalTier::Review);
        let outcome = check_approval_gate(&task, None, 3)
            .expect("check_approval_gate should not error for Review tier");
        assert!(
            outcome.needs_review_approval,
            "Review-tier task must set needs_review_approval=true"
        );
        assert!(
            outcome.requeue.is_none(),
            "Review-tier task must not be re-queued autonomically"
        );
    }

    #[test]
    fn confirm_tier_still_requeues() {
        // Confirm-tier should re-queue (not route to human inbox) when no attestation.
        let task = make_task_with_tier(crate::ApprovalTier::Confirm);
        let outcome = check_approval_gate(&task, None, 3)
            .expect("should not error on first iteration");
        assert!(
            !outcome.needs_review_approval,
            "Confirm-tier must not set needs_review_approval"
        );
        assert!(
            outcome.requeue.is_some(),
            "Confirm-tier should requeue when attestation is missing"
        );
    }

    #[test]
    fn auto_approve_tier_always_passes() {
        let task = make_task_with_tier(crate::ApprovalTier::AutoApprove);
        let outcome = check_approval_gate(&task, None, 3)
            .expect("AutoApprove should never fail");
        assert!(!outcome.needs_review_approval);
        assert!(outcome.requeue.is_none());
    }
}
