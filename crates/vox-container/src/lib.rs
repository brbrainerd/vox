//! # vox-container
//!
//! OCI-compatible container runtime abstraction for the Vox toolchain.
//!
//! Provides a unified [`ContainerRuntime`] trait over Docker and Podman,
//! automatic runtime detection (preferring rootless Podman), and Dockerfile /
//! Compose file generation from Vox `environment` declarations.
//!
//! Also provides Python environment management via `uv`:
//! - [`env::PythonEnv`] — detect Python, `uv`, and CUDA version.
//! - [`pyproject`] — generate `pyproject.toml` from `@py.import` declarations.
//! - [`python_dockerfile`] — generate CUDA-aware Dockerfiles.
//! - [`setup::run_py_setup`] — orchestrate the full Python setup flow.

/// Support for bare metal deployments (systemd, etc.)
pub mod bare_metal;
pub mod deploy_target;
pub mod detect;
pub mod docker;
pub mod env;
pub mod generate;
pub mod podman;
pub mod pyproject;
pub mod python_dockerfile;
pub mod setup;

mod runtime;

pub use bare_metal::generate_systemd_unit;
pub use deploy_target::{
    BareMetalTarget, ComposeTarget, ContainerTarget, DeployTarget, KubernetesTarget,
    build_container_target, resolve_target_kind,
};
pub use detect::detect_runtime;
pub use env::PythonEnv;
pub use pyproject::generate_pyproject_toml;
pub use python_dockerfile::generate_python_dockerfile;
pub use runtime::{BuildOpts, ContainerRuntime, RunOpts};
pub use setup::{run_py_setup, PySetupOpts};
