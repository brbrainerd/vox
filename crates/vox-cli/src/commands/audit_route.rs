//! `vox audit effort-route` — routes S1 effort-audit findings to verified,
//! drafted enforcement-artifact proposals.
//!
//! Reads S1's `findings.jsonl`, groups findings (deterministic enum-bucket +
//! conditional embedding sub-cluster), re-judges each cluster with adversarial
//! verification through the model-agnostic facade, and emits ranked
//! `recommendations.jsonl` + `recommendations.md` + staging-dir `.proposed`
//! draft enforcement artifacts.
//!
//! Composes the `vox-effort-route` crate with the workspace's model selection
//! (`vox-orchestrator::models::select`) and a TOML config layer (`vox.toml`
//! `[audit.route]`). All LLM/embedding I/O goes through the facade.
//!
//! See `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use std::path::PathBuf;

use vox_effort_route::config::EffortRouteConfig;
use vox_effort_route::embed::LlmEmbedder;
use vox_effort_route::route::{LlmRouter, MockRouter, ModelVoxCapability, Router};

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

/// `vox audit effort-route` flags.
#[derive(Args, Debug, Clone)]
pub struct EffortRouteArgs {
    /// Path to S1's `findings.jsonl` (output of `vox audit effort`).
    #[arg(long)]
    pub findings: PathBuf,
    /// Output directory. Defaults to `[audit.route].staging_dir/<run-id>/`
    /// under the repo root.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Override the judge model id (skips the orchestrator's registry-based
    /// selection). When set, a `tracing::warn!` records the override path.
    #[arg(long)]
    pub model: Option<String>,
    /// Use the deterministic `MockRouter` (fixed confidence, no LLM) instead of
    /// reaching the facade. Intended for offline smoke tests and CI.
    #[arg(long, default_value_t = false)]
    pub mock_router: bool,
}

// ---------------------------------------------------------------------------
// TOML config layer
// ---------------------------------------------------------------------------

/// Shape of `vox.toml` that this command consumes. Only the `[audit.route]`
/// table is read. `serde(default)` everywhere keeps a missing file / section
/// indistinguishable from "use defaults".
#[derive(Debug, Default, Deserialize)]
struct VoxTomlRoot {
    #[serde(default)]
    audit: VoxTomlAudit,
}

#[derive(Debug, Default, Deserialize)]
struct VoxTomlAudit {
    #[serde(default)]
    route: Option<RouteToml>,
}

/// `[audit.route]` table: the `EffortRouteConfig` plus a `vox_capable_models`
/// allowlist used to resolve the judge model's Vox-authoring capability (see
/// `resolve_vox_capability`; spec Q3 fallback — the orchestrator's
/// `ModelCapabilities` has no `writes_vox` field yet).
#[derive(Debug, Default, Deserialize)]
struct RouteToml {
    #[serde(flatten, default)]
    config: Option<EffortRouteConfig>,
    /// Model ids known to be able to author Vox source. Any resolved judge
    /// model not in this list (and not `mens`-prefixed) is treated as
    /// non-Vox-capable, gating `VoxScript` artifact forms.
    #[serde(default)]
    vox_capable_models: Vec<String>,
}

struct LoadedConfig {
    config: EffortRouteConfig,
    vox_capable_models: Vec<String>,
}

fn load_config(repo_root: &std::path::Path) -> Result<LoadedConfig> {
    let path = repo_root.join("vox.toml");
    if !path.exists() {
        return Ok(LoadedConfig {
            config: EffortRouteConfig::default(),
            vox_capable_models: Vec::new(),
        });
    }
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let root: VoxTomlRoot = toml::from_str(&raw)
        .with_context(|| format!("parse [audit.route] table from {}", path.display()))?;
    let route = root.audit.route.unwrap_or_default();
    Ok(LoadedConfig {
        config: route.config.unwrap_or_default(),
        vox_capable_models: route.vox_capable_models,
    })
}

