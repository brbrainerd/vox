//! # vox-skill-runtime
//!
//! Abstract sandbox runtime trait for Vox skill execution.
//!
//! Defines the `SkillRuntime` trait and the `detect_runtime()` dispatch surface.
//! Implementations are shipped as plugins:
//! - `vox-plugin-runtime-wasm` — wasmtime-based WASI sandbox (default for pure-compute skills)
//! - `vox-plugin-runtime-container` — Docker/Podman OCI sandbox (fallback for subprocess/GPU skills)
//!
//! # Architecture
//!
//! ```text
//! vox-skills::SandboxedSkillRunner
//!   └─ vox_skill_runtime::detect_runtime(pref)
//!        └─ SkillRuntime trait object
//!             ├─ WasmRuntime (plugin: vox-plugin-runtime-wasm)
//!             └─ DockerRuntime / PodmanRuntime (plugin: vox-plugin-runtime-container)
//! ```

pub mod detect;
pub mod microvm;
pub mod runtime;

pub use detect::{
    RuntimeChoice, RuntimePreference, detect_choice, detect_runtime, plan_for_min_tier,
};
pub use microvm::MicroVmRuntime;
pub use runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime, Tier};
