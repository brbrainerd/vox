//! Vox MCP Server.
//!
//! This crate provides the Model Context Protocol (MCP) server for Vox,
//! enabling LLMs to interact with the Vox orchestrator and tools.

#![allow(missing_docs)]
#![allow(unused)]

pub mod a2a;
pub mod affinity;
pub mod client;
pub mod context;
pub mod gamify;
pub mod memory;
pub mod models;
pub mod orchestrator_tools;
pub mod params;
pub mod qa;
pub mod server;
pub mod skills;
pub mod tools;

#[cfg(feature = "wasm")]
pub mod wasm;

// Re-export common types
pub use params::*;
pub use server::*;
