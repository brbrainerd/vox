//! Clap enums for `vox ci`.

use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

use super::completion_quality::CompletionGateMode;
use super::release_build;

/// Subcommands for [`CiCmd::CoolifyEval`].
#[derive(Subcommand, Debug, Clone)]
#[command(
    name = "coolify-eval",
    about = "Coolify eval sandbox: API discovery and compose sync (no SSH)."
)]
pub enum CoolifyEvalCmd {
    /// Print Coolify version (if supported) and list applications (uuid, name, fqdn).
    Discover,
    /// PATCH `COOLIFY_APP_UUID` with compose YAML from the repo and optionally trigger deploy.
    SyncCompose {
        /// Compose file path (repo-relative or absolute). Default: `vox-eval.compose.yml`.
        #[arg(long, default_value = "vox-eval.compose.yml")]
        compose: PathBuf,
        /// Override application UUID (default: Clavis `CoolifyAppUuid`).
        #[arg(long)]
        app_uuid: Option<String>,
        /// After PATCH, call `GET /api/v1/deploy?uuid=…`.
        #[arg(long, default_value_t = true)]
        deploy: bool,
        /// Coolify `domains` field (e.g. `https://eval.voxlang.org`). Omit to leave unchanged.
        #[arg(long)]
        domains: Option<String>,
    },
}

