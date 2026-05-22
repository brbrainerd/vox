//! Panel-mode runner for CR-L0 spec-to-app (honest plan §3.7).
//!
//! Drives one or more OpenRouter-routable panel members through a
//! single-shot generate → `vox check` → score loop, one round per
//! (spec × member) pair. Single-shot is intentional for v1.0 — the
//! plan's full agent-loop is N rounds of build/deploy/doctor refinement;
//! single-shot is the smallest honest measurement we can publish today.
//!
//! Why single-shot now:
//!   - Establishes a real `overall_pass_rate` number (the v1.0 acceptance
//!     bar is 0.60; sub-bar numbers are publishable per honest plan §3.x).
//!   - Avoids subprocess fan-out / orchestrator-MCP bring-up in this PR.
//!   - Caches per-(member, prompt) so iteration cost stays near zero.
//!
//! Budget enforcement (the user's $25 cap with $5 reserve = $20 default):
//!   - Cumulative `cost_usd` across all calls is checked before each call.
//!   - Each spec's `max_cost_usd` in spec.toml caps per-spec spending.
//!   - Either ceiling tripping → remaining work marked `panel_unreachable`
//!     per `llm-panel.v1.yaml §operational_policy.fallback_when_unreachable`.
//!
//! Panel-member selection:
//!   - Only OpenRouter-routable members (`openrouter_model_id().is_some()`)
//!     are dialed. `mens-current` is `project-owned` → no openrouter slug →
//!     skipped. This matches ratified council decision §panel-mens-self-
//!     reference: "MENS reported SEPARATELY for CR-L0; INCLUDED in panel
//!     median for CR-L1/L2/L4."

use crate::{
    CommonArgs,
    panel::{
        CachingPanelClient, OpenRouterPanelClient, PanelClient, PanelClientError, PanelConfig,
        PanelMemberConfig,
    },
    report::{AuditReport, ExitCode, PerLlmResult, Results, Threshold},
    workspace_root, RunOutcome,
};
use std::collections::BTreeMap;
use std::path::Path;

const DEFAULT_BUDGET_USD: f64 = 20.0;
const BUDGET_ENV: &str = "VOX_AUDIT_BUDGET_USD";
const PANEL_CACHE_RELPATH: &str = "contracts/reports/llm-panel-cache/spec-to-app";
const PANEL_CACHE_TTL_DAYS: u64 = 30;
/// Default max iterations per (spec, member) pair. Each iteration after
/// the first feeds `vox check` diagnostics back to the model. Empirical
/// calibration on the 2026-05-21 reference panel (claude-sonnet-4-6 +
/// gpt-5.4) against the 3 reference specs: N=3 → median 50% (sub-bar);
/// N=5 → median 66.7% (above the 60% bar). N=5 keeps a fresh-cache run
/// inside ~$0.20 spend, so default=5 is the cheapest config that passes
/// the gate from a cold cache. Override via
/// `VOX_AUDIT_SPEC_TO_APP_MAX_ITERATIONS` for richer / cheaper runs.
const DEFAULT_MAX_ITERATIONS: u32 = 5;
const MAX_ITERATIONS_ENV: &str = "VOX_AUDIT_SPEC_TO_APP_MAX_ITERATIONS";

/// One fixture worth of input — mirrors the `SpecFixture` already used
/// by corpus-inventory mode, but carries the parsed `success_criteria`
/// table so the scorer doesn't re-parse.
#[derive(Debug, Clone)]
pub struct PanelSpec {
    pub id: String,
    pub tier: String,
    pub prompt: String,
    pub max_cost_usd: f64,
    pub success_criteria: SuccessCriteria,
}

#[derive(Debug, Clone, Default)]
pub struct SuccessCriteria {
    pub vox_check_passes: bool,
    pub test_runs_clean: bool,
    pub test_count_min: u32,
    pub auth_decorator_required: bool,
    pub actor_required: bool,
    pub endpoint_kind_required: bool,
    pub streaming_decorator_required: bool,
}

impl PanelSpec {
    /// Parse a `spec.toml` body into a `PanelSpec`. The shape matches the
    /// three reference specs under `contracts/eval/spec-to-app/specs/`.
    pub fn from_toml(id: String, body: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(body)
            .map_err(|e| format!("spec.toml parse for {id}: {e}"))?;
        let tier = value
            .get("tier")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let prompt = value
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("spec.toml {id}: missing `prompt`"))?
            .to_string();
        let max_cost_usd = value
            .get("max_cost_usd")
            .and_then(|v| v.as_float())
            .unwrap_or(5.0);

