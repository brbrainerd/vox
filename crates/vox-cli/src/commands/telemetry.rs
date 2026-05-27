//! Optional telemetry upload queue (`vox telemetry …`). See ADR 023 and
//! `docs/src/architecture/telemetry-remote-sink-spec.md`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use clap::Subcommand;

use crate::telemetry_spool;
fn resolve_spool(spool: Option<PathBuf>) -> PathBuf {
    spool.unwrap_or_else(telemetry_spool::spool_root)
}

/// Subcommands for `vox telemetry`.
#[derive(Subcommand)]
pub enum TelemetryCmd {
    /// Print the effective telemetry configuration and recorder health.
    ///
    /// Shows: master switch state, per-category flags, recorder registration,
    /// and spool path. Exit 0 if telemetry is active; exit 1 if master switch is off.
    Doctor,
    /// Show spool path, pending count, and whether upload URL/token resolve (redacted).
    Status {
        /// Override spool root (default: cwd `.vox/telemetry-upload-queue` or `VOX_TELEMETRY_SPOOL_DIR`).
        #[arg(long)]
        spool: Option<PathBuf>,
    },
    /// Print pending payloads as JSON Lines to stdout (does not dequeue).
    Export {
        #[arg(long)]
        spool: Option<PathBuf>,
        /// Write JSONL to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Append one JSON object file to the pending queue.
    Enqueue {
        #[arg(long)]
        spool: Option<PathBuf>,
        /// Path to a JSON file (single object).
        #[arg(long)]
        json: PathBuf,
    },
    /// POST each pending JSON to the configured ingest URL; delete file on HTTP 2xx.
    Upload {
        #[arg(long)]
        spool: Option<PathBuf>,
        /// Log actions only; no network or deletes.
        #[arg(long)]
        dry_run: bool,
        /// Override ingest URL (else `VOX_TELEMETRY_UPLOAD_URL` via Clavis).
        #[arg(long)]
        url: Option<String>,
    },
}

pub async fn run(cmd: TelemetryCmd) -> Result<()> {
    match cmd {
        TelemetryCmd::Doctor => {
            let cfg = vox_telemetry::TelemetryConfig::from_env();
            let recorder_active = vox_telemetry::global_recorder().is_some();
            let master = vox_telemetry::is_master_enabled();

            println!("vox telemetry doctor");
            println!("─────────────────────────────────────");
            println!("master_enabled:        {}", master);
            println!("enabled:               {}", cfg.enabled);
            println!("remote_upload:         {}", cfg.remote_upload);
            println!("debug_to_stderr:       {}", cfg.debug_to_stderr);
            println!("─────────────────────────────────────");
            println!("categories:");
            println!("  research_metrics:    {}", cfg.research_metrics);
            println!("  model_calls:         {}", cfg.model_calls);
            println!("  agent_orchestration: {}", cfg.agent_orchestration);
            println!("  build:               {}", cfg.build);
            println!("  errors:              {}", cfg.errors);
            println!("─────────────────────────────────────");
            println!("recorder_registered:   {}", recorder_active);

            let spool_root = telemetry_spool::spool_root();
            println!("spool_root:            {}", spool_root.display());

            println!("─────────────────────────────────────");
            if !master {
                println!("STATUS: DISABLED  (VOX_TELEMETRY=off or equivalent)");
                std::process::exit(1);
            } else if !recorder_active {
                println!("STATUS: WARNING   (no recorder registered — events are no-ops)");
            } else if cfg.debug_to_stderr {
                println!("STATUS: DEBUG     (VOX_TELEMETRY=debug — events printed to stderr)");
            } else {
                println!("STATUS: OK");
            }
        }
        TelemetryCmd::Status { spool } => {
            let root = resolve_spool(spool);
            let pending_n = telemetry_spool::pending_count(&root);
            let url_r = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTelemetryUploadUrl);
            let tok_r = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTelemetryUploadToken);
            println!("spool_root: {}", root.display());
            println!("pending_files: {pending_n}");
            println!(
                "upload_url: {} ({})",
                url_r.redacted(),
                if url_r.is_present() {
                    "resolved"
                } else {
                    "missing"
                }
            );
            println!(
                "upload_token: {} ({})",
                tok_r.redacted(),
                if tok_r.is_present() {
                    "resolved"
                } else {
                    "missing"
                }
            );
        }
        TelemetryCmd::Export { spool, out } => {
            let root = resolve_spool(spool);
            let n = if let Some(p) = out {
                let mut f =
                    fs::File::create(&p).with_context(|| format!("create {}", p.display()))?;
                telemetry_spool::export_jsonl(&root, &mut f)?
            } else {
                let mut stdout = std::io::stdout().lock();
                telemetry_spool::export_jsonl(&root, &mut stdout)?
            };
            eprintln!("exported {n} record(s)");
        }
        TelemetryCmd::Enqueue { spool, json } => {
            let root = resolve_spool(spool);
            let raw =
                fs::read_to_string(&json).with_context(|| format!("read {}", json.display()))?;
            let value: serde_json::Value =
                serde_json::from_str(&raw).context("parse --json file as JSON")?;
            let path = telemetry_spool::enqueue(&root, &value)?;
            println!("{}", path.display());
        }
        TelemetryCmd::Upload {
            spool,
            dry_run,
            url,
        } => {
            let root = resolve_spool(spool);
            let url_str = if let Some(u) = url.filter(|s| !s.trim().is_empty()) {
                u
            } else {
                let r = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTelemetryUploadUrl);
                r.expose()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow!("{}", r.remediation))?
            };
            let bearer =
                vox_secrets::resolve_secret(vox_secrets::SecretId::VoxTelemetryUploadToken)
                    .expose()
                    .map(str::to_owned);
            let (ok, fail) =
                telemetry_spool::upload_pending(&root, url_str.trim(), bearer.as_deref(), dry_run)
                    .await?;
            if fail > 0 {
                anyhow::bail!("upload: {ok} ok, {fail} failed (see logs)");
            }
            eprintln!("upload: {ok} file(s) processed successfully");
        }
    }
    Ok(())
}
