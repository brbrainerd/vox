//! # vox-deploy-codegen
//!
//! Deployment artifact codegen for the Vox toolchain.
//!
//! Generates Dockerfiles, Compose files, Kubernetes manifests, Fly.io configs,
//! Coolify configurations, and systemd unit files from Vox `environment`
//! declarations. This is **pure text generation** — no runtime execution.
//!
//! Submodules:
//! - [`deploy_target`] — unified `DeployTarget` enum + per-target execution helpers
//! - [`generate`] — Dockerfile / Compose generation from `EnvironmentSpec`
//! - [`bare_metal`] — systemd unit file generation

#![allow(clippy::collapsible_if)]

pub mod bare_metal;
pub mod deploy_target;
pub mod generate;

// Re-export pure types from vox-container-types (L0) — no runtime backends needed here.
// deploy_target.rs uses BuildOpts/ContainerRuntime as data / trait-object types only.
pub use vox_container_types::{BuildOpts, ContainerRuntime};

pub use bare_metal::generate_systemd_unit;
pub use deploy_target::{
    BareMetalTarget, ComposeTarget, ContainerTarget, DeployTarget, KubernetesTarget,
    build_container_target, resolve_target_kind,
};
