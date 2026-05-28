//! Reliability service: records endpoint and agent performance observations.
//!
//! Bridges [`crate::events::AgentEvent`] signals to [`vox_db::VoxDb`] persistent EWMA.

use crate::events::{AgentEvent, AgentEventKind};
use crate::services::persistence_obs::log_persistence_failure;
use crate::types::AgentId;
use vox_db::VoxDb;

/// Service for persisting reliability observations to Codex.
pub struct ReliabilityService<'a> {
    store: &'a VoxDb,
}

impl<'a> ReliabilityService<'a> {
    /// Create a new service bound to the given store.
    pub fn new(store: &'a VoxDb) -> Self {
        Self { store }
    }

    /// Process an event and update reliability metrics if applicable.
    pub async fn handle_event(&self, event: &AgentEvent) {
        match &event.kind {
            AgentEventKind::EndpointReliabilityObservation {
                endpoint_url,
                model_id,
                hallucination_proxy,
                contradiction_ratio,
                infra_failure,
                rate_limit_hit,
                timeout_hit,
                ..
            } => {
                if let Err(e) = self
                    .store
                    .record_endpoint_observation(
                        endpoint_url,
                        model_id,
                        *hallucination_proxy,
                        *contradiction_ratio,
                        *infra_failure,
                        *rate_limit_hit,
                        *timeout_hit,
                    )
                    .await
                {
                    log_persistence_failure("reliability.endpoint_observation", e);
                }
            }
            AgentEventKind::TaskCompleted { agent_id, .. } => {
                let agent_str = agent_id.0.to_string();
                if let Err(e) = self
                    .store
                    .record_task_reliability_observation(&agent_str, true)
                    .await
                {
                    log_persistence_failure("reliability.task_completed_obs", e);
                }
            }
            AgentEventKind::TaskFailed { agent_id, .. } => {
                let agent_str = agent_id.0.to_string();
                if let Err(e) = self
                    .store
                    .record_task_reliability_observation(&agent_str, false)
                    .await
                {
                    log_persistence_failure("reliability.task_failed_obs", e);
                }
            }
            AgentEventKind::AgentHandoffAccepted { from, .. } => {
                let agent_str = from.0.to_string();
                if let Err(e) = self
                    .store
                    .record_task_reliability_observation(&agent_str, true)
                    .await
                {
                    log_persistence_failure("reliability.handoff_accepted_obs", e);
                }
            }
            AgentEventKind::AgentHandoffRejected { from, .. } => {
                let agent_str = from.0.to_string();
                if let Err(e) = self
                    .store
                    .record_task_reliability_observation(&agent_str, false)
                    .await
                {
                    log_persistence_failure("reliability.handoff_rejected_obs", e);
                }
            }
            _ => {}
        }
    }

    /// Helper for agents to report observations directly.
    pub async fn record_observation(
        &self,
        _agent_id: AgentId,
        endpoint_url: String,
        model_id: String,
        hallucination_proxy: f64,
        contradiction_ratio: f64,
        infra_failure: f64,
        rate_limit_hit: bool,
        timeout_hit: bool,
    ) {
        if let Err(e) = self
            .store
            .record_endpoint_observation(
                &endpoint_url,
                &model_id,
                hallucination_proxy,
                contradiction_ratio,
                infra_failure,
                rate_limit_hit,
                timeout_hit,
            )
            .await
        {
            log_persistence_failure("reliability.endpoint_observation_direct", e);
        }
    }
}
