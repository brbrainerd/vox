//! Shared [`clap::Args`] structs for top-level `vox` commands and Latin namespace groups.

use clap::{Args, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use vox_cli_core::cli_args::{BuildMode, BundleMode, CompileKind, UpgradeLane};

/// Build target for `vox build` / `vox dev`. See `vox_config::BuildTarget` for semantics.
///
/// `fullstack` is the default build mode. Use `--target=server` for Rust-only (no `dist/` TS),
/// or `--target=client` for Library-shaped TS (`vox-client.ts`, `openapi.json`, …; no `target/generated/`),
/// or `--target=mobile` for React Native + Expo TS that runs through `@vox/runtime-rn`.
#[derive(Clone, Copy, Debug, ValueEnum, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BuildTargetArg {
    /// Emit TypeScript/React frontend **and** Axum Rust backend (default).
    #[default]
    Fullstack,
    /// Emit only the Axum Rust backend; skip all TypeScript codegen.
    Server,
    /// Emit a zero-runtime TypeScript SDK package only; skip Rust codegen.
    Client,
    /// Emit React Native + Expo (Expo Router) TS; skip Axum Rust backend.
    /// The device-side Rust runtime ships via the uniffi-bridged `@vox/runtime-rn`
    /// package, not via this codegen path.
    Mobile,
}

impl From<BuildTargetArg> for vox_config::BuildTarget {
    fn from(arg: BuildTargetArg) -> Self {
        match arg {
            BuildTargetArg::Fullstack => vox_config::BuildTarget::Fullstack,
            BuildTargetArg::Server => vox_config::BuildTarget::Server,
            BuildTargetArg::Client => vox_config::BuildTarget::Client,
            BuildTargetArg::Mobile => vox_config::BuildTarget::Mobile,
        }
    }
}

/// `vox build` / `vox fabrica build`
#[derive(Args, Clone, Debug)]
pub struct BuildArgs {
    /// Path to the `.vox` file
    #[arg(required = true)]
    pub file: PathBuf,
    /// Build mode (App or Library)
    #[arg(long, value_enum, default_value_t = BuildMode::App)]
    pub mode: BuildMode,
    /// Output directory for generated TypeScript
    #[arg(short, long, default_value = "dist")]
    pub out_dir: PathBuf,
    /// Build target: `fullstack` (default), `server` (backend-only), or `client` (TS SDK).
    /// Overrides `[build] target` in `Vox.toml`.
    #[arg(long = "target", value_enum)]
    pub build_target: Option<BuildTargetArg>,
    /// Native mobile build target (e.g., ios, android, native). Distinct from `--target`.
    #[arg(long = "mobile-target")]
    pub mobile_target: Option<String>,
    /// Write one-shot user scaffold (`app/App.tsx`, Vite, Tailwind v4) next to output if files are missing.
    /// Same as `VOX_WEB_EMIT_SCAFFOLD=1` (flag takes precedence when either is set).
    #[arg(long)]
    pub scaffold: bool,
    /// Emit **WebIR** JSON (`web-ir.v1.json`) into the output directory (frontend IR only).
    /// For the full **VoxIrModule** bundle (HIR + embedded WebIR), use `vox check <file>.vox --emit-ir`.
    #[arg(long)]
    pub emit_ir: bool,
}

/// `vox check` / `vox fabrica check`
#[derive(Args, Clone, Debug)]
pub struct CheckArgs {
    #[arg(required = true)]
    pub file: PathBuf,

