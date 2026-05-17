//! Concrete `vox audit <thing>` subcommand implementations.
//!
//! Each subcommand impls [`crate::Subcommand`]. Two flavors:
//!
//! - **Real implementations** (`retirement`, `aci_default`) — wrap shipped
//!   library logic and return measured outcomes.
//! - **Stubs** (`stubs::*`) — corpus-driven gates whose fixtures are stubs
//!   today. They return [`crate::report::ExitCode::InfrastructureError`] with
//!   a structurally complete [`crate::report::AuditReport`] carrying
//!   `incomplete: true`. Per contract §exit-code-2-semantics, this does NOT
//!   block CI.

pub mod aci_default;
pub mod corpus_feedback;
pub mod humaneval;
pub mod retirement;
pub mod stubs;
