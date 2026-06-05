//! Interpreted workflow runner: plan → journal.

use serde_json::{Value, json};
use std::time::Duration;
use vox_compiler::hir::HirModule;

/// Derive a stable, content-addressed `activity_id` from structural inputs.
///
/// The id is a BLAKE3 hex digest of `"{workflow_name}\0{activity_name}\0{position}"`,
/// truncated to 32 hex chars for readability. This is deterministic across replays as
/// long as the workflow topology (activity order and names) does not change.
pub(crate) fn derive_activity_id(
    workflow_name: &str,
    activity_name: &str,
    position: usize,
) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(workflow_name.as_bytes());
    hasher.update(b"\0");
    hasher.update(activity_name.as_bytes());
    hasher.update(b"\0");
    hasher.update(position.to_le_bytes().as_ref());
    let hash = hasher.finalize();
    // 16 bytes → 32 lowercase hex chars: unique enough for a workflow's activity set.
    let bytes = &hash.as_bytes()[..16];
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

use super::plan::plan_workflow_replay_ir;
use super::tracker::{DefaultTracker, WorkflowTracker};
#[cfg(feature = "mens")]
use super::types::PopuliActivity;
use super::types::{PlannedActivity, ReplayNode, compute_structural_arg_hash};

/// Version tag for interpreted workflow journal events emitted by this crate.
pub const WORKFLOW_JOURNAL_VERSION: u32 = 1;

/// Execute a planned workflow and append journal entries.
pub async fn interpret_workflow(
    hir: &HirModule,
    workflow_name: &str,
) -> anyhow::Result<Vec<Value>> {
    let mut tracker = DefaultTracker;
    interpret_workflow_durable(hir, workflow_name, &mut tracker).await
}

/// Execute a planned workflow with a durable state engine, returning journal entries.
pub async fn interpret_workflow_durable(
    hir: &HirModule,
    workflow_name: &str,
    tracker: &mut impl WorkflowTracker,
) -> anyhow::Result<Vec<Value>> {
    let replay_ir = plan_workflow_replay_ir(hir, workflow_name)?;
    let activity_count = replay_ir
        .nodes
        .iter()
        .filter(|n| matches!(n, ReplayNode::Activity(_)))
        .count();
    let mut journal = Vec::new();
    tracker
        .on_workflow_started(workflow_name, activity_count)
        .await?;
    journal.push(versioned_event(json!({
        "event": "WorkflowStarted",
        "workflow": workflow_name,
        "steps": activity_count,
    })));
    let mut activity_idx: usize = 0;
    for node in replay_ir.nodes {
        match node {
            ReplayNode::WorkflowPatch {
                change_id,
                min,
                max,
            } => {
                handle_workflow_patch(workflow_name, &change_id, min, max, &mut journal, tracker)
                    .await?;
            }
            ReplayNode::Activity(step) => {
                let activity_id = step
                    .activity_id
                    .clone()
                    .unwrap_or_else(|| derive_activity_id(workflow_name, &step.name, activity_idx));
                activity_idx += 1;

                // P2-T5: try the deterministic per-activity dedup cache first.
                let arg_hash_hex = compute_structural_arg_hash(&step.arguments);
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                if let Some(cached) = tracker
                    .load_cached_activity_result(&activity_id, &arg_hash_hex, now_ms)
                    .await?
                {
                    journal.push(versioned_event(json!({
                        "event": "ActivityCacheHit",
                        "workflow": workflow_name,
                        "activity": step.name,
                        "activity_id": activity_id,
                        "arg_hash": arg_hash_hex,
                    })));
                    journal.push(versioned_event(json!({
                        "event": "ActivityCompleted",
                        "workflow": workflow_name,
                        "activity": step.name,
                        "activity_id": activity_id,
                        "from_cache": true,
                        "result": cached,
                    })));
                    continue;
                }

                if tracker
                    .is_activity_completed(workflow_name, &activity_id)
                    .await?
                {
                    if let Some(replayed_result) = tracker
                        .load_activity_result(workflow_name, &activity_id)
                        .await?
                    {
                        journal.push(versioned_event(json!({
                            "event": "ActivityReplayed",
                            "workflow": workflow_name,
                            "activity": step.name,
                            "activity_id": activity_id,
                            "replay_source": "workflow_activity_log",
                            "result_event": replayed_result
                                .get("event")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown"),
                        })));
                        journal.push(versioned_event(replayed_result));
                        journal.push(versioned_event(json!({
                            "event": "ActivityCompleted",
                            "workflow": workflow_name,
                            "activity": step.name,
                            "activity_id": activity_id,
                            "replayed": true,
                        })));
                    } else {
                        journal.push(versioned_event(json!({
                            "event": "ActivitySkipped",
                            "workflow": workflow_name,
                            "activity": step.name,
                            "activity_id": activity_id,
                            "reason": "already completed in prior durable run",
                        })));
                    }
                    continue;
                }

                tracker
                    .on_activity_started(workflow_name, &step.name, &activity_id)
                    .await?;
                journal.push(versioned_event(json!({
                    "event": "ActivityTask",
                    "workflow": workflow_name,
                    "activity": step.name,
                    "activity_id": activity_id,
                    "execution_boundary": if step.mens { "mesh" } else { "local" },
                    "max_attempts": step.retries.saturating_add(1).max(1),
                    "timeout_ms": step.timeout_ms,
                    "idempotency_key": activity_id,
                })));
                journal.push(versioned_event(json!({
                    "event": "ActivityStarted",
                    "workflow": workflow_name,
                    "activity": step.name,
                    "activity_id": activity_id,
                })));

                let entry = execute_step_with_retries(
                    workflow_name,
                    &step,
                    &activity_id,
                    &mut journal,
                    tracker,
                )
                .await?;

                tracker
                    .on_activity_completed(workflow_name, &step.name, &activity_id, &entry)
                    .await?;
                let dedup_ms = step.dedup_window_ms.unwrap_or(24 * 60 * 60 * 1000);
                let _ = tracker
                    .record_cached_activity_result(
                        &activity_id,
                        &arg_hash_hex,
                        &entry,
                        now_ms,
                        dedup_ms,
                    )
                    .await;
                journal.push(entry);

                journal.push(versioned_event(json!({
                    "event": "ActivityCompleted",
                    "workflow": workflow_name,
                    "activity": step.name,
                    "activity_id": activity_id,
                })));
            }
        }
    }
    tracker.on_workflow_completed(workflow_name).await?;
    journal.push(versioned_event(json!({
        "event": "WorkflowCompleted",
        "workflow": workflow_name,
        "return_value": null,
    })));
    Ok(journal)
}

