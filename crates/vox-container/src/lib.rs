//! # vox-container
//!
//! OCI container runtime abstraction for the Vox toolchain.
//!
//! Provides the [`ContainerRuntime`] trait, Docker + Podman implementations,
//! and automatic runtime detection (prefer rootless Podman, fall back to Docker).
//!
//! Pure types live in `vox-container-types` (L0). This crate (L3) adds the
//! Docker/Podman CLI backends and runtime detection.
//!
//! Callers wanting the abstract SkillRuntime interface see `vox-skill-runtime`.
//! Deployment artifact codegen (Dockerfile, Compose, K8s, etc.) is in `vox-deploy-codegen`.

#![allow(clippy::collapsible_if)]

pub mod detect;
pub mod docker;
pub mod podman;

pub use detect::detect_runtime;

// Re-export pure types from the L0 types crate so existing callers keep working.
pub use vox_container_types::exec_grammar;
pub use vox_container_types::{BuildOpts, ContainerRuntime, RunOpts, RuntimePreference};

/// Classify the exec risk of a container image or command string and log the result.
///
/// Called before any container run dispatch. Uses `vox-exec-grammar`'s risk classifier.
pub fn log_exec_risk(raw_command: &str) {
    match exec_grammar::parse(raw_command) {
        Ok(mut ast) => {
            let policy = exec_grammar::ExecPolicy::default();
            exec_grammar::risk::classify(&mut ast, &policy);
            tracing::info!(
                command = raw_command,
                risk = ?ast.risk,
                "exec-grammar risk classification"
            );
        }
        Err(e) => {
            tracing::debug!(
                command = raw_command,
                error = %e,
                "exec-grammar could not parse command; skipping risk classification"
            );
        }
    }
}
