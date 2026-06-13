//! HTTP handlers for Populi join / heartbeat / A2A / exec leases.
//!
//! Split into focused submodules:
//! - [`nodes`] — node registry (health, join, heartbeat, leave, bootstrap, list)
//! - [`leases`] — remote execution lease CRUD
//! - [`a2a`] — A2A message delivery, inbox, ack, lease-renew, admin quarantine/maintenance
//! - [`dispatch`] — script dispatch, results poll, queue stats, worker execute
//! - [`federation`] — federation directory and announce

mod a2a;
mod dispatch;
mod federation;
mod leases;
mod nodes;

// Re-export everything visible to `transport/` (same as the old flat `handlers.rs`).
// Handler functions must be pub(crate) in their modules so they can be re-exported here.
pub(super) use a2a::{
    a2a_ack, a2a_inbox, a2a_lease_renew, admin_maintenance, admin_quarantine, deliver_a2a,
};
pub(super) use dispatch::{dispatch_results_poll, dispatch_script, execute_on_worker, queue_stats};
pub(super) use federation::{federation_announce, federation_directory};
pub(super) use leases::{
    admin_exec_lease_revoke, exec_lease_grant, exec_lease_list, exec_lease_release,
    exec_lease_renew,
};
pub(super) use nodes::{
    bootstrap_exchange, health, heartbeat, join_node, leave_node, list_nodes, resources_summary,
};
