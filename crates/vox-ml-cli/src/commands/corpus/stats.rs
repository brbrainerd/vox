use anyhow::Context;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped_async;

pub(super) async fn run_fingerprint() -> Result<()> {
    let workspace_root = vox_corpus::training::contract::find_workspace_root();
    if let Some(ref root) = workspace_root {
        let current_fp = vox_corpus::corpus::preflight::compute_corpus_fingerprint(root);
        println!("{}", current_fp);
    } else {
        eprintln!("Could not determine workspace root");
    }
    Ok(())
}

#[cfg(all(feature = "mens-dei", feature = "gpu"))]
use vox_bounded_fs::read_utf8_path_capped;

/// Aggregated quality metrics for a training JSONL file (fractions 0.0–1.0).
#[cfg(all(feature = "mens-dei", feature = "gpu"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct TrainEvalMetrics {
    /// Fraction of rows whose `response` parses without frontend errors.
    pub parse_rate: f64,
    /// Fraction of the construct taxonomy covered by parsed samples.
    pub coverage_pct: f64,
}

struct EvalScan {
    total: usize,
    format_valid: u32,
    safety_rejected: u32,
    parse_passed: u32,
    construct_hits: HashSet<String>,
}

fn scan_train_jsonl_content(content: &str) -> Result<EvalScan> {
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

    let mut format_valid = 0u32;
    let mut safety_rejected = 0u32;
    let mut parse_passed = 0u32;
    let mut construct_hits: HashSet<String> = HashSet::new();

    let safety_patterns = [
        "ignore previous instructions",
        "ignore all above",
        "disregard your instructions",
        "you are now",
        "new instructions:",
    ];

    for line in &lines {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let response = record
            .get("response")
            .or_else(|| record.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !response.trim().is_empty() {
            format_valid += 1;
        }

        let lower = response.to_lowercase();
        if safety_patterns.iter().any(|p| lower.contains(p)) {
            safety_rejected += 1;
        }

        let dummy_path = Path::new("__eval__.vox");
        if let Ok(result) = crate::pipeline::run_frontend_str(response, dummy_path, false)
            && !result.has_errors()
        {
            parse_passed += 1;
            for c in crate::training::extract_constructs(&result.module) {
                construct_hits.insert(c);
            }
        }
    }

    Ok(EvalScan {
        total: lines.len(),
        format_valid,
        safety_rejected,
        parse_passed,
        construct_hits,
    })
}

/// Compute parse rate and taxonomy coverage for `train.jsonl` (used by post-training gates).
#[cfg(all(feature = "mens-dei", feature = "gpu"))]
pub(crate) fn eval_metrics(train_jsonl: &Path) -> Result<TrainEvalMetrics> {
    let content = read_utf8_path_capped(train_jsonl)?;
    let s = scan_train_jsonl_content(&content)?;
    let taxonomy: HashSet<&str> = crate::training::TAXONOMY.iter().copied().collect();
    let coverage = s
        .construct_hits
        .iter()
        .filter(|x| taxonomy.contains(x.as_str()))
        .count();
    let parse_rate = if s.total > 0 {
        s.parse_passed as f64 / s.total as f64
    } else {
        0.0
    };
    let coverage_pct = if taxonomy.is_empty() {
        0.0
    } else {
        coverage as f64 / taxonomy.len() as f64
    };
    Ok(TrainEvalMetrics {
        parse_rate,
        coverage_pct,
    })
}

/// When `VOX_BENCHMARK=1` (or `true`), runs `vox mens eval-local` against a held-out bench.
#[cfg(all(feature = "mens-dei", feature = "gpu"))]
pub(crate) async fn run_benchmark_gate(data_dir: &Path, output_dir: Option<&Path>) -> Result<()> {
    let enabled = std::env::var("VOX_BENCHMARK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return Ok(());
    }

    {
        use std::path::PathBuf;
        let exe = std::env::current_exe().context("resolve current exe for benchmark gate")?;
        let base = output_dir.unwrap_or(data_dir).to_path_buf();

        let model: Option<PathBuf> = std::env::var("VOX_BENCHMARK_MODEL")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                // Candle QLoRA adapter (.safetensors) checked first — primary backend.
                // Burn LoRA fallbacks (.bin) kept for legacy runs.
                [
                    "candle_qlora_adapter.safetensors",
                    "model.bin",
                    "adapter.bin",
                    "populi_adapter.bin",
                ]
                .iter()
                .map(|name| base.join(name))
                .find(|p| p.is_file())
            });

        let Some(model) = model else {
            tracing::warn!(
                "VOX_BENCHMARK=1 but no checkpoint found under {} (set VOX_BENCHMARK_MODEL)",
                base.display()
            );
            return Ok(());
        };

        let bench_raw = vox_config::env_parse::resolve_config_str("VOX_BENCHMARK_DIR", "");
        let bench: PathBuf = if bench_raw.trim().is_empty() {
            vox_corpus::training::contract::find_workspace_root()
                .map(|r| r.join("mens/data/heldout_bench"))
                .unwrap_or_else(|| PathBuf::from("mens/data/heldout_bench"))
        } else {
            PathBuf::from(bench_raw)
        };

        if !bench.is_dir() {
            tracing::warn!(
                bench = %bench.display(),
                "VOX_BENCHMARK=1 but benchmark directory missing; skipping gate"
            );
            return Ok(());
        }

        let out_json = base.join("benchmark_gate_eval.json");
        let status = tokio::process::Command::new(&exe)
            .arg("mens")
            .arg("eval-local")
            .arg("--model")
            .arg(&model)
            .arg("--bench")
            .arg(&bench)
            .arg("-o")
            .arg(&out_json)
            .status()
            .await
            .context("spawn vox mens eval-local")?;

        if !status.success() {
            anyhow::bail!(
                "benchmark gate failed: vox mens eval-local exited with {:?}",
                status.code()
            );
        }
        vox_cli_core::benchmark_telemetry::record_opt(
            "train_benchmark_gate",
            None,
            Some(serde_json::json!({
                "model": model.to_string_lossy(),
                "bench": bench.to_string_lossy(),
                "out_json": out_json.to_string_lossy(),
            })),
        )
        .await;
        Ok(())
    }
}

