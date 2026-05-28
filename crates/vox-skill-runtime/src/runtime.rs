//! Abstract skill runtime trait for sandboxed skill execution.
//!
//! `SkillRuntime` is the single abstraction over all execution backends:
//! - WASM (wasmtime-based, default for pure-compute skills)
//! - Container (Docker/Podman, fallback for skills requiring subprocess/GPU)
//! - Bare-metal (trusted process fork, for host-level skills)
//!
//! Implementations are shipped as plugins (`vox-plugin-runtime-container`,
//! `vox-plugin-runtime-wasm`).

use std::path::PathBuf;

/// Options for the "build" phase of a skill runtime.
///
/// For container runtimes: builds an OCI image.
/// For WASM runtimes: validates or precompiles the `.wasm` artifact.
/// For bare-metal: no-op.
#[derive(Debug, Clone)]
pub struct BuildOpts {
    /// Directory containing the build context (or the `.wasm` file for WASM runtimes).
    pub context_dir: PathBuf,
    /// Path to an explicit Dockerfile or WASM artifact. If `None`, auto-detected.
    pub artifact_path: Option<PathBuf>,
    /// Image tag or artifact label (used by container runtimes).
    pub tag: String,
    /// Key-value build arguments (used by container runtimes; ignored by WASM).
    pub build_args: Vec<(String, String)>,
}

/// Options for running a skill in its sandbox.
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// Image tag (container), path to `.wasm` artifact (WASM), or command (bare-metal).
    pub artifact_path: PathBuf,
    /// Port mappings as `(host, container)`. Ignored by WASM.
    pub ports: Vec<(u16, u16)>,
    /// Environment variables.
    pub env: Vec<(String, String)>,
    /// Volume mounts as `(host_path, container_path)`. Ignored by WASM (uses preopens).
    pub volumes: Vec<(String, String)>,
    /// Run in detached/background mode.
    pub detach: bool,
    /// Container/process name.
    pub name: Option<String>,
    /// Remove container after exit (container runtimes only).
    pub rm: bool,
    /// CPU fuel limit for WASM runtimes (in wasmtime fuel units).
    /// `None` means use the runtime's default.
    pub cpu_limit_fuel: Option<u64>,
}

impl Default for RunOpts {
    fn default() -> Self {
        Self {
            artifact_path: PathBuf::new(),
            ports: Vec::new(),
            env: Vec::new(),
            volumes: Vec::new(),
            detach: false,
            name: None,
            rm: true,
            cpu_limit_fuel: None,
        }
    }
}

/// The captured outcome of a skill execution.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// Process exit code (0 = success).
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Wall-clock execution time in milliseconds.
    pub wall_ms: u64,
}

impl RunOutcome {
    /// Returns `true` if the skill exited successfully (exit code 0).
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Sandbox isolation tier for skill execution (P6-T3).
///
/// Ordered from least to most isolated. The planner prefers the highest tier
/// available that satisfies the skill's declared requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Bare-metal (trusted process fork). Fastest; zero isolation.
    BareMetal = 0,
    /// In-process WASM (wasmtime WASI sandbox). Strong isolation; no syscalls.
    Wasm = 1,
    /// OCI container (Docker/Podman). Full filesystem + network isolation.
    Container = 2,
    /// Micro-VM (Firecracker/Kata). Strongest isolation; hardware-enforced.
    /// Not yet available in this phase — `MicroVmRuntime` always returns
    /// `Err(NotImplemented)`.
    MicroVm = 3,
}

/// Abstract sandbox runtime for skill execution.
///
/// Implementations exist for:
/// - WASM (`vox-plugin-runtime-wasm`, the default for pure-compute skills)
/// - Docker/Podman (`vox-plugin-runtime-container`, fallback for subprocess/GPU skills)
/// - MicroVm (`MicroVmRuntime` stub, always returns `Err(NotImplemented)` in this phase)
///
/// The runtime is selected by `vox-skill-runtime::detect::detect_runtime()` based
/// on the skill's declared requirements.
pub trait SkillRuntime: Send + Sync {
    /// Human-readable runtime name (e.g. `"wasm"`, `"docker"`, `"podman"`).
    fn name(&self) -> &str;

    /// Returns `true` when this runtime is installed and reachable.
    fn available(&self) -> bool;

    /// Build or prepare the skill artifact for execution.
    ///
    /// For container runtimes: builds an OCI image.
    /// For WASM runtimes: validates or precompiles; typically a no-op.
    fn build(&self, opts: &BuildOpts) -> anyhow::Result<()>;

    /// Run the skill in its sandbox and return the outcome.
    fn run(&self, opts: &RunOpts) -> anyhow::Result<RunOutcome>;

    /// Return the sandbox isolation tier for this runtime.
    ///
    /// Default is `Tier::Container`; override in concrete impls as appropriate.
    fn tier(&self) -> Tier {
        Tier::Container
    }

    /// Run a skill with task-scoped secret env vars merged into `opts.env`.
    ///
    /// Default impl extends `opts.env` with `secret_env` and calls `run`.
    /// Implementors may override to filter by a per-runtime allowlist.
    /// Phase 5 sandbox tiering overrides this to gate injection by trust level.
    fn run_with_secrets(
        &self,
        opts: &RunOpts,
        secret_env: &[(String, String)],
    ) -> anyhow::Result<RunOutcome> {
        if secret_env.is_empty() {
            return self.run(opts);
        }
        let mut merged = opts.clone();
        merged.env.extend(secret_env.iter().cloned());
        self.run(&merged)
    }
}
