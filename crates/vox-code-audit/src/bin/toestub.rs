//! TOEStub CLI — run architecture / TOE compliance checks from the terminal.
//!
//! Usage: `toestub [ROOT_DIR]` — defaults to the current directory. Prefer passing a crate path
//! in CI (e.g. `toestub crates/vox-repository`) for a fast, focused gate.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use vox_code_audit::engine::ToestubRunMode;
use vox_code_audit::rules::Severity;
use vox_code_audit::{OutputFormat, ToestubConfig, ToestubEngine, ToestubTestsMode};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum CliMode {
    Legacy,
    Audit,
    #[value(name = "enforce-warn")]
    EnforceWarn,
    #[value(name = "enforce-strict")]
    EnforceStrict,
}

impl From<CliMode> for ToestubRunMode {
    fn from(m: CliMode) -> Self {
        match m {
            CliMode::Legacy => ToestubRunMode::Legacy,
            CliMode::Audit => ToestubRunMode::Audit,
            CliMode::EnforceWarn => ToestubRunMode::EnforceWarn,
            CliMode::EnforceStrict => ToestubRunMode::EnforceStrict,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "toestub", about = "TOESTUB structural and policy checks")]
struct Opt {
    /// Root directory to scan (default `.`).
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Exit-code policy: `legacy` (errors fail), `audit` (never fail), `enforce-warn` (critical only), `enforce-strict` (warnings+).
    #[arg(long, value_enum, default_value_t = CliMode::Legacy)]
    mode: CliMode,
    /// Minimum severity to report (audit mode forces `info`).
    #[arg(long, value_enum, default_value_t = SeverityArg::Warning)]
    min_severity: SeverityArg,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Terminal)]
    format: FormatArg,
    /// Include fix suggestions in the task queue (Markdown / internal tooling).
    #[arg(long, default_value_t = false)]
    suggest_fixes: bool,
    /// Only run rules whose `DetectionRule::id` starts with one of these prefixes (repeatable; e.g. `--rules scaling`).
    #[arg(long = "rules", value_name = "PREFIX")]
    rule_prefix: Vec<String>,
    /// Structured suppressions JSON (`contracts/toestub/suppression.v1.schema.json`).
    #[arg(long, value_name = "PATH")]
    suppressions: Option<PathBuf>,
    /// Comma-separated crates: AST-enhanced unresolved-ref only under `crates/<name>/` (canary rollout).
    #[arg(long, value_name = "CRATES")]
    canary_crates: Option<String>,
    /// How to scan Rust files under `.../tests/...` (`off` skips noisy unresolved-ref there).
    #[arg(long, value_enum, default_value_t = TestsModeArg::Off)]
    tests_mode: TestsModeArg,
    /// Optional `prelude-allowlist.v1.json` path (extra unresolved-fn allow idents).
    #[arg(long, value_name = "PATH")]
    prelude_allowlist: Option<PathBuf>,
    /// Comma-separated feature flags (`unwired-graph`, `unresolved-regex-fallback`, ...).
    #[arg(long, value_name = "FLAGS")]
    feature_flags: Option<String>,
}

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
enum TestsModeArg {
    #[default]
    Off,
    Include,
    Strict,
}

impl From<TestsModeArg> for ToestubTestsMode {
    fn from(a: TestsModeArg) -> Self {
        match a {
            TestsModeArg::Off => ToestubTestsMode::Off,
            TestsModeArg::Include => ToestubTestsMode::Include,
            TestsModeArg::Strict => ToestubTestsMode::Strict,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum SeverityArg {
    Info,
    Warning,
    Error,
    Critical,
}

impl From<SeverityArg> for Severity {
    fn from(s: SeverityArg) -> Self {
        match s {
            SeverityArg::Info => Severity::Info,
            SeverityArg::Warning => Severity::Warning,
            SeverityArg::Error => Severity::Error,
            SeverityArg::Critical => Severity::Critical,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum FormatArg {
    Terminal,
    Json,
    Markdown,
}

impl From<FormatArg> for OutputFormat {
    fn from(f: FormatArg) -> Self {
        match f {
            FormatArg::Terminal => OutputFormat::Terminal,
            FormatArg::Json => OutputFormat::Json,
            FormatArg::Markdown => OutputFormat::Markdown,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    let run_mode: ToestubRunMode = opt.mode.into();
    let min_severity = match run_mode {
        ToestubRunMode::Audit => Severity::Info,
        _ => Severity::from(opt.min_severity),
    };

    let rule_filter = if opt.rule_prefix.is_empty() {
        None
    } else {
        Some(opt.rule_prefix)
    };
    let canary_crates = opt.canary_crates.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
    });
    let canary_crates = canary_crates.filter(|v| !v.is_empty());
    let feature_flags = opt.feature_flags.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
    });
    let feature_flags = feature_flags.unwrap_or_default();
    let config = ToestubConfig {
        roots: vec![opt.root],
        min_severity,
        format: OutputFormat::from(opt.format),
        suggest_fixes: opt.suggest_fixes,
        rule_filter,
        run_mode,
        suppression_path: opt.suppressions,
        canary_crates,
        tests_mode: ToestubTestsMode::from(opt.tests_mode),
        prelude_allowlist_path: opt.prelude_allowlist,
        feature_flags,
        ..ToestubConfig::default()
    };
    let run_mode = config.run_mode;
    let engine = ToestubEngine::new(config);
    let (result, output) = engine.run_and_report();

    println!("{}", output);

    if result.should_fail_build(run_mode) {
        std::process::exit(1);
    }

    Ok(())
}
