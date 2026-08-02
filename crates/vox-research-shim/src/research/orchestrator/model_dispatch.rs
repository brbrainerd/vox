//! Resolves a primary, multi-provider-aware LLM candidate for a research
//! stage via the full key-gated `vox_orchestrator::models` selector,
//! bridging the `vox-actor-runtime` <-> `vox-orchestrator` dependency gap
//! (the cascade builders in `vox_actor_runtime::llm::cascade` cannot
//! depend on `vox-orchestrator` directly; this crate already depends on
//! both, so the bridging happens here).

use std::sync::OnceLock;

use vox_actor_runtime::llm::LlmConfig;
use vox_orchestrator::models::{
    ModelRegistry, ModelSelectionRequest, SelectionIntent, decide, llm_config_for_spec,
};

static SHARED_REGISTRY: OnceLock<ModelRegistry> = OnceLock::new();

/// Returns a process-wide shared `ModelRegistry`, loaded from disk once on
/// first use. Prefer this over `ModelRegistry::from_cache()` in hot paths
/// (e.g. the claim-verifier's per-sample resampling loop) so the registry
/// isn't re-read/re-parsed from disk on every call within one research run.
fn shared_registry() -> &'static ModelRegistry {
    SHARED_REGISTRY.get_or_init(ModelRegistry::from_cache)
}

/// Resolves the winning `ModelSpec` for `intent` through the key-gated
/// `decide()` path and converts it to a dispatchable `LlmConfig`. Returns
/// `None` if no candidate clears selection (e.g. no keys configured for
/// any eligible provider) — callers should fall back to
/// `cascade_for_research_stage`'s local+OpenRouter lanes in that case,
/// never treat `None` as a hard error.
pub fn primary_candidate_for_intent(intent: SelectionIntent) -> Option<LlmConfig> {
    let registry = shared_registry();
    let task_type = intent.task;
    let request = ModelSelectionRequest::from_intent(intent);
    let decision = decide(&request, registry)?;
    Some(llm_config_for_spec(&decision.outcome.model_spec, task_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_or_some_without_panicking_for_research_intent() {
        // Smoke test, not a behavioral assertion: the real registry's
        // contents and configured keys vary by environment (CI has none
        // configured), so both None (nothing selectable) and Some (a
        // local/keyless candidate wins) are valid outcomes. What matters
        // is that this never panics.
        let _ = primary_candidate_for_intent(SelectionIntent::research());
    }
}
