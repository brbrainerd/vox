//! Fine-tune orchestration: same dataset artifact as `vox learn --export-dataset`.
//! Local provider runs uv-driven vox-train; remote supports Together AI (env TOGETHER_API_KEY).

use std::path::Path;
use std::process::Command;

const DEFAULT_DATA_DIR: &str = "target/dogfood";
const SCRIPTS_DIR: &str = "scripts";
const TOGETHER_FILES_UPLOAD: &str = "https://api.together.xyz/v1/files/upload";
const TOGETHER_FINE_TUNES: &str = "https://api.together.xyz/v1/fine-tunes";
const DEFAULT_TOGETHER_MODEL: &str = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo";

/// Run fine-tune orchestration (local or remote via Together AI).
pub async fn run(
    data_dir: Option<std::path::PathBuf>,
    output_dir: Option<std::path::PathBuf>,
    provider: Option<String>,
    native: bool,
) -> anyhow::Result<()> {
    let data_dir = data_dir.unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_DATA_DIR));

    if native {
        return run_native(&data_dir, output_dir.as_deref()).await;
    }

    let provider = provider.as_deref().unwrap_or("local");

    match provider {
        "local" => run_local(&data_dir, output_dir.as_deref()).await,
        "remote" | "together" => run_together(&data_dir).await,
        "replicate" => {
            eprintln!(
                "Replicate provider is not implemented. Use --provider together (set TOGETHER_API_KEY) or --provider local."
            );
            Ok(())
        }
        _ => {
            anyhow::bail!(
                "Unknown provider '{}'; use 'local', 'remote', or 'together'",
                provider
            );
        }
    }
}

async fn run_local(data_dir: &Path, output_dir: Option<&Path>) -> anyhow::Result<()> {
    let train_jsonl = data_dir.join("train.jsonl");
    if !train_jsonl.exists() {
        anyhow::bail!(
            "No train.jsonl at {}. Run: vox learn --export-dataset [{}]",
            train_jsonl.display(),
            data_dir.display()
        );
    }

    let workspace_root = workspace_root()?;
    let scripts = workspace_root.join(SCRIPTS_DIR);
    if !scripts.join("pyproject.toml").exists() {
        anyhow::bail!(
            "Scripts project not found at {}. Run from repo root.",
            scripts.display()
        );
    }

    let mut cmd = Command::new("uv");
    cmd.arg("run")
        .arg("--project")
        .arg(&scripts)
        .arg("vox-train")
        .arg("--data-dir")
        .arg(data_dir);
    if let Some(out) = output_dir {
        cmd.arg("--output-dir").arg(out);
    }
    cmd.current_dir(&workspace_root);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("vox-train exited with {}", status);
    }

    // ── 7.4: Eval-driven quality gate ────────────────────────────────────
    run_eval_gate(data_dir, output_dir).await?;

    Ok(())
}

/// Thresholds for the post-training eval gate.
const DEFAULT_MIN_PARSE_RATE: f64 = 0.80;
const DEFAULT_MIN_COVERAGE: f64 = 0.60;

