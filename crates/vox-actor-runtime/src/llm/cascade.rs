//! Research-oriented LLM cascade helpers.

use crate::model_resolution::{RouteResolutionInput, chat_route_to_llm_config};
use crate::{ActivityOptions, ActivityResult};

use super::{LlmChatMessage, LlmConfig, LlmResponse, infer_with_retry};
use vox_telemetry::{AiFixtureEvent, PromptDispatchTelemetryEvent, TelemetryEvent};

/// Research pipeline stage requesting an LLM call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchStage {
    Planner,
    ClaimExtraction,
    Verification,
    Synthesis,
    Judge,
    SelfVerification,
}

fn research_stage_label(stage: Option<ResearchStage>) -> String {
    stage
        .map(|s| format!("{s:?}"))
        .unwrap_or_else(|| "unspecified".to_string())
}

/// Run chat completion over an explicit candidate cascade.
///
/// When `research_stage` is `Some`, emits [`TelemetryEvent::AiFixture`] prompt-dispatch telemetry.
pub async fn chat_with_cascade(
    opts: &ActivityOptions,
    messages: Vec<LlmChatMessage>,
    candidates: Vec<LlmConfig>,
    research_stage: Option<ResearchStage>,
) -> Result<LlmResponse, String> {
    if candidates.is_empty() {
        let stage_lbl = research_stage_label(research_stage);
        vox_telemetry::record_event!(&TelemetryEvent::AiFixture(AiFixtureEvent::PromptDispatch(
            PromptDispatchTelemetryEvent {
                stage: stage_lbl,
                outcome: "error".into(),
                error: Some("no LLM candidates available for research cascade".into()),
                redact_count: 0,
            }
        )));
        return Err("no LLM candidates available for research cascade".to_string());
    }

    let res = infer_with_retry(opts, messages, candidates).await;
    let stage_lbl = research_stage_label(research_stage);
    let (outcome, err) = match &res {
        ActivityResult::Ok(Ok(_)) => ("ok", None),
        ActivityResult::Ok(Err(e)) => ("error", Some(e.clone())),
        ActivityResult::Failed(e) => (
            "error",
            Some(format!("research cascade activity failed: {e:?}")),
        ),
        ActivityResult::Cancelled => ("cancelled", Some("research cascade cancelled".into())),
    };
    vox_telemetry::record_event!(&TelemetryEvent::AiFixture(AiFixtureEvent::PromptDispatch(
        PromptDispatchTelemetryEvent {
            stage: stage_lbl,
            outcome: outcome.into(),
            error: err,
            redact_count: 0,
        }
    )));

    match res {
        ActivityResult::Ok(Ok((response, _cfg))) => Ok(response),
        ActivityResult::Ok(Err(e)) => Err(e),
        ActivityResult::Failed(e) => Err(format!("research cascade activity failed: {e:?}")),
        ActivityResult::Cancelled => Err("research cascade cancelled".to_string()),
    }
}

/// Ordered, dispatchable OpenRouter model ids for a research call.
///
/// Concrete `:free` slugs from [`vox_config::OPENROUTER_FREE_FALLBACK_MODELS`] are
/// ALWAYS appended as a zero-cost fallback floor so research degrades to free instead
/// of failing. The virtual `openrouter/free` route is intentionally NOT used here: it
/// is a registry-only id that the OpenRouter API rejects when dispatched raw, so real
/// `:free` model ids are used instead. `prefer_free` moves the free slugs ahead of the
/// caller-configured model; a configured model that is already a free slug is not
/// duplicated.
#[must_use]
fn research_openrouter_model_ids(configured: &str, prefer_free: bool) -> Vec<String> {
    let free: Vec<String> = vox_config::OPENROUTER_FREE_FALLBACK_MODELS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let configured_is_free = free.iter().any(|f| f == configured);

    let mut ordered = Vec::with_capacity(free.len() + 1);
    if prefer_free {
        ordered.extend(free.iter().cloned());
        if !configured_is_free {
            ordered.push(configured.to_string());
        }
    } else {
        if !configured_is_free {
            ordered.push(configured.to_string());
        }
        ordered.extend(free.iter().cloned());
    }
    ordered
}

