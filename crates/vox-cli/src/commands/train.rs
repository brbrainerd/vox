//! Fine-tune orchestration over corpus-generated train.jsonl artifacts.
//! Local provider runs uv-driven vox-train; remote supports Together AI (env TOGETHER_API_KEY).
//!
//! GPU detection: Rust probes nvidia-smi and rocminfo before spawning the Python subprocess,
//! injecting VOX_GPU_VENDOR so scripts/detect_gpu.py skips redundant detection.

use std::path::{Path, PathBuf};
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
    tracing::debug!(
        data_dir = %data_dir.display(),
        output_dir = ?output_dir.as_ref().map(|p| p.display().to_string()),
        provider = ?provider,
        native,
        "Resolved training request"
    );

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
    ensure_train_jsonl(&train_jsonl, data_dir)?;

    let workspace_root = workspace_root()?;
    let qlora_script = workspace_root.join(SCRIPTS_DIR).join("train_qlora.vox");
    if !qlora_script.exists() {
        anyhow::bail!(
            "QLoRA script not found at {}. Run from repo root.",
            qlora_script.display()
        );
    }

    // Detect GPU vendor and pass as env var — train_qlora.vox reads VOX_GPU_VENDOR
    let gpu_vendor = detect_gpu_vendor();
    tracing::info!(vendor = gpu_vendor, "GPU vendor detected for QLoRA training");

    let out = output_dir
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target/qlora_adapter".to_string());
    tracing::debug!(
        workspace_root = %workspace_root.display(),
        qlora_script = %qlora_script.display(),
        data_dir = %data_dir.display(),
        output_dir = %out,
        "Prepared local QLoRA training paths"
    );

    // Load system prompt for instruction template (vox corpus prompt output)
    let system_prompt_path = std::env::var("VOX_SYSTEM_PROMPT_PATH")
        .unwrap_or_else(|_| "scripts/vox_system_prompt.txt".to_string());
    let system_prompt = std::fs::read_to_string(workspace_root.join(&system_prompt_path))
        .unwrap_or_else(|_| "You are a Vox programming language expert. Generate valid, complete Vox code.".to_string());

    // Invoke via current executable when available, fallback to PATH lookup.
    let mut cmd = if let Ok(current_vox) = std::env::current_exe() {
        Command::new(current_vox)
    } else {
        Command::new("vox")
    };
    cmd.arg("run")
        .arg(&qlora_script)
        .env("VOX_GPU_VENDOR", gpu_vendor)
        .env("VOX_DATA_DIR", data_dir)
        .env("VOX_OUTPUT_DIR", &out)
        .env("VOX_SYSTEM_PROMPT", system_prompt.trim())
        .current_dir(&workspace_root);

    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("train_qlora.vox exited with {}", status);
    }

    // ── 7.4: Eval-driven quality gate ────────────────────────────────────
    run_eval_gate(data_dir, output_dir).await?;

    // ── 7.5: Held-out benchmark gate (VOX_BENCHMARK=1) ───────────────────────
    crate::commands::corpus::run_benchmark_gate(data_dir, output_dir).await?;

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
        "timestamp": "unknown",
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
        let strict = std::env::var("VOX_EVAL_STRICT").map_or(false, |v| v == "1" || v.eq_ignore_ascii_case("true"));
        if strict {
            anyhow::bail!(
                "Eval gate FAILED (VOX_EVAL_STRICT=1). Parse rate: {:.1}%, Coverage: {:.1}%",
                parse_rate * 100.0,
                coverage_pct * 100.0
            );
        }
    } else {
        println!("{}", "✓ Eval gate PASSED — training data meets quality thresholds.".green().bold());
        // Remove stale failure marker if present
        let marker = eval_output.parent().unwrap_or(data_dir).join("eval_gate_failed.json");
        std::fs::remove_file(&marker).ok();
    }

    Ok(())
}

async fn run_eval_inline(train_jsonl: &Path) -> anyhow::Result<(f64, f64)> {
    let m = crate::commands::corpus::eval_metrics(train_jsonl)?;
    Ok((m.parse_rate, m.coverage_pct))
}

