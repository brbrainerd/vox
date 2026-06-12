//! Isolation policy tiers for script execution.
//!
//! Defines policy levels from permissive (host) to strict (container, gVisor,
//! microVM, Wasm). Used for capability negotiation in the execution API.

use serde::{Deserialize, Serialize};

/// Isolation tier for untrusted script execution.
///
/// Higher tiers provide stronger isolation at the cost of latency and
/// resource overhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationPolicy {
    /// Run on host with optional VOX_SANDBOX env (network/filesystem hints).
    #[default]
    Permissive,
    /// Run inside OCI container (Docker/Podman) with default seccomp.
    Container,
    /// Run inside gVisor (runsc) for kernel-level isolation.
    Gvisor,
    /// Run inside microVM (e.g. Firecracker) for hardware isolation.
    MicroVM,
    /// Run as precompiled Wasm with Wasmtime for sandboxed execution.
    Wasm,
}

impl IsolationPolicy {
    /// Returns a one-line honest security statement for this isolation tier.
    pub fn security_statement(&self) -> &'static str {
        match self {
            IsolationPolicy::Permissive => {
                "Host process — no additional isolation. Do not use for untrusted code."
            }
            IsolationPolicy::Container => {
                "Shared-kernel OCI container with seccomp. Not safe for hostile code."
            }
            IsolationPolicy::Gvisor => {
                "User-space kernel via gVisor runsc. Strong isolation; shared hardware."
            }
            IsolationPolicy::MicroVM => {
                "Hardware VM boundary via Firecracker/Kata/Hyper-V. Strongest isolation."
            }
            IsolationPolicy::Wasm => {
                "Wasmtime WASI P1 sandbox. No network, no arbitrary FS. Portable."
            }
        }
    }
}

impl std::str::FromStr for IsolationPolicy {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "permissive" | "host" | "none" => Ok(Self::Permissive),
            "container" | "docker" | "podman" | "oci" => Ok(Self::Container),
            "gvisor" | "runsc" => Ok(Self::Gvisor),
            "microvm" | "firecracker" | "kata" | "hyperv" | "hyper-v" => Ok(Self::MicroVM),
            "wasm" | "wasi" | "wasmtime" => Ok(Self::Wasm),
            other => anyhow::bail!(
                "Unknown isolation policy: {other}. Valid: permissive, container, gvisor, microvm, wasm"
            ),
        }
    }
}

/// Capabilities for isolation tiers.
///
/// Returned by the execution API so clients can negotiate the strictest
/// available tier for their trust class.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct IsolationCapabilities {
    /// Supported isolation tiers (most permissive first).
    pub supported: Vec<IsolationPolicy>,
    /// Whether the WASI lane is available on this host.
    pub wasm_available: bool,
    /// Whether network is restricted in sandbox mode.
    pub network_restricted: bool,
    /// Whether filesystem writes are restricted to CWD.
    pub fs_restricted: bool,
    /// Per-tier security statements for UI display.
    pub security_statements: std::collections::BTreeMap<String, String>,
}
impl IsolationCapabilities {
    /// Detect available isolation backends on the current host.
    ///
    /// Results are cached using a `OnceLock` to avoid redundant process spawns
    /// (e.g. `docker --version`) on every command execution.
    #[allow(dead_code)]
    pub fn detect() -> Self {
        use std::sync::OnceLock;
        static CACHE: OnceLock<IsolationCapabilities> = OnceLock::new();

        CACHE
            .get_or_init(|| {
                let mut supported = vec![IsolationPolicy::Permissive];

                // Container tier is for `vox deploy` (vox-container), not `vox run` script mode.
                let _docker = std::process::Command::new("docker")
                    .arg("--version")
                    .output()
                    .is_ok()
                    || std::process::Command::new("podman")
                        .arg("--version")
                        .output()
                        .is_ok();

                // gVisor backend
                if std::process::Command::new("runsc")
                    .arg("--version")
                    .output()
                    .is_ok()
                {
                    supported.push(IsolationPolicy::Gvisor);
                }

                // WASI backend: available when wasmtime is in PATH *or* linked as a library.
                let wasm_available = cfg!(feature = "script-execution")
                    || std::process::Command::new("wasmtime")
                        .arg("--version")
                        .output()
                        .is_ok();

                if wasm_available {
                    supported.push(IsolationPolicy::Wasm);
                }

                // Build per-tier security statement map
                let security_statements = supported
                    .iter()
                    .map(|tier| {
                        let key = format!("{:?}", tier).to_lowercase();
                        (key, tier.security_statement().to_string())
                    })
                    .collect();

                Self {
                    supported,
                    wasm_available,
                    network_restricted: true,
                    fs_restricted: true,
                    security_statements,
                }
            })
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolation_policy_from_str() {
        assert!("fast".parse::<IsolationPolicy>().is_err());
        assert_eq!(
            "permissive".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Permissive
        );
        assert_eq!(
            "container".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Container
        );
        assert_eq!(
            "wasm".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Wasm
        );
        assert_eq!(
            "wasi".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Wasm
        );
        assert_eq!(
            "docker".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Container
        );
        assert_eq!(
            "gvisor".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Gvisor
        );
        assert_eq!(
            "runsc".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::Gvisor
        );
        assert_eq!(
            "hyperv".parse::<IsolationPolicy>().unwrap(),
            IsolationPolicy::MicroVM
        );
    }

    #[test]
    fn isolation_policy_security_statements_not_empty() {
        for tier in [
            IsolationPolicy::Permissive,
            IsolationPolicy::Container,
            IsolationPolicy::Gvisor,
            IsolationPolicy::MicroVM,
            IsolationPolicy::Wasm,
        ] {
            assert!(!tier.security_statement().is_empty());
        }
    }

    #[test]
    fn detect_always_has_permissive() {
        let caps = IsolationCapabilities::detect();
        assert!(caps.supported.contains(&IsolationPolicy::Permissive));
    }
}