/// Build the default research cascade: local Mens/Ollama first, then OpenRouter.
#[must_use]
pub fn cascade_for_research_stage(
    stage: ResearchStage,
    input: &RouteResolutionInput,
) -> Vec<LlmConfig> {
    let mut candidates = Vec::new();

    if vox_config::inference::inference_profile_allows_local_ollama_http() {
        let base = vox_config::inference::local_ollama_populi_base_url();
        let mut local = chat_route_to_llm_config(
            &vox_orchestrator_types::ChatProviderRouteKind::PopuliLocal {
                base_url: base,
                model: input.mens_chat_model.clone(),
            },
        );
        apply_stage_defaults(stage, &mut local);
        candidates.push(local);
    }

    if vox_config::inference::openrouter_api_key().is_some() {
        let prefer_free = vox_config::inference::research_prefer_free_tier();
        for model_id in research_openrouter_model_ids(&input.openrouter_model, prefer_free) {
            let mut openrouter = LlmConfig::openrouter(model_id);
            apply_stage_defaults(stage, &mut openrouter);
            candidates.push(openrouter);
        }
    }

    candidates
}

/// Add a manual OpenAI-compatible candidate before the default cascade.
#[must_use]
pub fn cascade_with_optional_manual(
    stage: ResearchStage,
    input: &RouteResolutionInput,
    endpoint: Option<&str>,
    api_key: Option<&str>,
    model: Option<&str>,
) -> Vec<LlmConfig> {
    let mut candidates = Vec::new();
    if let (Some(endpoint), Some(model)) = (endpoint, model) {
        let mut manual = LlmConfig {
            provider: "openai_compatible".to_string(),
            model: model.to_string(),
            cost_per_1k: None,
            base_url: Some(format!(
                "{}/v1/chat/completions",
                endpoint.trim_end_matches('/')
            )),
            api_key: api_key.map(str::to_string),
            temperature: None,
            top_p: None,
            max_tokens: None,
            response_format: None,
            tools: None,
            tool_choice: None,
            timeout_ms: Some(30_000),
            telemetry_session_id: None,
            telemetry_user_id: None,
            telemetry_task_category: Some("research".to_string()),
            telemetry_strength_tag: Some(format!("{stage:?}").to_ascii_lowercase()),
            telemetry_trace_id: None,
            telemetry_attempt_number: None,
            telemetry_skip_interaction: false,
        };
        apply_stage_defaults(stage, &mut manual);
        candidates.push(manual);
    }
    candidates.extend(cascade_for_research_stage(stage, input));
    candidates
}