/// Command variations for Continuous Integration guards and internal codebase hygiene.
#[derive(Subcommand)]
pub enum CiCmd {
    /// `cargo metadata --locked --format-version 1 --no-deps` (workspace manifest resolves).
    Manifest,
    /// Extract domain matrix from README.md to generate shipped-v0.4.md
    #[command(name = "capability-snapshot")]
    CapabilitySnapshot,
    /// Documentation SSOT guard (required pages, doc-inventory schema, orphan inventory crate list).
    #[command(name = "check-docs-ssot")]
    CheckDocsSsot,
    /// Enforces that no new crates are added outside of the 10 Frozen Core crates in `crates/_frozen.md`.
    #[command(name = "check-frozen")]
    CheckFrozen,
    /// Codex / Arca SSOT file and OpenAPI substring guard.
    #[command(name = "check-codex-ssot")]
    CheckCodexSsot,
    /// Validate `contracts/index.yaml` against JSON Schema and listed file paths.
    #[command(name = "contracts-index")]
    ContractsIndex,
    /// Validate `contracts/terminal/exec-policy.v1.yaml` against schema (+ pwsh smoke when available).
    #[command(name = "exec-policy-contract")]
    ExecPolicyContract,
    /// Validate OpenClaw gateway protocol fixture contracts.
    #[command(name = "openclaw-contract")]
    OpenClawContract,
    /// Validate unified operations catalog parity across MCP + CLI registries.
    #[command(name = "operations-verify")]
    OperationsVerify,
    /// Sync or verify derived registry artifacts from unified operations catalog.
    #[command(name = "operations-sync")]
    OperationsSync {
        /// Target projection.
        #[arg(long, value_enum)]
        target: OperationsSyncTarget,
        /// Write generated output. Without this flag, verify current file matches.
        #[arg(long)]
        write: bool,
    },
    /// Validate `publication-worthiness.default.yaml` against its JSON Schema + numeric invariants.
    #[command(name = "scientia-worthiness-contract")]
    ScientiaWorthinessContract,
    /// Validate `scientia-heuristics.default.yaml` against its struct defaults.
    #[command(name = "scientia-heuristics-parity")]
    ScientiaHeuristicsParity,
    /// Validate SCIENTIA finding-candidate + novelty-evidence example JSON against v1 schemas.
    #[command(name = "scientia-novelty-ledger-contracts")]
    ScientiaNoveltyLedgerContracts,
    /// Run documentation + Codex + command-compliance + contracts-index guards in one shot.
    #[command(name = "ssot-drift")]
    SsotDrift,
    /// Local pre-push aggregate: runs the merge-blocking subset (fmt, clippy,
    /// ssot-drift, line-endings, doc-inventory verify, scoped TOESTUB). Mirrors
    /// the `check-and-test` guards cluster so failures match CI before pushing.
    #[command(name = "pre-push")]
    PrePush {
        /// Skip clippy and TOESTUB (fmt + ssot-drift + line-endings only). ~30s.
        #[arg(long, conflicts_with = "full")]
        quick: bool,
        /// Also run `cargo nextest run --workspace --no-fail-fast` (slow). Off by default.
        #[arg(long)]
        full: bool,
        /// Print commands without executing.
        #[arg(long)]
        dry_run: bool,
        /// After Rust checks, also run the GitHub-hosted exception workflows through
        /// `act` (nektos/act must be on PATH; Docker daemon must be running).
        /// Catches failures in docs-quality, link_checker, and ts-emit-noemit before push.
        #[arg(long)]
        act: bool,
    },
    /// VoxDB connect policy doc, telemetry JSONL parsing, and `research_metrics` NULL-vs-zero invariants.
    #[command(name = "data-ssot-guards")]
    DataSsotGuards,
    /// Data storage policy guard checks.
    #[command(name = "data-storage-guard")]
    DataStorageGuard(GuardOpts),
    /// Finalize the ssot-audit for the orchestration layer, confirming parity between telemetry-based decisioning and the canonical routing architecture.
    #[command(name = "ssot-audit")]
    SsotAudit,
    /// `cargo check -p vox-cli` for each supported feature set.
    #[command(name = "feature-matrix")]
    FeatureMatrix,
    /// Ensures `vox-cli` sources do not reference the staging `vox-dei` crate via a Rust path import.
    #[command(name = "no-dei-import", visible_alias = "no-vox-dei-import")]
    NoDeiImport,
    /// Run `vox-doc-pipeline --check` to verify SUMMARY.md matches docs/src
    CheckSummaryDrift,
    /// Verify attention event tracking parity
    AttentionEventLedgerParity,
    /// Build all documentation artifacts
    BuildDocs,
    /// Doc inventory (schema v3): generate or verify.
    DocInventory {
        /// Subcommand execution variant.
        #[command(subcommand)]
        cmd: DocInventoryCmd,
    },
    /// Milestone benchmark matrix (`contracts/eval/benchmark-matrix.json`).
    #[command(name = "eval-matrix")]
    EvalMatrix {
        /// Subcommand execution variant.
        #[command(subcommand)]
        cmd: EvalMatrixCmd,
    },
    /// Mens model scorecard harness (`contracts/eval/mens-scorecard*.json`).
    #[command(name = "mens-scorecard")]
    MensScorecard {
        /// Subcommand execution variant.
        #[command(subcommand)]
        cmd: MensScorecardCmd,
    },
    /// Fail if workflow YAML references `scripts/` paths not in the allowlist file.
    #[command(name = "workflow-scripts")]
    WorkflowScripts {
        /// Allowlist path (one script path per line, repo-relative).
        #[arg(long, default_value = "docs/agents/workflow-script-allowlist.txt")]
        allowlist: PathBuf,
    },
    /// Fail if changed LF-policy text files contain CRLF / CR (`*.ps1` exempt). Forward-only unless `--all`.
    #[command(name = "line-endings")]
    LineEndings {
        /// Audit all tracked policy files (not just the diff).
        #[arg(long)]
        all: bool,
        /// Git ref for diff base (overrides `VOX_LINE_ENDINGS_BASE`; head defaults to `HEAD`).
        #[arg(long)]
        base: Option<String>,
        /// Automatically convert CRLF -> LF in violating files and stage them via `git add`.
        #[arg(long)]
        autofix: bool,
    },
    /// Run mesh / Populi CI gate steps from `scripts/populi/gates.yaml` (with legacy fallback).
    #[command(name = "mesh-gate", visible_alias = "mens-gate")]
    MeshGate {
        /// Profile name: `m1m4` or `training`.
        #[arg(long, default_value = "m1m4")]
        profile: String,
        /// Build `vox-cli` to a side `--target-dir`, copy the `vox` binary to a temp path, then run the gate from that copy (avoids file locks when the workspace `vox` is busy). **Windows + Unix.**
        #[arg(long)]
        isolated_runner: bool,
        /// Back-compat for `--isolated-runner` (older docs / scripts).
        #[arg(long, hide = true)]
        windows_isolated_runner: bool,
        /// Cargo `--target-dir` for the isolated runner build. Default: OS temp `…/vox-targets/<repo-hash>/mens-gate-safe`.
        #[arg(long)]
        gate_build_target_dir: Option<PathBuf>,
        /// With `--isolated-runner`: tee child stdout/stderr to this file while printing to the console.
        #[arg(long)]
        gate_log_file: Option<PathBuf>,
    },
    /// Verify every file matched by the given glob(s) is valid JSON. Exits non-zero on any failure.
    /// Replaces the `python3 -c "import json …"` steps in CI workflows.
    #[command(name = "json-parse-check")]
    JsonParseCheck {
        /// One or more glob patterns (e.g. `apps/foo/contracts/**/*.json`).
        #[arg(required = true)]
        globs: Vec<String>,
    },
    /// Verify every file matched by the given glob(s) is valid YAML. Exits non-zero on any failure.
    /// Replaces the `python3 - <<'PY' import yaml …` heredoc steps in CI workflows.
    #[command(name = "yaml-parse-check")]
    YamlParseCheck {
        /// One or more glob patterns (e.g. `apps/foo/contracts/export/*.yaml`).
        #[arg(required = true)]
        globs: Vec<String>,
    },
    /// Score rule-pack precision/recall/F1 against labeled fixture files.
    /// Authoring-time only: reads `contracts/code-audit/rules.v1.yaml` and
    /// the fixture corpus, emits a table (or JSON), exits non-zero if any rule
    /// falls below `--min-f1`.
    #[command(name = "detect-rules-bench")]
    DetectRulesBench {
        /// Path to the rules YAML to score.
        #[arg(long, default_value = "contracts/code-audit/rules.v1.yaml")]
        rules: PathBuf,
        /// Root directory of fixture files (`<root>/<parent-id>/<sub>_pos_*.txt`).
        #[arg(long, default_value = "contracts/code-audit/fixtures")]
        fixtures_root: PathBuf,
        /// Minimum acceptable F1 score per rule (0.0 = reporting only).
        #[arg(long, default_value_t = 0.0)]
        min_f1: f64,
        /// Emit machine-readable JSON instead of the human table.
        #[arg(long)]
        json: bool,
    },
    /// Read toestub JSON from stdin and enforce the `rust_parse_failures` budget.
    /// Cap is `VOX_TOESTUB_MAX_RUST_PARSE_FAILURES` (default 3). Replaces the
    /// `python3 -c "…"` pipeline step in ci.yml.
    #[command(name = "toestub-budget")]
    ToestubBudget,
    /// Full-repo TOESTUB: `cargo build -p vox-code-audit --release` then `cargo run -p vox-code-audit --bin toestub` (replaces `scripts/toestub_self_apply.*`).
    #[command(name = "toestub-self-apply")]
    ToestubSelfApply,
    /// Scoped TOESTUB: `cargo run -p vox-code-audit --bin toestub -- <ROOT>`.
    #[command(name = "toestub-scoped")]
    ToestubScoped {
        /// Root path for structural scope testing.
        #[arg(default_value = "crates/vox-repository")]
        root: PathBuf,
        /// Exit policy forwarded to `toestub --mode` (`legacy` keeps historical Error+ fail).
        #[arg(long, value_enum, default_value_t = ToestubCiMode::Legacy)]
        mode: ToestubCiMode,
    },
    /// Scaling SSOT: validate `contracts/scaling/policy.yaml`; optionally emit backlog + findings.
    #[command(name = "scaling-audit")]
    ScalingAudit {
        /// Subcommand.
        #[command(subcommand)]
        cmd: ScalingAuditCmd,
    },
    /// Optional CUDA feature compile checks when `nvcc` is on PATH (or skip via env).
    #[command(name = "cuda-features")]
    CudaFeatures,
    /// Release-build `vox` with `gpu,mens-candle-cuda`, tee output to `mens/runs/logs/cuda_build_<UTC>.log` (same intent as `cargo vox-cuda-release` + `cursor_background_cuda_build.ps1`).
    #[command(name = "cuda-release-build")]
    CudaReleaseBuild {
        /// Log directory (created if missing).
        #[arg(long, default_value = "mens/runs/logs")]
        log_dir: PathBuf,
    },
    /// Wall-clock timings for key `cargo check` lanes (default CLI, GPU+stub, optional CUDA).
    #[command(name = "build-timings")]
    BuildTimings {
        /// Print one JSON object per lane (machine-readable).
        #[arg(long)]
        json: bool,
        /// Also time isolated `cargo check -p <crate>` lanes (compiler vs data vs Oratio vs Mens train).
        #[arg(long)]
        crates: bool,
        /// Detailed per-crate telemetry persisted to Arca (V34+).
        #[arg(long)]
        deep: bool,
        /// Persist results to VoxDB (default: true if deep).
        #[arg(long)]
        persist: Option<bool>,
        /// Name for this build run (deep only).
        #[arg(long)]
        name: Option<String>,
        /// Profile: `dev` or `release` (deep only).
        #[arg(long, default_value = "dev")]
        profile: String,
    },
    /// Compare grammar taxonomy fingerprint (`emit_ebnf` SHA-256) to `mens/data/grammar_fingerprint.txt`; update file on drift.
    #[command(name = "grammar-drift")]
    GrammarDrift {
        /// Emit machine-readable `drift=true|false` for CI (e.g. append to `GITHUB_OUTPUT`).
        #[arg(long, value_enum)]
        emit: Option<GrammarDriftEmit>,
    },
    /// Audit K-complexity budget: verify compressed sizes of golden outputs vs `contracts/eval/complexity-budget.v1.json`.
    #[command(name = "k-complexity-budget")]
    KComplexityBudget {
        /// Fail if any fixture exceeds its budget by more than this percentage (default 0%).
        #[arg(long, default_value_t = 0.0)]
        tolerance_percent: f64,
        /// Update baseline budgets in `contracts/eval/complexity-budget.v1.json` (Wave 11 Task 211).
        #[arg(long)]
        update: bool,
    },
    /// Validate grammar export crate: emit all formats, verify rule counts are non-zero, assert semver alignment.
    #[command(name = "grammar-export-check")]
    GrammarExportCheck,
    /// Validate GRAMMAR_SSOT.md against LEXER_KEYWORDS and LEXER_DECORATORS.
    #[command(name = "grammar-ssot-parity")]
    GrammarSsotParity,
    /// Histogram of AST decl kinds across `examples/golden` (requires `vox-corpus/ast-extract`).
    #[command(name = "corpus-decl-coverage", visible_alias = "corpus-coverage")]
    CorpusDeclCoverage,
    /// Repository hygiene guards (`TypeVar(0)` in codegen crates only, filtered `open-code` refs, stray root files) — GitLab parity.
    #[command(name = "repo-guards")]
    RepoGuards,
    /// Fail when changed files add direct secret env reads outside Clavis-owned modules.
    /// Fail when changed files use environment variables not registered in Clavis or Operator Registry.
    #[command(name = "operator-env-guard")]
    OperatorEnvGuard {
        /// Scan all crate Rust files instead of only changed files.
        #[arg(long)]
        all: bool,
    },
    #[command(name = "secret-env-guard")]
    SecretEnvGuard {
        /// Scan all crate Rust files instead of only changed files.
        #[arg(long)]
        all: bool,
    },
    /// Fail when unknown crates use `db.connection().query|execute(` (transitional allowlist in docs).
    #[command(name = "sql-surface-guard")]
    SqlSurfaceGuard {
        /// Scan all `crates/**/*.rs` instead of only `git diff` changed files.
        #[arg(long)]
        all: bool,
    },
    /// Fail when unknown crates call `query_all(` on Codex (transitional allowlist in docs).
    #[command(name = "query-all-guard")]
    QueryAllGuard {
        /// Scan all `crates/**/*.rs` instead of only `git diff` changed files.
        #[arg(long)]
        all: bool,
    },
    /// Fail when unknown crates use the Turso Rust path prefix (transitional allowlist in docs).
    #[command(name = "turso-import-guard")]
    TursoImportGuard {
        /// Scan all `crates/**/*.rs` instead of only `git diff` changed files.
        #[arg(long)]
        all: bool,
    },
    /// Verify every CREATE TABLE in the workspace is owned by a crate in tiers.a_relational.owners.
    #[command(name = "db-schema-coverage")]
    DbSchemaCoverage,
    /// Verify allow_direct_access in data-storage-policy.v1.yaml matches docs/agents/turso-import-allowlist.txt.
    #[command(name = "policy-allowlist-parity")]
    PolicyAllowlistParity,
    /// Verify all public Row/Entry/Result/Summary/Pair/Report/Rollup/Snapshot/Profile/Job structs derive Serialize+Deserialize.
    #[command(name = "row-serde-lint")]
    RowSerdeLint,
    /// Report (never fail) stringly-typed *_id fields in vox-db-types rows that have a Db<Entity>Id newtype.
    #[command(name = "string-id-lint")]
    StringIdLint,
    /// Verify Secrets SSOT parity between managed secret spec and docs/guards.
    #[command(name = "secrets-parity", visible_alias = "clavis-parity")]
    SecretsParity,
    /// Generate Secrets SSOT manifest.
    #[command(name = "secrets-contracts", visible_alias = "clavis-contracts")]
    SecretsContracts,
    /// Machine-checkable Secrets cutover promotion/rollback gates for shadow/canary/enforce/decommission.
    #[command(name = "secrets-cutover-gates", visible_alias = "clavis-cutover-gates")]
    SecretsCutoverGates,
    /// Emit post-cutover policy-violation audit report for Secrets migration.
    #[command(name = "secrets-cutover-audit", visible_alias = "clavis-cutover-audit")]
    SecretsCutoverAudit {
        /// Scan all crate Rust files instead of only changed files.
        #[arg(long)]
        all: bool,
    },
    /// Enforce mapping between OrchestratorConfig, Vox Db and preferences for Attention Guarding.
    #[command(name = "attention-config-parity")]
    AttentionConfigParity,
    /// Command registry parity: `contracts/cli/command-registry.yaml` vs `ref-cli`, reachability, compilerd, dei, MCP tools, script duals.
    #[command(name = "command-compliance")]
    CommandCompliance,
    /// Scan for LLM premature-completion patterns; write `contracts/reports/completion-audit.v1.json`.
    #[command(name = "completion-audit")]
    CompletionAudit {
        /// Additional repo-relative or absolute directories to scan (must resolve under repo root). Default scan always includes `crates/`.
        #[arg(long = "scan-extra", value_name = "DIR")]
        scan_extra: Vec<PathBuf>,
    },
    /// Gate on the last completion audit (Tier A hard block; Tier B vs `completion-baseline.v1.json`).
    #[command(name = "completion-gates")]
    CompletionGates {
        #[arg(long, value_enum, default_value_t = CompletionGateMode::Enforce)]
        mode: CompletionGateMode,
    },
    /// Ingest a completion audit report into VoxDB `ci_completion_*` telemetry tables.
    #[command(name = "completion-ingest")]
    CompletionIngest {
        /// Audit JSON path (default: `contracts/reports/completion-audit.v1.json`).
        #[arg(long)]
        report: Option<PathBuf>,
        #[arg(long, default_value = "local")]
        workflow: String,
        #[arg(long, default_value = "completion-audit")]
        run_kind: String,
    },
    /// Run rust ecosystem support parity checks (`vox-compiler` contract + classifier test).
    #[command(name = "rust-ecosystem-policy")]
    RustEcosystemPolicy,
    /// Fast local smoke: orchestrator compile + command-compliance + rust ecosystem policy.
    #[command(name = "policy-smoke")]
    PolicySmoke,
    /// Targeted backend tests (`vox-actor-runtime` + orchestrator routing policy modules).
    #[command(name = "backend-tests")]
    BackendTests,
    /// GUI smoke: `web_ir_lower_emit` always; optional Vite (`VOX_WEB_VITE_SMOKE=1`) and Playwright (`VOX_GUI_PLAYWRIGHT=1`) lanes.
    #[command(name = "gui-smoke")]
    GuiSmoke,
    /// Compare `cargo llvm-cov report --json --summary-only` to `.config/coverage-gates.toml`.
    #[command(name = "coverage-gates")]
    CoverageGates {
        /// Output path from `cargo llvm-cov report --json --summary-only`.
        #[arg(long)]
        summary_json: PathBuf,
        #[arg(long, value_enum, default_value_t = CoverageGateMode::Warn)]
        mode: CoverageGateMode,
        /// Gate policy TOML (repo-relative unless absolute).
        #[arg(long, default_value = ".config/coverage-gates.toml")]
        config: PathBuf,
    },
    /// Regenerate or verify generated CLI/reference docs from registry + code constants.
    #[command(name = "command-sync")]
    CommandSync {
        /// Write generated Markdown; without this flag, verify it matches the registry.
        #[arg(long)]
        write: bool,
    },
    /// Regenerate or verify `contracts/capability/model-manifest.generated.json` (Mens / external models).
    #[command(name = "capability-sync")]
    CapabilitySync {
        /// Write the generated JSON manifest.
        #[arg(long)]
        write: bool,
    },
    /// Validate `vox.pm.provenance/1` JSON files under `.vox_modules/provenance/` (from `vox pm publish`).
    #[command(name = "pm-provenance")]
    PmProvenance {
        /// Fail when the provenance directory is missing or contains no `*.json`.
        #[arg(long)]
        strict: bool,
        /// Directory to scan (relative to repo root unless absolute); default `.`.
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Fail if internal Markdown links are broken in `docs/src` or root-level guides.
    #[command(name = "check-links")]
    CheckLinks {
        /// Optional target file or directory to check.
        #[arg(long)]
        target: Option<PathBuf>,
    },
    /// Build and package release artifacts for a target triple (binary + checksum manifest).
    #[command(name = "release-build")]
    ReleaseBuild {
        /// Rust target triple (for example `x86_64-unknown-linux-gnu`).
        #[arg(long)]
        target: String,
        /// Version tag used in artifact names (defaults to package version).
        #[arg(long)]
        version: Option<String>,
        /// Output directory for packaged artifacts.
        #[arg(long, default_value = "dist")]
        out_dir: PathBuf,
        /// Which binary packages to produce.
        #[arg(long, value_enum, default_value = "vox")]
        package: release_build::ReleasePackage,
    },
    /// Audit workspace artifacts for cleanup.
    #[command(name = "artifact-audit")]
    ArtifactAudit {
        #[arg(long)]
        json: bool,
    },
    /// Prune workspace artifacts cleanly.
    #[command(name = "artifact-prune")]
    ArtifactPrune {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        policy: Option<PathBuf>,
    },
    /// Nomenclature guard: fail when new Latin-only structural crate directories appear outside the allowlist (T189-T196).
    #[command(name = "nomenclature-guard")]
    NomenclatureGuard {
        /// Print a JSON array of violations instead of prose (for tooling).
        #[arg(long)]
        json: bool,
    },
    /// Scan for retired symbols inside `docs/` using the list in `contracts/documentation/retired-symbols.v1.yaml`.
    #[command(name = "retired-symbol-check")]
    RetiredSymbolCheck,
    /// **Placeholder:** prints a message only (no DB/corpus checks). Prefer `vox ci mesh-gate` and `vox mens corpus …` for real gates.
    #[command(name = "mens-corpus-health")]
    MensCorpusHealth {
        #[arg(long, default_value_t = 1000)]
        min_pairs: usize,
        #[arg(long, default_value_t = 0.15)]
        min_human_ratio: f64,
    },
    /// **Placeholder:** prints a message only (no GRPO validation).
    #[command(name = "grpo-reward-baseline")]
    GrpoRewardBaseline,
    /// **Placeholder:** prints a message only (no eval suite).
    #[command(name = "collateral-damage-gate")]
    CollateralDamageGate {
        #[arg(long, default_value_t = 0.05)]
        max_damage_rate: f64,
    },
    /// **Placeholder:** prints a message only (no constrained generation).
    #[command(name = "constrained-gen-smoke")]
    ConstrainedGenSmoke {
        #[arg(long, default_value_t = 50)]
        n_samples: usize,
    },
    /// Sync derived IDE ignore files (.cursorignore, .aiignore, .aiexclude) from .voxignore SSOT.
    #[command(name = "sync-ignore-files")]
    SyncIgnoreFiles {
        /// If true, fail CI if derived files are out of sync instead of regenerating them.
        #[arg(long)]
        verify: bool,
    },
    /// Stop cargo-driven unit test runs that are still attached to this workspace.
    #[command(name = "kill-stuck-tests")]
    KillStuckTests {
        /// List matching PIDs without stopping them.
        #[arg(long)]
        what_if: bool,
    },
    /// Install the local Git pre-commit hook to automate line-ending enforcement.
    #[command(name = "install-hooks")]
    InstallHooks,
    /// Check VoxScript hygiene: run `vox check` on all `.vox` files in `scripts/`.
    ScriptHygiene {
        /// Scan for retired patterns in script bodies.
        #[arg(long)]
        retired_check: bool,
    },
    /// Determinism audit: run `vox build` twice on each golden, assert byte-identical output (C.39).
    #[command(name = "determinism-audit")]
    DeterminismAudit,
    /// Dependency sprawl guard: fail if any core crate exceeds the direct dependency cap (H.82).
    #[command(name = "dep-sprawl")]
    DepSprawl {
        /// Per-crate direct dependency cap.
        #[arg(long, default_value_t = 25)]
        cap: usize,
    },
    /// Run vox doctest extraction and compile-check on one or more Markdown files.
    /// SSG-agnostic: reads .md files directly, does not require mdBook.
    #[command(name = "doctest-md")]
    DoctestMd {
        /// One or more paths: file.md or directory. Defaults to docs/src/.
        #[arg(default_value = "docs/src")]
        paths: Vec<PathBuf>,
        /// Exit non-zero if any doctest fails (default: warn only).
        #[arg(long)]
        strict: bool,
    },
    /// Coolify eval sandbox: discover apps and sync `vox-eval.compose.yml` via API.
    CoolifyEval {
        #[command(subcommand)]
        cmd: CoolifyEvalCmd,
    },
    /// Fetch and format the latest deploy-hetzner.yml GitHub Action status.
    #[command(name = "deploy-status")]
    DeployStatus {
        /// Optional file path to write the markdown summary to.
        #[arg(long)]
        write_to: Option<PathBuf>,
    },
    /// Regenerate plugin catalog and distribution bundles reference docs from `catalog.toml`.
    #[command(name = "generate-plugin-catalog-docs")]
    GeneratePluginCatalogDocs {
        /// Output path for the plugin catalog Markdown (default: `docs/src/reference/plugin-catalog.generated.md`).
        #[arg(long)]
        catalog_out: Option<PathBuf>,
        /// Output path for the distribution bundles Markdown (default: `docs/src/reference/distribution-bundles.generated.md`).
        #[arg(long)]
        bundles_out: Option<PathBuf>,
        /// Fail if either file is out of date instead of regenerating.
        #[arg(long)]
        check: bool,
    },
    /// Verify every in-tree `Plugin.toml` has a matching entry in the plugin catalog. Passes trivially when no Plugin.toml files exist (SP1).
    #[command(name = "plugin-catalog-parity")]
    PluginCatalogParity,
    /// Walk crates/ for code/composite Plugin.toml files and assert ABI matches the host. Skips intentionally-broken `noop-bad-*` fixtures.
    #[command(name = "plugin-abi-parity")]
    PluginAbiParity,
    /// Walk crates/ for skill/composite Plugin.toml files and assert skill-md exists, is non-empty, and tools.exposes is non-empty.
    #[command(name = "plugin-skill-parity")]
    PluginSkillParity,
    /// Walk crates/vox-plugin-* for *.skill.md files and enforce AgentSkills frontmatter contract (name, description, format, directory match).
    #[command(name = "agentskills-compliance")]
    AgentSkillsCompliance,
    /// Poll GitHub Actions checks for the current HEAD (or a specific SHA) and print failures.
    #[command(name = "watch-run")]
    WatchRun {
        /// Specific commit SHA to poll (defaults to HEAD).
        #[arg(long)]
        sha: Option<String>,
        /// Timeout in seconds.
        #[arg(long, default_value_t = 600)]
        timeout_secs: u64,
        /// Exit 0 even on failures (useful for advisory hooks).
        #[arg(long)]
        advisory: bool,
        /// Only print failed checks.
        #[arg(long)]
        failures_only: bool,
    },
}

#[derive(clap::Args, Debug, Clone)]
pub struct GuardOpts {
    /// Emit machine-readable JSON only.
    #[clap(long)]
    pub json: bool,
    /// Run only the specified checks (comma-separated or multiple flags).
    #[clap(long = "only", value_name = "CHECK")]
    pub only: Vec<String>,
    #[clap(long)]
    pub check_policy_only: bool,
}

/// Output channel for [`CiCmd::GrammarDrift`].
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum GrammarDriftEmit {
    /// One line: `drift=true` or `drift=false` (GitHub Actions / shell).
    Github,
    /// Writes `drift.env` in the repo root with `drift=true|false` (GitLab-style artifact).
    Gitlab,
}

/// Subcommands for the doc inventory schema verifier.
#[derive(Subcommand)]
pub enum DocInventoryCmd {
    /// Write `docs/agents/doc-inventory.json` (or `--output`).
    Generate {
        /// Optional path to override the default JSON inventory location.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Fail if committed inventory differs from a fresh generation (ignores `generated_at`).
    Verify,
}

/// `vox ci toestub-scoped --mode` ↔ `toestub --mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ToestubCiMode {
    #[default]
    Legacy,
    Audit,
    #[value(name = "enforce-warn")]
    EnforceWarn,
    #[value(name = "enforce-strict")]
    EnforceStrict,
}

impl ToestubCiMode {
    pub(crate) fn as_cli_str(self) -> &'static str {
        match self {
            ToestubCiMode::Legacy => "legacy",
            ToestubCiMode::Audit => "audit",
            ToestubCiMode::EnforceWarn => "enforce-warn",
            ToestubCiMode::EnforceStrict => "enforce-strict",
        }
    }
}

