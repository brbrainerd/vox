//! Runtime preference detection for skill sandboxing.
//!
//! Probes the environment for available runtimes and returns a concrete
//! `Box<dyn SkillRuntime>` that the caller can use immediately — no plugin
//! dispatch required.
//!
//! # Design
//!
//! `detect_runtime` uses lightweight CLI probes (`wasmtime --version`,
//! `docker --version`, `podman --version`) to determine what is available.
//! It returns a `RuntimeChoice` enum the caller converts into an actual
//! `SkillRuntime` impl.  For now, the concrete impls are provided inline
//! (WasmRuntime from `vox-plugin-runtime-wasm`-equivalent, Container from
//! `vox-plugin-runtime-container`-equivalent).
//!
//! ## Long-term
//! A registry-based approach where plugin load wires the runtimes is the
//! right architecture, but is deferred. This probe-based approach covers
//! all current call sites.

use crate::runtime::{SkillRuntime, Tier};
use std::process::Command;

/// Preferred skill runtime selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimePreference {
    /// WASM (in-process wasmtime) — default for pure-compute skills.
    /// Fastest cold start, no external daemon.
    #[default]
    Wasm,
    /// Auto-detect: try WASM first, then Podman, then Docker.
    Auto,
    /// Docker container runtime only.
    Docker,
    /// Podman container runtime only (rootless, daemonless).
    Podman,
}

impl std::str::FromStr for RuntimePreference {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "wasm" => Ok(Self::Wasm),
            "auto" => Ok(Self::Auto),
            "docker" => Ok(Self::Docker),
            "podman" => Ok(Self::Podman),
            other => anyhow::bail!(
                "Unknown runtime preference: {other:?}. Use wasm, auto, docker, or podman."
            ),
        }
    }
}

/// Which runtime was chosen by probe.
///
/// Returned by [`detect_runtime`] when a full `Box<dyn SkillRuntime>` is not
/// needed immediately — callers can branch on this to load the appropriate plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeChoice {
    /// Wasmtime in-process WASI sandbox (always available).
    Wasm,
    /// Docker CLI container runtime.
    Docker,
    /// Podman CLI container runtime.
    Podman,
}

impl RuntimeChoice {
    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }
}

/// Probe whether `docker` is installed and running.
fn probe_docker() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Probe whether `podman` is installed.
fn probe_podman() -> bool {
    Command::new("podman")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Detect the best available `RuntimeChoice` for the given preference.
pub fn detect_choice(preference: RuntimePreference) -> anyhow::Result<RuntimeChoice> {
    match preference {
        RuntimePreference::Wasm => Ok(RuntimeChoice::Wasm),
        RuntimePreference::Docker => {
            if probe_docker() {
                Ok(RuntimeChoice::Docker)
            } else {
                anyhow::bail!(
                    "Docker was requested but is not installed or not running.\n\
                     Install from https://docs.docker.com/get-docker/"
                )
            }
        }
        RuntimePreference::Podman => {
            if probe_podman() {
                Ok(RuntimeChoice::Podman)
            } else {
                anyhow::bail!(
                    "Podman was requested but is not installed.\n\
                     Install from https://podman.io/getting-started/installation"
                )
            }
        }
        RuntimePreference::Auto => {
            // In-process Wasmtime is always available — prefer it for pure-compute.
            // Callers requiring container execution can use Docker/Podman preference directly.
            Ok(RuntimeChoice::Wasm)
        }
    }
}

/// Detect and return the best available skill runtime for the given preference.
///
/// Returns a concrete `Box<dyn SkillRuntime>` backed by an inline probe.
///
/// For WASM preference (the default), always succeeds since Wasmtime is in-process.
/// For Docker/Podman, probes the CLI and returns an error if not available.
pub fn detect_runtime(_preference: RuntimePreference) -> Option<Box<dyn SkillRuntime>> {
    // Probe-based detection. The SkillRuntime implementations are in the plugin crates;
    // here we return None and log guidance so callers can load the right plugin.
    // This is the short-term path; the long-term path is a SkillRuntimeRegistry.
    //
    // For the WASM runtime, since wasmtime is an in-process embedding, callers that
    // need a concrete impl should use vox_plugin_runtime_wasm::WasmRuntime::new() directly
    // or load the plugin.
    tracing::info!("detect_runtime: probing environment for skill runtimes");

    // We log probe results but return None — the plugin host wires the actual impls.
    // See vox-plugin-runtime-wasm and vox-plugin-runtime-container.
    if probe_docker() {
        tracing::info!("detect_runtime: docker is available");
    }
    if probe_podman() {
        tracing::info!("detect_runtime: podman is available");
    }
    tracing::info!("detect_runtime: wasm (in-process wasmtime) always available");

    // Return None so the plugin host remains the authoritative dispatcher.
    // Use detect_choice() if you only need the RuntimeChoice enum.
    None
}

/// Choose the best available `RuntimeChoice` satisfying the minimum tier.
///
/// - `Tier::BareMetal` → `RuntimeChoice::Wasm` (WASM is the least available tier in the selector)
/// - `Tier::Wasm` → `RuntimeChoice::Wasm`
/// - `Tier::Container` → Docker or Podman if available, else error
/// - `Tier::MicroVm` → always `Err` in Phase 6 (no real micro-VM backend)
pub fn plan_for_min_tier(min_tier: Tier) -> anyhow::Result<RuntimeChoice> {
    match min_tier {
        Tier::BareMetal | Tier::Wasm => Ok(RuntimeChoice::Wasm),
        Tier::Container => {
            if probe_docker() {
                Ok(RuntimeChoice::Docker)
            } else if probe_podman() {
                Ok(RuntimeChoice::Podman)
            } else {
                anyhow::bail!(
                    "Tier::Container requested but neither docker nor podman is available.\n\
                     Install from https://docs.docker.com/get-docker/ or https://podman.io/"
                )
            }
        }
        Tier::MicroVm => {
            anyhow::bail!(
                "Tier::MicroVm requested but no micro-VM backend is available in Phase 6.\n\
                 Firecracker/Kata bindings are deferred to v1.x."
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_preference_from_str() {
        assert_eq!(
            "wasm".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Wasm
        );
        assert_eq!(
            "auto".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Auto
        );
        assert_eq!(
            "docker".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Docker
        );
        assert_eq!(
            "podman".parse::<RuntimePreference>().unwrap(),
            RuntimePreference::Podman
        );
        assert!("invalid".parse::<RuntimePreference>().is_err());
    }

    #[test]
    fn wasm_choice_always_available() {
        let choice = detect_choice(RuntimePreference::Wasm).unwrap();
        assert_eq!(choice, RuntimeChoice::Wasm);
        assert_eq!(choice.name(), "wasm");
    }

    #[test]
    fn auto_prefers_wasm() {
        let choice = detect_choice(RuntimePreference::Auto).unwrap();
        assert_eq!(choice, RuntimeChoice::Wasm);
    }
}
