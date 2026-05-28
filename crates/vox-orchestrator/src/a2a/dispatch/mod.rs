//! Dispatch layer: chooses between local executor, mesh A2A, and lease-gated
//! fallback. P0-T3 introduces `lease_gate` as the mandatory pre-check for any
//! "fall through to local" path.

pub mod bundle_fetch;
mod db;
pub mod lease_gate;
pub mod op_fragment_sync;

pub use db::{
    acknowledge_db_message, poll_inbox_from_db, prune_old_a2a_messages, send_to_db,
    send_to_db_with_breaker,
};
pub use op_fragment_sync::{
    GossipError, OP_FRAGMENT_SYNC_TYPE, OpFragmentBlob, OpFragmentSync, OpIdBloom, PeerEntry,
    PeerRegistry,
};

#[cfg(feature = "populi-transport")]
mod mesh;

#[cfg(feature = "populi-transport")]
pub use mesh::{
    drain_populi_remote_task_results, gate_local_fallback, relay_ai_fixture_distributed_subagent,
    relay_remote_task_cancel, relay_remote_task_envelope, relay_to_mesh,
};