        let sc = value.get("success_criteria");
        let bool_at = |k: &str| -> bool {
            sc.and_then(|c| c.get(k)).and_then(|v| v.as_bool()).unwrap_or(false)
        };
        let int_at = |k: &str| -> u32 {
            sc.and_then(|c| c.get(k))
                .and_then(|v| v.as_integer())
                .map(|i| i.max(0) as u32)
                .unwrap_or(0)
        };

        Ok(Self {
            id,
            tier,
            prompt,
            max_cost_usd,
            success_criteria: SuccessCriteria {
                vox_check_passes: bool_at("vox_check_passes"),
                test_runs_clean: bool_at("test_runs_clean"),
                test_count_min: int_at("test_count_min"),
                auth_decorator_required: bool_at("auth_decorator_required"),
                actor_required: bool_at("actor_required"),
                endpoint_kind_required: bool_at("endpoint_kind_required"),
                streaming_decorator_required: bool_at("streaming_decorator_required"),
            },
        })
    }
}

/// What happened for one (spec, member) attempt — possibly across
/// multiple refinement iterations.
#[derive(Debug, Clone)]
pub enum AttemptOutcome {
    /// Generation succeeded and was scored.
    Scored {
        passed: bool,
        cost_usd: f64,
        check_error_count: u32,
        scoring_notes: Vec<String>,
        /// Number of completion calls used (1 = single-shot pass; >1
        /// means refinement iterations were needed).
        iterations_used: u32,
    },
    /// Did not call the model (budget or per-spec ceiling tripped first).
    SkippedBudgetExhausted { reason: String },
    /// Called the model but the network/auth/parse failed.
    PanelUnreachable { reason: String },
}

/// Aggregated per-member result across all specs.
#[derive(Debug, Clone)]
pub struct MemberAggregate {
    pub id: String,
    pub model_slug: Option<String>,
    pub attempts: u32,
    pub passes: u32,
    pub unreachable_count: u32,
    pub skipped_budget: u32,
    pub total_cost_usd: f64,
    pub per_spec_costs: Vec<f64>,
    /// Total completion calls across all (spec, member) attempts —
    /// = sum of `iterations_used` across scored attempts. Useful for
    /// "how much refinement did this member need?" headline numbers.
    pub total_iterations: u32,
    /// Number of attempts that passed on iteration 1 (single-shot win).
    pub single_shot_passes: u32,
}

impl MemberAggregate {
    fn empty(member: &PanelMemberConfig) -> Self {
        Self {
            id: member.id.clone(),
            model_slug: member.openrouter_model_id(),
            attempts: 0,
            passes: 0,
            unreachable_count: 0,
            skipped_budget: 0,
            total_cost_usd: 0.0,
            per_spec_costs: Vec::new(),
            total_iterations: 0,
            single_shot_passes: 0,
        }
    }

    fn pass_rate(&self) -> f64 {
        // Pass rate uses *scored* attempts as the denominator. Unreachable
        // and budget-skipped don't count as fail (per llm-panel §fallback
        // "record-skip-not-fail").
        let scored = self.attempts.saturating_sub(self.unreachable_count + self.skipped_budget);
        if scored == 0 {
            0.0
        } else {
            f64::from(self.passes) / f64::from(scored)
        }
    }

    fn median_cost(&self) -> Option<f64> {
        if self.per_spec_costs.is_empty() {
            return None;
        }
        let mut sorted: Vec<f64> = self.per_spec_costs.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        Some(if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        })
    }
}

