//! `vox-audit` binary — clap-driven CLI dispatcher for the
//! `vox audit <thing>` registry.
//!
//! Conforms to [`contracts/ci/vox-audit-contract.v1.yaml`](../../../../contracts/ci/vox-audit-contract.v1.yaml):
//! flag set, exit-code convention, canonical report shape, atomic-write
//! `contracts/reports/<thing>/<date>.json`.

use std::path::PathBuf;
use std::process::ExitCode as ProcessExitCode;

use clap::{Parser, Subcommand as ClapSubcommand};
use vox_audit::{
    CommonArgs, CrlGate, aggregate_exit_code, gate_from_name, registry,
    report::{ExitCode, ReportFormat},
    run_all, run_gate,
};

/// Cargo-friendly CLI: `cargo run -p vox-audit -- <subcommand> [flags]`.
#[derive(Debug, Parser)]
#[command(
    name = "vox-audit",
    about = "Run the CR-L gate registry (`vox audit <thing>`). See contracts/ci/vox-audit-contract.v1.yaml.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    /// Output format (json | markdown | html). Default: json.
    #[arg(long, global = true, default_value = "json")]
    format: String,

    /// Compare results against a prior report JSON at this path.
    #[arg(long, global = true)]
    baseline: Option<PathBuf>,

    /// Override the bar from the corpus manifest (e.g. `--threshold 0.8`).
    #[arg(long, global = true)]
    threshold: Option<f64>,

    /// Override the default corpus / contract path for the subcommand.
    #[arg(long, global = true)]
    corpus: Option<PathBuf>,

    /// Override the default `contracts/eval/llm-panel.v1.yaml`.
    #[arg(long, global = true)]
    llm_panel: Option<PathBuf>,

    /// Validate inputs without running the underlying measurement. Exits 0 on
    /// success; useful as a fast CI pre-check.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Suppress writing the canonical
    /// `contracts/reports/<thing>/<YYYY-MM-DD>.json`. Default is to write it.
    #[arg(long, global = true)]
    no_canonical_report: bool,

    /// With `all`: evaluate the GA roll-up (foundation-first, with
    /// `blocked_by_foundation`) and return a non-zero exit when GA is not met.
    /// Writes `contracts/reports/_snapshot/<UTC>.json`. (CR-F0.)
    #[arg(long, global = true)]
    strict_block_ga: bool,
}

#[derive(Debug, ClapSubcommand)]
enum CliCommand {
    /// CR-F1: behavioral goldens — `// EXPECT:` stdout matches `vox run --mode interp`.
    BehavioralGoldens,
    /// CR-L0: end-to-end agent loop (spec → passing app).
    SpecToApp,
    /// CR-L1: HumanEval-Vox.
    Humaneval,
    /// CR-L2: MENS on-distribution rate.
    MensOnDistribution,
    /// CR-L3: project-scope `vox repair .` corpus.
    RepairCorpus,
    /// CR-L4: plan-mode fidelity.
    PlanFidelity,
    /// CR-L5: ACI envelope default-on check.
    AciDefault,
    /// CR-L6: retirement-guard parity vs AGENTS.md §Retired Surfaces.
    Retirement,
    /// CR-L7: `vox new → vox deploy → vox doctor` E2E.
    Deploy,
    /// CR-L8: corpus-feedback artifact freshness.
    CorpusFeedback,
    /// Tooling gate (non-CR-L): three-way stdlib drift detector
    /// (eval/builtins.rs ↔ ref-builtins-stdlib.md ↔ scripts/).
    /// Does NOT block GA; blocks PRs introducing drift.
    StdlibCoverage,
    /// Run every registered subcommand and aggregate the exit code.
    All,
    /// List every registered subcommand with its gate and description.
    List,
}

