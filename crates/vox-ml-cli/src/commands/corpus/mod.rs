//! `vox corpus` subcommand — native training data pipeline.
//!
//! Replaces the Python scripts with in-process Rust implementations:
//! - `extract` — walk .vox files, emit JSONL corpus
//! - `validate` — re-check entries, dedup, print coverage
//! - `pairs` — generate instruction→response training pairs
//! - `mix` — merge sources per `mens/config/mix.yaml`
//! - `prompt` — auto-generate system prompt from construct reference

pub(crate) mod generate;
mod stats;
mod validate;

use anyhow::{Context, Result};

use clap::Parser;
use vox_bounded_fs::read_utf8_path_capped_async;

#[cfg(all(feature = "mens-dei", feature = "gpu"))]
pub(crate) use stats::{eval_metrics, run_benchmark_gate};

/// Subcommands for invoking native Mens training data pipelines.
#[derive(Parser)]
pub enum CorpusAction {
    /// Generate synthetic training pairs (tools, orchestrator, A2A, etc)
    Generate {
        /// Output JSONL file
        #[arg(short, long, default_value = "mens/data/synthetic.jsonl")]
        output: std::path::PathBuf,
        /// Force regeneration even if corpus fingerprint is fresh
        #[arg(long)]
        force_regen: bool,
        /// Dry-run mode (print pairs generated but do not write output or update fingerprint)
        #[arg(long)]
        dry_run: bool,
    },
    /// Print current corpus fingerprint based on AST/generator files
    Fingerprint,
    /// Extract training corpus from .vox files
    Extract {
        /// Directory containing .vox files (recursive)
        #[arg(required = true)]
        dir: std::path::PathBuf,
        /// Output JSONL file
        #[arg(short, long, default_value = "mens/data/validated.jsonl")]
        output: std::path::PathBuf,
    },
    /// Extract training pairs from Rust source code (.rs)
    ExtractRs {
        /// Root directory (usually `crates/`)
        #[arg(default_value = "crates")]
        dir: std::path::PathBuf,
        /// Output JSONL file
        #[arg(short, long, default_value = "mens/data/mix_sources/rust_source.jsonl")]
        output: std::path::PathBuf,
    },
    /// Extract training pairs from documentation (.md)
    ExtractDocs {
        /// Documentation directory (usually `docs/src/`)
        #[arg(default_value = "docs/src")]
        dir: std::path::PathBuf,
        /// Output JSONL file
        #[arg(short, long, default_value = "mens/data/mix_sources/docs.jsonl")]
        output: std::path::PathBuf,
    },
    /// Synthesize Rust structural translation pairs
    RustMine {
        /// Source directory of Rust code
        #[arg(long, default_value = ".")]
        source_dir: std::path::PathBuf,
        /// Output JSONL file
        #[arg(short, long, default_value = "target/dogfood/rust_to_vox.jsonl")]
        output: std::path::PathBuf,
    },
    /// Generate SFT pairs by structurally mutating a source directory
    Mutate {
        /// Input source directory containing .vox files
        #[arg(long, default_value = "examples/golden")]
        source_dir: std::path::PathBuf,
        /// How many target pairs to aim for
        #[arg(short, long, default_value_t = 5000)]
        count: usize,
        /// Output JSONL file
        #[arg(short, long, default_value = "target/dogfood/mutated_vox.jsonl")]
        output: std::path::PathBuf,
    },
    /// Validate and deduplicate a corpus JSONL file
    #[command(name = "validate-batch", alias = "validate")]
    Validate {
        /// Input JSONL file
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Output JSONL file (optional, defaults to overwriting input if not specified)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Skip re-checking code through the compiler
        #[arg(long)]
        no_recheck: bool,
        /// Write compiler-rejected rows (original line + reason) as JSONL
        #[arg(long)]
        quarantine: Option<std::path::PathBuf>,
        /// Write JSON summary (counts, sample failures)
        #[arg(long)]
        report: Option<std::path::PathBuf>,
        /// Apply reward-based filter (e.g., cargo_build, cargo_test) to reject low-performing rows
        #[arg(long)]
        reward_hook: Option<String>,
    },
    /// Audit corpus diversity by measuring AST semantic entropy
    DiversityCheck {
        /// Input JSONL file
        #[arg(short, long)]
        input: std::path::PathBuf,
        /// Minimum allowed AST diversity (0.0-1.0, recommended 0.40)
        #[arg(long, default_value_t = 0.40)]
        min_diversity: f64,
        /// Optional training domain for segmented telemetry (vox-lang, rust-expert, agents)
        #[arg(short, long)]
        domain: Option<String>,
    },
    /// Generate instruction→response training pairs from validated corpus
    Pairs {
        /// Input validated JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output training JSONL file
        #[arg(short, long, default_value = "target/dogfood/train.jsonl")]
        output: std::path::PathBuf,
        /// Documentation root(s) for additional fenced `vox` pairs (repeatable; merged)
        #[arg(long, action = clap::ArgAction::Append)]
        docs: Vec<std::path::PathBuf>,
    },
    /// Replay Arca telemetry into Mens training pairs
    Replay {
        /// Emit full multi-turn ChatML sessions (groups events by session_id)
        #[arg(long)]
        chatml: bool,
        /// Minimum session quality score to include (0.0 = all)
        #[arg(long, default_value = "0.0")]
        min_score: f64,
        /// Output JSONL path (auto-picked by `vox mens mix`)
        #[arg(
            short,
            long,
            default_value = "mens/data/mix_sources/autofeedback.jsonl"
        )]
        output: std::path::PathBuf,
        /// Max session/event rows to query
        #[arg(long, default_value = "500")]
        limit: i64,
    },
    /// Export high-quality agent traces from VoxDB for A2A training
    Dogfood {
        /// Output JSONL path
        #[arg(short, long, default_value = "target/dogfood/agent_dogfood.jsonl")]
        output: std::path::PathBuf,
        /// Max records to query
        #[arg(short, long, default_value = "1000")]
        limit: i64,
    },
    /// Export high-quality agent DPO pairs (correction vs failure) from VoxDB
    DpoDogfood {
        /// Output JSONL path
        #[arg(short, long, default_value = "target/dogfood/agent_dpo.jsonl")]
        output: std::path::PathBuf,
        /// Max records to query
        #[arg(short, long, default_value = "1000")]
        limit: i64,
    },
    /// Auto-generate vox_system_prompt.txt from construct reference
    Prompt {
        /// Output file path
        #[arg(short, long, default_value = "scripts/vox_system_prompt.txt")]
        output: std::path::PathBuf,
    },
    /// Run eval metrics on training data (format, safety, parse, coverage)
    Eval {
        /// Input training JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output eval results JSON
        #[arg(short, long, default_value = "target/dogfood/eval_results.json")]
        output: std::path::PathBuf,
        /// Print one machine-readable summary line to stdout (for CI logs).
        #[arg(long)]
        print_summary: bool,
    },
    /// Merge corpus sources defined in a mix config (same as `vox mens train` preflight)
    Mix {
        /// Path to mix YAML (default: `mens/config/mix.yaml`)
        #[arg(long, default_value = "mens/config/mix.yaml")]
        config: std::path::PathBuf,
        /// Allow missing or empty required sources (warn only; default is strict fail for operator mixes).
        #[arg(long)]
        allow_missing_sources: bool,
    },
    /// Show corpus statistics (domain mixture, category counts)
    Stats {
        /// Input JSONL file (defaults to `target/dogfood/train.jsonl`)
        #[arg(short, long, default_value = "target/dogfood/train.jsonl")]
        input: std::path::PathBuf,
    },
    /// Export review-derived dataset rows from VoxDB.
    ReviewExport {
        /// Repository id, e.g. `owner/repo`.
        #[arg(long)]
        repository_id: String,
        /// Max findings window.
        #[arg(long, default_value_t = 500)]
        limit: i64,
        /// Output JSONL file.
        #[arg(
            short,
            long,
            default_value = "mens/data/mix_sources/review_findings.jsonl"
        )]
        output: std::path::PathBuf,
    },
    /// Validate review-derived dataset rows.
    ReviewValidate {
        /// Input JSONL file.
        #[arg(required = true)]
        input: std::path::PathBuf,
    },
    /// Show review-derived dataset stats.
    ReviewStats {
        /// Input JSONL file.
        #[arg(
            short,
            long,
            default_value = "mens/data/mix_sources/review_findings.jsonl"
        )]
        input: std::path::PathBuf,
    },
    /// Convert dogfood interaction JSONL (produced live by MCP tools) into train-ready format.
    DogfoodConvert {
        /// Input JSONL file.
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output JSONL file.
        #[arg(
            short,
            long,
            default_value = "mens/data/mix_sources/dogfood_converted.jsonl"
        )]
        output: std::path::PathBuf,
    },
    /// Convert heal_pairs.jsonl into DPO preference pairs
    #[command(name = "heal-to-dpo")]
    HealToDpo {
        /// Input JSONL file
        #[arg(short, long)]
        input: Option<std::path::PathBuf>,
        /// Output JSONL file
        #[arg(short, long, default_value = "target/dogfood/preference_pairs.jsonl")]
        output: std::path::PathBuf,
    },
    /// Generate synthetic research multi-hop chains
    ResearchGen {
        /// Output JSONL file
        #[arg(short, long, default_value = "mens/data/research-lane-sft.jsonl")]
        output: std::path::PathBuf,
        /// Number of chains to generate
        #[arg(short, long, default_value_t = 1000)]
        count: usize,
    },
    /// Curate prose-heavy lanes using a frontier LLM to filter logic hazards.
    CurateProse {
        /// Input JSONL file
        #[arg(required = true)]
        input: std::path::PathBuf,
        /// Output JSONL file (curated)
        #[arg(short, long)]
        output: std::path::PathBuf,
        /// Minimum semantic integrity score (0.0-1.0)
        #[arg(long, default_value_t = 0.7)]
        min_score: f64,
        /// Optional quarantine file for rejected records
        #[arg(long)]
        quarantine: Option<std::path::PathBuf>,
    },
    /// Ingest agent traces (JSON/JSONL) into SFT rows with semantic diversity gate.
    #[command(name = "trace-ingest")]
    TraceIngest {
        /// Input agent trace file (JSON array or JSONL of agent_trace_record objects)
        #[arg(short, long, default_value = "target/dogfood/a2a_traces.jsonl")]
        input: std::path::PathBuf,
        /// Output SFT JSONL
        #[arg(short, long, default_value = "mens/data/mix_sources/agent_traces.jsonl")]
        output: std::path::PathBuf,
        /// Minimum AST semantic diversity (0.0-1.0); corpus rejected if monoculture
        #[arg(long, default_value_t = 0.40)]
        min_diversity: f64,
    },
}