async fn run_together(data_dir: &Path) -> anyhow::Result<()> {
    let api_key = std::env::var("TOGETHER_API_KEY").map_err(|_| {
        anyhow::anyhow!("TOGETHER_API_KEY not set; required for --provider together")
    })?;
    let train_jsonl = data_dir.join("train.jsonl");
    ensure_train_jsonl(&train_jsonl, data_dir)?;
    let body = std::fs::read(&train_jsonl)?;
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| anyhow::anyhow!("reqwest client: {}", e))?;
    println!("Together AI dataset upload is temporarily disabled pending refactor.");
    /*
    let part = reqwest::multipart::Part::bytes(body).file_name("train.jsonl");
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("file_name", "train.jsonl")
        .text("purpose", "fine-tune");
    */
    let _ = body;
    let resp = client
        .post(TOGETHER_FILES_UPLOAD)
        .header("Authorization", format!("Bearer {}", api_key))
        // .multipart(form) // Temporarily disabled
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
    let train_jsonl = data_dir.join("train.jsonl");
    ensure_train_jsonl(&train_jsonl, data_dir)?;
    tracing::debug!(
        data_dir = %data_dir.display(),
        output_dir = ?output_dir.map(|p| p.display().to_string()),
        backend = ?std::env::var("VOX_BACKEND").ok(),
        "Starting native training"
    );

    #[cfg(feature = "gpu")]
    {
        crate::training::native::run_training(data_dir, output_dir).await?;
    }

    #[cfg(not(feature = "gpu"))]
    {
        anyhow::bail!(
            "Native training requires the gpu feature. Build with: cargo build -p vox-cli --features gpu"
        );
    }

    run_eval_gate(data_dir, output_dir).await?;
    crate::commands::corpus::run_benchmark_gate(data_dir, output_dir).await?;
    Ok(())
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("Cargo.toml").exists() && cwd.join("crates").exists() {
            return Ok(cwd);
        }
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest = PathBuf::from(manifest_dir);
        if let Some(workspace) = manifest.parent().and_then(|p| p.parent()) {
            if workspace.join("Cargo.toml").exists() && workspace.join("crates").exists() {
                return Ok(workspace.to_path_buf());
            }
        }
    }

    let exe = std::env::current_exe()?;
    let mut p = exe.parent().ok_or_else(|| anyhow::anyhow!("no exe dir"))?;
    // target/debug or target/release
    if p.ends_with("debug") || p.ends_with("release") {
        p = p.parent().ok_or_else(|| anyhow::anyhow!("no target dir"))?;
    }
    if p.ends_with("target") {
        p = p.parent().ok_or_else(|| anyhow::anyhow!("no workspace"))?;
    }
    if p.join("Cargo.toml").exists() && p.join("crates").exists() {
        Ok(p.to_path_buf())
    } else {
        anyhow::bail!(
            "Could not determine workspace root. Run from repository root or set current directory to the Vox workspace."
        )
    }
}

fn ensure_train_jsonl(train_jsonl: &Path, data_dir: &Path) -> anyhow::Result<()> {
    if train_jsonl.exists() {
        return Ok(());
    }
    anyhow::bail!(
        "No train.jsonl at {}. Generate corpus first: \
         vox corpus extract examples/ -o populi/data/validated.jsonl && \
         vox corpus validate populi/data/validated.jsonl --no-recheck -o populi/data/validated.jsonl && \
         vox corpus pairs populi/data/validated.jsonl -o {}/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/",
        train_jsonl.display(),
        data_dir.display()
    );
}

/// Detect the GPU vendor by probing system utilities.
///
/// Returns "nvidia", "amd", or "cpu".
/// This runs before the Python subprocess so `VOX_GPU_VENDOR` can be injected,
/// saving the Python scripts from running their own subprocess probes.
fn detect_gpu_vendor() -> &'static str {
    // Check NVIDIA first (most common in training environments)
    if probe_command("nvidia-smi", &["--query-gpu=name", "--format=csv,noheader"]) {
        return "nvidia";
    }
    // AMD ROCm
    if probe_command("rocminfo", &[]) || probe_command("rocm-smi", &["--showproductname"]) {
        return "amd";
    }
    "cpu"
}

/// Run a command and return true if it exits successfully with any output.
fn probe_command(binary: &str, args: &[&str]) -> bool {
    match Command::new(binary)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}