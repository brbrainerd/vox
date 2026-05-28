//! `vox-ml` CLI: training, inference serving, eval gates, mens telemetry.

pub mod commands;
pub mod dei_daemon;
pub mod dispatch {
    pub use vox_cli_core::daemon_ipc::dispatch::*;
}
pub mod dispatch_protocol {
    pub use vox_cli_core::daemon_ipc::dispatch_protocol::*;
}
pub mod pipeline;
pub mod process_supervision {
    pub use vox_cli_core::daemon_ipc::process_supervision::*;
}
pub mod training;
pub mod workspace_db;