// ---------------------------------------------------------------------------
// Judge model + Vox-capability resolution
// ---------------------------------------------------------------------------

/// Pick the judge model id. Order mirrors S1's `audit effort`:
/// 1. `--model <id>` from the CLI (logs a `warn!` override).
/// 2. `[audit.route.judge].model_preference` from `vox.toml`.
/// 3. `vox-orchestrator::models::select` with `TaskCategory::CodeEffortJudge`.
///
/// Returns `None` when (3) finds no registered model; the caller bails so the
/// user knows to point at a model rather than silently defaulting.
fn resolve_judge_model(args: &EffortRouteArgs, cfg: &EffortRouteConfig) -> Option<String> {
    if let Some(m) = args.model.as_deref() {
        tracing::warn!(
            target: "vox_audit_route",
            model = m,
            "judge model override active (--model); skipping orchestrator selection"
        );
        return Some(m.to_string());
    }
    if let Some(m) = cfg.judge.model_preference.as_deref() {
        tracing::debug!(
            target: "vox_audit_route",
            model = m,
            "judge model resolved from vox.toml [audit.route.judge].model_preference"
        );
        return Some(m.to_string());
    }
    use vox_orchestrator::models::TaskCategory;
    use vox_orchestrator::models::select::{SelectionIntent, select_with_default_registry};
    let outcome =
        select_with_default_registry(&SelectionIntent::for_task(TaskCategory::CodeEffortJudge))?;
    tracing::debug!(
        target: "vox_audit_route",
        model = outcome.model_id,
        reason = ?outcome.reason,
        "judge model resolved via vox-orchestrator::models::select"
    );
    Some(outcome.model_id)
}