/// Subcommands for [`CiCmd::ScalingAudit`].
#[derive(Subcommand)]
pub enum ScalingAuditCmd {
    /// Validate scaling policy YAML against JSON Schema and `vox-scaling-policy` parse.
    Verify,
    /// Regenerate `contracts/reports/scaling-audit/**` (≥300 templated tasks + TOESTUB JSON on `crates/`).
    #[command(name = "emit-reports")]
    EmitReports,
}

/// `vox ci coverage-gates --mode` (warn = print only; enforce = fail CI).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum CoverageGateMode {
    /// Print gaps; exit 0 (visibility without blocking merges).
    #[default]
    Warn,
    /// Exit non-zero when a configured threshold is not met.
    Enforce,
}

/// Projection target for `vox ci operations-sync`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum OperationsSyncTarget {
    /// Build or verify `contracts/operations/catalog.v1.yaml` from live registries.
    Catalog,
    /// Build or verify `contracts/mcp/tool-registry.canonical.yaml` from operations catalog.
    Mcp,
    /// Build or verify `contracts/cli/command-registry.yaml` from operations catalog.
    Cli,
    /// Build or verify `contracts/capability/capability-registry.yaml` from the catalog `capability` block.
    Capability,
    /// Verify or write MCP + CLI + capability registry projections (`mcp`, then `cli`, then `capability`).
    All,
}

