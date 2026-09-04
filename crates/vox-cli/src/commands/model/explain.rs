use clap::Parser;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::models::{
    MIN_CALLS_FOR_CONFIDENT_RANK, ModelRegistry, ModelScore, ModelSpec, pareto_frontier,
    pareto_point_for,
};
use vox_orchestrator::types::TaskCategory;

/// Explain model selection for a given task description.
#[derive(Parser)]
pub struct ExplainArgs {
    /// Task description or prompt.
    pub task: String,
    /// Explicit task category (optional).
    #[arg(long)]
    pub category: Option<String>,
    /// Estimated complexity (1-10).
    #[arg(long, default_value_t = 5)]
    pub complexity: u8,
    /// Also show the free-tier router's ranked candidates with the human-readable
    /// rationale for each (the same rationale the MCP selection path emits).
    #[arg(long)]
    pub free_tier: bool,
    /// (with --free-tier) Prioritize ultra-low-latency free models.
    #[arg(long)]
    pub latency_critical: bool,
    /// (with --free-tier) Request fill-in-the-middle-capable free models.
    #[arg(long)]
    pub fim: bool,
}

/// Task M3: renders a model's success rate with its 95% Wilson credible interval, and a
/// low-confidence marker below [`vox_orchestrator::models::MIN_CALLS_FOR_CONFIDENT_RANK`]
/// calls — a bare "Success: 100%" off 2 calls reads as far more certain than it is.
fn success_rate_display(success_count: i64, n_calls: i64) -> String {
    let Some((lo, hi)) = vox_orchestrator::models::wilson_score_interval(success_count, n_calls)
    else {
        return "Success: no data yet".to_string();
    };
    let point = if n_calls > 0 {
        success_count as f64 / n_calls as f64
    } else {
        0.0
    };
    if n_calls < vox_orchestrator::models::MIN_CALLS_FOR_CONFIDENT_RANK {
        format!(
            "Success: {:.0}% (low confidence, n={n_calls}, 95% CI {:.0}-{:.0}%)",
            point * 100.0,
            lo * 100.0,
            hi * 100.0
        )
    } else {
        format!(
            "Success: {:.1}% (95% CI {:.0}-{:.0}%, n={n_calls})",
            point * 100.0,
            lo * 100.0,
            hi * 100.0
        )
    }
}

/// Pure helper: rank free models via the FreeTierRouter and return each
/// candidate's id paired with its rationale. No I/O — unit-testable.
fn render_free_tier(
    req: &vox_research_shim::selection::FreeTierRouteRequest,
    models: &[vox_orchestrator::models::ModelSpec],
) -> Vec<(String, &'static str)> {
    vox_research_shim::selection::FreeTierRouter::new()
        .route(req, models)
        .into_iter()
        .map(|c| (c.model.id, c.rationale))
        .collect()
}

/// Task M3 ("suppress ranks below minimum-N"): splits candidates into those with enough
/// observations to hold a rank position and those without. `e497a82fb` only *marked* low-N
/// rows, so a 2-call model still printed as rank #1 — the false confidence the requirement
/// targets. `None` (no scoreboard row) is unranked: no data is not evidence of rank-worthiness.
fn partition_by_rank_confidence(
    candidates: &[ModelSpec],
    n_calls_of: impl Fn(&str) -> Option<i64>,
) -> (Vec<&ModelSpec>, Vec<&ModelSpec>) {
    candidates
        .iter()
        .partition(|m| n_calls_of(&m.id).is_some_and(|n| n >= MIN_CALLS_FOR_CONFIDENT_RANK))
}

/// Annotation for the `Selection:` line when the router's pick is not rankable. The pick itself
/// is never changed — printing the listed leader as "Selection" would fabricate a routing claim.
///
/// The note must not claim the list is ordered by measurement: `render_candidate_sections`
/// renders an order-preserving filter of `explain_selection`, which sorts by `auto_score_model`'s
/// composite priority (tier, cost, strength and scoreboard signal combined), not by observed
/// performance.
fn selection_note(n_calls: i64) -> String {
    if n_calls >= MIN_CALLS_FOR_CONFIDENT_RANK {
        String::new()
    } else {
        format!(
            " (unranked: {n_calls} observed call(s) < {MIN_CALLS_FOR_CONFIDENT_RANK}; the list \
             above is ordered by the router's composite priority score, not by observed \
             performance)"
        )
    }
}

