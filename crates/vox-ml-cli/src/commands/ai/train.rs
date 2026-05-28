//! Fine-tune orchestration over corpus-generated train.jsonl artifacts.
//!
//! **Canonical native training** is **`vox mens train`** (Burn LoRA or Candle QLoRA). This module keeps
//! **`vox train`** for Together remote upload, **`--native`** (legacy Burn scratch trainer behind `mens-dei`),
//! and a **local** path that **spawns** `vox mens train` with strict data mode (no implicit corpus regen).
//!
//! Remote: Together AI (`TOGETHER_API_KEY`). GPU vendor probing remains for any subprocess paths.
use std::path::Path;

const DEFAULT_DATA_DIR: &str = vox_corpus::training::CANONICAL_TRAIN_DATA_DIR;
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
    eprintln!(
        "Note: `vox train` is legacy. **Canonical training:** `vox mens train` (see docs/src/reference/mens-training.md). `--provider local` spawns `vox mens train` with a 4080-class QLoRA default."
    );
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
        "remote" | "together" => run_together(&data_dir, output_dir.as_deref()).await,
        "replicate" => {
            anyhow::bail!(
                "Replicate provider is not implemented. Use --provider together (set TOGETHER_API_KEY) or --provider local."
            );
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

    let out_hint = output_dir
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| vox_scaling_policy::DEFAULT_MENS_RUNS_QWEN_QLORA.to_string());

    eprintln!(
        "Note: `vox train --provider local` forwards to **`vox mens train`** (`--data-mode strict`: stale corpus triggers a blocking refresh before training)."
    );

    let exe = std::env::current_exe().map_err(|e| anyhow::anyhow!("current_exe: {e}"))?;
    let status = std::process::Command::new(&exe)
        .arg("mens")
        .arg("train")
        .arg("--backend")
        .arg("qlora")
        .arg("--tokenizer")
        .arg("hf")
        .arg("--preset")
        .arg("qwen_4080_16g")
        .arg("--model")
        .arg(vox_populi::mens::DEFAULT_MODEL_ID)
        .arg("--data-dir")
        .arg(data_dir)
        .arg("--output-dir")
        .arg(&out_hint)
        .arg("--device")
        .arg("cuda")
        .arg("--data-mode")
        .arg("strict")
        .status()
        .map_err(|e| anyhow::anyhow!("spawn `vox mens train`: {e}"))?;

    if !status.success() {
        anyhow::bail!(
            "`vox mens train` exited with {:?}. Rebuild with `--features gpu,mens-candle-cuda` for NVIDIA CUDA. Docs: docs/src/reference/mens-training.md.",
            status.code()
        );
    }
    Ok(())
}

async fn run_together(data_dir: &Path, output_dir: Option<&Path>) -> anyhow::Result<()> {
    if let Some(p) = output_dir {
        eprintln!(
            "Note: output directory ({}) is ignored for Together fine-tuning; checkpoints stay on Together. Use the job URL printed below.",
            p.display()
        );
    }
    let api_key = vox_secrets::resolve_secret(vox_secrets::SecretId::TogetherApiKey)
        .expose()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!("TOGETHER_API_KEY not set; required for --provider together")
        })?;
    let train_jsonl = data_dir.join("train.jsonl");
    ensure_train_jsonl(&train_jsonl, data_dir)?;
    let body = std::fs::read(&train_jsonl)?;
    let client = vox_http_client::client_builder()
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
    let model = vox_secrets::resolve_secret(vox_secrets::SecretId::TogetherFinetuneModel)
        .expose()
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| DEFAULT_TOGETHER_MODEL.to_string());
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

    // Native in-process training is currently delegated to the
    // `vox-plugin-mens-candle-cuda` plugin (see docs/src/architecture/external-frontend-interop-plan-2026.md).
    // The legacy `crate::training::native::run_training` stub never reached parity with the plugin path
    // and was removed; suppress the unused-arg warning under `gpu` while keeping the stable bail-out.
    let _ = data_dir;
    let _ = output_dir;
    anyhow::bail!(
        "Native training is provided by the vox-plugin-mens-candle-cuda plugin. \
         Install it via `cargo build -p vox-plugin-mens-candle-cuda` and run training through the plugin runtime."
    );
}

fn ensure_train_jsonl(train_jsonl: &Path, data_dir: &Path) -> anyhow::Result<()> {
    if train_jsonl.exists() {
        return Ok(());
    }
    anyhow::bail!(
        "No train.jsonl at {}. Generate corpus first: \
         vox mens corpus extract examples/ -o mens/data/validated.jsonl && \
         vox mens corpus validate mens/data/validated.jsonl --no-recheck -o mens/data/validated.jsonl && \
         vox mens corpus pairs mens/data/validated.jsonl -o {}/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/",
        train_jsonl.display(),
        data_dir.display()
    );
}
