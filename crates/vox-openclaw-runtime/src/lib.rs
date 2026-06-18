//! # vox-openclaw-runtime — Agent Runtime System (ARS)
//!
//! Sandbox isolation, trust classification, OpenClaw skill marketplace
//! integration, runtime context bundles, hook registry, and the lightweight
//! task executor used by `vox skill eval`.
//!
//! Extracted from `vox-skills::ars_shim` (now retired). Consumers that
//! previously imported `vox_skills::ars_shim::*` or `vox_ars::*` should
//! import from `vox_openclaw_runtime::*`.
#![allow(clippy::collapsible_if)]

pub mod context;
pub mod domain;
pub mod executor;
pub mod hooks;
pub mod manifest;
pub mod openclaw;
pub mod openclaw_adapter;
pub mod hermes_adapter;
pub mod openclaw_discovery;
pub mod openclaw_gateway_ws;
pub mod openclaw_protocol;
pub mod runtime;

/// Re-export `parse_skill_md` under a `parser` namespace for legacy callers
/// that imported it as `vox_ars::parser::parse_skill_md`.
pub mod parser {
    pub use vox_plugin_host::skill_parser::parse_skill_md;
}

pub use domain::ArsSkill;
pub use openclaw::{
    OpenClawClient, OpenClawError, OpenClawRemoteConfig, OpenClawSkillSpec, PublishResult,
};
pub use openclaw_adapter::{
    AgentProvider, AgentRuntimeConfig, AgentRuntimeAdapter,
    DefaultOpenClawRuntimeAdapter, OpenClawAdapterConfig, OpenClawAdapterError,
    OpenClawConnectionOverrides, OpenClawRuntimeAdapter, adapter_config_with_token_override,
    connect_default_runtime_adapter, connect_runtime_adapter_with_overrides,
    resolve_adapter_config,
};
pub use openclaw_discovery::{
    DEFAULT_HTTP_GATEWAY_URL, DEFAULT_WS_GATEWAY_URL, OpenClawDiscoveryOverrides,
    OpenClawResolvedEndpoints, resolve_openclaw_endpoints,
};
pub use openclaw_gateway_ws::{
    OpenClawGatewayWsClient, OpenClawGatewayWsConfig, OpenClawGatewayWsError,
};
pub use hermes_adapter::DefaultHermesRuntimeAdapter;

// Re-export skill manifest + registry types from vox-plugin-host so the
// historic `vox_ars::SkillManifest`, `vox_ars::SkillRegistry`, etc. paths
// keep working.
pub use vox_plugin_host::skill_manifest::{SkillCategory, SkillManifest, SkillPermission};
pub use vox_plugin_host::skill_registry::{InstallResult, SkillRegistry};