    /// Set individual output format (overrides global --json)
    #[arg(
        long,
        visible_alias = "format",
        value_name = "FORMAT",
        default_value = "text"
    )]
    pub output_format: String,
    /// Emit the full **VoxIrModule** JSON next to the source file as `<stem>.vox-ir.json`
    /// (HIR module fields plus `module.web_ir` when present).
    #[arg(long)]
    pub emit_ir: bool,

    /// Emit a single **stable JSON envelope** for LLM healing loops (includes structured diagnostics).
    /// Implies machine-readable output on stdout; does not change rustc-style stderr for parse failures.
    #[arg(long)]
    pub for_llm: bool,

    /// Treat **warnings** as errors — exit non-zero if any warning-severity diagnostic is produced.
    ///
    /// Required by CR-L2: `vox check --strict` is the gate used by `vox audit mens-on-distribution`
    /// to measure on-distribution quality of MENS-emitted programs.
    #[arg(long)]
    pub strict: bool,
}

/// `vox test` / `vox fabrica test`
#[derive(Args, Clone, Debug)]
pub struct TestArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    /// Filter tests by label
    #[arg(long)]
    pub filter: Option<String>,
    /// Number of property testing iterations
    #[arg(long)]
    pub forall_iterations: Option<u32>,
    /// Instrument for branch coverage
    #[arg(long)]
    pub coverage: bool,
    /// Update snapshot golden files
    #[arg(long)]
    pub update_snapshots: bool,
    /// Re-run tests on every `.vox` file change (Ctrl-C to stop)
    #[arg(long)]
    pub watch: bool,
}

/// `vox run` / `vox fabrica run`
#[derive(Args, Clone, Debug)]
pub struct RunArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    /// Backend listen port (sets `VOX_PORT` for generated Axum and Vite proxy)
    #[arg(long)]
    pub port: Option<u16>,
    /// `app` = generated server; `script` = `fn main()` script lane; `auto` = heuristic.
    #[arg(long, value_enum, default_value_t = crate::commands::run::RunMode::Auto)]
    pub mode: crate::commands::run::RunMode,
    /// Alias for --mode interp (HIR interpreter)
    #[arg(long, conflicts_with = "mode")]
    pub interp: bool,
    /// Alias for --mode script (WASI/Native execution)
    #[arg(long, conflicts_with = "mode")]
    pub script: bool,
    /// Alias for --mode app (full web app)
    #[arg(long, conflicts_with = "mode")]
    pub app: bool,
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

/// `vox script` / `vox fabrica script`
#[cfg(feature = "script-execution")]
#[derive(Args, Clone, Debug)]
pub struct ScriptArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    #[arg(long, default_value_t = false)]
    pub sandbox: bool,
    #[arg(long, default_value_t = false)]
    pub no_cache: bool,
    #[arg(long)]
    pub isolation: Option<String>,
    #[arg(long)]
    pub trust_class: Option<String>,
    /// Optional target triple for cross-compilation (Wave 4).
    #[arg(long)]
    pub target_triple: Option<String>,
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

/// `vox wasm run` — execute a raw precompiled WASI module via the in-process
/// wasmtime SSOT (vox-wasm-engine). NOT feature-gated: raw-`.wasm` execution must
/// always be available (the mesh worker + control plane shell out to it).
#[derive(Args, Clone, Debug)]
pub struct WasmRunArgs {
    /// Path to a precompiled `.wasm` (WASI preview1) module.
    #[arg(required = true)]
    pub file: std::path::PathBuf,
    /// Fuel limit (wasmtime instructions). Omitted / 0 = unlimited.
    #[arg(long)]
    pub fuel: Option<u64>,
    /// Read-only preopen, repeatable: HOST[:GUEST] (guest defaults to host).
    #[arg(long = "preopen-ro", value_name = "HOST[:GUEST]")]
    pub preopen_ro: Vec<String>,
    /// Read-write preopen, repeatable: HOST[:GUEST].
    #[arg(long = "preopen-rw", value_name = "HOST[:GUEST]")]
    pub preopen_rw: Vec<String>,
    /// Environment variable exposed to the guest (WASI), repeatable: KEY=VALUE.
    /// This is how the mesh worker forwards tier-gated secrets into the sandbox.
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    /// Guest argv (argv[0] is synthesized from the module stem).
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

/// `vox dev` / `vox fabrica dev`
#[derive(Args, Clone, Debug)]
pub struct DevArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    #[arg(short, long, default_value = "dist")]
    pub out_dir: PathBuf,
    /// Build target for watched rebuilds (`fullstack`, `server`, `client`). Same semantics as `vox build --target`.
    #[arg(long = "target", value_enum)]
    pub build_target: Option<BuildTargetArg>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long, default_value = "false")]
    pub open: bool,
}

