//! `vox generate` — generate validated Vox code using the QWEN fine-tuned model.
//!
//! Calls the inference server at localhost:7863 (started by `python scripts/vox_inference.py --serve`)
//! or starts it automatically if not running.
//!
//! Usage:
//!     vox generate "Create a counter actor with increment and decrement"
//!     vox generate "Write a todo app" --output todo.vox
//!     vox generate "Write unit tests for the factorial function" --no-validate

use anyhow::{Context, Result};
use std::io::Write;
use std::path::PathBuf;

const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:7863";

/// Run the generate command.
pub async fn run(
    prompt: &str,
    output: Option<PathBuf>,
    no_validate: bool,
    server_url: Option<&str>,
    max_retries: Option<u32>,
) -> Result<()> {
    let url = server_url.unwrap_or(DEFAULT_SERVER_URL);
    let endpoint = format!("{}/generate", url);

    // Check if server is running
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("Failed to build HTTP client")?;

    // Health check
    match client.get(format!("{}/health", url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("📡 Connected to inference server at {}", url);
        }
        _ => {
            eprintln!("⚠️  Inference server not running at {}", url);
            eprintln!("   Start it with: python scripts/vox_inference.py --serve");
            eprintln!();
            eprintln!(
                "   Or generate directly: python scripts/vox_inference.py --prompt \"{}\"",
                prompt
            );
            anyhow::bail!("Inference server not available");
        }
    }

    eprintln!("🔮 Generating Vox code...");
    eprintln!("   Prompt: {}", prompt);

    let body = serde_json::json!({
        "prompt": prompt,
        "validate": !no_validate,
        "max_retries": max_retries.unwrap_or(3),
    });

    let resp = client
        .post(&endpoint)
        .json(&body)
        .send()
        .await
        .context("Failed to connect to inference server")?;

    let status = resp.status();
    let text = resp.text().await.context("Failed to read response")?;

    if !status.is_success() {
        anyhow::bail!("Server error ({}): {}", status, text);
    }

    let result: serde_json::Value =
        serde_json::from_str(&text).context("Invalid JSON from server")?;

    let code = result["code"].as_str().unwrap_or("").to_string();
    let valid = result["valid"].as_bool();
    let attempts = result["attempts"].as_u64().unwrap_or(1);
    let errors: Vec<String> = result["errors"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let warnings: Vec<String> = result["warnings"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Print status line
    eprintln!();
    match valid {
        Some(true) => {
            eprintln!("✅ Valid Vox code generated (attempts: {})", attempts);
        }
        Some(false) => {
            eprintln!(
                "⚠️  Generated code may have issues (attempts: {})",
                attempts
            );
            for e in &errors {
                eprintln!("   ❌ {}", e);
            }
        }
        None => {
            eprintln!("ℹ️  Validation skipped");
        }
    }
    for w in &warnings {
        eprintln!("   ⚠ {}", w);
    }
    eprintln!();

    // Output the code
    if let Some(output_path) = output {
        std::fs::write(&output_path, &code)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        eprintln!("📄 Wrote {} bytes to {}", code.len(), output_path.display());
    }

    // Always print the code to stdout
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(code.as_bytes())?;
    handle.write_all(b"\n")?;

    // Exit non-zero if validation failed
    if valid == Some(false) && !errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