fn apply_stage_defaults(stage: ResearchStage, cfg: &mut LlmConfig) {
    cfg.telemetry_task_category = Some("research".to_string());
    cfg.telemetry_strength_tag = Some(format!("{stage:?}").to_ascii_lowercase());
    cfg.temperature = Some(match stage {
        ResearchStage::Planner => 0.2,
        ResearchStage::ClaimExtraction | ResearchStage::Judge => 0.0,
        // Nonzero so SelfCheckGPT-style resampling (see
        // `verify_claims_with_config` in vox-research-shim) produces
        // genuine variation across samples instead of near-identical
        // deterministic output.
        ResearchStage::Verification => 0.3,
        ResearchStage::Synthesis => 0.2,
        ResearchStage::SelfVerification => 0.0,
    });
    // Synthesis max_tokens is NOT set here — controlled by ResearchConfig::synthesis_max_tokens.
    if stage != ResearchStage::Synthesis {
        cfg.max_tokens = Some(match stage {
            ResearchStage::Planner => 700,
            ResearchStage::ClaimExtraction => 900,
            ResearchStage::Verification => 500,
            ResearchStage::Judge => 400,
            ResearchStage::SelfVerification => 700,
            ResearchStage::Synthesis => unreachable!("guarded by outer if"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_includes_local_candidate_when_profile_allows_it() {
        let candidates =
            cascade_for_research_stage(ResearchStage::Planner, &RouteResolutionInput::default());

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.provider == "ollama")
        );
    }

    #[test]
    fn manual_candidate_is_first_when_endpoint_and_model_are_supplied() {
        let candidates = cascade_with_optional_manual(
            ResearchStage::Verification,
            &RouteResolutionInput::default(),
            Some("http://localhost:9999"),
            None,
            Some("local-test-model"),
        );

        assert_eq!(candidates[0].provider, "openai_compatible");
        assert_eq!(candidates[0].model, "local-test-model");
        assert_eq!(
            candidates[0].base_url.as_deref(),
            Some("http://localhost:9999/v1/chat/completions")
        );
    }

    #[test]
    fn synthesis_stage_does_not_force_1800_max_tokens() {
        use crate::model_resolution::RouteResolutionInput;
        let candidates = cascade_with_optional_manual(
            ResearchStage::Synthesis,
            &RouteResolutionInput::default(),
            None,
            None,
            None,
        );
        if let Some(c) = candidates.first() {
            assert_ne!(
                c.max_tokens,
                Some(1_800),
                "Synthesis max_tokens must not be hard-coded; got {:?}",
                c.max_tokens
            );
        }
    }

    #[test]
    fn verification_stage_uses_nonzero_temperature() {
        let candidates = cascade_with_optional_manual(
            ResearchStage::Verification,
            &RouteResolutionInput::default(),
            None,
            None,
            None,
        );
        assert!(
            candidates.iter().all(|c| c.temperature == Some(0.3)),
            "Verification stage must use nonzero temperature for self-consistency resampling to work"
        );
    }

    #[test]
    fn claim_extraction_and_judge_stages_stay_deterministic() {
        let claim_extraction = cascade_with_optional_manual(
            ResearchStage::ClaimExtraction,
            &RouteResolutionInput::default(),
            None,
            None,
            None,
        );
        assert!(
            claim_extraction.iter().all(|c| c.temperature == Some(0.0))
        );

        let judge = cascade_with_optional_manual(
            ResearchStage::Judge,
            &RouteResolutionInput::default(),
            None,
            None,
            None,
        );
        assert!(judge.iter().all(|c| c.temperature == Some(0.0)));
    }

    fn expected_free() -> Vec<String> {
        vox_config::OPENROUTER_FREE_FALLBACK_MODELS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    #[test]
    fn research_models_append_free_floor_by_default() {
        let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", false);
        // configured model first, then the concrete dispatchable :free floor.
        assert_eq!(v[0], "anthropic/claude-sonnet-4.6");
        assert_eq!(v[1..].to_vec(), expected_free());
        // every floor entry is a real, dispatchable :free slug (not the virtual route).
        assert!(v[1..].iter().all(|m| m.ends_with(":free")));
        assert!(!v.iter().any(|m| m == vox_config::OPENROUTER_FREE));
    }

    #[test]
    fn research_models_prefer_free_moves_it_first() {
        let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", true);
        let n = expected_free().len();
        assert_eq!(v[..n].to_vec(), expected_free());
        assert_eq!(v.last().unwrap(), "anthropic/claude-sonnet-4.6");
        assert!(v[..n].iter().all(|m| m.ends_with(":free")));
    }

    #[test]
    fn research_models_no_duplicate_when_configured_is_already_free() {
        let slug = vox_config::OPENROUTER_FREE_FALLBACK_MODELS[0];
        let v = research_openrouter_model_ids(slug, false);
        // a configured model that is already a free slug appears exactly once.
        assert_eq!(v.iter().filter(|m| m.as_str() == slug).count(), 1);
        assert_eq!(v, expected_free());
    }
}