/// Subcommands for [`CiCmd::EvalMatrix`].
#[derive(Subcommand)]
pub enum EvalMatrixCmd {
    /// Validate committed JSON against `contracts/eval/benchmark-matrix.schema.json`.
    Verify,
    /// Run `cargo` checks/tests mapped from `benchmark_classes` (deduped across milestones).
    Run {
        /// Restrict to one milestone `id` from the matrix (e.g. `m3-dei-contracts`).
        #[arg(long)]
        milestone: Option<String>,
    },
}

/// Subcommands for [`CiCmd::MensScorecard`].
#[derive(Subcommand)]
pub enum MensScorecardCmd {
    /// Validate scorecard spec against `contracts/eval/mens-scorecard.schema.json`.
    Verify {
        /// Benchmark spec path (repo-relative unless absolute).
        #[arg(long, default_value = "contracts/eval/mens-scorecard.baseline.json")]
        spec: PathBuf,
    },
    /// Execute scorecard benchmark and emit artifacts (`events.jsonl`, `summary.json`).
    Run {
        /// Benchmark spec path (repo-relative unless absolute).
        #[arg(long, default_value = "contracts/eval/mens-scorecard.baseline.json")]
        spec: PathBuf,
        /// Optional output directory; default `mens/eval/runs/<utc_ts>`.
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Apply custom-model go/no-go threshold policy from one or more summary files.
    Decide {
        /// Summary JSON paths from prior `mens-scorecard run`.
        #[arg(long = "summary", required = true)]
        summaries: Vec<PathBuf>,
        /// Print machine-readable JSON only.
        #[arg(long)]
        json: bool,
    },
    /// Evaluate Burn R&D expected-value role against QLoRA summaries.
    #[command(name = "burn-rnd")]
    BurnRnd {
        /// Baseline QLoRA summary JSON.
        #[arg(long)]
        qlora_summary: PathBuf,
        /// Optional Burn/scratch summary JSON.
        #[arg(long)]
        burn_summary: Option<PathBuf>,
        /// Print machine-readable JSON only.
        #[arg(long)]
        json: bool,
    },
    /// Ingest `summary.json` from a scorecard run into VoxDb trust observations (needs Turso/Arca).
    #[command(name = "ingest-trust")]
    IngestTrust {
        /// Summary JSON path (repo-relative unless absolute).
        #[arg(long = "summary")]
        summary: PathBuf,
    },
}
