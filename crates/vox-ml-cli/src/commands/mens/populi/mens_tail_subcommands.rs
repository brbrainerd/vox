//! Tail `vox mens` subcommands (eval-gate, bench, system prompt) — separate module for TOESTUB line budget.

use std::path::PathBuf;

use clap::Subcommand;

/// Eval-gate, completion bench, and system-prompt-template (flattened under [`PopuliAction`](super::PopuliAction)).
#[derive(Subcommand)]
pub enum PopuliMensTail {
    /// Check training/eval run against eval-gate policy thresholds.
    ///
    /// Reads mens/config/eval-gates.yaml and validates run artifacts.
    /// Exits 1 if any blocking gate fails.
    #[command(name = "eval-gate")]
    EvalGate {
        /// Run directory (manifest.json, metrics.jsonl, eval_results.json)
        #[arg(long, default_value = vox_scaling_policy::DEFAULT_MENS_RUNS_V1)]
        run_dir: PathBuf,
        /// Policy file (default: mens/config/eval-gates.yaml)
        #[arg(long)]
        policy: Option<PathBuf>,
    },

    /// Capture a base-model BFCL baseline so the beat-base eval gate has a real
    /// point of comparison.
    ///
    /// Run the BFCL eval harness against the BASE model (no adapter) to produce
    /// `bfcl_results.json` in `--base-eval-dir`, then run this to write
    /// `baseline_report.json`. Without a captured baseline, beat-base silently
    /// skips.
    #[command(name = "baseline")]
    Baseline {
        /// Spoke identifier this baseline applies to (e.g. vox-lang, tool-selection).
        #[arg(long)]
        spoke: String,
        /// Directory containing the base-model `bfcl_results.json`.
        #[arg(long)]
        base_eval_dir: PathBuf,
        /// Output path for baseline_report.json
        /// (default: <base-eval-dir>/baseline_report.json).
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Check for catastrophic forgetting against a static benchmark
    #[command(name = "eval-collateral-damage", visible_alias = "eval")]
    EvalCollateralDamage {
        /// Baseline score JSON path
        #[arg(long)]
        pre_score: PathBuf,
        /// Adapter path to evaluate
        #[arg(long, id = "post")]
        post_adapter: PathBuf,
    },

    /// Benchmark FIM completion server latency
    BenchCompletion {
        /// URL of the completions API Endpoint
        #[arg(long, default_value = "http://127.0.0.1:8080/v1/completions")]
        url: String,
        /// Number of benchmark iteration runs
        #[arg(short = 'c', long, default_value = "100")]
        count: usize,
        /// Number of initial warmup requests
        #[arg(short = 'w', long, default_value = "5")]
        warmup: usize,
    },

    /// Generate the canonical system prompt template for IDE integration (Cursor, Claude, etc.)
    #[command(name = "system-prompt-template")]
    SystemPromptTemplate {
        /// Optional: write the template to a file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Optional: wrap in IDE-specific format: text (default), cursor, claude, copilot, or wind-pro (Windsurf)
        #[arg(long, default_value = "text")]
        format: String,
    },
}
