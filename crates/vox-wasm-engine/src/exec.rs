//! Execution options for WASM module runs.

use crate::Preopen;

/// Options controlling a single WASM module execution.
#[derive(Debug, Clone, Default)]
pub struct WasmExecOpts {
    /// Command-line arguments passed to the WASM guest (not including `argv[0]`).
    pub args: Vec<String>,

    /// Preopened host directories exposed to the WASM sandbox.
    pub preopens: Vec<Preopen>,

    /// Per-execution fuel override.  Takes precedence over the host-level fuel
    /// configured in `WasmHost::with_fuel`.  `None` means "use the host default".
    pub fuel_override: Option<u64>,

    /// Optional stdin bytes injected into the WASM guest.
    pub stdin: Option<Vec<u8>>,

    /// Environment variables visible to the WASM guest.
    pub env: Vec<(String, String)>,
}

impl WasmExecOpts {
    /// Build a minimal options value with no preopens, no stdin, no env.
    pub fn with_args(args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            args: args.into_iter().map(|s| s.into()).collect(),
            ..Default::default()
        }
    }
}