pub(super) async fn run_eval(input: &Path, output: &Path, print_summary: bool) -> Result<()> {
    if tokio::fs::metadata(input).await.is_err() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let content = read_utf8_path_capped_async(input).await?;
    let s = scan_train_jsonl_content(&content)?;
    let total = s.total;

    let taxonomy: HashSet<&str> = crate::training::TAXONOMY.iter().copied().collect();
    let coverage = s
        .construct_hits
        .iter()
        .filter(|x| taxonomy.contains(x.as_str()))
        .count();

    let results = serde_json::json!({
        "run_id": format!("eval_{}", crate::training::timestamp_string()),
        "total_samples": total,
        "format_validity": if total > 0 { s.format_valid as f64 / total as f64 } else { 0.0 },
        "safety_rejection_rate": if total > 0 { s.safety_rejected as f64 / total as f64 } else { 0.0 },
        "vox_parse_rate": if total > 0 { s.parse_passed as f64 / total as f64 } else { 0.0 },
        "construct_coverage": coverage,
        "construct_total": taxonomy.len(),
        "construct_coverage_pct": if taxonomy.is_empty() { 0.0 } else { 100.0 * coverage as f64 / taxonomy.len() as f64 },
        "quality_proxy": if total > 0 { s.parse_passed as f64 / total as f64 } else { 0.0 },
    });

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(output, serde_json::to_string_pretty(&results)?).await?;

    if print_summary {
        let parse_pct = 100.0 * s.parse_passed as f64 / total.max(1) as f64;
        let cov_pct = if taxonomy.is_empty() {
            0.0
        } else {
            100.0 * coverage as f64 / taxonomy.len() as f64
        };
        println!(
            "Samples: {} | Parse rate: {:.1}% | Coverage: {:.1}%",
            total, parse_pct, cov_pct
        );
        return Ok(());
    }

    println!("\n{}", "Vox Training Data Eval Report".bold().cyan());
    println!("  Total samples:      {}", total);
    println!(
        "  Format validity:    {:.1}%",
        100.0 * s.format_valid as f64 / total.max(1) as f64
    );
    println!(
        "  Safety rejection:   {:.1}%",
        100.0 * s.safety_rejected as f64 / total.max(1) as f64
    );
    println!(
        "  Vox parse rate:     {:.1}%",
        100.0 * s.parse_passed as f64 / total.max(1) as f64
    );
    println!(
        "  Construct coverage: {}/{} ({:.1}%)",
        coverage,
        taxonomy.len(),
        100.0 * coverage as f64 / taxonomy.len().max(1) as f64
    );
    println!("\n✓ Results written to {}", output.display());
    Ok(())
}

