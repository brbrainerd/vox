//! Wasmtime-based WASI sandbox runtime for skill execution.
//!
//! Implements [`SkillRuntime`] by delegating to [`vox_wasm_engine::WasmHost`] — the
//! single-source-of-truth Wasmtime engine + WASI wiring shared with `vox run --backend wasi`.
//!
//! # Capabilities
//! - Pure-compute: ✅ (text transforms, JSON, regex, parsing, classification)
//! - HTTP outbound: ✅ (via wasi-http — future wiring)
//! - File IO: ✅ (via WASI preopens — restricted to declared directories)
//! - Subprocess exec: ❌ (not in WASI — use runtime-container)
//! - GPU access: ❌ (not addressable in WASM — use runtime-container)

use anyhow::Result;
use vox_skill_runtime::{BuildOpts, RunOpts, RunOutcome, SkillRuntime};
use vox_wasm_engine::{Preopen, PreopenMode, WasmExecOpts, WasmHost};
use wasmtime::Module;

/// Wasmtime-based WASI sandbox runtime.
///
/// Delegates all Wasmtime engine construction, WASI context wiring, and
/// module execution to [`vox_wasm_engine::WasmHost`].
pub struct WasmRuntime {
    host: WasmHost,
}

impl WasmRuntime {
    /// Create a new `WasmRuntime` with a fuel-enabled engine.
    pub fn new() -> Result<Self> {
        // Default fuel: 1 billion instructions (~seconds of compute on modern hardware).
        let host = WasmHost::with_fuel(1_000_000_000)?;
        Ok(Self { host })
    }

    /// Create a `WasmRuntime` with a custom fuel limit.
    pub fn with_fuel(fuel: u64) -> Result<Self> {
        let host = WasmHost::with_fuel(fuel)?;
        Ok(Self { host })
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create WasmRuntime")
    }
}

impl SkillRuntime for WasmRuntime {
    fn name(&self) -> &str {
        "wasm"
    }

    fn available(&self) -> bool {
        // Wasmtime is an in-process embedding — always available.
        true
    }

    fn build(&self, opts: &BuildOpts) -> Result<()> {
        let artifact = opts
            .artifact_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| opts.context_dir.join("skill.wasm"));

        if !artifact.exists() {
            tracing::warn!(
                target: "wasm-runtime",
                path = ?artifact,
                "WASM artifact not found at expected path; \
                 skill must be pre-compiled to .wasm before execution"
            );
            return Ok(());
        }

        // Validate it's a parseable WASM module using a plain engine (no fuel needed for validation).
        let engine = wasmtime::Engine::default();
        let _module = Module::from_file(&engine, &artifact).map_err(|e| {
            anyhow::anyhow!(
                "WASM artifact {:?} is not a valid WASM module: {e}",
                artifact
            )
        })?;

        tracing::info!(
            target: "wasm-runtime",
            path = ?artifact,
            "WASM artifact validated successfully"
        );
        Ok(())
    }

    fn run(&self, opts: &RunOpts) -> Result<RunOutcome> {
        let artifact = &opts.artifact_path;

        if !artifact.exists() {
            anyhow::bail!(
                "WASM artifact not found: {:?}. \
                 Compile the skill to wasm32-wasip2 first.",
                artifact
            );
        }

        tracing::info!(
            target: "wasm-runtime",
            path = ?artifact,
            "Executing WASM module via vox-wasm-engine"
        );

        // Map RunOpts volumes → WasmHost preopens (read-write by default for skills).
        let preopens = opts
            .volumes
            .iter()
            .map(|(host_path, guest_path)| Preopen {
                host: host_path.into(),
                guest: guest_path.clone(),
                mode: PreopenMode::ReadWrite,
            })
            .collect();

        let exec_opts = WasmExecOpts {
            args: Vec::new(), // Skills don't take CLI args; invocation is via stdin/env.
            preopens,
            fuel_override: opts.cpu_limit_fuel,
            stdin: None,
            env: opts.env.clone(),
        };

        let outcome = self.host.execute(artifact, &exec_opts)?;

        Ok(RunOutcome {
            exit_code: outcome.exit_code,
            stdout: outcome.stdout_str().into_owned(),
            stderr: outcome.stderr_str().into_owned(),
            wall_ms: outcome.wall_ms,
        })
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use vox_skill_runtime::SkillRuntime;

    #[test]
    fn with_fuel_constructs_wasm_runtime_successfully() {
        // WasmHost::with_fuel configures a Wasmtime engine with fuel enabled;
        // construction should succeed on any platform that has wasmtime support.
        let rt = WasmRuntime::with_fuel(500_000).expect("WasmRuntime::with_fuel should succeed");
        // The constructed runtime must identify itself as the wasm backend.
        assert_eq!(rt.name(), "wasm");
        // And must report as available (wasmtime is in-process).
        assert!(rt.available());
    }

    #[test]
    fn with_fuel_zero_constructs_without_panic() {
        // Zero fuel is a legal (if immediately-exhausting) value; construction
        // must not panic or return Err — execution would fail, not construction.
        let rt = WasmRuntime::with_fuel(0);
        assert!(rt.is_ok(), "with_fuel(0) should construct successfully");
    }

    #[test]
    fn with_fuel_large_value_constructs_successfully() {
        let rt = WasmRuntime::with_fuel(u64::MAX);
        assert!(
            rt.is_ok(),
            "with_fuel(u64::MAX) should construct successfully"
        );
    }
}