/// Resolve whether the judge model may author Vox source.
///
/// DEVIATION (spec Q3 fallback): the orchestrator's `ModelCapabilities` has no
/// `writes_vox` field yet, so we cannot read capability off the registry entry.
/// Instead we use a two-part allowlist:
///   1. an explicit `[audit.route] vox_capable_models` list in `vox.toml`, OR
///   2. a `mens`-prefix heuristic (MENS models are the first-class Vox authors).
/// Adding a real `writes_vox` capability field to the orchestrator registry is
/// tracked as a follow-up; it is out of scope for S2.
fn resolve_vox_capability(model_id: &str, allowlist: &[String]) -> ModelVoxCapability {
    let capable = allowlist.iter().any(|m| m == model_id) || model_id.starts_with("mens");
    ModelVoxCapability(capable)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// `vox audit effort-route` handler.
pub async fn run(args: EffortRouteArgs) -> Result<()> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let loaded = load_config(&repo_root)?;
    let cfg = loaded.config.clone();

    // Resolve out_dir up-front so the user sees where output will land.
    let run_id = uuid::Uuid::new_v4().to_string();
    let out_dir = match &args.out_dir {
        Some(p) => p.clone(),
        None => cfg.staging_dir.join(&run_id),
    };
    println!("vox audit effort-route: writing to {}", out_dir.display());

    // Build router + embedder + capability.
    let (router, vox_capable): (Box<dyn Router>, ModelVoxCapability) = if args.mock_router {
        (
            Box::new(MockRouter { confidence: 0.9 }),
            ModelVoxCapability(false),
        )
    } else {
        let model = resolve_judge_model(&args, &cfg).ok_or_else(|| {
            anyhow::anyhow!(
                "no judge model could be resolved (none in vox.toml [audit.route.judge], \
                 none picked by vox-orchestrator::models::select for CodeEffortJudge, \
                 and no --model override given). Pass `--model <id>` or `--mock-router` \
                 for a deterministic offline run."
            )
        })?;
        let vox_capable = resolve_vox_capability(&model, &loaded.vox_capable_models);
        tracing::debug!(
            target: "vox_audit_route",
            model = model,
            vox_capable = vox_capable.0,
            "resolved judge model vox-authoring capability (spec Q3 allowlist fallback)"
        );
        let router = LlmRouter {
            resolved_model: model,
            timeout: vox_config::timeouts::EFFORT_AUDIT_JUDGE_TIMEOUT,
            repo_root: repo_root.clone(),
            max_context_commits: cfg.max_context_commits,
            verify: cfg.judge.verify,
        };
        (Box::new(router), vox_capable)
    };

    let embedder = LlmEmbedder {
        model: cfg
            .judge
            .model_preference
            .clone()
            .unwrap_or_else(|| "auto".to_string()),
        timeout: vox_config::timeouts::EFFORT_AUDIT_JUDGE_TIMEOUT,
    };

    let summary = vox_effort_route::run(
        &args.findings,
        &out_dir,
        cfg,
        router,
        Box::new(embedder),
        vox_capable,
    )
    .await?;

    println!(
        "vox audit effort-route: run {} — {} findings → {} buckets → {} clusters ({} verified)",
        summary.run_id,
        summary.findings_loaded,
        summary.buckets,
        summary.clusters_routed,
        summary.verified,
    );
    println!(
        "  report:          {}",
        out_dir.join("recommendations.md").display()
    );
    println!(
        "  recommendations: {}",
        out_dir.join("recommendations.jsonl").display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn args_default() -> EffortRouteArgs {
        EffortRouteArgs {
            findings: PathBuf::from("findings.jsonl"),
            out_dir: None,
            model: None,
            mock_router: false,
        }
    }

    #[test]
    fn load_config_missing_file_yields_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = load_config(tmp.path()).unwrap();
        assert_eq!(loaded.config, EffortRouteConfig::default());
        assert!(loaded.vox_capable_models.is_empty());
    }

    #[test]
    fn load_config_reads_audit_route_section() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("vox.toml"),
            r#"
[audit.route]
min_waste_score = 6
vox_capable_models = ["mens/code-1", "custom/vox-writer"]

[audit.route.judge]
model_preference = "anthropic/claude-haiku-4.6"
verify = false
"#,
        )
        .unwrap();
        let loaded = load_config(tmp.path()).unwrap();
        assert_eq!(loaded.config.min_waste_score, 6);
        assert!(!loaded.config.judge.verify);
        assert_eq!(
            loaded.config.judge.model_preference.as_deref(),
            Some("anthropic/claude-haiku-4.6")
        );
        assert_eq!(
            loaded.vox_capable_models,
            vec!["mens/code-1".to_string(), "custom/vox-writer".to_string()]
        );
    }

    #[test]
    fn resolve_judge_model_honors_cli_override() {
        let mut args = args_default();
        args.model = Some("custom-model".into());
        let cfg = EffortRouteConfig::default();
        assert_eq!(
            resolve_judge_model(&args, &cfg).as_deref(),
            Some("custom-model")
        );
    }

    #[test]
    fn resolve_judge_model_falls_through_to_config() {
        let args = args_default();
        let mut cfg = EffortRouteConfig::default();
        cfg.judge.model_preference = Some("toml-model".into());
        assert_eq!(
            resolve_judge_model(&args, &cfg).as_deref(),
            Some("toml-model")
        );
    }

    #[test]
    fn vox_capability_uses_allowlist_then_mens_heuristic() {
        let allow = vec!["custom/vox-writer".to_string()];
        // Explicit allowlist entry → capable.
        assert!(resolve_vox_capability("custom/vox-writer", &allow).0);
        // mens-prefix heuristic → capable even when not listed.
        assert!(resolve_vox_capability("mens/code-1", &allow).0);
        // Neither → not capable (VoxScript forms get gated out).
        assert!(!resolve_vox_capability("anthropic/claude-haiku-4.6", &allow).0);
        assert!(!resolve_vox_capability("openai/gpt", &[]).0);
    }
}