/// Pure render of both candidate sections, extracted so the partition *and* its presentation
/// are testable — `run()` is async and touches the DB and env, so it never can be.
fn render_candidate_sections(
    ranked: &[&ModelSpec],
    unranked: &[&ModelSpec],
    score_of: impl Fn(&str) -> Option<ModelScore>,
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    // Frontier over the ranked half only: `partition_by_rank_confidence` already excluded
    // unobserved rows, so this is confidence-gated by construction.
    //
    // Deliberately computed over ALL ranked candidates, not only the five printed below. A
    // displayed row can therefore lack the mark because something outside the top five
    // dominates it. That is the honest direction to err: scoring only the printed five would
    // hand out marks a wider view contradicts, and the legend claims "no other row", not
    // "no other row shown".
    let points: Vec<_> = ranked
        .iter()
        .map(|m| pareto_point_for(score_of(&m.id).as_ref()))
        .collect();
    let frontier = pareto_frontier(&points);

    let _ = writeln!(
        out,
        "Top Candidates (sorted by the router's composite priority score; models with < \
         {MIN_CALLS_FOR_CONFIDENT_RANK} observed calls are listed separately):"
    );
    for (i, entry) in ranked.iter().take(5).enumerate() {
        let prefix = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        let mut details = vec![format!("Tier: {:?}", entry.capabilities.tier)];
        if let Some(score) = score_of(&entry.id) {
            details.push(success_rate_display(score.success_count, score.n_calls));
        }
        let mark = if frontier.contains(&i) {
            " [pareto-optimal]"
        } else {
            ""
        };
        let _ = writeln!(out, "{prefix} {}: {}{mark}", entry.id, details.join(", "));
    }
    if ranked.is_empty() {
        let _ = writeln!(out, "  (no model has enough observations to rank yet)");
    } else {
        // The `[pareto-optimal]` mark needs the same disclosures the scoreboard's `*` needs:
        // which axes, that the ranking axis is the Wilson lower bound rather than the raw rate
        // printed beside it, and that "success" is a non-error provider response. Reused
        // verbatim so the two surfaces cannot drift into explaining the same mark differently.
        let _ = writeln!(
            out,
            "\n{}",
            crate::commands::model::scoreboard::pareto_legend()
        );
        // Honesty about the slice these figures come from: `run()` injects the scoreboard via
        // `ModelRegistry::inject_scoreboard`, whose registry-side API is keyed by `model_id`
        // alone, so each model's counts are whichever (task_category, strength_tag) row landed
        // last. `vox model scoreboard` renders the triples separately.
        let _ = writeln!(
            out,
            "Per-model figures above are one arbitrary (task_category, strength_tag) scoreboard \
             slice, not a per-model total — see `vox model scoreboard` for the per-triple rows."
        );
    }

    if !unranked.is_empty() {
        let _ = writeln!(
            out,
            "\nInsufficient data to rank ({} model(s)):",
            unranked.len()
        );
        for entry in unranked.iter().take(5) {
            let n = score_of(&entry.id).map_or(0, |s| s.n_calls);
            let _ = writeln!(out, "  - {}: {n} observed call(s)", entry.id);
        }
    }
    out
}