/// `vox emit openapi` — standalone OpenAPI 3.1 JSON from a Vox source file.
#[derive(Args, Clone, Debug)]
pub struct EmitOpenapiArgs {
    /// Vox source file to compile.
    #[arg(required = true)]
    pub file: PathBuf,
    /// Write OpenAPI JSON to this file (default: `openapi.json`).
    #[arg(short, long, default_value = "openapi.json")]
    pub out: PathBuf,
    /// Package name for the `info.title` field.
    #[arg(long, default_value = "vox-api")]
    pub package_name: String,
    /// Package version for the `info.version` field.
    #[arg(long, default_value = "0.1.0")]
    pub package_version: String,
}

/// `vox emit client` — Library-shaped TypeScript SDK only.
#[derive(Args, Clone, Debug)]
pub struct EmitClientArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    #[arg(short, long, default_value = "dist")]
    pub out_dir: PathBuf,
    #[arg(long = "mobile-target")]
    pub mobile_target: Option<String>,
    #[arg(long)]
    pub emit_ir: bool,
}

/// `vox bundle` / `vox fabrica bundle`
#[derive(Args, Clone, Debug)]
pub struct BundleArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    /// Bundling mode.
    #[arg(long, value_enum, default_value_t = BundleMode::App)]
    pub mode: BundleMode,
    #[arg(short, long, default_value = "dist")]
    pub out_dir: PathBuf,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long, default_value = "true")]
    pub release: bool,
}

/// `vox compile` / `vox fabrica compile`
#[derive(Args, Clone, Debug)]
pub struct CompileArgs {
    /// Packaging target (`native-binary` matches `vox bundle-app`).
    #[arg(long = "target", value_enum, default_value_t = CompileKind::NativeBinary)]
    pub kind: CompileKind,
    #[arg(short, long, default_value = "dist")]
    pub out_dir: PathBuf,
    /// Rust target triple for cross-compilation (archive layout uses `vox-release-artifacts`).
    #[arg(long)]
    pub triple: Option<String>,
    #[arg(long, default_value_t = true)]
    pub release: bool,
    /// Build every `[workspace].members` package from repo-root `Vox.toml`.
    #[arg(long, default_value_t = false)]
    pub workspace: bool,
    /// After compile, emit `.zip` / `.tar.gz` + `checksums-compile.txt` beside the binary.
    #[arg(long, default_value_t = false)]
    pub archive: bool,
    /// Entry `.vox` file (optional when `--workspace`; positional must trail flags for stable clap parsing).
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,
}

#[cfg(test)]
mod compile_args_parse_tests {
    use super::CompileArgs;
    use clap::Parser;
    use vox_cli_core::cli_args::CompileKind;

    /// Minimal wrapper so `CompileArgs` can be exercised without building the full `VoxCliRoot` tree.
    /// Integration coverage for the full root parser lives in `tests/vox_cli_root_parsing.rs` (Windows:
    /// those tests run clap work on an 8 MiB stack thread).
    #[derive(Debug, Parser)]
    #[command(name = "vox-compile-args-test")]
    struct CompileArgsHarness {
        #[command(flatten)]
        inner: CompileArgs,
    }

    #[test]
    fn desktop_target_and_trailing_file() {
        let c = CompileArgsHarness::try_parse_from([
            "vox-compile-args-test",
            "--target",
            "desktop",
            "foo.vox",
        ])
        .expect("parse compile args");
        assert_eq!(c.inner.kind, CompileKind::Desktop);
        assert_eq!(
            c.inner.file.as_deref(),
            Some(std::path::Path::new("foo.vox"))
        );
    }
}