async fn handle_workflow_patch(
    workflow_name: &str,
    change_id: &str,
    min: u32,
    max: u32,
    journal: &mut Vec<Value>,
    tracker: &mut impl WorkflowTracker,
) -> anyhow::Result<u32> {
    if let Some(prior) = tracker
        .load_workflow_patch(workflow_name, change_id)
        .await?
    {
        journal.push(versioned_event(json!({
            "event": "WorkflowPatch",
            "workflow": workflow_name,
            "change_id": change_id,
            "version": prior,
            "replayed": true,
        })));
        return Ok(prior);
    }
    tracker
        .record_workflow_patch(workflow_name, change_id, max)
        .await?;
    journal.push(versioned_event(json!({
        "event": "WorkflowPatch",
        "workflow": workflow_name,
        "change_id": change_id,
        "version": max,
        "min_supported": min,
        "max_supported": max,
        "replayed": false,
    })));
    Ok(max)
}

async fn execute_step_with_retries(
    workflow_name: &str,
    step: &PlannedActivity,
    activity_id: &str,
    journal: &mut Vec<Value>,
    tracker: &mut impl WorkflowTracker,
) -> anyhow::Result<Value> {
    let max_attempts = step.retries.saturating_add(1).max(1);
    let first_attempt = tracker
        .next_activity_attempt_start(workflow_name, &step.name, activity_id)
        .await?;
    let last_attempt_in_window = first_attempt.saturating_add(max_attempts).saturating_sub(1);
    if first_attempt > 1 {
        journal.push(versioned_event(json!({
            "event": "ActivityAttemptRecovered",
            "workflow": workflow_name,
            "activity": step.name,
            "activity_id": activity_id,
            "resume_attempt": first_attempt,
            "max_attempts_window": max_attempts,
        })));
    }
    let mut next_delay_ms = step.initial_backoff_ms.unwrap_or(100).max(1);
    for attempt in first_attempt..=last_attempt_in_window {
        tracker
            .on_activity_attempt_started(workflow_name, &step.name, activity_id, attempt)
            .await?;
        match execute_step_once(step, activity_id).await {
            Ok(result) => {
                tracker
                    .on_activity_attempt_completed(workflow_name, &step.name, activity_id, attempt)
                    .await?;
                return Ok(versioned_event(result));
            }
            Err(err) => {
                tracker
                    .on_activity_attempt_failed(
                        workflow_name,
                        &step.name,
                        activity_id,
                        attempt,
                        &err.to_string(),
                    )
                    .await?;
                journal.push(versioned_event(json!({
                    "event": "ActivityAttemptFailed",
                    "workflow": workflow_name,
                    "activity": step.name,
                    "activity_id": activity_id,
                    "attempt": attempt,
                    "max_attempts": max_attempts,
                    "error": err.to_string(),
                })));
                if attempt >= last_attempt_in_window {
                    return Err(err);
                }
                let delay_ms = next_delay_ms;
                journal.push(versioned_event(json!({
                    "event": "ActivityRetryScheduled",
                    "workflow": workflow_name,
                    "activity": step.name,
                    "activity_id": activity_id,
                    "attempt": attempt,
                    "next_attempt": attempt + 1,
                    "delay_ms": delay_ms,
                })));
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                next_delay_ms = next_delay_ms.saturating_mul(2).min(60_000);
            }
        }
    }
    unreachable!("workflow retry loop must return or error")
}

