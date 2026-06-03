//! Native inference server for serving trained Vox Mens models.
//!
//! Provides a minimal HTTP API compatible with the OpenAI `/v1/completions`
//! schema, backed by the native Burn model loaded from a checkpoint.
//!
//! ## Architecture
//!
//! Because Burn's GPU backend types (`Wgpu`/`Fusion`) are not `Send`, the model
//! lives exclusively on a dedicated inference thread. Axum handlers communicate
//! with it via a `tokio::sync::mpsc` channel + per-request `oneshot` channel.
//!
//! ## Endpoints
//!
//! - `GET  /health`           — Liveness probe
//! - `GET  /ready`            — Readiness probe
//! - `GET  /v1/models`        — List loaded model
//! - `POST /v1/generate`      — Legacy single-prompt generation
//! - `POST /v1/completions`   — OpenAI-compatible completions endpoint

mod config;
#[cfg(feature = "execution-api")]
mod handlers;
mod prompt;
mod schema;
mod worker;

pub use config::ServeConfig;
#[cfg(feature = "execution-api")]
#[allow(unused_imports)]
pub use prompt::validate_structured_output;
#[cfg(feature = "execution-api")]
#[allow(unused_imports)]
pub use schema::{GenerateRequest, GenerateResponse};

use anyhow::Result;

/// Run the inference server using Axum (`execution-api` feature).
///
/// With default features this is unused (serve subcommands are `cfg` gated); keep it for
/// `cargo build --features execution-api` and for unit tests below.
#[cfg_attr(not(feature = "execution-api"), allow(dead_code))]
pub fn run_serve(config: &ServeConfig) -> Result<()> {
    use owo_colors::OwoColorize;

    eprintln!("{}", "╔══════════════════════════════════════════╗".cyan());
    eprintln!("{}", "║   Vox Mens Inference Server            ║".cyan());
    eprintln!("{}", "╚══════════════════════════════════════════╝".cyan());
    eprintln!();

    if !config.model_path.exists() {
        anyhow::bail!(
            "Model checkpoint not found at {}.\n\
             Run `vox mens train` first to produce a checkpoint.",
            config.model_path.display()
        );
    }

    let model_size = std::fs::metadata(&config.model_path)?.len();
    eprintln!(
        "  Model:       {} ({:.1} MB)",
        config.model_path.display(),
        model_size as f64 / 1_048_576.0
    );
    eprintln!("  Host:        {}", config.host);
    eprintln!("  Port:        {}", config.port);
    eprintln!("  Max tokens:  {}", config.max_tokens);
    eprintln!("  Temperature: {}", config.temperature);
    eprintln!();

    run_serve_inner(config)
}

#[cfg(feature = "execution-api")]
#[cfg_attr(not(feature = "execution-api"), allow(dead_code))]
fn run_serve_inner(config: &ServeConfig) -> Result<()> {
    use axum::Router;
    use axum::routing::{get, post};
    use std::sync::Arc;

    use owo_colors::OwoColorize;

    let run_dir = config
        .model_path
        .parent()
        .unwrap_or(std::path::Path::new("."));

    let arch = vox_populi::mens::tensor::manifest::ArchParams::from_manifest(run_dir)?;
    if let Err(e) = vox_populi::mens::tensor::manifest::validate_checkpoint_manifest(
        &config.model_path,
        run_dir,
        arch.to_validate_params(Some(
            vox_populi::mens::tensor::manifest::CheckpointKind::Lora,
        )),
    ) {
        anyhow::bail!("{e}");
    }

    let model_name = config
        .model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vox-ml-cli-model")
        .to_string();

    let system_prompt = config
        .system_prompt
        .clone()
        .unwrap_or_else(vox_corpus::training::generate_training_system_prompt);
    let tx = worker::spawn_inference_worker(config, &model_name, &system_prompt);

    let state = handlers::AppState {
        tx,
        model_name: Arc::from(model_name.as_str()),
    };

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/v1/models", get(handlers::list_models))
        .route("/v1/generate", post(handlers::do_generate))
        .route("/generate", post(handlers::do_generate))
        .route("/v1/completions", post(handlers::do_generate))
        .route(
            "/v1/completions/stream",
            post(handlers::do_completions_stream),
        )
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    eprintln!("  {} Listening on http://{}", "✓".green(), addr.cyan());
    eprintln!("  Press Ctrl-C to stop.");
    eprintln!();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind {}: {}", addr, e))?;
        eprintln!("  {} Server ready", "✓".green());
        axum::serve(listener, app)
            .await
            .map_err(|e| anyhow::anyhow!("Server error: {}", e))
    })
}