pub(super) async fn run_stats(input: &Path) -> Result<()> {
    if tokio::fs::metadata(input).await.is_err() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let file = tokio::fs::File::open(input).await?;
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(file).lines();

    let mut total = 0usize;

    let mut source_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut category_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        total += 1;
        if let Ok(record) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(src) = record.get("source").and_then(|v| v.as_str()) {
                *source_counts.entry(src.to_string()).or_insert(0) += 1;
            }
            if let Some(cat) = record.get("category").and_then(|v| v.as_str()) {
                *category_counts.entry(cat.to_string()).or_insert(0) += 1;
            }
        }
    }

    use owo_colors::OwoColorize;
    println!("\n{}", "Vox Training Corpus Stats".bold().cyan());
    println!("  Total records: {}\n", total);

    if !source_counts.is_empty() {
        println!("{}", "  By Source:".bold().white());
        let mut sources: Vec<_> = source_counts.iter().collect();
        sources.sort_by_key(|s| std::cmp::Reverse(*s.1));
        for (src, count) in sources {
            let pct = 100.0 * (*count as f64) / (total as f64);
            println!("    - {:<24} {:>8} ({:.1}%)", src, count, pct);
        }
        println!();
    }

    if !category_counts.is_empty() {
        println!("{}", "  By Category:".bold().white());
        let mut cats: Vec<_> = category_counts.iter().collect();
        cats.sort_by_key(|c| std::cmp::Reverse(*c.1));
        for (cat, count) in cats {
            let pct = 100.0 * (*count as f64) / (total as f64);
            println!("    - {:<24} {:>8} ({:.1}%)", cat, count, pct);
        }
    }

    Ok(())
}

pub(super) async fn run_review_stats(input: &Path) -> Result<()> {
    if tokio::fs::metadata(input).await.is_err() {
        anyhow::bail!("Input file not found: {}", input.display());
    }

    let file = tokio::fs::File::open(input).await?;
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(file).lines();

    let mut total = 0usize;
    let mut sample_kind: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut category: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut correctness: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let row: vox_corpus::external_review_replay::ExternalReviewReplayRow =
            serde_json::from_str(line).context("parse review stats row")?;
        total += 1;
        *sample_kind.entry(row.sample_kind).or_insert(0) += 1;
        *category.entry(row.category).or_insert(0) += 1;
        *correctness.entry(row.correctness_state).or_insert(0) += 1;
    }

    println!("Review dataset stats for {}", input.display());
    println!("  Total rows: {}", total);
    println!("  By sample kind:");
    for (k, v) in sample_kind {
        println!("    - {k}: {v}");
    }
    println!("  By category:");
    for (k, v) in category {
        println!("    - {k}: {v}");
    }
    println!("  By correctness state:");
    for (k, v) in correctness {
        println!("    - {k}: {v}");
    }
    Ok(())
}
