//! `vox audit effort` — AI-judged audit of recent git commit history.
//!
//! Estimates per-commit agent-token spend and tags suggested remediations
//! (script automation, test investment, etc.). Composes the
//! `vox-effort-audit` crate (range → walk → shape → hybrid → judge → emit)
//! with the workspace's model selection (`vox-orchestrator::models::select`)
//! and a TOML config layer (`vox.toml` `[audit.effort]`).
//!
//! See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use std::path::PathBuf;

use vox_effort_audit::config::EffortAuditConfig;
use vox_effort_audit::judge::{Judge, LlmJudge, MockJudge};
use vox_effort_audit::pipeline;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// `vox audit effort` flags.
#[derive(Args, Debug, Clone)]
pub struct EffortArgs {
    /// Range start (git ref like `HEAD~30` or a duration like `"30 days ago"` / `"7d"`).
    /// Defaults to `[audit.effort].default_since` (`"30 days ago"` if unset).
    #[arg(long)]
    pub since: Option<String>,
    /// Range end (git ref). Defaults to `HEAD`.
    #[arg(long)]
    pub until: Option<String>,
    /// Override the judge model id (skips the orchestrator's registry-based
    /// selection). When set, a `tracing::warn!` records the override path per
    /// spec §4.2 so transcripts retain a breadcrumb of manual routing.
    #[arg(long)]
    pub model: Option<String>,
    /// Cap number of commits actually sent to the judge. Past the cap, every
    /// remaining commit becomes a `Skipped` row in `findings.jsonl`. Useful
    /// for CI smoke runs.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Disable Claude-Code transcript correlation (the "measured" half of the
    /// hybrid signal). When set, `with_transcripts = false` overrides the
    /// `vox.toml` value.
    #[arg(long, default_value_t = false)]
    pub no_transcripts: bool,
    /// Output directory. Defaults to `target/audit/effort/<uuid>/` under the
    /// repo root (so a `cargo clean` wipes the cache cleanly).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Use the deterministic `MockJudge` (fixed score, zero tokens) instead of
    /// reaching the LLM facade. Intended for offline smoke tests, CI, and
    /// scaffold-verification. Implies no LLM cost.
    #[arg(long, default_value_t = false)]
    pub mock_judge: bool,
}

// ---------------------------------------------------------------------------
// TOML config layer
// ---------------------------------------------------------------------------

/// Shape of `vox.toml` that this command consumes. Only the `[audit.effort]`
/// table is read; the rest of `vox.toml` is ignored at this layer (other
/// commands own their own slices). `serde(default)` on every level keeps a
/// missing file or missing section indistinguishable from "use defaults".
#[derive(Debug, Default, Deserialize)]
struct VoxTomlRoot {
    #[serde(default)]
    audit: VoxTomlAudit,
}

#[derive(Debug, Default, Deserialize)]
struct VoxTomlAudit {
    #[serde(default)]
    effort: Option<EffortAuditConfig>,
}