/// Execute the native training data extraction or validation logic.
pub async fn run(action: CorpusAction) -> Result<()> {
    match action {
        CorpusAction::Fingerprint => stats::run_fingerprint().await,
        CorpusAction::Generate {
            output,
            force_regen,
            dry_run,
        } => generate::run_generate(output, force_regen, dry_run).await,
        CorpusAction::Extract { dir, output } => generate::run_extract(&dir, &output).await,
        CorpusAction::ExtractRs { dir, output } => {
            let config = vox_corpus::corpus::extract_rs::ExtractRsConfig {
                root: dir,
                ..Default::default()
            };
            let pairs = vox_corpus::corpus::extract_rs::walk_and_extract(&config)?;
            let count = vox_corpus::corpus::extract_rs::write_to_jsonl(&pairs, &output)?;
            println!(
                "✓ Extracted {} Rust source pairs → {}",
                count,
                output.display()
            );
            Ok(())
        }
        CorpusAction::ExtractDocs { dir, output } => {
            let config = vox_corpus::corpus::extract_docs::ExtractDocsConfig {
                root: dir,
                ..Default::default()
            };
            let pairs = vox_corpus::corpus::extract_docs::walk_and_extract_docs(&config)?;
            let count = vox_corpus::corpus::extract_docs::write_docs_to_jsonl(&pairs, &output)?;
            println!(
                "✓ Extracted {} documentation pairs → {}",
                count,
                output.display()
            );
            Ok(())
        }
        CorpusAction::RustMine { source_dir, output } => {
            generate::run_rust_mine(&source_dir, &output).await
        }
        CorpusAction::Mutate {
            source_dir,
            count,
            output,
        } => generate::run_mutate(&source_dir, count, &output).await,
        CorpusAction::Validate {
            input,
            output,
            no_recheck,
            quarantine,
            report,
            reward_hook,
        } => {
            let out = output.as_deref().unwrap_or(&input);
            validate::run_validate(
                &input,
                out,
                !no_recheck,
                quarantine.as_deref(),
                report.as_deref(),
                reward_hook.as_deref(),
            )
            .await
        }
        CorpusAction::DiversityCheck {
            input,
            min_diversity,
            domain,
        } => generate::run_diversity_check(&input, min_diversity, domain.as_deref()).await,
        CorpusAction::Pairs {
            input,
            output,
            docs,
        } => generate::run_pairs(&input, &output, &docs).await,
        CorpusAction::Prompt { output } => generate::run_prompt(&output).await,
        CorpusAction::Eval {
            input,
            output,
            print_summary,
        } => stats::run_eval(&input, &output, print_summary).await,
        CorpusAction::Mix {
            config,
            allow_missing_sources,
        } => vox_corpus::corpus::run_mix_with_options(
            &config,
            None,
            vox_corpus::corpus::MixRunOptions {
                strict: !allow_missing_sources,
                write_report: true,
            },
        ),
        CorpusAction::Replay {
            chatml: _chatml,
            min_score: _min_score,
            output: _output,
            limit: _limit,
        } => {
            #[cfg(feature = "database")]
            {
                let db = vox_db::VoxDb::connect_default()
                    .await
                    .context("Replay requires a database connection (set VOX_DB_URL)")?;
                let rows =
                    vox_corpus::arca_replay::extract_arca_pairs(&db, _limit, _chatml, _min_score)
                        .await
                        .context("Failed to extract Arca replay pairs")?;
                let parent = _output.parent().unwrap_or(std::path::Path::new("."));
                tokio::fs::create_dir_all(parent).await?;
                let mut body = String::new();
                for row in &rows {
                    body.push_str(&serde_json::to_string(row)?);
                    body.push('\n');
                }
                tokio::fs::write(&_output, body)
                    .await
                    .with_context(|| format!("Cannot create output: {}", _output.display()))?;
                println!(
                    "✓ Wrote {} replay pairs → {}",
                    rows.len(),
                    _output.display()
                );
            }
            #[cfg(not(feature = "database"))]
            {
                eprintln!(
                    "Replay requires the `database` feature. Rebuild with --features database"
                );
            }
            Ok(())
        }
        CorpusAction::Dogfood { output, limit } => {
            let db = vox_db::VoxDb::connect_default()
                .await
                .context("dogfood-export requires DB connection")?;
            let exporter = vox_corpus::corpus::dogfood::DogfoodExporter::new(&db);

            if let Some(p) = output.parent() {
                tokio::fs::create_dir_all(p).await?;
            }

            let mut file = std::fs::File::create(&output)?;
            let count = exporter.export_agent_traces(limit, &mut file).await?;
            println!(
                "✓ Exported {} high-quality agent traces → {}",
                count,
                output.display()
            );
            Ok(())
        }
        CorpusAction::DpoDogfood { output, limit } => {
            let db = vox_db::VoxDb::connect_default()
                .await
                .context("dpo-dogfood-export requires DB connection")?;

            if let Some(p) = output.parent() {
                tokio::fs::create_dir_all(p).await?;
            }

            let count = vox_corpus::corpus::dpo::export_dogfood_dpo(&db, limit, &output).await?;
            println!(
                "✓ Exported {} agent DPO preference pairs → {}",
                count,
                output.display()
            );
            Ok(())
        }
        CorpusAction::Stats { input } => stats::run_stats(&input).await,
        CorpusAction::ReviewExport {
            repository_id,
            limit,
            output,
        } => {
            let db = vox_db::VoxDb::connect_default()
                .await
                .context("review-export requires DB connection")?;
            let findings = db
                .list_external_review_findings_for_training_window(&repository_id, limit)
                .await
                .context("list external review findings for export")?;
            let mut rows = Vec::new();
            for f in findings {
                let prompt = format!(
                    "Review finding in {}:{} category={} severity={}: {}",
                    f.file_path.as_deref().unwrap_or("global"),
                    f.line_start.unwrap_or(0),
                    f.category,
                    f.severity,
                    f.title
                );
                let response = f.suggested_fix.clone().unwrap_or_else(|| f.details.clone());
                rows.push(
                    vox_corpus::external_review_replay::ExternalReviewReplayRow {
                        prompt,
                        response,
                        category: f.category,
                        severity: f.severity,
                        placement_kind: f.placement_kind,
                        source_id: f.finding_identity,
                        repository_id: f.repository_id,
                        pr_number: f.pr_number,
                        file_path: f.file_path,
                        line_start: f.line_start,
                        correctness_state: f.status,
                        sample_kind: "review_fix_pairs".to_string(),
                    },
                );
            }
            vox_corpus::external_review_replay::validate_external_review_rows(&rows)
                .context("validate extracted review rows")?;
            let parent = output.parent().unwrap_or(std::path::Path::new("."));
            tokio::fs::create_dir_all(parent).await?;
            let mut body = String::new();
            for row in &rows {
                body.push_str(&serde_json::to_string(row)?);
                body.push('\n');
            }
            tokio::fs::write(&output, body).await?;
            println!(
                "✓ Wrote {} review-derived rows -> {}",
                rows.len(),
                output.display()
            );
            Ok(())
        }
        CorpusAction::ReviewValidate { input } => {
            let raw: String = read_utf8_path_capped_async(&input).await?;
            let mut rows = Vec::new();
            for (idx, line) in raw.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let row: vox_corpus::external_review_replay::ExternalReviewReplayRow =
                    serde_json::from_str(line)
                        .with_context(|| format!("parse review row at line {}", idx + 1))?;
                rows.push(row);
            }
            vox_corpus::external_review_replay::validate_external_review_rows(&rows)?;
            println!("✓ Review dataset valid: {} rows", rows.len());
            Ok(())
        }
        CorpusAction::ReviewStats { input } => stats::run_review_stats(&input).await,
        CorpusAction::DogfoodConvert { input, output } => {
            let raw: String = read_utf8_path_capped_async(&input).await?;
            let mut out = String::new();
            let mut converted = 0;
            // Best effort parse as Vox ToolTraceRecord
            for line in raw.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(_msgs) = value.get("messages") {
                        // Already formatted
                        out.push_str(line);
                        out.push('\n');
                        converted += 1;
                        continue;
                    }
                    if let Some(tool) = value.get("tool").and_then(|t| t.as_str()) {
                        let ok = value
                            .get("success")
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        if !ok {
                            continue;
                        }
                        let args = value.get("args_json").unwrap_or(&serde_json::Value::Null);
                        let user = format!("Call tool `{}` with: {}", tool, args);
                        let asst = value
                            .get("result_preview")
                            .and_then(|t| t.as_str())
                            .unwrap_or("[ok]");
                        let chatml = serde_json::json!({
                            "messages": [
                                {"role": "user", "content": user},
                                {"role": "assistant", "content": asst}
                            ],
                            "category": "tool_trace",
                            "tool": tool,
                        });
                        out.push_str(&serde_json::to_string(&chatml)?);
                        out.push('\n');
                        converted += 1;
                    }
                }
            }
            if let Some(p) = output.parent() {
                tokio::fs::create_dir_all(p).await?;
            }
            tokio::fs::write(&output, out).await?;
            println!(
                "✓ Converted {} dogfood traces -> {}",
                converted,
                output.display()
            );
            Ok(())
        }
        CorpusAction::HealToDpo { input, output } => {
            generate::run_heal_to_dpo(input, &output).await
        }
        CorpusAction::ResearchGen { output, count } => {
            if let Some(p) = output.parent() {
                tokio::fs::create_dir_all(p).await?;
            }
            let mut file = std::fs::File::create(&output)?;
            let actual = vox_corpus::research_gen::generate_research_chains(&mut file, count)?;
            println!(
                "✓ Generated {} research multi-hop chains -> {}",
                actual,
                output.display()
            );
            Ok(())
        }
        CorpusAction::CurateProse {
            input,
            output,
            min_score,
            quarantine,
        } => generate::run_curate_prose(&input, &output, min_score, quarantine.as_deref()).await,
        CorpusAction::TraceIngest {
            input,
            output,
            min_diversity,
        } => {
            let count = vox_corpus::corpus::trace_ingest::generate_agent_traces_sft_file(
                &input,
                &output,
                min_diversity,
            )
            .with_context(|| format!("trace-ingest: {}", input.display()))?;
            println!(
                "✓ Ingested {} agent trace SFT rows → {}",
                count,
                output.display()
            );
            Ok(())
        }
    }
}