pub async fn run(args: ExplainArgs) -> anyhow::Result<()> {
    // 1. Setup Registry
    let mut registry = ModelRegistry::new();

    // 2. Load Scoreboard from DB
    let db_config = DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?;
    let db = VoxDb::connect(db_config).await?;
    let db_scores = db.get_model_scoreboard(7).await?;

    let mut scores = HashMap::new();
    for row in db_scores {
        scores.insert(row.model_id.clone(), ModelScore::from(row));
    }
    registry.inject_scoreboard(scores);

    // 3. Construct simulation parameters
    let category = if let Some(cat_str) = args.category {
        use std::str::FromStr;
        TaskCategory::from_str(&cat_str).unwrap_or(TaskCategory::General)
    } else {
        TaskCategory::General
    };

    let complexity = args.complexity;
    let description = args.task;

    // 4. Run Selection Explain
    println!(
        "{} Model Selection for task: \"{}\"",
        " EXPLAIN ".on_blue().white().bold(),
        description.italic()
    );
    println!("Category: {:?}, Complexity: {}", category, complexity);
    let snap =
        vox_actor_runtime::route_capability_policy::RouteCapabilityPolicySnapshot::from_env();
    println!(
        "Route policy profile: {} (net={}, provider_net={}, local_http={})",
        snap.profile, snap.allow_net, snap.allow_provider_network, snap.allow_local_model_http
    );
    let exclusions = registry.explain_route_policy_exclusions();
    if !exclusions.is_empty() {
        println!("{}", " Policy exclusions (VOX_ROUTE_*):".yellow().bold());
        for (id, reason) in exclusions.iter().take(25) {
            println!("  - {}: {}", id.dimmed(), reason);
        }
        if exclusions.len() > 25 {
            println!("  … {} more", exclusions.len() - 25);
        }
    }
    println!("---");

    let strength = vox_orchestrator::models::task_category_strength(category);
    let candidates = registry.explain_selection(
        category,
        strength,
        complexity,
        vox_orchestrator::config::CostPreference::Performance,
    );

    if candidates.is_empty() {
        println!("{}", "❌ No suitable models found in registry.".red());
        return Ok(());
    }

    let (ranked, unranked) =
        partition_by_rank_confidence(&candidates, |id| registry.get_score(id).map(|s| s.n_calls));
    println!(
        "{} {}",
        " RANK ".on_green().black().bold(),
        render_candidate_sections(&ranked, &unranked, |id| registry.get_score(id).cloned())
    );

    let selected_calls = registry
        .get_score(&candidates[0].id)
        .map_or(0, |s| s.n_calls);
    println!(
        "\nSelection: {}{}",
        candidates[0].id.green().bold(),
        selection_note(selected_calls).yellow()
    );

    // 4b. Free-tier router rationale (opt-in) — surfaces WHY each free model is
    // chosen, using the same FreeTierRouter the MCP selection path uses.
    if args.free_tier {
        let req = vox_research_shim::selection::FreeTierRouteRequest {
            task: category,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: args.fim,
            latency_critical: args.latency_critical,
            max_candidates: 5,
        };
        let ranked = render_free_tier(&req, &registry.free_models());
        println!(
            "\n{} Free-tier candidates (with rationale):",
            " FREE ".on_cyan().black().bold()
        );
        if ranked.is_empty() {
            println!("  (no free models satisfy the request)");
        } else {
            for (id, rationale) in &ranked {
                println!("  - {}: {}", id.bold(), rationale.dimmed());
            }
        }
    }

    // 5. Show most recent trace ID
    if let Ok(Some(tid)) = db
        .get_last_interaction_trace_id(&category.to_string())
        .await
    {
        println!("Recent Trace ID: {}", tid.dimmed());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MIN_CALLS_FOR_CONFIDENT_RANK, partition_by_rank_confidence, render_candidate_sections,
        render_free_tier, selection_note, success_rate_display,
    };
    use vox_orchestrator::models::{
        ModelCapabilities, ModelScore, ModelSpec, ModelTier, PricingSource, ProviderType,
        StrengthTag,
    };
    use vox_orchestrator::types::TaskCategory;
    use vox_research_shim::selection::FreeTierRouteRequest;

    #[test]
    fn success_rate_display_flags_low_confidence_below_the_threshold() {
        let s = success_rate_display(2, 2);
        assert!(s.contains("low confidence"), "{s}");
        assert!(s.contains("100%"), "{s}");
    }

    #[test]
    fn success_rate_display_omits_low_confidence_marker_at_and_above_the_threshold() {
        let s = success_rate_display(18, 20);
        assert!(!s.contains("low confidence"), "{s}");
        assert!(s.contains("90.0%"), "{s}");
    }

    #[test]
    fn success_rate_display_handles_no_data() {
        assert_eq!(success_rate_display(0, 0), "Success: no data yet");
    }

    fn free_spec(id: &str, provider_type: ProviderType, tier: ModelTier) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: String::new(),
            provider: "test".to_string(),
            provider_type,
            max_tokens: 32_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::OpenRouter,
            is_free: true,
            strengths: vec![StrengthTag::Generalist],
            capabilities: ModelCapabilities {
                max_context: 32_000,
                tier,
                ..Default::default()
            },
            supported_parameters: vec![],
        }
    }

    #[test]
    fn render_free_tier_returns_ids_with_nonempty_rationale() {
        let models = vec![
            free_spec("groq/fast", ProviderType::Groq, ModelTier::Fast),
            free_spec("or/pro", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let req = FreeTierRouteRequest {
            task: TaskCategory::CodeGen,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: false,
            latency_critical: true,
            max_candidates: 5,
        };
        let ranked = render_free_tier(&req, &models);
        assert!(!ranked.is_empty(), "free models should be ranked");
        for (id, rationale) in &ranked {
            assert!(
                !rationale.is_empty(),
                "every candidate must carry a rationale ({id})"
            );
        }
        // The Fast-tier model under latency_critical earns the low-latency rationale.
        let fast = ranked
            .iter()
            .find(|(id, _)| id == "groq/fast")
            .expect("fast present");
        assert_eq!(fast.1, "Candidate selected for ultra-low latency");
    }

    #[test]
    fn render_free_tier_empty_when_no_free_models() {
        let req = FreeTierRouteRequest {
            task: TaskCategory::CodeGen,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: false,
            latency_critical: false,
            max_candidates: 5,
        };
        assert!(render_free_tier(&req, &[]).is_empty());
    }

    #[test]
    fn partition_by_rank_confidence_demotes_low_call_models_out_of_the_ranked_list() {
        let models = vec![
            free_spec("a/low-n", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("b/confident", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |id| match id {
            "a/low-n" => Some(2),
            "b/confident" => Some(40),
            _ => None,
        });
        assert_eq!(
            ranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["b/confident"]
        );
        assert_eq!(
            unranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["a/low-n"]
        );
    }

    #[test]
    fn partition_by_rank_confidence_treats_a_missing_scoreboard_row_as_unranked() {
        let models = vec![free_spec(
            "novel/model",
            ProviderType::OpenRouter,
            ModelTier::Pro,
        )];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |_| None);
        assert!(
            ranked.is_empty(),
            "no data is not evidence of rank-worthiness"
        );
        assert_eq!(unranked.len(), 1);
    }

    #[test]
    fn partition_by_rank_confidence_admits_exactly_at_the_threshold() {
        // Uses the symbolic constant: a hardcoded `n >= 5` that drifts when the constant changes
        // would still pass a literal-valued test.
        let models = vec![free_spec(
            "edge/model",
            ProviderType::OpenRouter,
            ModelTier::Pro,
        )];
        let (ranked, _) =
            partition_by_rank_confidence(&models, |_| Some(MIN_CALLS_FOR_CONFIDENT_RANK));
        assert_eq!(ranked.len(), 1, "the threshold is inclusive");
        let (ranked, _) =
            partition_by_rank_confidence(&models, |_| Some(MIN_CALLS_FOR_CONFIDENT_RANK - 1));
        assert!(
            ranked.is_empty(),
            "one call below the threshold is not rankable"
        );
    }

    #[test]
    fn partition_by_rank_confidence_preserves_input_order_within_each_half() {
        let models = vec![
            free_spec("a/hi", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("b/lo", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("c/hi", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("d/lo", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |id| {
            if id.ends_with("/hi") {
                Some(50)
            } else {
                Some(1)
            }
        });
        assert_eq!(
            ranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["a/hi", "c/hi"]
        );
        assert_eq!(
            unranked.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["b/lo", "d/lo"]
        );
    }

    #[test]
    fn partition_by_rank_confidence_handles_an_empty_candidate_list() {
        let (ranked, unranked) = partition_by_rank_confidence(&[], |_| Some(99));
        assert!(ranked.is_empty() && unranked.is_empty());
    }

    #[test]
    fn render_candidate_sections_never_puts_a_medal_on_an_unranked_model() {
        // The defect e497a82fb shipped: present-but-marked is not suppression.
        let models = vec![
            free_spec("a/low-n", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("b/confident", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |id| {
            if id == "b/confident" {
                Some(40)
            } else {
                Some(2)
            }
        });
        let out = render_candidate_sections(&ranked, &unranked, |_| None::<ModelScore>);
        let medal_line = out
            .lines()
            .find(|l| l.contains('🥇'))
            .expect("a medal is rendered");
        assert!(
            medal_line.contains("b/confident"),
            "medal must go to the ranked model: {medal_line}"
        );
        assert!(
            out.contains("a/low-n"),
            "the demoted model must still be listed"
        );
        let low_n_line = out.lines().find(|l| l.contains("a/low-n")).expect("listed");
        assert!(
            !low_n_line.contains('🥇') && !low_n_line.contains('🥈') && !low_n_line.contains('🥉'),
            "a 2-call model must hold no rank position: {low_n_line}"
        );
    }

    #[test]
    fn render_candidate_sections_says_so_when_nothing_is_rankable() {
        let models = vec![free_spec("x/new", ProviderType::OpenRouter, ModelTier::Pro)];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |_| None);
        let out = render_candidate_sections(&ranked, &unranked, |_| None::<ModelScore>);
        assert!(out.contains("no model has enough observations"), "{out}");
        assert!(
            !out.contains('🥇'),
            "an empty ranked half renders no medals: {out}"
        );
    }

    fn model_score(success_count: i64, n_calls: i64, cost: f64, p50: i64) -> ModelScore {
        ModelScore {
            success_count,
            n_calls,
            cost_per_success_usd: Some(cost),
            p50_latency_ms: Some(p50),
            ..ModelScore::default()
        }
    }

    /// Renders both candidates as ranked, with real scores on every axis — the state `run()`
    /// actually produces. The pre-existing tests pass `|_| None`, which makes every point
    /// all-unknown, hence incomparable, hence trivially marked.
    fn marks_with_scores(a: ModelScore, b: ModelScore) -> (bool, bool) {
        let models = vec![
            free_spec("a/x", ProviderType::OpenRouter, ModelTier::Pro),
            free_spec("b/y", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |_| Some(100));
        assert!(unranked.is_empty(), "both candidates must be rankable");
        let out = render_candidate_sections(&ranked, &unranked, |id| match id {
            "a/x" => Some(a.clone()),
            "b/y" => Some(b.clone()),
            _ => None,
        });
        let marked = |id: &str| {
            out.lines()
                .find(|l| l.contains(id))
                .unwrap_or_else(|| panic!("{id} listed:\n{out}"))
                .contains("[pareto-optimal]")
        };
        (marked("a/x"), marked("b/y"))
    }

    #[test]
    fn render_candidate_sections_withholds_the_mark_from_a_dominated_candidate() {
        // a/x is better on all three axes, so b/y is dominated and must lose the mark.
        let (a, b) = marks_with_scores(
            model_score(95, 100, 0.01, 100),
            model_score(50, 100, 0.10, 900),
        );
        assert!(a, "the dominating candidate keeps the mark");
        assert!(
            !b,
            "a dominated candidate must not be marked pareto-optimal"
        );
    }

    #[test]
    fn render_candidate_sections_marks_both_halves_of_a_genuine_tradeoff() {
        // a/x is more reliable, b/y is cheaper and faster: neither dominates, so both are marked.
        let (a, b) = marks_with_scores(
            model_score(95, 100, 0.10, 900),
            model_score(50, 100, 0.01, 100),
        );
        assert!(a && b, "a trade-off pair are both on the frontier");
    }

    #[test]
    fn render_candidate_sections_legend_explains_the_pareto_mark() {
        // The mark shipped with no legend (F3): the reader's only nearby number is the raw
        // success rate, which is not what the mark is computed from.
        let models = vec![free_spec("a/x", ProviderType::OpenRouter, ModelTier::Pro)];
        let (ranked, unranked) = partition_by_rank_confidence(&models, |_| Some(100));
        let out = render_candidate_sections(&ranked, &unranked, |_| {
            Some(model_score(95, 100, 0.01, 100))
        });
        assert!(out.contains("[pareto-optimal]"), "{out}");
        assert!(out.contains("Wilson lower bound"), "{out}");
        assert!(out.contains("not answer correctness"), "{out}");
        // F4: the per-model figures are one arbitrary triple slice, and must say so.
        assert!(out.contains("(task_category, strength_tag)"), "{out}");
    }

    #[test]
    fn selection_note_does_not_claim_the_list_is_ordered_by_measurement() {
        // `explain_selection` sorts by `auto_score_model`'s composite priority, not by observed
        // performance.
        let note = selection_note(2);
        assert!(note.contains("composite priority score"), "{note}");
        assert!(!note.contains("ordered by observed performance"), "{note}");
    }

    #[test]
    fn selection_note_flags_a_selection_that_is_not_rankable() {
        // The router's pick is reported as-is (changing it would fabricate a routing claim), but
        // it must not read as endorsed when it sits in the UNRANKED section.
        assert!(
            selection_note(2).contains("unranked"),
            "{}",
            selection_note(2)
        );
        assert_eq!(selection_note(MIN_CALLS_FOR_CONFIDENT_RANK), "");
    }
}