impl CliCommand {
    fn to_gate_name(&self) -> Option<&'static str> {
        match self {
            CliCommand::BehavioralGoldens => Some("behavioral-goldens"),
            CliCommand::SpecToApp => Some("spec-to-app"),
            CliCommand::Humaneval => Some("humaneval"),
            CliCommand::MensOnDistribution => Some("mens-on-distribution"),
            CliCommand::RepairCorpus => Some("repair-corpus"),
            CliCommand::PlanFidelity => Some("plan-fidelity"),
            CliCommand::AciDefault => Some("aci-default"),
            CliCommand::Retirement => Some("retirement"),
            CliCommand::Deploy => Some("deploy"),
            CliCommand::CorpusFeedback => Some("corpus-feedback"),
            CliCommand::StdlibCoverage => Some("stdlib-coverage"),
            CliCommand::All | CliCommand::List => None,
        }
    }
}

fn main() -> ProcessExitCode {
    let cli = Cli::parse();

    let format = match ReportFormat::parse(&cli.format) {
        Ok(fmt) => fmt,
        Err(msg) => {
            eprintln!("vox-audit: {msg}");
            return ProcessExitCode::from(ExitCode::InvalidInput.as_i32() as u8);
        }
    };

    let common = CommonArgs {
        format,
        baseline: cli.baseline,
        threshold: cli.threshold,
        corpus: cli.corpus,
        llm_panel: cli.llm_panel,
        dry_run: cli.dry_run,
        // Dry-run reports carry sentinel values (e.g. `corpus_hash: "blake3:dry-run"`,
        // `overall_pass_rate: 0.0`); writing them to the canonical dated path would
        // clobber the real artifact from the most recent non-dry-run.
        write_canonical_report: !cli.no_canonical_report && !cli.dry_run,
    };

    match cli.command {
        CliCommand::List => {
            list_subcommands();
            ProcessExitCode::SUCCESS
        }
        CliCommand::All if cli.strict_block_ga => {
            // GA roll-up: foundation-first, blocked_by_foundation, _snapshot.
            let snap = vox_audit::run_ga_snapshot(&common, true);
            match serde_json::to_string_pretty(&snap) {
                Ok(text) => println!("{text}"),
                Err(err) => eprintln!("vox-audit: failed to render GA snapshot: {err}"),
            }
            ProcessExitCode::from(snap.exit_code as u8)
        }
        CliCommand::All => {
            let outcomes = run_all(&common);
            for outcome in &outcomes {
                render_and_maybe_write(&outcome.report, &common);
            }
            ProcessExitCode::from(aggregate_exit_code(&outcomes).as_i32() as u8)
        }
        ref cmd => {
            let gate_name = cmd
                .to_gate_name()
                .expect("non-All/List branches always yield a gate name");
            let Some(gate) = gate_from_name(gate_name) else {
                eprintln!("vox-audit: unknown gate `{gate_name}`");
                return ProcessExitCode::from(ExitCode::InvalidInput.as_i32() as u8);
            };
            let outcome = run_gate(gate, &common);
            render_and_maybe_write(&outcome.report, &common);
            ProcessExitCode::from(outcome.exit_code.as_i32() as u8)
        }
    }
}

fn render_and_maybe_write(report: &vox_audit::report::AuditReport, args: &CommonArgs) {
    match report.render(args.format) {
        Ok(text) => println!("{text}"),
        Err(err) => eprintln!("vox-audit: failed to render report: {err}"),
    }
    if args.write_canonical_report {
        let path = report.canonical_report_path();
        if let Err(err) = report.write_json_atomic(&path) {
            eprintln!(
                "vox-audit: failed to write canonical report to {}: {err}",
                path.display()
            );
        }
    }
}

fn list_subcommands() {
    println!("Registered vox audit subcommands ({}):", registry().len());
    for sub in registry() {
        let gate = sub.gate();
        let gate_debug = format!("{gate:?}");
        println!(
            "  - {name:<24} (gate {gate_debug:<24} block_ga={block:<5} cost_metered={cost})\n      {desc}",
            name = gate.thing_name(),
            block = gate.block_ga(),
            cost = gate.cost_metered(),
            desc = sub.description(),
        );
    }
    // Keep the explicit import live so non-List branches see the same item set.
    let _ = CrlGate::L0SpecToApp;
}
