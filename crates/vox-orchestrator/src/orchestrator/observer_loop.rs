use crate::orchestrator::Orchestrator;
use crate::types::AgentId;
use std::sync::Arc;
use std::time::Duration;

/// Autonomous observer daemon that watches for workflow blockages
/// and orchestrates MENS anomaly reporting.
pub async fn run_observer_loop(orch: Arc<Orchestrator>) {
    let cfg = orch.config.read().unwrap().clone();
    if !cfg.observer_enabled {
        return;
    }

    let interval = cfg.observer_poll_interval_ms;
    let mut ticker = tokio::time::interval(Duration::from_millis(interval));
    let mut hardware_ticker = tokio::time::interval(vox_config::timeouts::D_60S);
    let mut scoreboard_ticker = tokio::time::interval(vox_config::timeouts::D_300S); // Refresh every 5 minutes
    let _repository_id =
        vox_repository::discover_repository_or_fallback(std::path::Path::new(".")).repository_id;

    let _node_id = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
        .expose()
        .map(|s| s.trim().to_string());

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let assignments = {
                    let lock = orch.task_assignments.read().unwrap();
                    lock.clone()
                };

                for (_, agent_id) in assignments {
                    if let Some(queue) = orch.agent_queue(agent_id) {
                        let q = queue.read().unwrap();
                        let queue_depth = q.tasks().len();

                        if queue_depth > 10 {
                            tracing::warn!(
                                "MENS Observer: Agent {} is saturated with {} tasks.",
                                agent_id,
                                queue_depth
                            );

                            let msg_payload = serde_json::json!({
                                "observation_type": "QueueSaturation",
                                "queue_depth": queue_depth
                            });

                            let system_id = AgentId(1);
                            orch.message_bus.send(
                                system_id,
                                agent_id,
                                crate::a2a::A2AMessageType::SocratesResearchRequest,
                                msg_payload.to_string(),
                            );

                            orch.event_bus()
                                .emit(crate::events::AgentEventKind::MensObserverObservation {
                                    agent_id,
                                    observation_type: "QueueSaturation".to_string(),
                                    queue_depth,
                                });
                        } else if queue_depth == 0 && agent_id.0 > 0 {
                            tracing::info!(
                                "MENS Observer: Agent {} is idle. Triggering scale-down evaluation.",
                                agent_id
                            );

                            let msg_payload = serde_json::json!({
                                "observation_type": "IdleAgentDetected",
                                "queue_depth": 0
                            });

                            orch.message_bus.send(
                                AgentId(1),
                                agent_id,
                                crate::a2a::A2AMessageType::SocratesResearchRequest,
                                msg_payload.to_string(),
                            );

                            orch.event_bus()
                                .emit(crate::events::AgentEventKind::MensObserverObservation {
                                    agent_id,
                                    observation_type: "IdleScaleDown".to_string(),
                                    queue_depth: 0,
                                });
                        }
                    }
                }
            }
            _ = hardware_ticker.tick() => {
                #[cfg(feature = "populi-transport")]
                if let Some(telemetry) = vox_populi::mens::hardware::HardwareRegistry::monitor() {
                    let tel_json = serde_json::to_value(telemetry).unwrap();
                    vox_db::populi_registry_telemetry::record_hardware_telemetry_opt(
                        &_repository_id,
                        _node_id.as_deref(),
                        &tel_json,
                    ).await;
                }
            }
            _ = scoreboard_ticker.tick() => {
                orch.refresh_model_scoreboard().await;
            }
        }
    }
}
