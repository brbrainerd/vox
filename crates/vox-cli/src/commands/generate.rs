//! `vox generate` — generate validated Vox code using the MENS fine-tuned model.
//!
//! By default this routes through the orchestrator's VoxLocal path, which gives:
//!   - TTL-cached health probes (no redundant /health calls per invocation)
//!   - Consistent endpoint resolution from `VOX_LOCAL_ENDPOINT`
//!   - Aligned telemetry with MCP codegen calls
//!
//! `--legacy-direct` (deprecated): bypasses the orchestrator and calls the inference server
//! directly. Pre-Task 1.9 behavior. Prefer orchestrator mode; this flag is an escape hatch.

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
    legacy_direct: bool,
) -> Result<()> {
    let retries = max_retries.unwrap_or(3);
    let validate = !no_validate;

    let client = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("Failed to build HTTP client")?;

    if server_url.is_some() && !legacy_direct {
        anyhow::bail!(
            "--server-url only applies with --legacy-direct; use VOX_LOCAL_ENDPOINT for orchestrator mode"
        );
    }

    let (code, valid, errors, warnings, attempts) = if legacy_direct {
        run_legacy_direct(&client, prompt, server_url, validate, retries).await?
    } else {
        #[cfg(feature = "mcp-server")]
        {
            run_via_orchestrator(&client, prompt, validate, retries).await?
        }
        #[cfg(not(feature = "mcp-server"))]
        {
            eprintln!(
                "ℹ️  Orchestrator mode requires the mcp-server feature. Falling back to --legacy-direct."
            );
            run_legacy_direct(&client, prompt, server_url, validate, retries).await?
        }
    };

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

    if let Some(output_path) = output {
        std::fs::write(&output_path, &code)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        eprintln!("📄 Wrote {} bytes to {}", code.len(), output_path.display());
    }

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(code.as_bytes())?;
    handle.write_all(b"\n")?;

    if valid == Some(false) {
        std::process::exit(1);
    }

    Ok(())
}

type GenerateOutput = (String, Option<bool>, Vec<String>, Vec<String>, u64);

#[cfg(feature = "mcp-server")]
async fn run_via_orchestrator(
    client: &reqwest::Client,
    prompt: &str,
    validate: bool,
    max_retries: u32,
) -> Result<GenerateOutput> {
    eprintln!("🔮 Generating Vox code via orchestrator...");
    eprintln!("   Prompt: {}", prompt);

    let result =
        vox_orchestrator_mcp::llm_bridge::vox_local_generate(client, prompt, validate, max_retries)
            .await
            .map_err(|e| {
                eprintln!("⚠️  VoxLocal inference unavailable: {e}");
                eprintln!("   Start it with: vox run scripts/vox_populi::inference.vox --serve");
                anyhow::anyhow!(e)
            })?;

    Ok((
        result.code,
        result.valid,
        result.errors,
        result.warnings,
        result.attempts,
    ))
}

async fn run_legacy_direct(
    client: &reqwest::Client,
    prompt: &str,
    server_url: Option<&str>,
    validate: bool,
    max_retries: u32,
) -> Result<GenerateOutput> {
    let url = server_url.unwrap_or(DEFAULT_SERVER_URL);
    let endpoint = format!("{}/generate", url);

    match client.get(format!("{}/health", url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("📡 Connected to inference server at {}", url);
        }
        _ => {
            eprintln!("⚠️  Inference server not running at {}", url);
            eprintln!("   Start it with: vox run scripts/vox_populi::inference.vox --serve");
            eprintln!();
            eprintln!(
                "   Or generate directly: vox run scripts/vox_populi::inference.vox --prompt \"{}\"",
                prompt
            );
            anyhow::bail!("Inference server not available");
        }
    }

    eprintln!("🔮 Generating Vox code...");
    eprintln!("   Prompt: {}", prompt);

    let body = serde_json::json!({
        "prompt": prompt,
        "validate": validate,
        "max_retries": max_retries,
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

    Ok((code, valid, errors, warnings, attempts))
}