/// After training, evaluate the training data quality and flag if below thresholds.
async fn run_eval_gate(data_dir: &Path, output_dir: Option<&Path>) -> anyhow::Result<()> {
    use owo_colors::OwoColorize;

    let train_jsonl = data_dir.join("train.jsonl");
    if !train_jsonl.exists() {
        return Ok(());
    }

    let min_parse_rate = std::env::var("VOX_EVAL_MIN_PARSE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MIN_PARSE_RATE);
    let min_coverage = std::env::var("VOX_EVAL_MIN_COVERAGE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MIN_COVERAGE);

    let eval_output = output_dir
        .unwrap_or(data_dir)
        .to_path_buf()
        .join("eval_results.json");

    println!("{}", "\n── Post-Training Eval Gate ──".bold());

    // Run eval inline using the corpus module
    let eval_result = run_eval_inline(&train_jsonl).await;

    let (parse_rate, coverage_pct) = match eval_result {
        Ok((p, c)) => (p, c),
        Err(e) => {
            eprintln!("{} Eval gate error: {}", "⚠".yellow(), e);
            return Ok(()); // Non-fatal — don't block training on eval errors
        }
    };

    let parse_ok = parse_rate >= min_parse_rate;
    let coverage_ok = coverage_pct >= min_coverage;

    println!("  Vox parse rate:      {:.1}%  (threshold: {:.0}%) {}",
        parse_rate * 100.0, min_parse_rate * 100.0,
        if parse_ok { "✓".green().to_string() } else { "✗".red().to_string() });
    println!("  Construct coverage:  {:.1}%  (threshold: {:.0}%) {}",
        coverage_pct * 100.0, min_coverage * 100.0,
        if coverage_ok { "✓".green().to_string() } else { "✗".red().to_string() });

    let gate_result = serde_json::json!({
        "vox_parse_rate": parse_rate,
        "construct_coverage_pct": coverage_pct,
        "min_parse_rate": min_parse_rate,
        "min_coverage": min_coverage,
        "gate_passed": parse_ok && coverage_ok,
        "timestamp": crate::training::timestamp_string(),
    });

    std::fs::write(
        &eval_output,
        serde_json::to_string_pretty(&gate_result)?,
    ).ok(); // Write best-effort

    if !parse_ok || !coverage_ok {
        eprintln!("{}", "\n⚠ Eval gate FAILED — training data quality below thresholds.".red().bold());
        eprintln!("  Review eval_results.json and regenerate corpus before promoting this model.");
        // Write a marker file CI can detect
        let marker = eval_output.parent().unwrap_or(data_dir).join("eval_gate_failed.json");
        std::fs::write(&marker, serde_json::to_string_pretty(&gate_result)?).ok();
    } else {
        println!("{}", "✓ Eval gate PASSED — training data meets quality thresholds.".green().bold());
        // Remove stale failure marker if present
        let marker = eval_output.parent().unwrap_or(data_dir).join("eval_gate_failed.json");
        std::fs::remove_file(&marker).ok();
    }

    Ok(())
}

/// Run eval metrics inline (without subprocess). Returns (parse_rate, coverage_pct).
async fn run_eval_inline(train_jsonl: &Path) -> anyhow::Result<(f64, f64)> {
    let content = std::fs::read_to_string(train_jsonl)?;
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    let total = lines.len();
    if total == 0 {
        return Ok((0.0, 0.0));
    }

    let mut parse_passed = 0u32;
    let mut construct_hits: std::collections::HashSet<String> = std::collections::HashSet::new();
    let dummy_path = std::path::Path::new("__eval_gate__.vox");

    for line in &lines {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let response = record.get("response")
            .or_else(|| record.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if let Ok(result) = crate::pipeline::run_frontend_str(response, dummy_path, false) {
            if !result.has_errors() {
                parse_passed += 1;
                for c in crate::training::extract_constructs(&result.module) {
                    construct_hits.insert(c);
                }
            }
        }
    }

    let taxonomy_len = crate::training::TAXONOMY.len();
    let coverage = construct_hits.iter()
        .filter(|s| crate::training::TAXONOMY.contains(&s.as_str()))
        .count();

    let parse_rate = parse_passed as f64 / total as f64;
    let coverage_pct = if taxonomy_len == 0 { 0.0 } else { coverage as f64 / taxonomy_len as f64 };

    Ok((parse_rate, coverage_pct))
}

async fn run_together(data_dir: &Path) -> anyhow::Result<()> {
    let api_key = std::env::var("TOGETHER_API_KEY").map_err(|_| {
        anyhow::anyhow!("TOGETHER_API_KEY not set; required for --provider together")
    })?;
    let train_jsonl = data_dir.join("train.jsonl");
    if !train_jsonl.exists() {
        anyhow::bail!(
            "No train.jsonl at {}. Run: vox learn --export-dataset [{}]",
            train_jsonl.display(),
            data_dir.display()
        );
    }
    let body = std::fs::read(&train_jsonl)?;
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest client: {}", e))?;
    let part = reqwest::multipart::Part::bytes(body).file_name("train.jsonl");
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("file_name", "train.jsonl")
        .text("purpose", "fine-tune");
    let resp = client
        .post(TOGETHER_FILES_UPLOAD)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Together file upload failed ({}): {}", status, text);
    }
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Together upload response JSON: {}", e))?;
    let file_id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow::anyhow!("Together response missing id: {}", text))?;
    let model = std::env::var("TOGETHER_FINETUNE_MODEL")
        .unwrap_or_else(|_| DEFAULT_TOGETHER_MODEL.to_string());
    let body = serde_json::json!({
        "training_file": file_id,
        "model": model,
    });
    let resp = client
        .post(TOGETHER_FINE_TUNES)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Together fine-tune create failed ({}): {}", status, text);
    }
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Together fine-tune response JSON: {}", e))?;
    let job_id = v.get("id").and_then(|x| x.as_str()).unwrap_or("unknown");
    println!(
        "Together fine-tune job created: id={}. Monitor at https://api.together.xyz/v1/fine-tunes/{}",
        job_id, job_id
    );
    Ok(())
}

async fn run_native(data_dir: &Path, output_dir: Option<&Path>) -> anyhow::Result<()> {
    crate::training::native::run_training(data_dir, output_dir).await
}

fn workspace_root() -> anyhow::Result<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let mut p = exe.parent().ok_or_else(|| anyhow::anyhow!("no exe dir"))?;
    // target/debug or target/release
    if p.ends_with("debug") || p.ends_with("release") {
        p = p.parent().ok_or_else(|| anyhow::anyhow!("no target dir"))?;
    }
    if p.ends_with("target") {
        p = p.parent().ok_or_else(|| anyhow::anyhow!("no workspace"))?;
    }
    Ok(p.to_path_buf())
}
