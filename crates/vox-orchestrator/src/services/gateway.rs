//! Message gateway: unified fan-out to bulletin, A2A bus, and event bus.
//!
//! Provides a single API to publish notifications so that dashboard,
//! monitors, and other agents see consistent updates.

use crate::a2a::MessageBus;
use crate::bulletin::BulletinBoard;
use crate::events::{AgentEventKind, EventBus};
use crate::types::A2AMessageType;
use crate::types::{AgentId, AgentMessage, TaskId};

/// Unified message gateway for orchestrator notifications.
///
/// Use the associated functions to publish to bulletin, A2A, and event bus
/// in one place so all consumers stay in sync.
pub struct MessageGateway;

impl MessageGateway {
    /// Publish task completion to bulletin, A2A audit, and event stream.
    pub fn publish_task_completed(
        bulletin: &mut BulletinBoard,
        message_bus: &mut MessageBus,
        event_bus: &EventBus,
        task_id: TaskId,
        agent_id: AgentId,
    ) {
        bulletin.publish(AgentMessage::TaskCompleted { task_id, agent_id });
        let _ = message_bus.broadcast(
            agent_id,
            A2AMessageType::CompletionNotice,
            format!("Task {} completed", task_id),
        );
        event_bus.emit(AgentEventKind::TaskCompleted { task_id, agent_id });
    }

    /// Publish task failure to bulletin and event stream.
    pub fn publish_task_failed(
        bulletin: &mut BulletinBoard,
        event_bus: &EventBus,
        task_id: TaskId,
        agent_id: AgentId,
        error: String,
    ) {
        bulletin.publish(AgentMessage::TaskFailed {
            agent_id,
            task_id,
            error: error.clone(),
        });
        event_bus.emit(AgentEventKind::TaskFailed {
            task_id,
            agent_id,
            error,
        });
    }

    /// Publish agent spawned to bulletin and event stream.
    pub fn publish_agent_spawned(
        bulletin: &mut BulletinBoard,
        event_bus: &EventBus,
        agent_id: AgentId,
        name: String,
    ) {
        bulletin.publish(AgentMessage::AgentSpawned {
            agent_id,
            name: name.clone(),
        });
        event_bus.emit(AgentEventKind::AgentSpawned { agent_id, name });
    }

    /// Publish agent retired to event stream (bulletin has no AgentRetired variant; event is primary).
    pub fn publish_agent_retired(event_bus: &EventBus, agent_id: AgentId) {
        event_bus.emit(AgentEventKind::AgentRetired { agent_id });
    }
}
