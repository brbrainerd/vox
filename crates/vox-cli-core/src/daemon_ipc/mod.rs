//! Newline-delimited JSON IPC to managed daemons (`vox-compilerd`, `vox-orchestrator-d`).
//! Shared by `vox-cli`, `vox-ml-cli`, and tooling that spawns the same binaries.

pub mod dispatch;
pub mod dispatch_protocol;
pub mod process_supervision;