/// `vox fmt` / `vox fabrica fmt`
#[derive(Args, Clone, Debug)]
pub struct FmtArgs {
    #[arg(required = true)]
    pub file: PathBuf,
    /// Exit with error if the file would be reformatted (does not write).
    #[arg(long, default_value_t = false)]
    pub check: bool,
}

/// `vox play` / `vox fabrica play`
#[derive(Args, Clone, Debug)]
pub struct PlayArgs {
    /// Optional file to execute or project name to scaffold.
    pub path: Option<PathBuf>,
    /// Start an interactive REPL session.
    #[arg(long)]
    pub repl: bool,
}

/// `vox repair`
#[derive(Args, Clone, Debug)]
pub struct RepairArgs {
    /// File to repair. Required unless `--project` is set.
    #[arg(required_unless_present = "project", conflicts_with = "project")]
    pub file: Option<PathBuf>,
    /// Project-scope mode (CR-L3): walk `PATH` for `.vox` files and run
    /// the single-file repair loop on each one that produces error-level
    /// diagnostics. PATH defaults to `.`. Aggregates outcomes into a
    /// structured JSON report with `--json`.
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub project: Option<PathBuf>,
    /// Emit a structured JSON report when running in `--project` mode.
    /// Ignored in single-file mode (existing rustc-style output stays).
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

/// `vox doctor` / `vox mens doctor`
#[derive(Args, Clone, Debug)]
pub struct DoctorArgs {
    /// Preflight checks for `vox compile` / cross-target toolchains (rustup target, ANDROID_HOME, Xcode).
    #[arg(long, value_name = "TRIPLE")]
    pub compile_target: Option<String>,
    #[arg(long, default_value_t = false)]
    pub auto_heal: bool,
    #[arg(long, default_value_t = false)]
    pub test_health: bool,
    #[arg(long, default_value_t = false)]
    pub build_perf: bool,
    #[arg(long, default_value_t = false)]
    pub scope: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
    /// OCI / automation: run default doctor checks and exit with non-zero status if any fail (no banner; stable for HEALTHCHECK).
    #[arg(long, default_value_t = false)]
    pub probe: bool,
    /// Prepend NVIDIA CUDA toolkit bin dirs to the User PATH and set User CUDA_PATH.
    #[arg(long, default_value_t = false)]
    pub fix_cuda_path: bool,
    /// Project-health check mode (CR-L7): compile-check every `.vox` file under PATH.
    /// When set, environment-check flags (--compile-target, --auto-heal, --test-health,
    /// --build-perf, --scope, --probe, --fix-cuda-path) are ignored. Use `--json` for
    /// structured output (the deploy integration test consumes this).
    #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = ".")]
    pub project: Option<PathBuf>,
}

/// `vox stub-check` / `vox mens stub-check`
#[cfg(feature = "stub-check")]
#[derive(Args, Clone, Debug)]
pub struct StubCheckArgs {
    #[arg(long, short = 'p', value_name = "PATH", conflicts_with = "scan_pos")]
    pub path: Option<PathBuf>,
    #[arg(value_name = "PATH", conflicts_with = "path")]
    pub scan_pos: Option<PathBuf>,
    #[arg(short = 'f', long)]
    pub format: Option<String>,
    #[arg(short = 's', long)]
    pub severity: Option<String>,
    #[arg(long, default_value = "true")]
    pub suggest_fixes: bool,
    #[arg(long)]
    pub rules: Option<String>,
    #[arg(long)]
    pub excludes: Vec<String>,
    #[arg(long)]
    pub langs: Option<String>,
    #[arg(long)]
    pub baseline: Option<String>,
    #[arg(long)]
    pub save_baseline: Option<String>,
    #[arg(long)]
    pub task_list: bool,
    #[arg(long)]
    pub import_suppressions: bool,
    #[arg(long)]
    pub ingest_findings: Option<PathBuf>,
    #[arg(long)]
    pub fix_pipeline: bool,
    #[arg(long)]
    pub fix_pipeline_apply: bool,
    #[arg(long, value_name = "MODE")]
    pub gate: Option<String>,
    #[arg(long, value_name = "PATH")]
    pub gate_budget_path: Option<PathBuf>,
    #[arg(long)]
    pub verify_impacted: bool,
    #[arg(long, default_value = "1", value_name = "N")]
    pub max_escalation: u8,
    #[arg(long)]
    pub self_heal_safe_mode: bool,
    /// Print the explanation and bad/good examples for a specific diagnostic ID (e.g.
    /// `vox/llm/direct-provider-call`) and exit. No scanning is performed.
    #[arg(long, value_name = "DIAGNOSTIC_ID")]
    pub explain: Option<String>,
    /// List all known stable diagnostic IDs and exit.
    #[arg(long)]
    pub list_diagnostics: bool,
    /// Require every suppression comment (`// vox:skip`, `// toestub-ignore(...)`) to
    /// include a `— <reason>` of at least 20 characters. Exits non-zero if any are missing.
    #[arg(long)]
    pub rationale_required: bool,
}

/// `vox add` — manifest dependency declaration.
#[derive(Args, Clone, Debug)]
pub struct AddDependencyArgs {
    /// Dependency package name.
    #[arg(required = true)]
    pub name: String,
    /// Version requirement (default `*`).
    #[arg(long)]
    pub version: Option<String>,
    /// Local path dependency.
    #[arg(long)]
    pub path: Option<String>,
}

/// `vox remove`
#[derive(Args, Clone, Debug)]
pub struct RemoveDependencyArgs {
    #[arg(required = true)]
    pub name: String,
}

/// `vox lock`
#[derive(Args, Clone, Debug)]
pub struct LockArgs {
    /// Verify `vox.lock` is current without rewriting.
    #[arg(long)]
    pub locked: bool,
}

/// `vox sync`
#[derive(Args, Clone, Debug)]
pub struct SyncArgs {
    #[arg(long)]
    pub registry: Option<String>,
    /// Fail when the lockfile does not strictly match `Vox.toml`.
    #[arg(long)]
    pub frozen: bool,
}

/// `vox deploy` — apply `Vox.toml` `[deploy]` via container / compose / Kubernetes / bare-metal.
#[derive(Args, Clone, Debug)]
pub struct DeployArgs {
    /// Deployment environment label (image tag suffix, e.g. `production`).
    #[arg(default_value = "production")]
    pub environment: String,
    /// Override `[deploy].target` (`container`, `compose`, `kubernetes`, `bare-metal`, …).
    #[arg(long)]
    pub target: Option<String>,
    /// Override `[deploy].runtime` for OCI builds (`auto`, `docker`, `podman`).
    #[arg(long)]
    pub runtime: Option<String>,
    /// Print actions without mutating remote systems or registries (best-effort).
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    /// For compose targets: run `up` detached (`-d`).
    #[arg(long, default_value_t = false)]
    pub detach: bool,
    /// Require `vox.lock` to exist (CI / reproducibility gate).
    #[arg(long, default_value_t = false)]
    pub locked: bool,
}

/// Binary release host for `vox upgrade --source release` (`VOX_UPGRADE_PROVIDER`).
#[derive(Clone, Copy, Debug, Default, ValueEnum, PartialEq, Eq)]
pub enum UpgradeReleaseProvider {
    /// GitHub Releases (default for upstream).
    #[default]
    Github,
    /// GitLab Releases API.
    Gitlab,
    /// Static HTTP mirror using the binary release URL layout (`…/releases/download/<tag>/…`).
    Http,
}

/// `vox upgrade` — toolchain only (never `Vox.toml` / `vox.lock`).
#[derive(Args, Clone, Debug)]
pub struct UpgradeToolchainArgs {
    /// `release` = binary lane; `repo` = git + source install lane.
    #[arg(long = "source", value_enum, default_value_t = UpgradeLane::Release)]
    pub lane: UpgradeLane,
    /// Repository root for `--source repo`. Defaults to `VOX_REPO_ROOT` or walk-up (same as `vox ci`).
    #[arg(long, value_name = "PATH")]
    pub repo_root: Option<PathBuf>,
    /// After fetch, check out this tag, branch, or SHA. When omitted on `--source repo`, fast-forwards the current branch to upstream (or `--remote`/`--branch`).
    #[arg(long = "ref", value_name = "REF")]
    pub git_ref: Option<String>,
    /// When the current branch has no upstream, use this remote with `--branch` instead.
    #[arg(long)]
    pub remote: Option<String>,
    /// When the current branch has no upstream, fast-forward to `remote/branch`.
    #[arg(long)]
    pub branch: Option<String>,
    /// Allow `git fetch` / `merge` / `checkout` when the worktree is not clean.
    #[arg(long, default_value_t = false)]
    pub allow_dirty: bool,
    /// Check for updates only (default). Use `--apply` to mutate (install binary or update repo + reinstall).
    #[arg(long)]
    pub apply: bool,
    /// Channel: `stable` (no prereleases unless `--allow-prerelease`) or `next` (prereleases allowed).
    #[arg(long, default_value = "stable")]
    pub channel: String,
    /// Pin a release tag (e.g. `v1.2.3` or `1.2.3`). Skips “latest” discovery.
    #[arg(long = "version", value_name = "TAG")]
    pub version: Option<String>,
    /// Where to fetch releases (`VOX_UPGRADE_PROVIDER`).
    #[arg(long, value_enum)]
    pub provider: Option<UpgradeReleaseProvider>,
    /// Repository `owner/name` (GitHub) or `namespace/project` (GitLab). Default: `vox-foundation/vox`.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,
    /// For `--provider http`: base URL such as `https://github.com/org/repo/releases`.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// For `--provider gitlab`: API host (default `https://gitlab.com`). `VOX_UPGRADE_GITLAB_HOST`.
    #[arg(long, value_name = "URL")]
    pub gitlab_host: Option<String>,
    /// Custom GitHub API root (Enterprise/CN mirror). `VOX_UPGRADE_GITHUB_API_URL`.
    #[arg(long, value_name = "URL")]
    pub github_api_url: Option<String>,
    /// Allow major / semver-incompatible jumps (language may ship breaking `vox` releases).
    #[arg(long)]
    pub allow_breaking: bool,
    /// Allow prerelease versions on the `stable` channel (normally `next` only).
    #[arg(long)]
    pub allow_prerelease: bool,
}

/// `vox generate` — generate Vox code from a natural-language prompt.
#[derive(Args, Clone, Debug)]
pub struct GenerateArgs {
    /// Natural-language description of the Vox code to generate.
    #[arg(required = true)]
    pub prompt: String,
    /// Write the generated code to this file (always printed to stdout).
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,
    /// Skip server-side validation of the generated code.
    #[arg(long, default_value_t = false)]
    pub no_validate: bool,
    /// Maximum validation/retry attempts on the server.
    #[arg(long)]
    pub max_retries: Option<u32>,
    /// Bypass the orchestrator and call the inference server directly.
    /// Use this if the orchestrator is unavailable or for debugging.
    #[arg(long, default_value_t = false)]
    pub legacy_direct: bool,
    /// Inference server base URL (only used with `--legacy-direct`; default: http://127.0.0.1:7863).
    #[arg(long, value_name = "URL", requires = "legacy_direct")]
    pub server_url: Option<String>,
}

#[derive(clap::Args, Clone, Debug)]
pub struct GuiArgs {
    /// Open directly to a specific command panel.
    #[arg(
        long,
        value_name = "COMMAND",
        help = "Open to a specific command panel"
    )]
    pub command: Option<String>,
}
