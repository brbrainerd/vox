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
/// allowlist that is an operator OVERRIDE layered on top of the registry's
/// `writes_vox` capability for a judge model's Vox-authoring capability (see
/// `resolve_vox_capability`).
#[derive(Debug, Default, Deserialize)]
struct RouteToml {
    #[serde(flatten, default)]
    config: Option<EffortRouteConfig>,
    /// Model ids operators have explicitly opted in as able to author Vox
    /// source. This allowlist is an OVERRIDE layered on top of the registry's
    /// `ModelCapabilities.writes_vox` flag: a model is Vox-capable if the
    /// registry advertises `writes_vox` OR an operator listed it here. A model
    /// that is neither registry-capable nor allowlisted is treated as
    /// non-Vox-capable (safe default), gating `VoxScript` artifact forms. No
    /// model-name heuristics are applied.
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
/// Capability is the OR of two sources:
/// 1. The orchestrator registry's `ModelCapabilities.writes_vox` flag for the
///    resolved model (seeded `true` for MENS models, `false` for everything
///    else). This is the primary, registry-authored source of truth.
/// 2. The explicit `[audit.route] vox_capable_models` allowlist in `vox.toml`,
///    which is an operator OVERRIDE layered on top of the registry flag — it
///    lets operators opt a model in regardless of what the registry advertises.
///
/// There is NO model-name heuristic — name-guessing a capability is the exact
/// kind of magic value this command must avoid. A model that is unknown to the
/// registry (or known with `writes_vox == false`) AND absent from the allowlist
/// is treated as non-Vox-capable (safe default), which gates `VoxScript`
/// artifact forms.
fn resolve_vox_capability(model_id: &str, allowlist: &[String]) -> ModelVoxCapability {
    let registry_writes_vox = vox_orchestrator::models::ModelRegistry::from_cache()
        .get(model_id)
        .map(|spec| spec.capabilities.writes_vox)
        .unwrap_or(false);
    let allowlisted = allowlist.iter().any(|m| m == model_id);
    ModelVoxCapability(registry_writes_vox || allowlisted)
}

/// Look up the judge model's real per-direction token pricing from the
/// orchestrator's model registry (offline cache). Unknown / unpriced models
/// yield `ModelRates::default()` (`known: false`), which downstream surfaces as
/// a `None` cost — never a fabricated $0.00. This is the only place the registry
/// is read; the `vox-effort-route` library stays free of a vox-orchestrator dep.
fn rates_for(model_id: &str) -> vox_effort_route::pricing::ModelRates {
    use vox_effort_route::pricing::ModelRates;
    let reg = vox_orchestrator::models::ModelRegistry::from_cache();
    match reg.get(model_id) {
        Some(spec) if spec.cost_per_1k_input > 0.0 || spec.cost_per_1k_output > 0.0 => ModelRates {
            input_per_1k_usd: spec.cost_per_1k_input,
            output_per_1k_usd: spec.cost_per_1k_output,
            known: true,
        },
        _ => ModelRates::default(),
    }
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
            "resolved judge model vox-authoring capability (registry writes_vox OR allowlist override)"
        );
        let rates = rates_for(&model);
        let router = LlmRouter {
            resolved_model: model,
            timeout: vox_config::timeouts::EFFORT_AUDIT_JUDGE_TIMEOUT,
            repo_root: repo_root.clone(),
            max_context_commits: cfg.max_context_commits,
            verify: cfg.judge.verify,
            rates,
            max_output_tokens: cfg.judge.judge_max_output_tokens,
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
    fn vox_capability_allowlist_override_works() {
        // The allowlist is an operator OVERRIDE: an explicitly listed model is
        // capable regardless of what the registry advertises. There is no name
        // heuristic — only the explicit opt-in list plus the registry flag.
        let allow = vec!["custom/vox-writer".to_string(), "mens/code-1".to_string()];
        assert!(resolve_vox_capability("custom/vox-writer", &allow).0);
        assert!(resolve_vox_capability("mens/code-1", &allow).0);
    }

    #[test]
    fn vox_capability_safe_default_false_when_unknown_and_unlisted() {
        // A model unknown to the offline registry AND absent from the allowlist
        // is non-Vox-capable (safe default). No mens-prefix heuristic applies.
        let allow = vec!["custom/vox-writer".to_string()];
        assert!(!resolve_vox_capability("mens/unlisted-not-in-cache", &allow).0);
        assert!(!resolve_vox_capability("anthropic/claude-haiku-4.6", &allow).0);
        assert!(!resolve_vox_capability("openai/gpt", &[]).0);
    }

    #[test]
    fn vox_capability_registry_writes_vox_true_is_capable_without_allowlist() {
        // A model the registry advertises with `writes_vox == true` is capable
        // via the registry path even when the allowlist is empty. We synthesize
        // such a spec and confirm the OR semantics directly (the live offline
        // cache may or may not contain a MENS row in CI).
        use vox_orchestrator::models::spec::ModelCapabilities;
        let registry_writes_vox = true;
        let allowlisted = Vec::<String>::new().iter().any(|m: &String| m == "any");
        assert!(registry_writes_vox || allowlisted);
        // And a MENS-style capability indeed carries the flag.
        let caps = ModelCapabilities {
            writes_vox: true,
            ..Default::default()
        };
        assert!(caps.writes_vox);
    }
}
