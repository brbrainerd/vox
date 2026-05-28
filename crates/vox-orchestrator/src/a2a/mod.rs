//! Agent-to-agent messaging types and helpers.

mod bus;
pub mod dispatch;
mod envelope;
#[cfg(feature = "populi-transport")]
pub mod jwe;
#[cfg(feature = "populi-transport")]
mod remote_poller;
#[cfg(not(feature = "populi-transport"))]
#[path = "remote_poller_noop.rs"]
mod remote_poller;
#[cfg(feature = "populi-transport")]
mod remote_worker;
#[cfg(not(feature = "populi-transport"))]
#[path = "remote_worker_noop.rs"]
mod remote_worker;
pub mod secret_bag;
pub mod traceparent;

pub use crate::types::{A2AMessage, A2AMessageType, MessageId};

pub use bus::MessageBus;
pub use dispatch::{
    acknowledge_db_message, poll_inbox_from_db, prune_old_a2a_messages, send_to_db,
    send_to_db_with_breaker,
};
#[cfg(feature = "populi-transport")]
pub use dispatch::{
    drain_populi_remote_task_results, relay_ai_fixture_distributed_subagent,
    relay_remote_task_cancel, relay_remote_task_envelope, relay_to_mesh,
};
pub use envelope::{
    A2ADeliveryPlane, A2AInboxPlane, A2ARoute, DbA2AMessage, REMOTE_TASK_ACK_TYPE,
    REMOTE_TASK_CANCEL_TYPE, REMOTE_TASK_ENVELOPE_TYPE, REMOTE_TASK_RESULT_TYPE, RemoteTaskAck,
    RemoteTaskCancel, RemoteTaskEnvelope, RemoteTaskResult,
};
pub use remote_poller::{populi_remote_result_poll_once, spawn_populi_remote_result_poller};
pub use remote_worker::{populi_remote_worker_tick_once, spawn_populi_remote_worker_poller};

pub use vox_search::{A2ARetrievalRefinement, A2ARetrievalRequest, A2ARetrievalResponse};
