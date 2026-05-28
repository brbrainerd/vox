use std::collections::BTreeMap;

pub(crate) const MAX_REPLAY_PER_TICK: usize = 20;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ReplayStats {
    pub replayed_last_run: u64,
    pub replay_failed_last_run: u64,
    pub replay_failed_by_op: BTreeMap<String, u64>,
}

pub(crate) fn replay_op_label(entry: &serde_json::Value) -> String {
    let op = entry
        .get("replay")
        .and_then(serde_json::Value::as_object)
        .and_then(|replay| {
            replay
                .get("op")
                .and_then(serde_json::Value::as_str)
                .or_else(|| {
                    if replay.get("repository_id").is_some() && replay.get("kind").is_some() {
                        Some("append_orchestration_lineage_event")
                    } else {
                        None
                    }
                })
        });
    op.unwrap_or("unknown").to_string()
}

pub(crate) async fn replay_one_entry(
    db: &std::sync::Arc<vox_db::VoxDb>,
    entry: &serde_json::Value,
) -> bool {
    let Some(obj) = entry.as_object() else {
        return false;
    };
    let Some(replay) = obj.get("replay").and_then(serde_json::Value::as_object) else {
        return false;
    };
    let op = replay
        .get("op")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            // Backward-compat for lineage entries created before explicit replay op tagging.
            if replay.get("repository_id").is_some() && replay.get("kind").is_some() {
                Some("append_orchestration_lineage_event")
            } else {
                None
            }
        });

    match op {
        Some("append_orchestration_lineage_event") => {
            let Some(repository_id) = replay
                .get("repository_id")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(kind) = replay.get("kind").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(task_id) = replay.get("task_id").and_then(serde_json::Value::as_i64) else {
                return false;
            };
            let agent_id = replay.get("agent_id").and_then(serde_json::Value::as_i64);
            let session_id = replay.get("session_id").and_then(serde_json::Value::as_str);
            let workflow_id = replay
                .get("workflow_id")
                .and_then(serde_json::Value::as_str);
            let plan_session_id = replay
                .get("plan_session_id")
                .and_then(serde_json::Value::as_str);
            let plan_node_id = replay
                .get("plan_node_id")
                .and_then(serde_json::Value::as_str);
            let payload_json = replay
                .get("payload_json")
                .and_then(serde_json::Value::as_str);

            db.append_orchestration_lineage_event(
                repository_id,
                kind,
                task_id,
                agent_id,
                session_id,
                workflow_id,
                plan_session_id,
                plan_node_id,
                payload_json,
            )
            .await
            .is_ok()
        }
        Some("record_task_reliability_observation") => {
            let Some(agent_id) = replay.get("agent_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(success) = replay.get("success").and_then(serde_json::Value::as_bool) else {
                return false;
            };
            db.record_task_reliability_observation(agent_id, success)
                .await
                .is_ok()
        }
        Some("record_trust_observation") => {
            let Some(entity_type) = replay
                .get("entity_type")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(entity_id) = replay.get("entity_id").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(dimension) = replay.get("dimension").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let observation = vox_db::TrustObservationInput {
                entity_type,
                entity_id,
                dimension,
                domain: replay.get("domain").and_then(serde_json::Value::as_str),
                task_class: replay.get("task_class").and_then(serde_json::Value::as_str),
                provider: replay.get("provider").and_then(serde_json::Value::as_str),
                model_id: replay.get("model_id").and_then(serde_json::Value::as_str),
                repository_id: replay
                    .get("repository_id")
                    .and_then(serde_json::Value::as_str),
                source_kind: replay
                    .get("source_kind")
                    .and_then(serde_json::Value::as_str),
                observation_value: replay
                    .get("observation_value")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.5),
                confidence_weight: replay
                    .get("confidence_weight")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1.0),
                sample_size: replay
                    .get("sample_size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1) as i64,
                artifact_ref: replay
                    .get("artifact_ref")
                    .and_then(serde_json::Value::as_str),
                metadata_json: replay
                    .get("metadata_json")
                    .and_then(serde_json::Value::as_str),
                ewma_alpha: replay
                    .get("ewma_alpha")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.10),
            };
            db.record_trust_observation(observation).await.is_ok()
        }
        Some("record_plan_node_attempt") => {
            let Some(plan_session_id) = replay
                .get("plan_session_id")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(version) = replay.get("version").and_then(serde_json::Value::as_i64) else {
                return false;
            };
            let Some(node_id) = replay.get("node_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let attempt_no = replay
                .get("attempt_no")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(1);
            let Some(outcome) = replay.get("outcome").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let task_id = replay.get("task_id").and_then(serde_json::Value::as_str);
            let error_text = replay.get("error_text").and_then(serde_json::Value::as_str);
            let latency_ms = replay.get("latency_ms").and_then(serde_json::Value::as_i64);
            db.record_plan_node_attempt(
                plan_session_id,
                version,
                node_id,
                attempt_no,
                task_id,
                outcome,
                error_text,
                latency_ms,
            )
            .await
            .is_ok()
        }
        Some("set_plan_node_status") => {
            let Some(plan_session_id) = replay
                .get("plan_session_id")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(version) = replay.get("version").and_then(serde_json::Value::as_i64) else {
                return false;
            };
            let Some(node_id) = replay.get("node_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(status) = replay.get("status").and_then(serde_json::Value::as_str) else {
                return false;
            };
            db.set_plan_node_status(plan_session_id, version, node_id, status)
                .await
                .is_ok()
        }
        Some("append_plan_version") => {
            let Some(plan_session_id) = replay
                .get("plan_session_id")
                .and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(version) = replay.get("version").and_then(serde_json::Value::as_i64) else {
                return false;
            };
            let parent_version = replay
                .get("parent_version")
                .and_then(serde_json::Value::as_i64);
            let origin = replay.get("origin").and_then(serde_json::Value::as_str);
            let metadata_json = replay
                .get("metadata_json")
                .and_then(serde_json::Value::as_str);
            db.append_plan_version(
                plan_session_id,
                version,
                parent_version,
                origin,
                metadata_json,
            )
            .await
            .is_ok()
        }
        Some("insert_victory_verdict") => {
            let Some(task_id) = replay.get("task_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(verdict_val) = replay.get("verdict") else {
                return false;
            };
            // VictoryVerdict is in crates/vox-orchestrator/src/gate/victory.rs or similar
            // But we can use serde_json::from_value if it's available.
            // Wait! VictoryVerdict is used in success handler.
            if let Ok(verdict) =
                serde_json::from_value::<crate::VictoryVerdict>(verdict_val.clone())
            {
                db.insert_victory_verdict(task_id, &verdict).await.is_ok()
            } else {
                false
            }
        }
        Some("insert_observer_event") => {
            let Some(session_id) = replay.get("session_id").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(task_id) = replay.get("task_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(report_val) = replay.get("report") else {
                return false;
            };
            if let Ok(report) =
                serde_json::from_value::<crate::ObservationReport>(report_val.clone())
            {
                db.insert_observer_event(session_id, task_id, &report)
                    .await
                    .is_ok()
            } else {
                false
            }
        }
        Some("insert_telemetry_flat_raw") => {
            let Some(agent_id) = replay.get("agent_id").and_then(serde_json::Value::as_str) else {
                return false;
            };
            let Some(session_id) = replay.get("session_id").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let Some(event_kind) = replay.get("event_kind").and_then(serde_json::Value::as_str)
            else {
                return false;
            };
            let tenant_id = replay.get("tenant_id").and_then(serde_json::Value::as_str);
            let repository_id = crate::lineage::repository_id();

            let _ = db.insert_telemetry_flat_raw(
                agent_id,
                session_id,
                tenant_id,
                &repository_id,
                event_kind,
                replay.get("tool_name").and_then(serde_json::Value::as_str),
                replay.get("model_id").and_then(serde_json::Value::as_str),
                replay.get("provider").and_then(serde_json::Value::as_str),
                replay.get("duration_ms").and_then(serde_json::Value::as_i64),
                replay.get("input_tokens").and_then(serde_json::Value::as_i64),
                replay.get("output_tokens").and_then(serde_json::Value::as_i64),
                replay.get("cost_usd").and_then(serde_json::Value::as_f64),
                replay.get("payload_json").and_then(serde_json::Value::as_str),
            ).await;
            true
        }
        _ => false,
    }
}
