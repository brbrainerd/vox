use clap::Parser;
use std::collections::HashSet;
use std::path::PathBuf;
use vox_code_audit::rules::Severity;
use vox_drift_check::{engine::DriftEngine, report};

#[derive(Parser)]
#[command(
    name = "vox-drift-check",
    about = "Workspace-wide drift & repetition linter"
)]
struct Cli {
    /// Workspace root (defaults to current directory)
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Emit JSON output
    #[arg(long)]
    json: bool,
    /// Minimum severity to show (info/warning/error)
    #[arg(long, default_value = "info")]
    severity: String,
    /// Exit non-zero if any findings at this level (info/warning/error)
    #[arg(long, default_value = "error")]
    fail_on: String,
    /// Path to baseline JSON file; only report new findings vs baseline
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Write current findings to baseline file and exit 0
    #[arg(long)]
    update_baseline: bool,
}

fn parse_sev(s: &str) -> Severity {
    match s {
        "info" => Severity::Info,
        "error" | "critical" => Severity::Error,
        _ => Severity::Warning,
    }
}

fn main() {
    let cli = Cli::parse();
    let engine = DriftEngine::new(&cli.root);
    let mut findings = match engine.run_all() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(2);
        }
    };

    if cli.update_baseline {
        let baseline_path = cli
            .baseline
            .unwrap_or_else(|| cli.root.join(vox_config::paths::REPO_DRIFT_BASELINE_FILE));
        if let Some(parent) = baseline_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let json = serde_json::to_string_pretty(&findings).unwrap_or_default();
        std::fs::write(&baseline_path, json).unwrap_or_else(|e| {
            eprintln!("Failed to write baseline: {}", e);
            std::process::exit(2);
        });
        println!(
            "Baseline updated ({} findings) → {}",
            findings.len(),
            baseline_path.display()
        );
        std::process::exit(0);
    }

    if let Some(base_path) = &cli.baseline {
        let base_json = std::fs::read_to_string(base_path).unwrap_or_else(|_| "[]".to_string());
        let base: Vec<vox_code_audit::rules::Finding> =
            serde_json::from_str(&base_json).unwrap_or_default();
        let base_keys: HashSet<String> = base
            .iter()
            .map(|f| format!("{}:{}:{}", f.rule_id, f.file.display(), f.line))
            .collect();
        findings.retain(|f| {
            !base_keys.contains(&format!("{}:{}:{}", f.rule_id, f.file.display(), f.line))
        });
    }

    if cli.json {
        report::print_json(&findings);
    } else {
        report::print_terminal(&findings, parse_sev(&cli.severity));
    }
    std::process::exit(report::exit_code(&findings, parse_sev(&cli.fail_on)));
}