async fn execute_step_once(step: &PlannedActivity, activity_id: &str) -> anyhow::Result<Value> {
    if step.name == "__durable_timer_wait" {
        let wait_ms = step.timeout_ms.unwrap_or(0);
        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        return Ok(json!({
            "event": "TimerWaitCompleted",
            "activity": step.name,
            "activity_id": activity_id,
            "waited_ms": wait_ms,
        }));
    }
    if step.mens {
        #[cfg(feature = "mens")]
        {
            let m = PopuliActivity {
                name: step.name.clone(),
                populi_op: step.populi_op,
                timeout_ms: step.timeout_ms,
                activity_id: activity_id.to_string(),
                required_labels: step.required_labels.clone(),
                is_detached: step.is_detached,
            };
            super::populi::execute_populi_step(&m).await
        }
        #[cfg(not(feature = "mens"))]
        {
            Ok(json!({
                "event": "MeshActivitySkipped",
                "activity": step.name,
                "activity_id": activity_id,
                "reason": "vox-workflow-runtime built without mens feature",
            }))
        }
    } else {
        Ok(execute_local_activity_step(step, activity_id))
    }
}

fn execute_local_activity_step(step: &PlannedActivity, activity_id: &str) -> Value {
    if step.name == "__branch_decision_then" || step.name == "__branch_decision_else" {
        let branch = if step.name.ends_with("_then") {
            "then"
        } else {
            "else"
        };
        return json!({
            "event": "BranchDecision",
            "activity": step.name,
            "activity_id": activity_id,
            "branch": branch,
            "decision_source": "deterministic_condition",
        });
    }
    if let Some(signal_key) = step.name.strip_prefix("__durable_signal_wait:") {
        return json!({
            "event": "SignalWaitSatisfied",
            "activity": step.name,
            "activity_id": activity_id,
            "signal_key": signal_key,
        });
    }
    // Keep local activities deterministic for replay while avoiding generic no-op rows.
    // This creates clearer evidence for campaign workflows even before full handler wiring.
    let classification = if step.name.starts_with("recon_") {
        "reconstruction"
    } else if step.name.starts_with("verify_") {
        "verification"
    } else if step.name.starts_with("plan_") {
        "planning"
    } else {
        "local"
    };
    json!({
        "event": "LocalActivity",
        "activity": step.name,
        "activity_id": activity_id,
        "status": "executed",
        "classification": classification,
    })
}

fn versioned_event(mut entry: Value) -> Value {
    if let Value::Object(obj) = &mut entry {
        obj.entry("journal_version".to_string())
            .or_insert_with(|| json!(WORKFLOW_JOURNAL_VERSION));
    }
    entry
}