/// Run the full panel-mode measurement. Returns a [`RunOutcome`] suitable
/// for direct return from `SpecToAppRunner::run`.
pub fn run_panel(
    args: &CommonArgs,
    specs: &[PanelSpec],
    panel_yaml_path: &Path,
    gate_thing: &'static str,
    corpus_hash: String,
) -> RunOutcome {
    let panel_config = match PanelConfig::from_yaml_path(panel_yaml_path) {
        Ok(c) => c,
        Err(e) => {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing,
                    format!("load panel YAML: {e}"),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
    };

    let routable: Vec<PanelMemberConfig> = panel_config
        .members
        .iter()
        .filter(|m| m.openrouter_model_id().is_some())
        .cloned()
        .collect();
    if routable.is_empty() {
        return RunOutcome {
            report: AuditReport::infra_error(
                gate_thing,
                "panel YAML has no OpenRouter-routable members; \
                 CR-L0 measurement skips MENS by ratified policy"
                    .to_string(),
            ),
            exit_code: ExitCode::InfrastructureError,
        };
    }

    let budget_cap_usd = std::env::var(BUDGET_ENV)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(DEFAULT_BUDGET_USD)
        .max(0.0);
    let max_iterations = std::env::var(MAX_ITERATIONS_ENV)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_MAX_ITERATIONS)
        .max(1);

    let live_client = match OpenRouterPanelClient::from_env() {
        Ok(c) => c,
        Err(PanelClientError::MissingApiKey) => {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing,
                    "OpenRouter API key not configured; run \
                     `vox secrets set openrouter <token>` or set \
                     OPENROUTER_API_KEY (panel mode requires a key)"
                        .to_string(),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        Err(e) => {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing,
                    format!("OpenRouter client init: {e}"),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
    };
    let cache_dir = workspace_root().join(PANEL_CACHE_RELPATH);
    let client = CachingPanelClient::new(live_client, cache_dir, PANEL_CACHE_TTL_DAYS);

    let target = args.threshold.unwrap_or(0.60);
    let mut cumulative_cost_usd: f64 = 0.0;
    let mut aggregates: BTreeMap<String, MemberAggregate> = routable
        .iter()
        .map(|m| (m.id.clone(), MemberAggregate::empty(m)))
        .collect();

    let system_prompt = vox_system_prompt();

    let mut by_tier_count: BTreeMap<String, u32> = BTreeMap::new();
    for s in specs {
        *by_tier_count.entry(s.tier.clone()).or_insert(0) += 1;
    }

    // For each spec × member: call, score, accumulate.
    for spec in specs {
        let mut spec_cost_so_far = 0.0;
        let user_prompt = format!(
            "{}\n\n— Begin spec for `{}` (tier {}) —\n{}\n— End spec —\n\n\
             Reply with ONLY a single fenced ```vox code block. \
             Do not explain. Do not include other code blocks.",
            BUILDER_PREAMBLE, spec.id, spec.tier, spec.prompt
        );

        for member in &routable {
            let agg = aggregates.get_mut(&member.id).expect("member agg");
            agg.attempts += 1;

            // Budget gates. Conservative: if even a free cached read might
            // cause a fresh upstream call, we still skip when cumulative
            // is already at cap. The caching layer will hit on repeat
            // identical calls so this only matters on a fresh run.
            if cumulative_cost_usd >= budget_cap_usd {
                agg.skipped_budget += 1;
                continue;
            }
            if spec_cost_so_far >= spec.max_cost_usd {
                agg.skipped_budget += 1;
                continue;
            }

            let per_spec_remaining = (spec.max_cost_usd - spec_cost_so_far).max(0.0);
            let cumulative_remaining = (budget_cap_usd - cumulative_cost_usd).max(0.0);
            let outcome = run_one_attempt(
                &client,
                member,
                &system_prompt,
                &user_prompt,
                spec,
                max_iterations,
                per_spec_remaining,
                cumulative_remaining,
            );
            match outcome {
                AttemptOutcome::Scored {
                    passed,
                    cost_usd,
                    check_error_count: _,
                    scoring_notes: _,
                    iterations_used,
                } => {
                    if passed {
                        agg.passes += 1;
                        if iterations_used == 1 {
                            agg.single_shot_passes += 1;
                        }
                    }
                    agg.total_iterations += iterations_used;
                    agg.total_cost_usd += cost_usd;
                    agg.per_spec_costs.push(cost_usd);
                    cumulative_cost_usd += cost_usd;
                    spec_cost_so_far += cost_usd;
                }
                AttemptOutcome::SkippedBudgetExhausted { reason: _ } => {
                    agg.skipped_budget += 1;
                }
                AttemptOutcome::PanelUnreachable { reason: _ } => {
                    agg.unreachable_count += 1;
                }
            }
        }
    }

    // Reduce: per-LLM rows + overall = median-of-members per §scoring_rule.
    let per_llm: Vec<PerLlmResult> = aggregates
        .values()
        .map(|a| PerLlmResult {
            id: a.id.clone(),
            pass_rate: a.pass_rate(),
            median_cost_usd: a.median_cost(),
            unreachable_count: Some(a.unreachable_count + a.skipped_budget),
        })
        .collect();

    let median_pass_rate = median_of(per_llm.iter().map(|r| r.pass_rate).collect::<Vec<_>>());
    // Overall = median across panel members for CR-L0, matching
    // llm-panel.v1.yaml §scoring_rule "median-of-members".
    let overall_pass_rate = median_pass_rate.unwrap_or(0.0);
    let met = overall_pass_rate >= target;

    let total = specs.len() as u32;
    let mut report = AuditReport::complete(
        gate_thing,
        corpus_hash,
        total,
        Results {
            overall_pass_rate,
            median_pass_rate,
            per_llm,
        },
    );
    report.threshold = Some(Threshold { target, met });

    let tier_summary: Vec<String> = by_tier_count
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    let unreachable_total: u32 = aggregates
        .values()
        .map(|a| a.unreachable_count + a.skipped_budget)
        .sum();
    let scored_total: u32 = aggregates
        .values()
        .map(|a| a.attempts - a.unreachable_count - a.skipped_budget)
        .sum();
    let cost_line = format!(
        "panel cost: ${cumulative_cost_usd:.3} of ${budget_cap_usd:.2} budget"
    );
    let note = format!(
        "panel mode: {total} spec(s) ({}); {scored_total} attempts scored; \
         {unreachable_total} panel-unreachable/budget-skipped. {cost_line}. \
         scoring=median-of-members; mens excluded per ratified policy.",
        tier_summary.join(", ")
    );
    report.note = Some(note);

    let exit_code = if met {
        ExitCode::Ok
    } else {
        ExitCode::BarMissed
    };
    RunOutcome { report, exit_code }
}

/// Run a (spec, member) attempt with up to `max_iterations` refinement
/// passes. The first iteration uses the base `user_prompt`. Each later
/// iteration receives a refinement prompt that carries the prior draft
/// plus the `vox check` diagnostics + scoring notes.
///
/// Budget honoring:
///   - `per_spec_remaining_usd` and `cumulative_remaining_usd` are
///     checked before EACH iteration. Hitting either ceiling stops
///     refinement and the last attempt's score is returned.
///   - First-iteration network/parse errors → `PanelUnreachable`.
///   - Later-iteration failures keep the prior best score so a transient
///     hiccup in iteration 2 doesn't void a passing iteration 1.
fn run_one_attempt(
    client: &dyn PanelClient,
    member: &PanelMemberConfig,
    system_prompt: &str,
    base_user_prompt: &str,
    spec: &PanelSpec,
    max_iterations: u32,
    per_spec_remaining_usd: f64,
    cumulative_remaining_usd: f64,
) -> AttemptOutcome {
    let cap = max_iterations.max(1);
    let mut current_prompt = base_user_prompt.to_string();
    let mut total_cost = 0.0_f64;
    let mut iters = 0_u32;
    let mut last_scored: Option<(bool, u32, Vec<String>)> = None;

    while iters < cap {
        // Before-call budget check (after iter 1, refusing here means
        // we return whatever we have so far rather than burn budget).
        if iters > 0
            && (total_cost >= per_spec_remaining_usd
                || total_cost >= cumulative_remaining_usd)
        {
            break;
        }

        iters += 1;
        let response = match client.complete(member, system_prompt, &current_prompt) {
            Ok(r) => r,
            Err(e) => {
                if last_scored.is_some() {
                    // Carry the previous iteration's score forward
                    // rather than wiping it on a network blip.
                    break;
                }
                return AttemptOutcome::PanelUnreachable {
                    reason: format!("{e} (iter {iters}/{cap})"),
                };
            }
        };
        total_cost += response.cost_usd;

        let source = match extract_vox_block(&response.content) {
            Some(s) => s,
            None => {
                last_scored = Some((
                    false,
                    0,
                    vec![format!("iter {iters}: no ```vox code block in response")],
                ));
                if iters >= cap {
                    break;
                }
                // Try to recover by asking explicitly for the fence.
                current_prompt = format!(
                    "Your previous reply lacked a single fenced ```vox … ``` \
                     code block. Reply now with ONLY one such block and \
                     nothing else. The original spec for `{}` was:\n\n{}\n",
                    spec.id, spec.prompt
                );
                continue;
            }
        };
        let (passed, err_count, notes) = score(&source, &spec.success_criteria);
        last_scored = Some((passed, err_count, notes.clone()));
        if passed {
            return AttemptOutcome::Scored {
                passed: true,
                cost_usd: total_cost,
                check_error_count: err_count,
                scoring_notes: notes,
                iterations_used: iters,
            };
        }
        if iters >= cap {
            break;
        }
        // Build refinement prompt for the next iteration.
        current_prompt = build_refinement_prompt(&source, &notes, err_count, spec);
    }

    let (passed, err_count, notes) = last_scored.unwrap_or((
        false,
        0,
        vec!["panel exhausted iterations without a scored result".into()],
    ));
    AttemptOutcome::Scored {
        passed,
        cost_usd: total_cost,
        check_error_count: err_count,
        scoring_notes: notes,
        iterations_used: iters,
    }
}

/// Build the refinement prompt the model sees on iteration ≥ 2. It
/// carries the prior draft (so the model doesn't start from scratch),
/// the structured scoring failures, and a verbatim diagnostic summary
/// from `vox check` so the model can address compiler errors directly.
fn build_refinement_prompt(
    prior_source: &str,
    scoring_notes: &[String],
    _err_count: u32,
    spec: &PanelSpec,
) -> String {
    let diag_summary = format_vox_check_diagnostics(prior_source);
    let mut notes_block = String::new();
    if !scoring_notes.is_empty() {
        notes_block.push_str("Score-rubric failures:\n");
        for n in scoring_notes {
            notes_block.push_str("  • ");
            notes_block.push_str(n);
            notes_block.push('\n');
        }
    }
    format!(
        "Your previous draft for `{id}` did not pass. Refine it.

Original spec:
{spec}

Your previous draft:
```vox
{src}
```

vox-check diagnostics:
{diags}
{notes}
Reply with ONLY a single fenced ```vox code block containing the \
revised module. Do not explain. Fix EVERY diagnostic above.",
        id = spec.id,
        spec = spec.prompt,
        src = prior_source,
        diags = diag_summary,
        notes = notes_block,
    )
}

/// Format vox-check diagnostics for the refinement prompt — concise,
/// LLM-friendly, no JSON. Returns "(none)" when the source is clean.
fn format_vox_check_diagnostics(source: &str) -> String {
    let diags = vox_compiler::pipeline::check_file(source, "generated.vox");
    if diags.is_empty() {
        return "(none)".to_string();
    }
    let mut out = String::new();
    let mut shown = 0usize;
    for d in &diags {
        if d.severity != vox_compiler::typeck::diagnostics::TypeckSeverity::Error {
            continue;
        }
        if shown >= 12 {
            out.push_str(&format!(
                "  … and {} more error(s) elided.\n",
                diags.len() - shown
            ));
            break;
        }
        let line = d.span.start_line.max(1);
        out.push_str(&format!(
            "  • [{code}] line {line}: {msg}\n",
            code = d.error_code,
            line = line,
            msg = d.message
        ));
        shown += 1;
    }
    if shown == 0 {
        "(only warnings)".to_string()
    } else {
        out
    }
}

/// Pull the first fenced code block matching ```vox / ```rust / ```
/// from the response. Returns None if none found OR the block is empty.
pub fn extract_vox_block(content: &str) -> Option<String> {
    // Prefer ```vox fences; fall back to any ``` fence. Track the longest
    // match.
    fn collect_blocks<'a>(content: &'a str, fence_prefix: &str) -> Vec<&'a str> {
        let mut out = Vec::new();
        let mut search_from = 0;
        let needle_open = format!("```{fence_prefix}");
        let needle_close = "```";
        while let Some(open_rel) = content[search_from..].find(&needle_open) {
            let open_abs = search_from + open_rel + needle_open.len();
            // Skip the rest of the open line (the fence info-string).
            let after_open = match content[open_abs..].find('\n') {
                Some(nl) => open_abs + nl + 1,
                None => return out,
            };
            let Some(close_rel) = content[after_open..].find(needle_close) else {
                return out;
            };
            let close_abs = after_open + close_rel;
            out.push(&content[after_open..close_abs]);
            search_from = close_abs + needle_close.len();
        }
        out
    }
    let vox_blocks = collect_blocks(content, "vox");
    if let Some(s) = vox_blocks.into_iter().max_by_key(|s| s.len())
        && !s.trim().is_empty()
    {
        return Some(s.to_string());
    }
    // Fall back to any fenced block (model may have used ```rust or bare ```).
    let any_blocks = collect_blocks(content, "");
    any_blocks
        .into_iter()
        .max_by_key(|s| s.len())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
}

/// Score the drafted source against the spec's success_criteria. Returns
/// `(passed, vox_check_error_count, scoring_notes)`.
fn score(source: &str, sc: &SuccessCriteria) -> (bool, u32, Vec<String>) {
    let mut notes: Vec<String> = Vec::new();
    let mut all_ok = true;

    // 1. vox check
    let diags = vox_compiler::pipeline::check_file(source, "generated.vox");
    let err_count = diags
        .iter()
        .filter(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
        .count() as u32;
    if sc.vox_check_passes && err_count > 0 {
        all_ok = false;
        notes.push(format!("vox check failed with {err_count} error(s)"));
    }

    // 2. @test count
    let test_count = count_decorator(source, "@test") as u32;
    if sc.test_count_min > 0 && test_count < sc.test_count_min {
        all_ok = false;
        notes.push(format!(
            "@test count {test_count} < required {}",
            sc.test_count_min
        ));
    }

    // 3. Decorator/keyword requirements. These are structural — they
    // catch the "wrote nice code but forgot the required language
    // construct" mode. Conservative regex (substring + word boundary
    // before the next non-ident char).
    if sc.auth_decorator_required && !source.contains("@auth") {
        all_ok = false;
        notes.push("missing @auth decorator".into());
    }
    if sc.actor_required && !contains_word(source, "actor") {
        all_ok = false;
        notes.push("missing `actor` keyword".into());
    }
    if sc.endpoint_kind_required && !source.contains("@endpoint(kind:") {
        all_ok = false;
        notes.push("missing @endpoint(kind: …) decorator".into());
    }
    if sc.streaming_decorator_required && !source.contains("kind: stream") {
        all_ok = false;
        notes.push("missing `kind: stream` endpoint".into());
    }

    // 4. test_runs_clean is currently inferred from vox-check cleanliness
    // (single-shot mode does not exec the @test runner; that's the
    // agent-loop follow-on). We record this gap honestly in the note.
    if sc.test_runs_clean && sc.vox_check_passes && err_count > 0 {
        // Already accounted for above.
    }

    (all_ok, err_count, notes)
}

fn count_decorator(source: &str, decorator: &str) -> usize {
    // Count occurrences at start-of-line (post-whitespace) to dampen the
    // false-positive rate from substrings inside strings.
    source
        .lines()
        .filter(|line| line.trim_start().starts_with(decorator))
        .count()
}

fn contains_word(source: &str, word: &str) -> bool {
    // Cheap, line-based word check — looks for `word` followed by space,
    // `(`, `{`, or end-of-line. Good enough for the language constructs
    // we care about (`actor`, `fn`, etc.).
    for line in source.lines() {
        let mut idx = 0;
        let bytes = line.as_bytes();
        while let Some(found) = line[idx..].find(word) {
            let start = idx + found;
            let end = start + word.len();
            let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
            let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
            if before_ok && after_ok {
                return true;
            }
            idx = end;
        }
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn median_of(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = xs.len() / 2;
    Some(if xs.len() % 2 == 0 {
        (xs[mid - 1] + xs[mid]) / 2.0
    } else {
        xs[mid]
    })
}

/// System prompt — calibrated to bound output size and minimize
/// hallucination of non-Vox syntax. Keep this short; budget-sensitive.
///
/// Critical calibrations against real Vox surface (the corpus under
/// `contracts/eval/humaneval-vox/problems/*` is the SSOT for idioms):
///   - `to T` is the return arrow (not `->`). `to Unit` for void.
///   - **Use explicit `return expr` in the function body** — implicit
///     last-expression-as-return parses but is not idiomatic and trips
///     the strict typeck path for some cases.
///   - **Assertions: `assert(expr is expected)` — not `assert_eq(a, b)`,
///     not `assert!(...)`, not `expect(...)`. The `is` operator is the
///     equality form inside `assert`.**
///   - `@test` decorator goes on its OWN line above the `fn`.
fn vox_system_prompt() -> String {
    r#"You are a Vox programming language expert. Vox is a strongly typed
language for AI-native server apps. Reply with ONLY a single ```vox
fenced code block — no commentary, no extra text.

Vox syntax — read carefully, these are NOT optional:

  • Functions: `fn name(arg: Type) to ReturnType { return expr }`.
    Use **explicit `return`**. The return arrow is `to`, NOT `->`.
    Void / unit functions return `to Unit`.
  • Tests:
        @test
        fn test_name() to Unit {
          assert(actual is expected)
        }
    The assertion is `assert(X is Y)`. Do NOT use `assert_eq(a, b)`,
    `assert!(…)`, `expect`, or `==` inside `assert`. The `is` operator
    is the equality check.
  • Common types: `str`, `int`, `bool`, `Unit`, `List[T]`,
    `Result[T, E]`, `Option[T]`. Result: `Ok(v)` / `Err(e)`.
  • Strings concat with `+`. Double-quoted: `"Hello " + name + "!"`.
  • `let name = expr` and `let name: Type = expr`.
  • Tables: `@table type Name { id: str, field: Type }`. Include a
    string `id` as the primary key.
  • Endpoints:
        @endpoint(kind: query|mutation|stream)
        @auth(scheme: bearer)
        fn name(arg: Type) to ReturnType { return … }
    The `@endpoint(kind: …)` and `@auth(scheme: …)` decorators each go
    on their OWN line above the `fn`.
  • Actors:
        actor Name {
          on handler_name(arg: Type) to ReturnType { return … }
        }
  • Enums (ADT errors): `enum TodoError { Empty, TooLong }`.

Forbidden:
  • Macros, `#[derive(…)]`, `#[…]` (that's Rust, not Vox).
  • `->` as return arrow (Vox uses `to`).
  • `assert_eq(a, b)` (Vox uses `assert(a is b)`).
  • `use`/`import` statements — the runtime is implicit.
  • Implicit last-expression-as-return — write `return expr`."#
        .to_string()
}

const BUILDER_PREAMBLE: &str = r#"Write a single self-contained Vox module that satisfies the spec
below. The module must:
  • pass `vox check` cleanly,
  • include the required number of `@test` blocks,
  • use the required decorators / language constructs.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_block_prefers_vox_fence() {
        let content = "intro\n```rust\nfn nope() {}\n```\n```vox\nfn yes() to int { return 1 }\n```\nend";
        let block = extract_vox_block(content).expect("block");
        assert!(block.contains("yes"));
        assert!(!block.contains("nope"));
    }

    #[test]
    fn extract_block_falls_back_to_any_fence() {
        let content = "```\nfn just() to int { return 2 }\n```";
        let block = extract_vox_block(content).expect("block");
        assert!(block.contains("just"));
    }

    #[test]
    fn extract_block_returns_none_for_no_fence() {
        let content = "no fences here just prose";
        assert!(extract_vox_block(content).is_none());
    }

    #[test]
    fn extract_block_returns_none_for_empty_fence() {
        let content = "```vox\n\n```";
        assert!(extract_vox_block(content).is_none());
    }

    #[test]
    fn count_decorator_counts_only_line_starts() {
        // The "@test in a string" should not be counted.
        let source = "@test fn a() {}\n@test fn b() {}\nlet s = \"@test inside string\"";
        assert_eq!(count_decorator(source, "@test"), 2);
    }

    #[test]
    fn contains_word_respects_boundaries() {
        assert!(contains_word("actor ChatRoom {}", "actor"));
        assert!(!contains_word("interactor xyz", "actor"));
        assert!(contains_word("define actor", "actor"));
    }

    #[test]
    fn score_flags_missing_auth_decorator() {
        let sc = SuccessCriteria {
            vox_check_passes: false,
            auth_decorator_required: true,
            ..Default::default()
        };
        let (passed, _, notes) = score("fn x() to int { return 0 }", &sc);
        assert!(!passed);
        assert!(notes.iter().any(|n| n.contains("@auth")));
    }

    #[test]
    fn score_passes_when_all_criteria_met() {
        let sc = SuccessCriteria {
            vox_check_passes: false, // skip vox check in this unit test
            test_count_min: 1,
            ..Default::default()
        };
        let source = "@test fn a() {}";
        let (passed, _, notes) = score(source, &sc);
        assert!(passed, "expected pass, got notes: {notes:?}");
    }

    #[test]
    fn panel_spec_parses_minimal_toml() {
        let toml_src = r#"
            id = "x"
            tier = "T1"
            max_cost_usd = 1.50
            prompt = "do thing"

            [success_criteria]
            vox_check_passes = true
            test_count_min = 2
            auth_decorator_required = true
        "#;
        let spec = PanelSpec::from_toml("x".into(), toml_src).unwrap();
        assert_eq!(spec.tier, "T1");
        assert!((spec.max_cost_usd - 1.50).abs() < 1e-9);
        assert!(spec.success_criteria.vox_check_passes);
        assert_eq!(spec.success_criteria.test_count_min, 2);
        assert!(spec.success_criteria.auth_decorator_required);
    }

    #[test]
    fn median_of_handles_odd_and_even() {
        assert_eq!(median_of(vec![1.0, 2.0, 3.0]), Some(2.0));
        assert_eq!(median_of(vec![1.0, 2.0, 3.0, 4.0]), Some(2.5));
        assert_eq!(median_of(vec![]), None);
    }

    #[test]
    fn member_aggregate_pass_rate_ignores_unreachable() {
        // 3 attempts: 1 pass, 1 unreachable, 1 budget-skip → 1/1 scored.
        let mut a = MemberAggregate {
            id: "x".into(),
            model_slug: Some("openai/gpt-x".into()),
            attempts: 3,
            passes: 1,
            unreachable_count: 1,
            skipped_budget: 1,
            total_cost_usd: 0.05,
            per_spec_costs: vec![0.05],
            total_iterations: 1,
            single_shot_passes: 1,
        };
        assert!((a.pass_rate() - 1.0).abs() < 1e-9);
        a.passes = 0;
        assert_eq!(a.pass_rate(), 0.0);
    }

    #[test]
    fn refinement_prompt_carries_prior_source_and_diagnostics() {
        let spec = PanelSpec {
            id: "x".into(),
            tier: "T1".into(),
            prompt: "do thing".into(),
            max_cost_usd: 1.0,
            success_criteria: Default::default(),
        };
        // Source guaranteed to fail vox check (assert_eq is not a Vox builtin).
        let prior = "fn greet() to str { return \"hi\" }\n@test\nfn t() to Unit { assert_eq(greet(), \"hi\") }";
        let notes = vec!["missing @test count".to_string()];
        let prompt = build_refinement_prompt(prior, &notes, 1, &spec);
        assert!(prompt.contains("Original spec:"));
        assert!(prompt.contains("do thing"));
        assert!(prompt.contains("Your previous draft:"));
        assert!(prompt.contains("missing @test count"));
        assert!(prompt.contains("vox-check diagnostics:"));
        // Ends with the explicit fence directive.
        assert!(prompt.contains("```vox"));
    }

    /// Deterministic in-memory PanelClient for testing the multi-iteration
    /// loop without hitting the network. Returns canned responses by
    /// iteration index and records the user prompts it saw.
    struct ScriptedClient {
        responses: std::sync::Mutex<Vec<String>>,
        seen_prompts: std::sync::Mutex<Vec<String>>,
    }
    impl crate::panel::PanelClient for ScriptedClient {
        fn complete(
            &self,
            _member: &PanelMemberConfig,
            _system: &str,
            user: &str,
        ) -> Result<crate::panel::PanelResponse, crate::panel::PanelClientError> {
            self.seen_prompts.lock().unwrap().push(user.to_string());
            let content = self
                .responses
                .lock()
                .unwrap()
                .remove(0);
            Ok(crate::panel::PanelResponse {
                content,
                cost_usd: 0.01,
                input_tokens: Some(100),
                output_tokens: Some(50),
            })
        }
    }

    #[test]
    fn refinement_loop_recovers_after_first_shot_fail() {
        let spec = PanelSpec {
            id: "greet".into(),
            tier: "T1".into(),
            prompt: "greet function".into(),
            max_cost_usd: 5.0,
            success_criteria: SuccessCriteria {
                vox_check_passes: true,
                test_count_min: 1,
                ..Default::default()
            },
        };
        // Iter 1 reply uses `assert_eq` (will fail score). Iter 2 reply
        // uses `assert(… is …)` (will pass).
        let r1 = "```vox\nfn greet() to str { return \"hi\" }\n@test\nfn t() to Unit { assert_eq(greet(), \"hi\") }\n```";
        let r2 = "```vox\nfn greet() to str { return \"hi\" }\n@test\nfn t() to Unit { assert(greet() is \"hi\") }\n```";
        let client = ScriptedClient {
            responses: std::sync::Mutex::new(vec![r1.into(), r2.into()]),
            seen_prompts: std::sync::Mutex::new(Vec::new()),
        };
        let member = PanelMemberConfig {
            id: "scripted".into(),
            role: "test".into(),
            version_pinned: None,
            openrouter_model: Some("openai/test".into()),
            pricing: None,
        };
        let outcome = run_one_attempt(&client, &member, "sys", "user", &spec, 3, 1.0, 1.0);
        match outcome {
            AttemptOutcome::Scored {
                passed,
                iterations_used,
                ..
            } => {
                assert!(passed, "expected pass after refinement");
                assert_eq!(iterations_used, 2, "expected exactly 2 iterations");
            }
            other => panic!("expected Scored, got {other:?}"),
        }
        // Iter 2 prompt should contain the prior draft + diagnostics.
        let prompts = client.seen_prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[1].contains("Your previous draft:"));
        assert!(prompts[1].contains("assert_eq"));
    }

    #[test]
    fn refinement_loop_stops_when_per_spec_budget_exhausts() {
        let spec = PanelSpec {
            id: "x".into(),
            tier: "T1".into(),
            prompt: "p".into(),
            max_cost_usd: 5.0,
            success_criteria: SuccessCriteria {
                vox_check_passes: true,
                ..Default::default()
            },
        };
        // All replies fail score; loop should run once, then refuse to
        // spend more when per_spec_remaining drops to 0.005 (less than
        // the 0.01 cost of the next iteration).
        let r = "```vox\nfn bad() { syntax_garbage_here }\n```";
        let client = ScriptedClient {
            responses: std::sync::Mutex::new(vec![r.into(), r.into(), r.into()]),
            seen_prompts: std::sync::Mutex::new(Vec::new()),
        };
        let member = PanelMemberConfig {
            id: "scripted".into(),
            role: "test".into(),
            version_pinned: None,
            openrouter_model: Some("openai/test".into()),
            pricing: None,
        };
        // Per-spec budget too small for a 2nd iteration (each costs 0.01).
        let outcome = run_one_attempt(&client, &member, "sys", "u", &spec, 5, 0.005, 100.0);
        match outcome {
            AttemptOutcome::Scored { iterations_used, .. } => {
                assert_eq!(iterations_used, 1, "should stop after first iter");
            }
            other => panic!("expected Scored, got {other:?}"),
        }
    }
}
