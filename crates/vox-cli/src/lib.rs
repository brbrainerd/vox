//! Shared library for the **`vox`** and **`vox-compilerd`** binaries.
//!
//! For the end-user CLI surface and subcommand map, see the `vox` binary crate root
//! (`src/main.rs`) and repository `docs/src/reference/cli.md`.

#![allow(clippy::collapsible_if)]
#![allow(clippy::drop_non_drop)]
#![allow(deprecated, unexpected_cfgs)]

#[cfg(feature = "script-execution")]
mod build_lock;
pub use vox_cli_core::build_service;
pub mod cli_actions;
pub mod cli_args;
/// Re-exported from `vox-codegen` for callers that drive the build pipeline
/// without taking a direct `vox-codegen` dependency.
pub use vox_codegen::codegen_rust::RustAppShell;
mod cli_dispatch;
mod codex_cmd;
use crate::codex_cmd::CodexCmd;
pub use vox_cli_core::artifact_policy;
pub use vox_cli_core::command_contract;
pub use vox_cli_core::command_registry_model;
pub mod command_catalog;
pub mod commands;
pub mod compilerd;
pub mod config;
/// Spec §9.1 contributor-mode detector — see module docs for the exemption it implements.
pub mod contributor_mode;
/// External `vox-orchestrator-d` RPC boundary (method id SSOT).
pub mod dei_daemon;
/// Colored CLI output helpers (`print_info`, `print_success`, …).
pub mod diagnostics;
/// Daemon newline-JSON IPC (`call_daemon`, …) — SSOT in `vox-cli-core::daemon_ipc`.
pub mod dispatch {
    pub use vox_cli_core::daemon_ipc::dispatch::*;
}
pub mod dispatch_protocol {
    pub use vox_cli_core::daemon_ipc::dispatch_protocol::*;
}
/// Binary-freshness self-check (stale installed `vox` detection for `vox ci *` / `vox doctor`).
pub mod freshness;
/// Vite/React scaffold helpers and shared **pnpm** executable resolution (`pnpm_executable`).
pub mod frontend;
pub mod fs_utils;
/// Fuzzy ranking for command catalog, MCP tool picker, and dashboard palette.
/// Gated behind the `fuzzy-search` feature; falls back to identity ordering when disabled.
pub mod fuzzy;
#[cfg(feature = "script-execution")]
mod isolation;
mod latin_cmd;
/// Lock-wait JSONL metrics (`vox lock-report`, recursive script guard).
#[cfg(any(
    feature = "codex",
    feature = "stub-check",
    feature = "script-execution"
))]
mod lock_telemetry;
pub mod pipeline;
pub mod process_supervision {
    pub use vox_cli_core::daemon_ipc::process_supervision::*;
}
pub mod telemetry_corpus_feedback_sink;
pub mod telemetry_sink;
pub mod telemetry_spool;
pub mod templates;
/// WASI preopen mode for `script-execution` / `execution-api` runners.
#[cfg(any(feature = "script-execution", feature = "execution-api"))]
mod wasi_dir_mode;
mod watcher;
pub mod workflow_journal_codex;
/// Workspace journey VoxDb connect for repo-scoped CLI subcommands.
pub mod workspace_db;

/// Legacy v0 integration helpers (external codegen API).
pub mod v0;
/// Normalize v0.dev TSX for Vox `routes:` named imports.
pub(crate) mod v0_tsx_normalize;
/// Accessibility validator for v0.dev TSX output (TASK-5.4).
pub(crate) mod v0_tsx_validate;
/// TASK-5.4: pre-flight validation of v0.dev TSX output (a11y + design-token checks).
pub(crate) mod v0_validate;

pub use dispatch_protocol::{DispatchPayload, DispatchRequest, DispatchResponse};

use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// Build version string: `0.x.y+build.N (githash)`
pub const VOX_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+build.",
    env!("VOX_BUILD_NUMBER"),
    " (",
    env!("VOX_GIT_HASH"),
    ")",
);

pub use vox_cli_core::GlobalOpts;
pub use vox_cli_core::apply_global_opts;
pub use vox_cli_core::init_tracing_for_cli;

