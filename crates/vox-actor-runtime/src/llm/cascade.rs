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

/// Ordered, de-duplicated OpenRouter model ids for a research call.
///
/// The free-tier router (`openrouter/free`) is ALWAYS included as a fallback floor
/// so research degrades to zero cost instead of failing. `prefer_free` moves it to
/// the front (opt-in via `VOX_RESEARCH_PREFER_FREE_TIER`); otherwise it trails the
/// caller-configured model. If the configured model already IS the free router,
/// the list collapses to a single entry.
#[must_use]
fn research_openrouter_model_ids(configured: &str, prefer_free: bool) -> Vec<String> {
    let free = vox_config::OPENROUTER_FREE.to_string();
    if configured == free {
        return vec![free];
    }
    if prefer_free {
        vec![free, configured.to_string()]
    } else {
        vec![configured.to_string(), free]
    }
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
        ResearchStage::ClaimExtraction | ResearchStage::Verification | ResearchStage::Judge => 0.0,
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
    fn research_models_append_free_floor_by_default() {
        let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", false);
        assert_eq!(
            v,
            vec![
                "anthropic/claude-sonnet-4.6".to_string(),
                vox_config::OPENROUTER_FREE.to_string(),
            ]
        );
    }

    #[test]
    fn research_models_prefer_free_moves_it_first() {
        let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", true);
        assert_eq!(
            v,
            vec![
                vox_config::OPENROUTER_FREE.to_string(),
                "anthropic/claude-sonnet-4.6".to_string(),
            ]
        );
    }

    #[test]
    fn research_models_dedup_when_configured_is_already_free() {
        let v = research_openrouter_model_ids(vox_config::OPENROUTER_FREE, false);
        assert_eq!(v, vec![vox_config::OPENROUTER_FREE.to_string()]);
    }
}
