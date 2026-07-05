//! Newline-delimited JSON IPC to managed daemons (`vox-compilerd`, `vox-orchestrator-d`).
//! Shared by `vox-cli`, `vox-ml-cli`, and tooling that spawns the same binaries.

pub mod dispatch;
pub mod dispatch_protocol;
#[cfg(feature = "orchestrator")]
pub mod orchestrator_daemon_ensure;
pub mod process_supervision;