/// Stub when Axum stack is off — only reached from [`run_serve`], which is unused in default CLI builds.
#[cfg(not(feature = "execution-api"))]
#[allow(dead_code)]
fn run_serve_inner(config: &ServeConfig) -> Result<()> {
    use owo_colors::OwoColorize;
    eprintln!(
        "  {} Axum HTTP server requires the `execution-api` feature.",
        "⚠".yellow()
    );
    eprintln!("  Rebuild with: cargo build --features execution-api");
    eprintln!();
    eprintln!(
        "  Server would bind to http://{}:{}",
        config.host, config.port
    );
    eprintln!("  Endpoints: GET /health, GET /ready, GET /v1/models, POST /v1/completions");
    Ok(())
}

#[cfg(all(test, feature = "execution-api"))]
mod prompt_tests {
    use super::*;

    #[test]
    fn prompt_for_output_mode_wraps_strict_json() {
        let out = prompt::prompt_for_output_mode("hello", Some("strict_json"));
        assert!(out.contains("single valid JSON object"));
        assert!(out.contains("hello"));
        assert!(out.contains("No markdown fences"));
    }

    #[test]
    fn prompt_for_output_mode_passthrough_when_none() {
        let out = prompt::prompt_for_output_mode("hello", None);
        assert_eq!(out, "hello");
    }

    #[test]
    fn prompt_for_output_mode_passthrough_when_empty() {
        let out = prompt::prompt_for_output_mode("hello", Some(""));
        assert_eq!(out, "hello");
    }

    #[test]
    fn validate_structured_output_schema_accepts_valid() {
        // Matches `vox_corpus::corpus::structured_eval::validate_against_schema` flat key → type string form.
        let schema = serde_json::json!({"name": "string"});
        assert!(validate_structured_output(
            r#"{"name":"ok"}"#,
            Some("strict_json"),
            Some(&schema)
        ));
    }

    #[test]
    fn validate_structured_output_schema_rejects_invalid() {
        let schema = serde_json::json!({"name": "string"});
        assert!(!validate_structured_output(
            r#"{"other":1}"#,
            Some("strict_json"),
            Some(&schema)
        ));
    }

    #[test]
    fn is_valid_json_prefix_accepts_prefixes() {
        assert!(prompt::is_valid_json_prefix(""));
        assert!(prompt::is_valid_json_prefix("{"));
        assert!(prompt::is_valid_json_prefix(r#"{"a"#));
        assert!(prompt::is_valid_json_prefix(r#"{"a":1}"#));
    }

    #[test]
    fn validate_structured_output_jsonl_with_schema() {
        let schema = serde_json::json!({"x": "number"});
        assert!(validate_structured_output(
            r#"{"x":1}
{"x":2}"#,
            Some("jsonl_records"),
            Some(&schema)
        ));
        assert!(!validate_structured_output(
            r#"{"x":"not a number"}"#,
            Some("jsonl_records"),
            Some(&schema)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn serve_config_defaults() {
        let cfg = ServeConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.max_tokens, 256);
        assert!((cfg.temperature - 0.7).abs() < 1e-6);
        assert_eq!(cfg.host, "127.0.0.1");
    }

    #[test]
    fn serve_fails_on_missing_model() {
        let cfg = ServeConfig {
            model_path: PathBuf::from("/nonexistent/path/model.bin"),
            ..Default::default()
        };
        let result = run_serve(&cfg);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found"),
            "error should mention 'not found': {msg}"
        );
    }
}