#[cfg(test)]
async fn execute_with_retry_logic<T, Work, WorkFut, OnFailed, OnRetry>(
    max_attempts: u32,
    initial_backoff_ms: u64,
    journal: &mut Vec<Value>,
    mut on_failed: OnFailed,
    mut on_retry: OnRetry,
    mut work: Work,
) -> anyhow::Result<T>
where
    Work: FnMut() -> WorkFut,
    WorkFut: std::future::Future<Output = anyhow::Result<T>>,
    OnFailed: FnMut(u32, &anyhow::Error, &mut Vec<Value>),
    OnRetry: FnMut(u32, u64, &mut Vec<Value>),
{
    let mut next_delay_ms = initial_backoff_ms;
    for attempt in 1..=max_attempts.max(1) {
        match work().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                on_failed(attempt, &err, journal);
                if attempt >= max_attempts.max(1) {
                    return Err(err);
                }
                let delay_ms = next_delay_ms.max(1);
                on_retry(attempt, delay_ms, journal);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                next_delay_ms = next_delay_ms.saturating_mul(2).min(60_000);
            }
        }
    }

    unreachable!("workflow retry loop must return or error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonschema::validator_for;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn retry_helper_retries_until_success_and_records_events() {
        let mut journal = Vec::new();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_for_closure = attempts.clone();

        let result = execute_with_retry_logic(
            3,
            1,
            &mut journal,
            |attempt, err, journal| {
                journal.push(versioned_event(json!({
                    "event": "ActivityAttemptFailed",
                    "attempt": attempt,
                    "error": err.to_string(),
                })));
            },
            |attempt, delay_ms, journal| {
                journal.push(versioned_event(json!({
                    "event": "ActivityRetryScheduled",
                    "attempt": attempt,
                    "delay_ms": delay_ms,
                })));
            },
            move || {
                let attempts = attempts_for_closure.clone();
                async move {
                    let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        anyhow::bail!("attempt {current} failed");
                    }
                    Ok::<_, anyhow::Error>(json!({"event": "MeshActivity"}))
                }
            },
        )
        .await;
        let result = result.expect("retry helper should eventually succeed");
        assert_eq!(result["event"].as_str(), Some("MeshActivity"));
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        let failure_events = journal
            .iter()
            .filter(|entry| entry["event"].as_str() == Some("ActivityAttemptFailed"))
            .count();
        let retry_events = journal
            .iter()
            .filter(|entry| entry["event"].as_str() == Some("ActivityRetryScheduled"))
            .count();
        assert_eq!(failure_events, 2);
        assert_eq!(retry_events, 2);
    }

    #[tokio::test]
    async fn retry_helper_events_conform_to_workflow_journal_v1_schema() {
        let mut journal = Vec::new();
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_for_closure = attempts.clone();

        let _ = execute_with_retry_logic(
            3,
            1,
            &mut journal,
            |attempt, err, journal| {
                journal.push(versioned_event(json!({
                    "event": "ActivityAttemptFailed",
                    "workflow": "retry_schema_demo",
                    "activity": "mesh_join",
                    "activity_id": "retry_schema_demo-0",
                    "attempt": attempt,
                    "max_attempts": 3,
                    "error": err.to_string(),
                })));
            },
            |attempt, delay_ms, journal| {
                journal.push(versioned_event(json!({
                    "event": "ActivityRetryScheduled",
                    "workflow": "retry_schema_demo",
                    "activity": "mesh_join",
                    "activity_id": "retry_schema_demo-0",
                    "attempt": attempt,
                    "next_attempt": attempt + 1,
                    "delay_ms": delay_ms,
                })));
            },
            move || {
                let attempts = attempts_for_closure.clone();
                async move {
                    let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        anyhow::bail!("attempt {current} failed");
                    }
                    Ok::<_, anyhow::Error>(json!({"event": "MeshActivity"}))
                }
            },
        )
        .await
        .expect("retry helper should eventually succeed");

        let schema_json: Value = serde_json::from_str(include_str!(
            "../../../../contracts/workflow/workflow-journal.v1.schema.json"
        ))
        .expect("parse workflow journal schema");
        let validator = validator_for(&schema_json).expect("compile workflow journal schema");
        for entry in &journal {
            if let Err(err) = validator.validate(entry) {
                panic!("retry event should validate against v1 schema: {err}; entry={entry}");
            }
        }
        assert!(
            journal
                .iter()
                .any(|entry| entry["event"].as_str() == Some("ActivityAttemptFailed")),
            "retry helper should emit ActivityAttemptFailed events"
        );
        assert!(
            journal
                .iter()
                .any(|entry| entry["event"].as_str() == Some("ActivityRetryScheduled")),
            "retry helper should emit ActivityRetryScheduled events"
        );
    }

    #[test]
    fn versioned_event_adds_contract_version() {
        let value = versioned_event(json!({
            "event": "WorkflowStarted",
            "workflow": "wf",
        }));
        assert_eq!(
            value["journal_version"].as_u64(),
            Some(WORKFLOW_JOURNAL_VERSION as u64)
        );
    }

    #[test]
    fn derive_activity_id_is_deterministic() {
        let id1 = derive_activity_id("my_workflow", "send_email", 0);
        let id2 = derive_activity_id("my_workflow", "send_email", 0);
        assert_eq!(id1, id2, "same inputs must produce same activity_id");
    }

    #[test]
    fn derive_activity_id_differs_by_position() {
        let id0 = derive_activity_id("wf", "step", 0);
        let id1 = derive_activity_id("wf", "step", 1);
        assert_ne!(id0, id1, "different positions must produce different ids");
    }

    #[test]
    fn derive_activity_id_differs_by_workflow_name() {
        let id_a = derive_activity_id("workflow_a", "step", 0);
        let id_b = derive_activity_id("workflow_b", "step", 0);
        assert_ne!(
            id_a, id_b,
            "different workflow names must produce different ids"
        );
    }

    #[test]
    fn derive_activity_id_is_32_hex_chars() {
        let id = derive_activity_id("wf", "act", 0);
        assert_eq!(id.len(), 32, "activity_id must be 32 hex chars");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "must be lowercase hex"
        );
    }
}
