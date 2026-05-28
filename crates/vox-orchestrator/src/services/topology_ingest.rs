//! Ingestion service for Network Neuroscience Theory (NNT) routing telemetry.
//!
//! Reconstructs agent topology from persistent lineage events and calculates
//! `routing_efficiency` scalars for reinforcement learning.

use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;

use crate::topology::{
    AffinityMatrix, AgentRole, AgentTopologyNode, AgentTopologySnapshot, DelegationEdge,
};
use crate::types::{AgentId, TaskId};
use vox_corpus::tool_workflow_corpus::WorkflowTraceRecord;
use vox_db::VoxDb;

/// Scans the persistent lineage for one repository and emits `WorkflowTraceRecord` pairs.
/// These pairs carry the `routing_efficiency` scalar used as a reward signal in GRPO.
pub async fn ingest_workflow_traces_to_jsonl(
    db: &VoxDb,
    repository_id: &str,
    out: &mut impl Write,
) -> anyhow::Result<usize> {
    // 1. Fetch lineage events (capped at 5k for the ingestion pass)
    let events = db
        .list_orchestration_lineage_events(repository_id, None, 5000)
        .await?;

    // 2. Group events by session_id to isolate individual workflows
    let mut sessions: HashMap<String, Vec<Value>> = HashMap::new();
    for ev in events {
        if let Some(sid) = ev.get("session_id").and_then(|v| v.as_str()) {
            sessions.entry(sid.to_string()).or_default().push(ev);
        }
    }

    let mut count = 0;
    for (session_id, evs) in sessions {
        // 3. Reconstruct the topology for this session
        let mut nodes: HashMap<AgentId, AgentTopologyNode> = HashMap::new();
        let mut edges: Vec<DelegationEdge> = Vec::new();

        for ev in evs {
            let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let payload: Value = ev
                .get("payload_json")
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(Value::Null);

            match kind {
                "agent_spawned" => {
                    if let (Some(aid), Some(name)) = (
                        payload.get("agent_id").and_then(|v| v.as_i64()),
                        payload.get("name").and_then(|v| v.as_str()),
                    ) {
                        let agent_id = AgentId(aid as u64);
                        // Heuristic for role from name (consistent with orchestrator role assignment)
                        let role = if name.contains("verify") || name.contains("review") {
                            AgentRole::Verifier
                        } else if name.contains("plan") {
                            AgentRole::Planner
                        } else if name.contains("research") {
                            AgentRole::Researcher
                        } else if name.contains("synth") {
                            AgentRole::Synthesizer
                        } else if name.contains("exec") || name.contains("worker") {
                            AgentRole::Executor
                        } else {
                            AgentRole::Generalist
                        };

                        nodes.insert(
                            agent_id,
                            AgentTopologyNode {
                                agent_id,
                                name: name.to_string(),
                                role,
                                dynamic: true,
                                parent_agent_id: None,
                                source_task_id: None,
                                spawn_reason: None,
                                child_count: 0,
                                queued: 0,
                                in_progress: false,
                                paused: false,
                                agent_session_id: Some(session_id.clone()),
                                current_phase: None,
                            },
                        );
                    }
                }
                "task_delegated" | "plan_handoff" => {
                    let from = payload
                        .get("from")
                        .or_else(|| payload.get("parent_agent_id"))
                        .and_then(|v| v.as_i64());
                    let to = payload
                        .get("to")
                        .or_else(|| payload.get("child_agent_id"))
                        .and_then(|v| v.as_i64());
                    let task_id = payload
                        .get("task_id")
                        .and_then(|v| v.as_i64())
                        .map(|i| TaskId(i as u64));

                    if let (Some(f), Some(t)) = (from, to) {
                        let parent = AgentId(f as u64);
                        let child = AgentId(t as u64);
                        edges.push(DelegationEdge {
                            parent_agent_id: parent,
                            child_agent_id: child,
                            source_task_id: task_id,
                            reason: payload
                                .get("reason")
                                .or_else(|| payload.get("plan_summary"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        });

                        // Update child's parent link in the reconstructed node set
                        if let Some(node) = nodes.get_mut(&child) {
                            node.parent_agent_id = Some(parent);
                        }
                    }
                }
                _ => {}
            }
        }

        // Skip sessions with no delegation (no routing to evaluate)
        if edges.is_empty() {
            continue;
        }

        // 4. Calculate NNT routing efficiency
        let snapshot = AgentTopologySnapshot {
            generated_at_ms: 0,
            nodes: nodes.into_values().collect(),
            delegation_edges: edges,
            known_gaps: vec![],
        };

        let penalty = AffinityMatrix::routing_efficiency_penalty(&snapshot);
        // Normalize: higher is better. Max distance per hop is 3.
        let efficiency = if !snapshot.delegation_edges.is_empty() {
            (1.0 - (penalty as f64 / (snapshot.delegation_edges.len() as f64 * 3.0)))
                .clamp(0.0, 1.0)
        } else {
            1.0
        };

        // 5. Create WorkflowTraceRecord for SFT/GRPO training
        let record = WorkflowTraceRecord {
            intent: "NNT Small-World Routing Evaluation".to_string(),
            workflow_name: Some("autonomous_routing".to_string()),
            execution_log_excerpt: format!("Session {} Efficiency: {:.4}", session_id, efficiency),
            synthesized_vox: None,
            routing_efficiency: Some(efficiency),
        };

        writeln!(out, "{}", serde_json::to_string(&record)?)?;
        count += 1;
    }

    Ok(count)
}
