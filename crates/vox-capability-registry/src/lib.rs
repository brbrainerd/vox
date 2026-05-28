//! Capability registry: transport-independent SSOT in [`contracts/capability/capability-registry.yaml`](../../contracts/capability/capability-registry.yaml).
//!
//! - Mens chat uses [`default_registry`] (curated MCP rows).
//! - `vox ci command-compliance` validates cross-registry consistency.
//! - [`crate::manifest::build_model_manifest`] emits planner / external-model JSON.

mod command_registry;
mod document;
mod ids;
mod loader;
mod manifest;
mod openai;
mod registry;
mod types;
mod validate;

pub use command_registry::{COMMAND_REGISTRY_REL, active_vox_cli_paths_from_command_registry_yaml};
pub use document::{CapabilityRegistryDoc, CuratedCapability, Exemptions, RuntimeBuiltinMap};
pub use ids::{implicit_cli_capability_id, implicit_mcp_capability_id};
pub use loader::{CAPABILITY_REGISTRY_REL, load_document};
pub use manifest::{ModelCapabilityManifest, build_model_manifest};
pub use openai::{capability_to_openai_function, mens_chat_parameters};
pub use registry::{bundled_document, default_registry, registry_from_document};
pub use types::{CapabilityDescriptor, CapabilityRegistry, InvocationForms, PopuliExposure};
pub use validate::validate_cross_registry;
