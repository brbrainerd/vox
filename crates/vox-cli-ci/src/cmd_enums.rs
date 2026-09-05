//! Clap enums for `vox ci`.

use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

/// Release-build target tier (used by [`CiCmd::ReleaseBuild`]); the guard logic lives
/// in vox-cli's `commands::ci::release_build`, which imports this back.
///
/// `Bootstrap` and `Both` were removed: `vox-bootstrap` is retired
/// (contracts/distribution/profiles.v1.yaml) and building it failed every release.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReleasePackage {
    /// Core `vox` CLI only (lean install — no ML/scientia plugins).
    Vox,
    /// `vox-ml-cli` plugin: ML/oratio/speech/populi/train subcommands (heavy: Candle).
    Mens,
    /// `vox-langtool`: DB-free language toolchain only (check/fmt/run/build). The `minimal` tier.
    Langtool,
    /// Every artifact: vox + every plugin binary. The "full" tier.
    All,
}

/// Enforcement mode for the completion-quality gate (used by [`CiCmd`]); the guard
/// logic lives in vox-cli's `commands::ci::completion_quality`, which imports this back.
#[derive(Clone, Copy, Debug, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum CompletionGateMode {
    Warn,
    Enforce,
}

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
    /// No-op since `crates/_frozen.md` was superseded by `layers.toml` and `contracts/db/data-storage-policy.v1.yaml`. Kept for backwards-compatibility.
    #[command(name = "check-frozen")]
    CheckFrozen,
    /// Codex / Arca SSOT file and OpenAPI substring guard.
    #[command(name = "check-codex-ssot")]
    CheckCodexSsot,
    /// Validate `contracts/index.yaml` against JSON Schema and listed file paths.
    #[command(name = "contracts-index")]
    ContractsIndex,
    /// Verify AI fixture catalog parity with lexer tokens and HIR fixture variants.
    #[command(name = "ai-fixtures-coverage")]
    AiFixturesCoverage,
    /// Validate `contracts/terminal/exec-policy.v1.yaml` against schema (+ pwsh smoke when available).
    #[command(name = "exec-policy-contract")]
    ExecPolicyContract,
    /// Enforce GUI / CLI synchronization (Taure configuration parity and command descriptions).
    #[command(name = "gui-catalog-parity")]
    GuiCatalogParity,
    /// Sync or verify GUI/runtime package versions against workspace.package.version.
    #[command(name = "gui-version-sync")]
    GuiVersionSync {
        /// Write generated/synced versions. Without this flag, verify only.
        #[arg(long)]
        write: bool,
    },
    /// Generate or verify machine-readable GUI coverage classification report.
    #[command(name = "gui-surface-coverage")]
    GuiSurfaceCoverage {
        /// Write/update report output. Without this flag, verify only.
        #[arg(long)]
        write: bool,
    },
    /// Generate or verify the GUI surface registry (forces every CLI group to be classified).
    #[command(name = "gui-surface-registry")]
    GuiSurfaceRegistry {
        /// Write/update the registry, generated TS, and report. Without this flag, verify only.
        #[arg(long)]
        write: bool,
    },
    /// Gate: GUI honesty — typed toasts + no placeholder/dead elements in surfaces.
    #[command(name = "gui-honesty")]
    GuiHonesty,
    /// Gate: harness-trust-guard — single-daemon regression guard (T2.4).
    #[command(name = "harness-trust-guard")]
    HarnessTrustGuard,
    /// Validate the YAML contract schema against the system's expected defaults.
    #[command(name = "model-routing-check")]
    ModelRoutingCheck,
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
    /// Execute the speech-to-code MUST+SHOULD runtime matrix and emit KPI artifacts.
    #[command(name = "speech-runtime-suite")]
    SpeechRuntimeSuite {
        /// Stable run id under `.vox/audit/<run-id>`.
        #[arg(long)]
        run_id: Option<String>,
        /// JSONL runtime eval manifest.
        #[arg(
            long,
            default_value = "tests/speech-to-code/fixtures/corpus_v1/eval_audio_runtime_no_lang.jsonl"
        )]
        eval_manifest: PathBuf,
        /// Limit audio samples for the CPU Candle runtime eval.
        #[arg(long, default_value_t = 30)]
        limit: usize,
        /// Plugin install root containing `oratio/0.1.0/Plugin.toml` and native artifact.
        #[arg(long)]
        plugins_dir: Option<PathBuf>,
        /// Emit classification artifacts without running the audio runtime.
        #[arg(long)]
        skip_runtime: bool,
    },
    /// Regenerate the unified policy registry from live sources.
    #[command(name = "policy-registry")]
    PolicyRegistry {
        /// Write the registry to disk instead of printing it.
        #[arg(long)]
        write: bool,
    },
    /// Fail if the policy registry has drifted from the live detector set.
    #[command(name = "policy-registry-parity")]
    PolicyRegistryParity,
    /// Config hygiene: no cwd-relative contract paths, no env reads in protected
    /// (never-configure) modules. Run before the configurability plan.
    #[command(name = "config-hygiene")]
    ConfigHygiene {
        /// Regenerate the grandfathered-violations baseline file.
        #[arg(long)]
        update_baseline: bool,
        /// Auto-register stub rows for unregistered env vars and prune orphan rows.
        #[arg(long)]
        write: bool,
    },
    /// Every operational VOX_* env knob read in code must be in the config registry.
    #[command(name = "config-registry-parity")]
    ConfigRegistryParity {
        /// Regenerate the registration-backlog baseline.
        #[arg(long)]
        update_baseline: bool,
    },
    /// Generate the GUI settings search index from CONFIG_KEYS (searchable SSOT view).
    #[command(name = "config-gui-codegen")]
    ConfigGuiCodegen {
        /// Drift gate: fail if the generated TS differs from CONFIG_KEYS (do not write).
        #[arg(long)]
        check: bool,
        /// Also generate/check the Rust FIELDS catalog (generated_fields.rs).
        #[arg(long)]
        fields: bool,
    },
    /// Run documentation + Codex + command-compliance + contracts-index guards in one shot.
    #[command(name = "ssot-drift")]
    SsotDrift,
    /// Local pre-push aggregate. **Default (fast):** fmt, line-endings, ssot-drift,
    /// **scoped** doc lint + doctest on changed `docs/src/**/*.md` (excludes `archive/`),
    /// and workspace drift-check — tuned for responsive `git push`.
    /// Use **`--complete`** for the historical full static gate (whole-tree docs, clippy,
    /// doc-inventory, scoped TOESTUB). **`--full`** adds workspace nextest (CI profile,
    /// slow `#[ignore]` tests excluded by default; add **`--include-slow`** to run them).
    #[command(name = "pre-push")]
    PrePush {
        /// Legacy alias for the default **fast** profile (same as omitting `--complete`/`--full`).
        #[arg(long, conflicts_with_all = ["complete", "full"])]
        quick: bool,
        /// Full static analysis: whole-tree doc lint + doctest, doc-inventory, workspace clippy,
        /// scoped TOESTUB (same shape as the pre-2026-05-11 default pre-push, without nextest).
        #[arg(long, conflicts_with = "quick")]
        complete: bool,
        /// Implies **`--complete`** and appends **`cargo nextest run --workspace --profile ci --no-fail-fast`**
        /// (slow `#[ignore]` tests excluded; add `--include-slow` to include them).
        /// Add `--with-coverage` for llvm-cov; add `--since <ref>` to run only impacted crates.
        #[arg(long, conflicts_with = "quick")]
        full: bool,
        /// Print commands without executing.
        #[arg(long)]
        dry_run: bool,
        /// After Rust checks, also run the GitHub-hosted exception workflows through
        /// `act` (nektos/act must be on PATH; Docker daemon must be running).
        /// Catches failures in docs-quality, link_checker, and ts-emit-noemit before push.
        #[arg(long)]
        act: bool,
        /// Write JSON timing report (`contracts/reports/pre-push-report.v1.schema.json`).
        #[arg(long, value_name = "PATH")]
        report_json: Option<PathBuf>,
        /// Also run the slow `#[ignore]` test partition (arch-check smoke, scientia timeout,
        /// codegen bundle check). Only meaningful with `--full`. Adds ~3–5 min.
        #[arg(long)]
        include_slow: bool,
        /// Run nextest under `cargo llvm-cov nextest` and emit a coverage report under
        /// `target/llvm-cov/`. Only valid with `--full`; errors if used without it.
        /// Adds ~60s overhead vs. plain nextest. Requires `cargo-llvm-cov` on PATH.
        #[arg(long)]
        with_coverage: bool,
        /// Run nextest only for the packages affected by changes since `<REF>` (plus their
        /// transitive reverse-deps). Large impacted sets run in chunks (`VOX_PREPUSH_SINCE_CHUNK_SIZE`,
        /// default 10). Falls back to `--workspace` only on git/metadata hard failures.
        /// Only meaningful with `--full`. Typical wall-clock for a 1–3 crate edit: 3–20s.
        #[arg(long, value_name = "REF")]
        since: Option<String>,
        /// Compare total elapsed time against the tier budgets in
        /// `contracts/budgets/test-tier-budgets.v1.yaml` after a successful run.
        /// Warns to stderr when elapsed > `warn_ms`; fails when elapsed > `fail_ms`.
        /// No-op if the budgets file is absent (safe on first clone).
        /// Skipped in `--dry-run` mode (no elapsed times to compare).
        #[arg(long)]
        enforce_budgets: bool,
        /// With `--full`, skip the complete-tier static checks (whole-tree docs, clippy,
        /// doc-inventory, scoped TOESTUB). Use after a green `vox ci pre-push --complete`
        /// in the same iteration to avoid duplicate heavy work before push.
        #[arg(long, requires = "full")]
        skip_complete: bool,
    },
    /// Compare elapsed time from a nextest JUnit artifact against the tier budgets in
    /// `contracts/budgets/test-tier-budgets.v1.yaml`.  Reads `<testsuites time="...">` from
    /// the JUnit XML and maps it to the supplied profile's `warn_ms` / `fail_ms` thresholds.
    ///
    /// Typical CI use (runs after the existing nextest step without re-running tests):
    /// ```text
    /// cargo run -p vox-cli -- ci tier-budget-check \
    ///   --junit target/nextest/ci/junit.xml --profile full
    /// ```
    #[command(name = "tier-budget-check")]
    TierBudgetCheck {
        /// Path to the nextest JUnit XML artifact.
        #[arg(long, value_name = "PATH")]
        junit: PathBuf,
        /// Profile name used to look up the budget entry.
        /// Valid values: `fast`, `complete`, `full`, `full+cov`, `full+since`, `full+cov+since`.
        #[arg(long, value_name = "PROFILE")]
        profile: String,
    },
    /// Heuristics for Cargo cache fragmentation and expensive local verification habits (AI / inner-loop).
    #[command(name = "dev-loop-audit")]
    DevLoopAudit {
        /// Emit JSON (`contracts/reports/dev-loop-audit.v1.schema.json`).
        #[arg(long)]
        json: bool,
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
    /// Smoke `vox compile --help` via `cargo run -p vox-cli` (cross-host parity with `compile-matrix.yml`).
    #[command(name = "compile-matrix")]
    CompileMatrix,
    /// Scan `vox-deprecated-since` markers and fail when `retire-by` semver is <= the workspace version.
    #[command(name = "retirement-audit")]
    RetirementAudit,
    /// Ensures `vox-cli` sources do not reference the staging `vox-dei` crate via a Rust path import.
    /// `no-vox-orchestrator-import` is a historical alias predating the `vox_dei` rename (see
    /// docs/src/reference/cli.md and docs/src/ci/command-surface-duals.md).
    #[command(
        name = "no-dei-import",
        visible_aliases = ["no-vox-dei-import", "no-vox-orchestrator-import"]
    )]
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
    /// Documentation Reality Audit — validate inventory/findings/metrics JSON + path hints (`contracts/documentation/docs-reality-audit.program.v1.yaml`).
    #[command(name = "docs-reality-audit")]
    DocsRealityAudit {
        /// Subcommand execution variant.
        #[command(subcommand)]
        cmd: DocsRealityAuditCmd,
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
    /// Conventional commit-msg and line-churn lint gate.
    #[command(name = "commit-lint")]
    CommitLint {
        /// Base branch or ref to compare against (e.g. HEAD~1 or origin/main).
        #[arg(long, default_value = "HEAD~1")]
        base: String,
    },
    /// Cross-platform `rustfmt --check` over the whole workspace. Chunked over crate
    /// target roots, so it avoids the Windows os-206 command-line overflow of
    /// `cargo fmt --all` and stays robust as crates are added/removed.
    #[command(name = "fmt-check")]
    FmtCheck,
    /// Warn when workflow YAML uses GitHub-hosted runners without a registered exception.
    #[command(name = "runner-policy-check")]
    RunnerPolicyCheck {
        /// Fail (exit 1) instead of advisory warn.
        #[arg(long)]
        strict: bool,
    },
    /// Require a `concurrency:` block on push/PR-triggered workflows (flood prevention);
    /// exceptions registered in docs/src/ci/concurrency-exceptions.md.
    #[command(name = "workflow-concurrency-guard")]
    WorkflowConcurrencyGuard {
        /// Fail (exit 1) instead of advisory warn.
        #[arg(long)]
        strict: bool,
    },
    /// Require every `softprops/action-gh-release` workflow step to set
    /// `draft: true` (boolean). Unlike the advisory guards, this one always
    /// fails on a violation — a published public release is not advisory.
    #[command(name = "release-draft-guard")]
    ReleaseDraftGuard,
    /// Forbid any workflow step from installing Rust directly via
    /// `dtolnay/rust-toolchain` instead of `./.github/actions/setup-rust`.
    /// Always fails on a violation.
    #[command(name = "toolchain-workflow-lint")]
    ToolchainWorkflowLint,
    /// Forbid an `actions/cache` key that hashes `Cargo.lock` without also
    /// keying on the Rust toolchain. Always fails on a violation.
    #[command(name = "cache-key-lint")]
    CacheKeyLint,
    /// Advisory GUI visual AI review (screenshots vs design principles). Always exits 0; never gates.
    #[command(name = "gui-visual-review")]
    GuiVisualReview {
        /// Skip the AI model calls (offline / structural-only).
        #[arg(long)]
        no_ai: bool,
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
    /// Fail if any git-tracked file begins with a UTF-8 byte-order mark (0xEF 0xBB 0xBF).
    /// BOMs corrupt `include_str!()` output and break JSON parsing.
    #[command(name = "bom-check")]
    BomCheck,
    /// Validate domain profiles spoke configurations, ensuring base models/methods/presets are correct
    /// and required paths exist.
    #[command(name = "spoke-check")]
    SpokeCheck,
    /// Reap stale `vox*` processes that lock this worktree's `target/` build
    /// output (Windows os-error-5 on relink). Dry-run unless `--apply`.
    #[command(name = "free-binary")]
    FreeBinary {
        /// Target dir to free (defaults to `<root>/target`).
        #[arg(long)]
        target: Option<std::path::PathBuf>,
        /// Actually kill the stale processes (default: dry-run).
        #[arg(long)]
        apply: bool,
    },
    /// Regenerate or verify `examples/PARSE_STATUS.md` from `examples/golden/*.vox`.
    #[command(name = "parse-status")]
    ParseStatus {
        /// Write `examples/PARSE_STATUS.md` if it differs from the generator output.
        #[arg(long)]
        write: bool,
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
    /// Verify every `.vox` file matched by the given glob(s) parses cleanly (lex + parse,
    /// no type-check). Regression guard for corpus-wide lexer/parser changes — e.g. the
    /// `Token::Unknown` catch-all (commit c3446892847e) silently broke 4 `scripts/**/*.vox`
    /// files with a bare `return;` that no `cargo test` covered, because no test walked the
    /// `.vox` script corpus. Exits non-zero on any parse failure (Error-severity diagnostics
    /// only; tolerated-semicolon and similar Warning-severity diagnostics do not fail the gate).
    #[command(name = "vox-parse-check")]
    VoxParseCheck {
        /// One or more glob patterns (e.g. `scripts/**/*.vox`, `apps/**/*.vox`).
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
    /// Scoped TOESTUB: `cargo run -p vox-code-audit --bin toestub -- [ROOT...]`.
    #[command(name = "toestub-scoped")]
    ToestubScoped {
        /// Root path(s) for structural scope testing (default `crates/vox-repository` when omitted).
        #[arg(value_name = "ROOT")]
        roots: Vec<PathBuf>,
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
    /// Check sccache setup and advise on correct configuration.
    #[command(name = "build-cache-doctor")]
    BuildCacheDoctor,
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
    /// Audit source-token budget: verify lex(source).len() + raw byte count per ladder fixture vs `contracts/eval/source-token-budget.v1.json`.
    #[command(name = "source-token-budget")]
    SourceTokenBudget {
        /// Fail if any fixture exceeds its budget by more than this percentage (default 0%).
        #[arg(long, default_value_t = 0.0)]
        tolerance_percent: f64,
        /// Update baseline budgets in `contracts/eval/source-token-budget.v1.json`.
        #[arg(long)]
        update: bool,
    },
    /// Validate grammar export crate: emit all formats, verify rule counts are non-zero, assert semver alignment.
    #[command(name = "grammar-export-check")]
    GrammarExportCheck,
    /// Validate GRAMMAR_SSOT.md against LEXER_KEYWORDS and LEXER_DECORATORS.
    #[command(name = "grammar-ssot-parity")]
    GrammarSsotParity,
    /// Umbrella gate: grammar SSOT, canonical golden ladder, feature matrix smoke, ladder-scoped k-budget.
    #[command(name = "pipeline-parity")]
    PipelineParity,
    /// Histogram of AST decl kinds across `examples/golden` (requires `vox-corpus/ast-extract`).
    #[command(name = "corpus-decl-coverage", visible_alias = "corpus-coverage")]
    CorpusDeclCoverage,
    /// Repository hygiene guards (`TypeVar(0)` in codegen crates only, filtered `open-code` refs, stray root files).
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
    /// Fail when changed files add direct secret env reads outside Clavis-owned modules.
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
    /// Report (never fail) stringly-typed *_id fields in vox-db-types rows that have a `DbEntityId` newtype.
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
    /// Targeted backend tests (`vox-actor-runtime`, orchestrator routing policy modules, VoxDb contract checks, and vox-sql P2/P3/P5 interop smoke).
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
    /// Validate `contracts/documentation/canonical-map.v1.yaml` structure (B-canon paths, aliases, globs).
    #[command(name = "canonical-map-verify")]
    CanonicalMapVerify,
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
        package: ReleasePackage,
    },
    /// Audit workspace artifacts for cleanup.
    #[command(name = "artifact-audit")]
    ArtifactAudit {
        #[arg(long)]
        json: bool,
        /// Also report per-worktree `target/` dirs and stale worktrees (read-only).
        #[arg(long)]
        include_worktrees: bool,
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
        /// Clean per-worktree `target/` dirs (gated: never touches an active build).
        #[arg(long)]
        include_worktrees: bool,
        /// Additionally remove whole stale, clean, unlocked worktrees.
        #[arg(long)]
        remove_stale_worktrees: bool,
        /// Clean target dirs even in worktrees with uncommitted source (never the source).
        #[arg(long)]
        include_dirty_targets: bool,
        /// Clean only `target/{debug,release}/incremental/` (keep `deps/` for fast rebuilds).
        #[arg(long)]
        incremental_only: bool,
        /// Override the policy staleness threshold (days); `0` cleans regardless of age.
        #[arg(long)]
        max_age_days: Option<u32>,
    },
    /// Autoscale the ephemeral self-hosted CI runner pool to current demand (dry-run unless `--apply`).
    #[command(name = "runner-scale")]
    RunnerScale {
        /// Actually spawn/reap runners (default is dry-run).
        #[arg(long)]
        apply: bool,
    },
    /// Reproducible wall-clock build scenarios with a committed baseline; --compare emits a phase delta.
    #[command(name = "build-bench")]
    BuildBench {
        /// Label for this run (used in the delta report heading + snapshot filename).
        #[arg(long)]
        label: Option<String>,
        /// Write the full snapshot JSON to this path.
        #[arg(long)]
        write: Option<String>,
        /// Compare against a baseline snapshot JSON and emit a delta report.
        #[arg(long)]
        compare: Option<String>,
        /// Run each scenario N times and keep the min wall time (default 3).
        #[arg(long, default_value_t = 3)]
        repeat: u32,
        /// After build: parse the newest cargo-timings HTML and append to history JSONL.
        #[arg(long)]
        ingest: bool,
    },
    /// Gate keystone crates' blast-radius-seconds against committed thresholds.
    /// Reads `.vox/cache/graphify/crate-map/graph.json` (produced by `vox graphify crate-map`).
    #[command(name = "crate-budget")]
    CrateBudget {
        /// Emit advisory output and exit 0 even on violations (use until baseline is populated).
        #[arg(long)]
        exit_zero: bool,
    },
    /// Verify contracts/ci/crate-build-map.v1.json is in sync with crate-graph.v1.json
    /// (recomputes derived blast_s/dependents and fails on drift).
    #[command(name = "crate-build-map-parity")]
    CrateBuildMapParity,
    /// Gate workspace fan-in growth: fail when any crate gains new dependents vs the
    /// committed snapshot in `contracts/ci/fan-in-snapshot.v1.json`.
    /// Uses `contracts/ci/crate-graph.v1.json` for actual counts.
    #[command(name = "fan-in-budget")]
    FanInBudget {
        /// Emit advisory output and exit 0 even on regressions.
        #[arg(long)]
        exit_zero: bool,
    },
    /// Exact edge-set ratchet + layer rule for workspace crate dependencies.
    /// Live graph from `cargo metadata` vs `contracts/ci/crate-edges.allow.v1.json`
    /// (+ `crate-layers.v1.json`). New edges require a user-authorized ledger entry.
    #[command(name = "crate-edges")]
    CrateEdges {
        /// Regenerate the baseline from the live graph (removal-only) and drop
        /// stale exceptions. Bootstraps both contract files when missing.
        #[arg(long)]
        tighten: bool,
    },
    /// Detect dependency cycles (HARD on normal-dep cycles) and inventory dev-dep back-edges.
    /// With --deny-new, fails when a new advisory cycle appears not in the committed allowlist.
    #[command(name = "dep-cycles")]
    DepCycles {
        /// Fail if any advisory back-edge cycle is not in the committed allowlist.
        #[arg(long)]
        deny_new: bool,
        /// Path to allowlist JSON (default: contracts/ci/dep-backedges.allow.json).
        #[arg(long)]
        allowlist: Option<std::path::PathBuf>,
    },
    /// Compute or verify the set of workspace crates affected by a set of changed files.
    /// Reads `contracts/ci/crate-graph.v1.json` (BFS reverse-dep closure).
    #[command(name = "affected-crates")]
    AffectedCrates {
        /// Newline-delimited file listing changed paths (relative to repo root).
        #[arg(long)]
        changed: Option<String>,
        /// Override graph file path.
        #[arg(long)]
        graph: Option<String>,
        /// Regenerate `contracts/ci/crate-graph.v1.json` from `cargo metadata`.
        #[arg(long)]
        regen: bool,
        /// Path for `--regen` output.
        #[arg(long)]
        out: Option<String>,
        /// Verify committed graph matches `cargo metadata` (hard-fail on drift).
        #[arg(long)]
        check: bool,
        /// Write computed outputs to `$GITHUB_OUTPUT`.
        #[arg(long)]
        github_output: Option<String>,
    },
    /// Fail-fast: error immediately when no online self-hosted runner can serve the gate.
    #[command(name = "runner-preflight")]
    RunnerPreflight,
    /// Print per-runner state (container + GitHub status), current queue depth, and recent
    /// autoscaler decisions from the decision log. Read-only; never mutates fleet state.
    #[command(name = "runner-status")]
    RunnerStatus,
    /// Run-centric CI queue snapshot: classifies runs active/superseded/stale, carries the
    /// async failure signal, emits machine-readable `advice`, and clears cancellable backlog.
    /// The SSOT queue interaction for agents under the local-first CI contract.
    #[command(name = "queue")]
    Queue {
        /// Emit the full QueueSnapshot as JSON.
        #[arg(long)]
        json: bool,
        /// ≤7-line summary incl. FAILED lines (SessionStart hook uses this).
        #[arg(long)]
        brief: bool,
        /// Read ~/.vox/ci-queue-snapshot.json (no network; refuses >10 min old).
        #[arg(long)]
        from_snapshot: bool,
        /// Cancel superseded + stale runs (live data only; exempt-aware; ≤50/sweep).
        #[arg(long)]
        clear: bool,
        /// With --clear: print the cancellation plan without cancelling.
        #[arg(long)]
        dry_run: bool,
        /// Stale TTL in minutes for queued/pending runs (default 45).
        #[arg(long)]
        ttl_mins: Option<i64>,
        /// PreToolUse hook mode: read hook JSON on stdin; exit 2 on banned remote-watch commands.
        #[arg(long)]
        hook_guard: bool,
    },
    /// Measure CI job run-time (execution, not queue) and warn on anything over the budget (default 10m).
    #[command(name = "job-timings")]
    JobTimings {
        /// Analyze a specific workflow run's jobs (default: scan recent completed runs).
        #[arg(long)]
        run_id: Option<u64>,
        /// Budget in minutes (default 10).
        #[arg(long)]
        threshold_mins: Option<i64>,
        /// How many recent completed runs to scan when `--run-id` is omitted.
        #[arg(long, default_value_t = 5)]
        limit: u32,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
        /// Emit GitHub `::warning::` annotations for over-budget jobs (for CI use).
        #[arg(long)]
        annotate: bool,
        /// Exit non-zero if any job is over budget (default: warn only).
        #[arg(long)]
        strict: bool,
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
    /// Regenerable workspace test inventory (Rust tests, ignores, golden Vox, app E2E paths).
    #[command(name = "test-inventory")]
    TestInventory {
        /// Print deterministic JSON to stdout.
        #[arg(long)]
        json: bool,
        /// Write JSON to this path.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Write Markdown summary to this path.
        #[arg(long)]
        markdown: Option<PathBuf>,
        /// Fail if this JSON file differs after parsing both sides and comparing structured data (semantic equality for `TestInventoryReport`, not a raw text diff).
        #[arg(long)]
        check: Option<PathBuf>,
    },
    /// Safety / suppression-debt baseline (unsafe-ish counts, ignored tests, crate `#![allow]`, TS eslint-disable).
    #[command(name = "safety-inventory")]
    SafetyInventory {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        check: Option<PathBuf>,
    },
    /// Summarize nextest/JUnit XML: slow tests, retries/flaky heuristics, optional threshold warnings (report-only).
    #[command(name = "test-runtime-report")]
    TestRuntimeReport {
        /// Path to JUnit XML (e.g. `target/nextest/ci/junit.xml` when nextest JUnit is enabled).
        #[arg(long)]
        junit: PathBuf,
        /// Emit machine-readable JSON to stdout (suppresses default human summary).
        #[arg(long)]
        json: bool,
        /// Write Markdown summary to this path.
        #[arg(long)]
        markdown: Option<PathBuf>,
        /// Number of slow tests to list (max-time per classname+name).
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Warn when any listed slow test exceeds this duration in milliseconds (does not fail the command).
        #[arg(long)]
        fail_over_ms: Option<u64>,
        /// Warn when the number of retry/flaky candidate rows exceeds this value (does not fail the command).
        #[arg(long)]
        fail_retry_count: Option<usize>,
    },
    /// Governance gate: ignored tests must use `#[ignore = "..."]` with owner/sunset/date-style markers (default warn).
    #[command(name = "ignored-test-age")]
    IgnoredTestAge {
        #[arg(long, value_enum, default_value_t = GovernanceGateMode::Warn)]
        mode: GovernanceGateMode,
        /// Optional `test-inventory` JSON; when set, requires `summary.cargo_ignored_test_functions` to match the live scan count.
        #[arg(long)]
        inventory: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Governance gate: retry/flaky candidate rows vs budget (`test-runtime-report` JSON or JUnit).
    #[command(name = "flake-budget")]
    FlakeBudget {
        #[arg(long, value_enum, default_value_t = GovernanceGateMode::Warn)]
        mode: GovernanceGateMode,
        #[arg(long)]
        report_json: Option<PathBuf>,
        #[arg(long)]
        junit: Option<PathBuf>,
        #[arg(long, default_value_t = 20)]
        top: usize,
        #[arg(long = "max-candidates", default_value_t = 50)]
        max_candidates: usize,
        #[arg(long)]
        json: bool,
    },
    /// Governance gate: compare two `test-runtime-report` JSON files for top-test slowdowns vs baseline.
    #[command(name = "runtime-regress")]
    RuntimeRegress {
        #[arg(long, value_enum, default_value_t = GovernanceGateMode::Warn)]
        mode: GovernanceGateMode,
        #[arg(long)]
        current: PathBuf,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long, default_value_t = 25.0)]
        percent: f64,
        #[arg(long, default_value_t = 500)]
        absolute_ms: u64,
        #[arg(long)]
        json: bool,
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
    /// Enforce no-tauri-in-core architectural boundary.
    #[command(name = "no-tauri-in-core")]
    NoTauriInCore,
    /// Guard that no non-plugin crate takes a compile-time dep on a cdylib plugin (D-2).
    #[command(name = "no-plugin-cdylib-as-compile-dep")]
    NoPluginCdylibAsCompileDep,
    /// Guard that cdylib plugins do not statically link the heavy spine (compiler/db/orchestrator/cli). Warns on known debt; fails on new linkage.
    #[command(name = "plugin-dep-boundary")]
    PluginDepBoundary,
    /// Walk crates/ for code/composite Plugin.toml files and assert ABI matches the host. Skips intentionally-broken `noop-bad-*` fixtures.
    #[command(name = "plugin-abi-parity")]
    PluginAbiParity {
        /// Build each discovered plugin cdylib (cargo build -p `<crate>`) before loading it.
        /// Use in CI so the gate covers newly-added plugins without a manual build list.
        #[arg(long)]
        build: bool,
    },
    /// Extract the plugin extension-point surface from vox-plugin-api into
    /// contracts/plugin/extension-points.v1.yaml (SSOT). Also enforces VoxPlugin accessor parity.
    #[command(name = "plugin-surface-sync")]
    PluginSurfaceSync {
        /// Regenerate the committed file. Without this flag, verify it is in sync.
        #[arg(long)]
        write: bool,
    },
    /// Derive the per-plugin rows of the plugin catalog from the per-plugin Plugin.toml
    /// manifests (description/status/payload-kind/extension-points/exposes-tools).
    #[command(name = "plugin-catalog-sync")]
    PluginCatalogSync {
        /// Regenerate the committed catalog. Without this flag, verify it is in sync.
        #[arg(long)]
        write: bool,
    },
    /// Walk crates/ for skill/composite Plugin.toml files and assert skill-md exists, is non-empty, tools.exposes is non-empty, and the SKILL.md `vox-tools` frontmatter matches the manifest.
    #[command(name = "plugin-skill-parity")]
    PluginSkillParity {
        /// Rewrite each SKILL.md `vox-tools` list from its manifest `tools.exposes`. Without this flag, verify they match.
        #[arg(long)]
        write: bool,
    },
    /// Walk crates/vox-plugin-* for *.skill.md files and enforce AgentSkills frontmatter contract (name, description, format, directory match).
    #[command(name = "agentskills-compliance")]
    AgentSkillsCompliance,
    /// Enforce lean-CLI crate-count budget and forbidden-crate list (Track C Phase 0).
    #[command(name = "profile-parity")]
    ProfileParity,
    /// Federated workspace @tool surface parity (schemas + fixture round-trips).
    #[command(name = "mcp-vox-surface-parity")]
    McpVoxSurfaceParity,
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
    /// Rust toolchain SSOT drift guard: `contracts/toolchain/workspace-toolchain.v1.yaml`
    /// (`versions.rust`) against every restatement (rust-toolchain.toml, the
    /// Cargo.toml `rust-version` MSRV floor, both CI-runner Dockerfiles, the
    /// distribution profile, the stable channel manifest, and the voxup
    /// profiles test fixture). Portable (POSIX-only parsing, no `grep -oP`)
    /// replacement for the guard at ci.yml:788-808, which checks only two of
    /// the rows and cannot run on macOS.
    #[command(name = "toolchain-ssot")]
    ToolchainSsot,
}

impl CiCmd {
    /// The policy-registry id this gate is recorded under in the per-branch
    /// status store, or `None` if it is not run-status-tracked.
    ///
    /// Ids MUST match the `ci-gate` entries the Plan 1b generator emits from
    /// `contracts/operations/catalog.v1.yaml` (id scheme `ci-gate/ci.<command>`).
    /// New gates that should appear in the status overlay add a row here. A
    /// variant returning `None` is simply untracked (honest grey), never faked.
    ///
    /// DEVIATION FROM PLAN: the plan assumed ids of the form `ci/<command>`. The
    /// committed registry (Plan 1b, landed) uses `ci-gate/ci.<command>` derived
    /// from the operations catalog id `ci.<command>`. This map uses the real ids
    /// (cross-checked against `contracts/policy/policy-registry.v1.yaml`).
    pub fn gate_policy_id(&self) -> Option<&'static str> {
        match self {
            CiCmd::Manifest => Some("ci-gate/ci.manifest"),
            CiCmd::SsotDrift => Some("ci-gate/ci.ssot-drift"),
            CiCmd::CommandCompliance => Some("ci-gate/ci.command-compliance"),
            CiCmd::RepoGuards => Some("ci-gate/ci.repo-guards"),
            CiCmd::LineEndings { .. } => Some("ci-gate/ci.line-endings"),
            CiCmd::BomCheck => Some("ci-gate/ci.bom-check"),
            CiCmd::SpokeCheck => Some("ci-gate/ci.spoke-check"),
            CiCmd::FreeBinary { .. } => Some("ci-gate/ci.free-binary"),
            CiCmd::DataSsotGuards => Some("ci-gate/ci.data-ssot-guards"),
            CiCmd::FeatureMatrix => Some("ci-gate/ci.feature-matrix"),
            CiCmd::CompileMatrix => Some("ci-gate/ci.compile-matrix"),
            CiCmd::RetirementAudit => Some("ci-gate/ci.retirement-audit"),
            CiCmd::NoDeiImport => Some("ci-gate/ci.no-dei-import"),
            CiCmd::CheckSummaryDrift => Some("ci-gate/ci.check-summary-drift"),
            CiCmd::BuildDocs => Some("ci-gate/ci.build-docs"),
            CiCmd::CheckDocsSsot => Some("ci-gate/ci.check-docs-ssot"),
            CiCmd::CheckCodexSsot => Some("ci-gate/ci.check-codex-ssot"),
            CiCmd::CheckLinks { .. } => Some("ci-gate/ci.check-links"),
            CiCmd::ContractsIndex => Some("ci-gate/ci.contracts-index"),
            CiCmd::AiFixturesCoverage => Some("ci-gate/ci.ai-fixtures-coverage"),
            CiCmd::ExecPolicyContract => Some("ci-gate/ci.exec-policy-contract"),
            CiCmd::OpenClawContract => Some("ci-gate/ci.openclaw-contract"),
            CiCmd::OperationsVerify => Some("ci-gate/ci.operations-verify"),
            CiCmd::NomenclatureGuard { .. } => Some("ci-gate/ci.nomenclature-guard"),
            CiCmd::RustEcosystemPolicy => Some("ci-gate/ci.rust-ecosystem-policy"),
            CiCmd::PolicySmoke => Some("ci-gate/ci.policy-smoke"),
            CiCmd::SecretEnvGuard { .. } => Some("ci-gate/ci.secret-env-guard"),
            CiCmd::SecretsParity => Some("ci-gate/ci.secrets-parity"),
            CiCmd::SqlSurfaceGuard { .. } => Some("ci-gate/ci.sql-surface-guard"),
            CiCmd::QueryAllGuard { .. } => Some("ci-gate/ci.query-all-guard"),
            CiCmd::TursoImportGuard { .. } => Some("ci-gate/ci.turso-import-guard"),
            CiCmd::DbSchemaCoverage => Some("ci-gate/ci.db-schema-coverage"),
            CiCmd::PolicyAllowlistParity => Some("ci-gate/ci.policy-allowlist-parity"),
            CiCmd::BackendTests => Some("ci-gate/ci.backend-tests"),
            CiCmd::DocsRealityAudit { .. } => Some("ci-gate/ci.docs-reality-audit"),
            CiCmd::DocInventory { .. } => Some("ci-gate/ci.doc-inventory"),
            CiCmd::WorkflowScripts { .. } => Some("ci-gate/ci.workflow-scripts"),
            CiCmd::ScientiaWorthinessContract => Some("ci-gate/ci.scientia-worthiness-contract"),
            CiCmd::ScientiaNoveltyLedgerContracts => {
                Some("ci-gate/ci.scientia-novelty-ledger-contracts")
            }
            CiCmd::BuildCacheDoctor => Some("ci-gate/ci.build-cache-doctor"),
            CiCmd::ProfileParity => Some("ci-gate/ci.profile-parity"),
            // The registry machinery itself is intentionally untracked, and any
            // gate without a registry-backed `ci-gate` row stays grey.
            _ => None,
        }
    }
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

/// Subcommands for [`CiCmd::DocsRealityAudit`].
#[derive(Subcommand)]
pub enum DocsRealityAuditCmd {
    /// Validate JSON artifacts against schemas; ensure inventory path hints resolve.
    Verify,
    /// Emit rollout metrics for `findings.v1.json` (stdout; use `--write` to refresh `metrics.v1.json`).
    Metrics {
        /// Write `contracts/reports/docs-reality-audit/metrics.v1.json`.
        #[arg(long)]
        write: bool,
    },
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
    pub fn as_cli_str(self) -> &'static str {
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

/// `vox ci ignored-test-age` / `flake-budget` / `runtime-regress` exit behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum GovernanceGateMode {
    /// Print findings; exit 0 unless paired with other failures.
    #[default]
    Warn,
    /// Exit non-zero when the gate detects drift.
    Enforce,
}

impl GovernanceGateMode {
    pub fn label(self) -> &'static str {
        match self {
            GovernanceGateMode::Warn => "warn",
            GovernanceGateMode::Enforce => "enforce",
        }
    }
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

#[cfg(test)]
mod policy_id_tests {
    use super::*;

    #[test]
    fn known_gates_map_to_registry_ids() {
        assert_eq!(
            CiCmd::Manifest.gate_policy_id(),
            Some("ci-gate/ci.manifest")
        );
        assert_eq!(
            CiCmd::SsotDrift.gate_policy_id(),
            Some("ci-gate/ci.ssot-drift")
        );
        // Generator/parity gates are not run-status-tracked (they ARE the catalog).
        assert_eq!(CiCmd::PolicyRegistryParity.gate_policy_id(), None);
        assert_eq!(
            CiCmd::PolicyRegistry { write: false }.gate_policy_id(),
            None
        );
    }

    #[test]
    fn gate_policy_ids_are_well_formed() {
        // Every mapped id is `ci-gate/ci.<kebab>` (matches the 1b ci-gate namespace).
        for id in [
            CiCmd::Manifest.gate_policy_id(),
            CiCmd::SsotDrift.gate_policy_id(),
            CiCmd::LineEndings {
                all: false,
                base: None,
                autofix: false,
            }
            .gate_policy_id(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                id.starts_with("ci-gate/ci."),
                "{id} must be ci-gate/ci.-namespaced"
            );
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '/' || c == '-' || c == '.')
            );
        }
    }
}