fn load_config(repo_root: &std::path::Path) -> Result<EffortAuditConfig> {
    let path = repo_root.join("vox.toml");
    if !path.exists() {
        return Ok(EffortAuditConfig::default());
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let root: VoxTomlRoot = toml::from_str(&raw)
        .with_context(|| format!("parse [audit.effort] table from {}", path.display()))?;
    Ok(root.audit.effort.unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Judge model resolution
// ---------------------------------------------------------------------------

/// Pick the judge model id. Order:
/// 1. `--model <id>` from the CLI (logs a `warn!` override).
/// 2. `[audit.effort.judge].model_preference` from `vox.toml`.
/// 3. `vox-orchestrator::models::select` with
///    `SelectionIntent::for_task(TaskCategory::CodeEffortJudge)`.
///
/// Returns `None` when (3) fails to find any registered model. The caller is
/// expected to bail with a clear error so the user knows to point at a
/// model — defaulting silently would obscure routing surprises.
fn resolve_judge_model(args: &EffortArgs, cfg: &EffortAuditConfig) -> Option<String> {
    if let Some(m) = args.model.as_deref() {
        tracing::warn!(
            target: "vox_audit_effort",
            model = m,
            "judge model override active (--model); skipping orchestrator selection"
        );
        return Some(m.to_string());
    }
    if let Some(m) = cfg.judge.model_preference.as_deref() {
        tracing::debug!(
            target: "vox_audit_effort",
            model = m,
            "judge model resolved from vox.toml [audit.effort.judge].model_preference"
        );
        return Some(m.to_string());
    }
    use vox_orchestrator::models::TaskCategory;
    use vox_orchestrator::models::select::{SelectionIntent, select_with_default_registry};
    let outcome =
        select_with_default_registry(&SelectionIntent::for_task(TaskCategory::CodeEffortJudge))?;
    tracing::debug!(
        target: "vox_audit_effort",
        model = outcome.model_id,
        reason = ?outcome.reason,
        "judge model resolved via vox-orchestrator::models::select"
    );
    Some(outcome.model_id)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// `vox audit effort` handler.
pub async fn run(args: EffortArgs) -> Result<()> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let mut cfg = load_config(&repo_root)?;

    // CLI-level overrides on top of TOML.
    if args.no_transcripts {
        cfg.with_transcripts = false;
    }
    if let Some(n) = args.limit {
        cfg.limit = Some(n);
    }

    // Resolve out_dir before the (potentially-slow) run so the user sees
    // where output will land even if the run takes 30 seconds.
    let out_dir = match &args.out_dir {
        Some(p) => p.clone(),
        None => {
            // Generate a UUID up-front so the path is stable / discoverable
            // before `pipeline::run` finishes. `pipeline::run` allocates its
            // own internal run_id for the manifest — those are intentionally
            // distinct (filesystem-stable vs. content-stable).
            let id = uuid::Uuid::new_v4();
            repo_root.join("target/audit/effort").join(id.to_string())
        }
    };
    println!("vox audit effort: writing to {}", out_dir.display());

    // Build judge.
    let judge: Box<dyn Judge> = if args.mock_judge {
        Box::new(MockJudge {
            fixed_score: 50,
            model: "mock-judge".into(),
        })
    } else {
        let model = resolve_judge_model(&args, &cfg).ok_or_else(|| {
            anyhow::anyhow!(
                "no judge model could be resolved (none in vox.toml [audit.effort.judge], \
                 none picked by vox-orchestrator::models::select for CodeEffortJudge, \
                 and no --model override given). Pass `--model <id>` or \
                 `--mock-judge` for a deterministic offline run."
            )
        })?;
        Box::new(LlmJudge {
            config: cfg.judge.clone(),
            resolved_model: model,
            timeout: vox_config::timeouts::EFFORT_AUDIT_JUDGE_TIMEOUT,
        })
    };

    let summary = pipeline::run_with_overrides(
        &repo_root,
        &out_dir,
        cfg,
        judge,
        None,
        args.since.clone(),
        args.until.clone(),
    )
    .await?;

    println!(
        "vox audit effort: run {} — judged {}/{} commits ({} skipped)",
        summary.run_id, summary.commits_judged, summary.commits_in_range, summary.commits_skipped,
    );
    println!("  report:   {}", out_dir.join("report.md").display());
    println!("  findings: {}", out_dir.join("findings.jsonl").display());
    println!("  manifest: {}", out_dir.join("manifest.json").display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = load_config(tmp.path()).unwrap();
        assert_eq!(cfg, EffortAuditConfig::default());
    }

    #[test]
    fn load_config_reads_audit_effort_section() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("vox.toml"),
            r#"
[audit.effort]
default_since = "7 days ago"
limit = 3

[audit.effort.judge]
model_preference = "anthropic/claude-haiku-4.6"
"#,
        )
        .unwrap();
        let cfg = load_config(tmp.path()).unwrap();
        assert_eq!(cfg.default_since, "7 days ago");
        assert_eq!(cfg.limit, Some(3));
        assert_eq!(
            cfg.judge.model_preference.as_deref(),
            Some("anthropic/claude-haiku-4.6"),
        );
    }

    #[test]
    fn load_config_missing_section_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("vox.toml"),
            "[some.other.section]\nkey = 1\n",
        )
        .unwrap();
        let cfg = load_config(tmp.path()).unwrap();
        assert_eq!(cfg, EffortAuditConfig::default());
    }

    #[test]
    fn resolve_judge_model_honors_cli_override() {
        let mut args = effort_args_default();
        args.model = Some("custom-model".into());
        let cfg = EffortAuditConfig::default();
        assert_eq!(
            resolve_judge_model(&args, &cfg).as_deref(),
            Some("custom-model")
        );
    }

    #[test]
    fn resolve_judge_model_falls_through_to_config() {
        let args = effort_args_default();
        let mut cfg = EffortAuditConfig::default();
        cfg.judge.model_preference = Some("toml-model".into());
        assert_eq!(
            resolve_judge_model(&args, &cfg).as_deref(),
            Some("toml-model")
        );
    }

    fn effort_args_default() -> EffortArgs {
        EffortArgs {
            since: None,
            until: None,
            model: None,
            limit: None,
            no_transcripts: false,
            out_dir: None,
            mock_judge: false,
        }
    }
}