/// Full `vox` invocation: global options + subcommand.
#[derive(Parser)]
#[command(
    name = "vox",
    about = "The Vox AI-native language compiler",
    long_about = "The Vox AI-native language compiler.\n\nDiscover commands dynamically:\n  vox commands --recommended\n  vox commands --format json --include-nested\n\nVisualization:\n  vox gui (alias: vox axis)   — launch Axis, the native Vox GUI dashboard & catalog",
    version = VOX_VERSION
)]
pub struct VoxCliRoot {
    /// Global application options.
    #[command(flatten)]
    pub global: GlobalOpts,
    /// Core parsed CLI subcommand execution variant.
    #[command(subcommand)]
    pub cmd: Cli,
}

/// Collection of subcommands exposing all features of the `vox` binary.
#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Cli {
    /// Emit shell completions for `vox` (bash, zsh, fish, powershell, elvish).
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Print a dynamic command catalog generated from the clap command tree.
    Commands {
        /// Output format.
        #[arg(long, value_enum, default_value_t = command_catalog::CatalogFormat::Text)]
        format: command_catalog::CatalogFormat,
        /// Show only commands recommended for first-time users.
        #[arg(long)]
        recommended: bool,
        /// Include nested subcommands (default shows top-level only).
        #[arg(long)]
        include_nested: bool,
        /// Fuzzy-search the catalog (implies --include-nested when set).
        #[arg(long, value_name = "PATTERN")]
        search: Option<String>,
    },
    /// Workshop lane — same as top-level `build` (`fabrica` = Latin *workshop*).
    #[command(name = "fabrica", visible_alias = "fab")]
    Fabrica {
        /// Subcommand.
        #[command(subcommand)]
        cmd: latin_cmd::FabricaCmd,
    },
    /// Diagnostics lane — doctor, architect, stub-check (`diag`).
    #[command(name = "diag")]
    Diag {
        /// Subcommand.
        #[command(subcommand)]
        cmd: latin_cmd::DiagCmd,
    },
    /// Extensions lane — unified entry for legacy and ML subcommands (`ext`).
    #[command(name = "ext")]
    Ext {
        /// Subcommand.
        #[command(subcommand)]
        cmd: crate::commands::ext::ExtCmd,
    },
    /// Craft / skills lane — snippet, share, skill, … (`ars`).
    #[command(name = "ars", hide = true)]
    Ars {
        /// Subcommand.
        #[command(subcommand)]
        cmd: latin_cmd::ArsCmd,
    },
    /// Central secret lifecycle and diagnostics (`secrets`; alias: `clavis`).
    #[command(name = "secrets", visible_alias = "clavis")]
    Secrets {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::secrets::SecretsCmd,
    },
    /// Review lane — CodeRabbit flows (`recensio`; alias of `review` when built with `coderabbit`).
    #[cfg(feature = "coderabbit")]
    #[command(name = "recensio", visible_alias = "rec")]
    Recensio {
        /// Subcommand.
        #[command(subcommand)]
        cmd: vox_cli_review::ReviewCli,
    },
    /// Manage global configuration and preferences.
    Config {
        /// Subcommand
        #[command(subcommand)]
        cmd: commands::config::ConfigCmd,
    },
    /// View the unified policy catalog (CI gates, language rules, audits).
    Policy {
        #[command(subcommand)]
        cmd: commands::policy::PolicyCmd,
    },
    /// Vox Graph — code-intelligence over the structural index (`vox graph`; formerly `vox graphify`).
    #[command(name = "graph", alias = "graphify", alias = "search")]
    Search {
        #[command(subcommand)]
        cmd: commands::graphify::GraphifyCmd,
    },
    /// Identity and master key integration (`vox auth`).
    Auth {
        /// Subcommand
        #[command(subcommand)]
        cmd: commands::auth::AuthCmd,
    },
    /// Build a Vox source file, producing TypeScript output
    Build {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::BuildArgs,
    },
    /// Compile native / mobile packaging artifacts (`vox fabrica compile`).
    Compile {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::CompileArgs,
    },
    /// Type-check a Vox source file without producing output
    Check {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::CheckArgs,
    },
    /// Run tests for the Vox program
    Test {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::TestArgs,
    },
    /// Run a Vox source file (build + cargo run in generated project)
    Run {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::RunArgs,
    },
    /// Raw WASI module execution (`vox wasm run <file>`) via the in-process wasmtime SSOT.
    #[cfg(feature = "script-wasi")]
    Wasm {
        #[command(subcommand)]
        cmd: commands::wasm::WasmCmd,
    },
    #[cfg(not(feature = "script-wasi"))]
    /// Raw precompiled WASI module execution (needs `--features script-wasi`)
    #[command(name = "wasm")]
    WasmStub {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        _args: Vec<String>,
    },
    /// Run a `.vox` script (`fn main()`) via the native script cache (needs `--features script-execution`).
    #[cfg(feature = "script-execution")]
    Script {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::ScriptArgs,
    },
    #[cfg(not(feature = "script-execution"))]
    /// Execute a standalone Vox script
    #[command(name = "script")]
    ScriptStub {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        _args: Vec<String>,
    },
    /// Watch and rebuild via `vox-compilerd` (install daemon next to `vox` or on PATH)
    Dev {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::DevArgs,
    },
    /// Emit focused codegen artifacts (`emit client`, …).
    Emit {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::emit::EmitCmd,
    },
    /// In-process orchestrator event dashboard (requires `--features live`)
    #[cfg(feature = "live")]
    Live,
    /// Bundle a Vox source file into a complete web application (use `vox fab bundle` or `vox fabrica bundle`).
    #[command(name = "bundle-app", visible_alias = "bapp", hide = true)]
    BundleApp {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::BundleArgs,
    },
    /// Manage plugin distribution bundles (`list`, `build`, `apply`).
    Bundle {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::plugin_bundle::BundlePluginCmd,
    },
    /// Install, remove, list, and inspect Vox plugins.
    Plugin {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::plugin::PluginCmd,
    },
    /// Format a Vox source file in place
    Fmt {
        /// Arguments.
        #[command(flatten)]
        args: cli_args::FmtArgs,
    },
    /// Add a dependency to `Vox.toml` (manifest only; run `vox lock` then `vox sync` to materialize).
    Add {
        #[command(flatten)]
        args: cli_args::AddDependencyArgs,
    },
    /// Remove a dependency from `Vox.toml`.
    Remove {
        #[command(flatten)]
        args: cli_args::RemoveDependencyArgs,
    },
    /// Vendor a shadcn/ui component into the project (fetches source from
    /// ui.shadcn.com and writes it under your `components.json` aliases).
    Component {
        /// Component name from the shadcn registry (e.g. `button`).
        name: String,
    },
    /// Refresh `vox.lock` from the local PM index (project graph — not the Vox toolchain).
    Update,
    /// Resolve `Vox.toml` and write `vox.lock` without downloading artifacts.
    Lock {
        #[command(flatten)]
        args: cli_args::LockArgs,
    },
    /// Check toolchain and local environment readiness (`vox doctor`).
    Doctor {
        #[command(flatten)]
        args: cli_args::DoctorArgs,
    },
    /// Workspace layout validation + god-object scan (`vox architect`).
    #[cfg(any(feature = "codex", feature = "stub-check"))]
    Architect {
        #[command(subcommand)]
        cmd: crate::cli_actions::ArchitectAction,
    },
    /// TOESTUB scan + Codex baselines / Ludus rewards (`vox stub-check`).
    #[cfg(feature = "stub-check")]
    StubCheck {
        #[command(flatten)]
        args: cli_args::StubCheckArgs,
    },
    /// Materialize registry packages from `vox.lock` into `.vox_modules/dl/`.
    Sync {
        #[command(flatten)]
        args: cli_args::SyncArgs,
    },
    /// Sign in: configure cloud vault URL/token and Clavis profile (`canonical login`).
    #[command(name = "login")]
    Login {
        #[command(flatten)]
        args: commands::login_shared::LoginArgs,
    },
    /// Sign out: clear vault credentials from local keyring and `~/.vox/login.toml`.
    #[command(name = "logout")]
    Logout,
    /// Share a Vox app via a public URL tunnel (`vox share`).
    Share {
        #[command(flatten)]
        args: crate::commands::share::ShareArgs,
    },
    /// Deprecated: use `vox mens train` instead.
    #[command(hide = true)]
    Train {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Snippet helpers (local `vox-package` store).
    Snippet {
        #[command(subcommand)]
        cmd: crate::commands::extras::snippet_cli::SnippetCli,
    },
    /// ARS skill registry + promote / context (`vox skill`).
    #[cfg(feature = "ars")]
    Skill {
        #[command(subcommand)]
        cmd: crate::commands::extras::skill_cmd::SkillCmd,
    },
    /// Gamification: profile, companions, quests, and battle simulations. `ludus` alias retained.
    #[cfg(feature = "extras-ludus")]
    #[command(name = "gamify", visible_alias = "ludus")]
    Ludus {
        /// Subcommand.
        #[command(subcommand)]
        cmd: crate::commands::extras::ludus_cli::LudusCli,
    },
    /// Deploy from `Vox.toml` `[deploy]` (OCI build/push, compose, Kubernetes, or bare-metal SSH).
    Deploy {
        #[command(flatten)]
        args: cli_args::DeployArgs,
    },
    /// Manage OCI containers: build/run the current project with resource limits.
    Container {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::container::ContainerAction,
    },
    /// Advanced package manager / registry commands (`search`, `publish`, `vendor`, …).
    Pm {
        #[command(subcommand)]
        cmd: commands::pm::PmCli,
    },
    /// Agentic Planning tools: Create, replan, and bypass planning steps (`vox plan`).
    Plan {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::plan::PlanCmd,
    },
    /// LLM-native context and prompt generation tools (`vox llm prompt`)
    Llm {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::llm::LlmCmd,
    },
    /// Generate Vox code from a natural-language prompt using the MENS inference model.
    Generate {
        #[command(flatten)]
        args: cli_args::GenerateArgs,
    },
    /// Vox Visus: Voice of Vision. Agentic GUI visual intelligence and bug detection.
    #[cfg(feature = "dei")]
    Visus {
        #[command(subcommand)]
        cmd: commands::visus::VisusCmd,
    },
    /// Launch the native Vox Axis (Axis) GUI — the Vox GUI under its product brand.
    /// Alias: `vox axis`. Use --command <view> to open directly to a surface (e.g. 'catalog', 'flow').
    #[cfg(feature = "gui")]
    #[command(visible_alias = "axis")]
    Gui {
        #[command(flatten)]
        args: cli_args::GuiArgs,
    },
    /// Toolchain upgrade: `--source release` (checksums.txt binary) or `--source repo` (git + `cargo install --locked`); never edits `Vox.toml` / `vox.lock`.
    Upgrade {
        #[command(flatten)]
        args: cli_args::UpgradeToolchainArgs,
    },
    /// Scaffold a new Vox project (`Vox.toml`, `src/main.vox`, `.vox_modules/`, or `<name>.skill.md`).
    Init {
        /// Project / package name (defaults to current directory name).
        name: Option<String>,
        /// Package kind: `application`, `skill`, `agent`, `workflow`, `chatbot`, `library`, …
        #[arg(long)]
        kind: Option<String>,
        /// Application template: `chatbot`, `dashboard`, or `api` (with `--kind application` or default).
        #[arg(long)]
        template: Option<String>,
    },
    /// Scaffold a new Vox application from opinionated presets (`vox new web`)
    New {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::new::NewCmd,
    },
    /// Scaffold and immediately run a temporary Vox project (`vox play`)
    Play {
        #[command(flatten)]
        args: cli_args::PlayArgs,
    },
    /// Automatically repair syntax and type errors in a `.vox` file via LLM (`vox repair`)
    Repair {
        #[command(flatten)]
        args: cli_args::RepairArgs,
    },
    /// Source migrations for React interop / retired web syntax (`migrate web`, …).
    Migrate {
        #[command(subcommand)]
        cmd: commands::migrate::MigrateCmd,
    },
    /// Start the Vox MCP (Model Context Protocol) server
    Mcp,
    /// Export the Vox language grammar in various formats for MENS training.
    Grammar {
        /// Arguments.
        #[command(flatten)]
        args: commands::grammar::GrammarParams,
    },
    /// Start the Vox Language Server
    Lsp,
    /// Interactive shell or PowerShell AST exec-policy check (`shell check`, `shell repl`).
    Shell {
        /// Subcommand (default: `repl`).
        #[command(subcommand)]
        cmd: Option<commands::runtime::shell::ShellCmd>,
    },
    /// Interactive Vox expression REPL (read-eval-print loop).
    Repl,
    /// Codex / Arca database tools (verify, legacy JSONL export/import)
    Codex {
        /// Subcommand.
        #[command(subcommand)]
        cmd: CodexCmd,
    },
    /// Manage the Vox attention-budgeting system and A2A thresholds.
    #[cfg(feature = "dei")]
    Attention {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::attention::AttentionCommand,
    },
    /// Repository discovery status, catalog (`.vox/repositories.yaml`), and cross-repo queries.
    Repo {
        /// Subcommand (`Option` so bare `vox repo` defaults to status in dispatch).
        #[command(subcommand)]
        cmd: Option<commands::repo::RepoCmd>,
    },
    /// Local VoxDB: schema, samples, research ingest, preferences
    Db {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::db_cli::DbCli,
    },
    /// Hybrid retrieval over memory logs and project DB (`memory search`).
    Memory {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::memory_cli::MemoryCmd,
    },
    /// Manage the `.vox/repositories.yaml` cross-repo catalog.
    Catalog {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::catalog::CatalogCmd,
    },
    /// Vox Scientia — research / capability map facade (delegates to `vox db` tools)
    Scientia {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::scientia::ScientiaCmd,
    },
    /// Manage the DEI (Distributed Execution Intelligence) orchestrator.
    #[cfg(feature = "dei")]
    #[command(visible_alias = "orchestrator")]
    Dei {
        /// Subcommand.
        #[command(subcommand)]
        cmd: crate::commands::dei::DeiCli,
    },
    /// OpenClaw / ClawHub gateway (skill import, approvals); requires `--features ars`
    #[cfg(feature = "ars")]
    #[command(visible_alias = "oc", hide = true)]
    Openclaw {
        /// Action.
        #[command(subcommand)]
        action: commands::openclaw::OpenClawAction,
    },
    /// Manage and inspect Vox safety, coherence, and agent guardrails.
    #[cfg(feature = "dei")]
    Safety {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::safety::SafetyCommand,
    },
    /// Run quality-gate checks from `contracts/ci/check-targets.v1.yaml` (`vox audit`).
    Audit {
        /// Arguments.
        #[command(flatten)]
        args: commands::audit::AuditArgs,
    },
    /// CI guards: manifest, SSOT checks, feature matrix, doc inventory (no shell/Python required).
    Ci {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::ci::CiCmd,
    },
    /// Manage models: discovery, scoreboard, and explainability (`vox model`).
    #[command(name = "model", visible_alias = "models")]
    Model {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::model::ModelCmd,
    },

    /// Evaluate the harness itself (not a model) against a golden task set
    /// with a pass^k multi-sample gate (`vox harness eval`).
    Harness {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::harness::HarnessCmd,
    },

    /// Send one message through the harness's chat pipeline and print the reply (`vox chat`).
    Chat {
        /// Arguments.
        #[command(flatten)]
        args: commands::chat::ChatArgs,
    },

    /// Workspace drift and pattern-repetition linter.
    #[command(name = "drift-check")]
    DriftCheck {
        #[command(flatten)]
        args: commands::drift_check::DriftCheckArgs,
    },

    /// Unified research operations: infrastructure (up/down/status), eval harness, and `run` (`run_research`).
    Research {
        /// Subcommand.
        #[command(subcommand)]
        cmd: vox_cli_research::ResearchCmd,
    },

    /// CodeRabbit batch PRs + ingest (`--features coderabbit`).
    #[cfg(feature = "coderabbit")]
    Review {
        /// Subcommand.
        #[command(subcommand)]
        cmd: vox_cli_review::ReviewCli,
    },
    /// Emergency stop the orchestrator (MCP/daemon local stop request)
    Stop {
        /// Reason for stopping
        reason: Option<String>,
    },
    /// ML/AI domain: train, serve, probe (Delegated to `vox-ml-cli`).
    #[command(
        name = "mens",
        long_about = "ML/AI domain: train, serve, probe (Delegated to `vox-ml-cli`).\n\nQuick-start:\n  vox mens train   — run MENS fine-tuning on the current corpus\n  vox mens serve   — launch the local inference endpoint\n  vox mens probe   — run eval probes against the live model"
    )]
    Mens {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Mesh coordination: join, status, admin (Delegated to `vox-ml-cli`).
    #[command(name = "populi")]
    Populi {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Speech-to-Code: transcribe, listen (Delegated to `vox-ml-cli`). `oratio` alias retained.
    #[command(name = "speech", visible_alias = "oratio")]
    Oratio {
        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Optional telemetry upload queue (local spool + explicit upload; ADR 023).
    Telemetry {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::telemetry::TelemetryCmd,
    },

    /// Insta snapshot helpers: detect and clean up orphaned `.snap` files (`vox snapshot orphans [--clean]`).
    Snapshot {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::snapshot::SnapshotCmd,
    },

    /// Roll back the orchestration stack or task execution state using the vox-bounded-fs ledger.
    Rollback {
        /// Optional specific task or transaction ID to roll back.
        #[arg(long)]
        id: Option<String>,
    },

    /// Workflow introspection: dry-run schedule preview and analysis (P1-T8).
    Workflow {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::workflow::WorkflowCmd,
    },

    /// Dispatch-time routing preview — project what the dispatcher would do (P2-T6).
    Dispatch {
        /// Subcommand.
        #[command(subcommand)]
        cmd: commands::dispatch::DispatchCmd,
    },

    /// Headless-capable ratatui terminal UI — block-model shell + AI agent strip.
    ///
    /// Runs under a real terminal, over SSH, or with TERM=dumb (plain-text output).
    /// Ctrl-C exits cleanly.
    Term,
}

/// A sink that emits each event as a compact JSON line to stderr.
/// Activated when `VOX_TELEMETRY=debug`. Useful for local development and
/// diagnosing telemetry plumbing without reading the database.
struct StderrDebugSink;

impl vox_telemetry::TelemetryRecorder for StderrDebugSink {
    fn record(&self, event: &vox_telemetry::TelemetryEvent) {
        if let Ok(line) = serde_json::to_string(event) {
            eprintln!("[vox-telemetry:debug] {line}");
        }
    }
}

/// Register the process-wide telemetry sinks.
///
/// `db` is `Some` when the workspace journey DB opened successfully at startup.
pub fn init_telemetry_sinks(db: Option<vox_db::VoxDb>) {
    use std::sync::Arc;
    use vox_telemetry::{CompositeRecorder, TelemetryRecorder};

    let mut sinks: Vec<Arc<dyn TelemetryRecorder>> = Vec::new();

    if let Some(db) = db {
        sinks.push(Arc::new(vox_db::telemetry_sink::ResearchMetricsSink::new(
            db,
        )));
    }

    sinks.push(Arc::new(crate::telemetry_sink::SpoolSink::new(
        crate::telemetry_spool::spool_root(),
    )));

    // CR-L8 corpus-feedback events sink (P2.1c, ratified 2026-05-15).
    // Filters to the four CR-L8 telemetry variants (LintFinding, LintAutofix,
    // RepairAttempt, RepairOutcome) and persists them to
    // `<cwd>/contracts/reports/corpus-feedback-events/<YYYY-MM-DD>.jsonl`. The
    // `vox audit corpus-feedback` subcommand reads these files to aggregate
    // the quarterly substantive report. Override / disable via
    // `$VOX_CORPUS_FEEDBACK_EVENTS_DIR`.
    if let Some(root) = crate::telemetry_corpus_feedback_sink::resolve_events_root() {
        sinks.push(Arc::new(
            crate::telemetry_corpus_feedback_sink::CorpusFeedbackJsonlSink::new(root),
        ));
    }

    // Phase D: if VOX_TELEMETRY=debug, also emit every event as JSON to stderr.
    let cfg = vox_telemetry::TelemetryConfig::from_env();
    if cfg.debug_to_stderr {
        sinks.push(Arc::new(StderrDebugSink));
    }

    vox_telemetry::set_global_recorder(Arc::new(CompositeRecorder::new(sinks)));
}

/// Run the `vox` CLI (parsed from `std::env::args`).
pub async fn run_vox_cli() -> anyhow::Result<()> {
    let root = VoxCliRoot::parse();
    run_vox_cli_from_parsed(root).await
}

/// Run after parsing a [`VoxCliRoot`]: global env hints (`VOX_CLI_*`, `RUST_LOG`), [`init_tracing_for_cli`], then dispatch.
#[allow(unsafe_code)]
pub async fn run_vox_cli_from_parsed(root: VoxCliRoot) -> anyhow::Result<()> {
    // SAFETY: CLI startup is single-threaded before Tokio workers read env vars used by tracing/subcommands.
    unsafe {
        if root.global.json {
            crate::config::set_process_env("VOX_CLI_GLOBAL_JSON", "1");
        }
        if root.global.quiet {
            crate::config::set_process_env("VOX_CLI_QUIET", "1");
        }
        if std::env::var_os("RUST_LOG").is_none() {
            if root.global.verbose > 0 {
                crate::config::set_process_env("RUST_LOG", "debug");
            } else if root.global.quiet {
                crate::config::set_process_env("RUST_LOG", "warn");
            }
        }
    }
    init_tracing_for_cli();
    let telemetry_db = crate::workspace_db::connect_cli_workspace_voxdb_with_overrides(true)
        .await
        .ok();
    init_telemetry_sinks(telemetry_db);
    apply_global_opts(&root.global);
    cli_dispatch::dispatch_cli(root.cmd, &root.global).await
}

pub mod utils;
