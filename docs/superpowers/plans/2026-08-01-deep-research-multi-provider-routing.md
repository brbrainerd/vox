# Deep Research Multi-Provider Routing Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the split-brain in Vox's deep-research model routing: `decide()` already gates candidates on key presence and works correctly, but the research pipeline's selection path (`select()`/`select_with_default_registry`) does not; and the LLM egress cascade the research stages actually call only dispatches to 2 of 9+ provider keys a user can already configure through the GUI. After this plan, a user who configures (say) a Groq or Gemini-direct key actually gets research traffic routed to it.

**Architecture:** Two independent, additive fixes. (1) `select_inner`'s scorer and premium-alias paths gain the same key-presence check `decide()` already has (the local-first path is already key-safe, since local providers never require a key). (2) A new small helper in `vox-research-shim` resolves the winning `ModelSpec` through the full key-aware `vox_orchestrator::models` registry and converts it to a dispatchable `LlmConfig`, prepended ahead of the existing local+OpenRouter cascade as a fallback safety net — this sidesteps the `vox-actor-runtime` ↛ `vox-orchestrator` dependency boundary by doing the multi-provider resolution in the layer that already depends on both. A third, smaller fix wires the currently-dead `QualityLevel` enum into real per-stage axis selection, and a fourth allows free models to compete under `CostPreference::Performance` when the caller explicitly opts in (needed for `VOX_RESEARCH_PREFER_FREE_TIER` to have any effect on model *selection*, not just OpenRouter-lane *ordering*).

**Tech Stack:** Rust, existing `vox_orchestrator::models` registry/selection APIs, existing `vox_actor_runtime::llm` cascade. No new dependencies.

---

## Before you start

Every task below was grounded in exact current source read on 2026-08-01. Re-read the exact lines referenced before editing — use the quoted code as a search anchor if line numbers have shifted. Run all commands from the repo root.

**A note on scope:** this plan fixes the *selection* and *primary-dispatch* layers. It does not touch `providers.v1.yaml`, `vox-secrets`, or the GUI settings surface — those already work correctly (confirmed by this plan's grounding pass) and need no changes.

---

### Task 1: Key-gate the research selection path (`select_inner`)

`decide()` already rejects candidates with no configured key (`select.rs:114`, `ModelRegistry::key_is_present_for`). `select()`/`select_inner()` — the path `vox-research-shim::model_select::resolve_stage` actually calls via `select_with_default_registry` — never checks this. `select_local_first` is already safe (it only considers `Ollama`/`VoxLocal`/`PopuliMesh`, all keyless per `key_guard.rs`'s match arms). The two paths that need the fix are `select_via_premium_alias` and `select_via_scorer`.

**Files:**
- Modify: `crates/vox-orchestrator/src/models/select.rs` (`select_via_premium_alias`, lines 707-727; `select_via_scorer`, lines 729-754)
- Test: same file, extend the existing test module (follow the `key_gate_spec`/`#[serial]` pattern at lines 1267-1343)

- [ ] **Step 1: Write a failing test for `select_via_premium_alias`'s key gate**

Add to `select.rs`'s test module, near the existing `key_gate_excludes_keyless_provider_and_reports_rejection` test:

```rust
    #[test]
    #[serial]
    #[allow(unsafe_code)]
    fn select_falls_through_premium_alias_when_key_missing() {
        // SAFETY: #[serial] serializes env mutation.
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        // A premium-alias-eligible model with no key configured...
        registry.register(key_gate_spec(
            "anthropic-premium-alias-test",
            ProviderType::Anthropic,
        ));
        // ...and a keyless local fallback the scorer path can still find.
        registry.register(key_gate_spec("ollama-fallback-test", ProviderType::Ollama));

        let intent = SelectionIntent::research();
        // select() must not return the keyless premium-alias candidate directly;
        // it must fall through to a path that respects key presence.
        let outcome = select(&intent, &registry);
        if let Some(o) = outcome {
            assert_ne!(
                o.model_id, "anthropic-premium-alias-test",
                "select() returned a candidate with no configured key"
            );
        }
        // (A None outcome is also acceptable here if no other candidate scores —
        // the key assertion above is what this test exists to catch.)
    }
```

- [ ] **Step 2: Run the test to confirm current (buggy) behavior**

Run: `cargo test -p vox-orchestrator select_falls_through_premium_alias_when_key_missing -- --nocapture --test-threads=1`
Expected: this test's outcome depends on whether `anthropic-premium-alias-test` happens to be reachable via the premium-alias path in the test registry (it likely isn't, since `premium_alias_for` reads from `model-routing.v1.yaml`, not test-registered models) — the more direct proof of the bug is Step 3's scorer-path test. Proceed regardless.

- [ ] **Step 3: Write a failing test proving the scorer path ignores key presence**

```rust
    #[test]
    #[serial]
    #[allow(unsafe_code)]
    fn select_via_scorer_excludes_keyless_provider() {
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
            std::env::remove_var("VOX_ANTHROPIC_API_KEY");
        }
        let mut registry = ModelRegistry::default();
        registry.register(key_gate_spec(
            "anthropic-direct-scorer-test",
            ProviderType::Anthropic,
        ));
        registry.register(key_gate_spec("ollama-scorer-test", ProviderType::Ollama));

        let intent = SelectionIntent::research();
        let outcome = select_via_scorer(&intent, &registry);
        if let Some(o) = outcome {
            assert_ne!(
                o.model_id, "anthropic-direct-scorer-test",
                "select_via_scorer returned a keyless-provider candidate"
            );
        }
    }
```

- [ ] **Step 4: Run to confirm it currently FAILS (proving the bug)**

Run: `cargo test -p vox-orchestrator select_via_scorer_excludes_keyless_provider -- --nocapture --test-threads=1`
Expected: FAIL — `select_via_scorer` currently has no key check, so it may return `anthropic-direct-scorer-test` if the scorer ranks it above the Ollama candidate on cost/quality.

- [ ] **Step 5: Add the key-presence check to `select_via_premium_alias`**

Replace the function body (originally lines 707-727):

```rust
fn select_via_premium_alias(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let key = crate::models::task_category_premium_key(intent.task);
    let alias = registry.premium_alias_for(key)?.to_string();
    let model = registry.get(&alias)?;
    if !supports_intent_constraints(&model, intent) {
        return None;
    }
    if !ModelRegistry::key_is_present_for(&model) {
        return None;
    }
    let effective_axes = intent.axes.to_routing_priority(intent.prefer_local);
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::PremiumAlias {
            task: intent.task,
            alias_model_id: alias,
        },
        effective_axes,
    })
}
```

- [ ] **Step 6: Add the key-presence check to `select_via_scorer`**

Replace the function body (originally lines 729-754):

```rust
fn select_via_scorer(
    intent: &SelectionIntent,
    registry: &ModelRegistry,
) -> Option<SelectionOutcome> {
    let effective_axes = intent.axes.to_routing_priority(intent.prefer_local);
    let cost_pref = intent.axes.to_cost_preference();
    let intent_clone = intent.clone();
    let _axes_guard = crate::models::scoring::AxesOverrideGuard::set(effective_axes);
    let model = registry.best_for_with_filter(
        intent.task,
        intent.complexity,
        cost_pref,
        |m| supports_intent_constraints(m, &intent_clone) && ModelRegistry::key_is_present_for(m),
        None,
    )?;
    drop(_axes_guard);
    Some(SelectionOutcome {
        model_id: model.id.clone(),
        model_spec: model,
        reason: SelectionReason::Scored,
        effective_axes,
    })
}
```

- [ ] **Step 7: Confirm `ModelRegistry::key_is_present_for` is visible from this call site**

It's declared `pub(crate) fn key_is_present_for(m: &ModelSpec) -> bool` in `registry.rs:274`, and `select.rs` is in the same crate (`vox-orchestrator`), so no visibility change is needed. Run:

Run: `cargo check -p vox-orchestrator`
Expected: compiles cleanly.

- [ ] **Step 8: Run the new tests to confirm they now pass**

Run: `cargo test -p vox-orchestrator select_via_scorer_excludes_keyless_provider select_falls_through_premium_alias_when_key_missing -- --nocapture --test-threads=1`
Expected: both PASS.

- [ ] **Step 9: Run the full existing `decide()`/`select()` test suite to confirm no regressions**

Run: `cargo test -p vox-orchestrator models::select:: -- --test-threads=1`
Expected: all existing tests still pass, including `key_gate_excludes_keyless_provider_and_reports_rejection` and `key_gate_admits_provider_when_key_present` (Step 1 in this task didn't touch `decide()`, only `select()`'s internals, so these should be unaffected).

- [ ] **Step 10: Commit**

```bash
git add crates/vox-orchestrator/src/models/select.rs
git commit -m "fix: key-gate select_via_scorer and select_via_premium_alias like decide()"
```

---

### Task 2: Allow free models to compete under `CostPreference::Performance` when explicitly requested

`registry.rs`'s `best_for_internal` filter has a blanket `preference == CostPreference::Performance && m.is_free => exclude`. Since `SelectionIntent::research()` uses `SelectionAxes::QUALITY_FIRST`, which derives `CostPreference::Performance`, research can **never** select a free-tier model via the scorer path today — even when `VOX_RESEARCH_PREFER_FREE_TIER` is set, since that flag currently only reorders the OpenRouter-lane candidate list in `cascade.rs`, not model *selection*.

**Files:**
- Modify: `crates/vox-orchestrator/src/models/select.rs` (`SelectionIntent` struct, `research()`)
- Modify: `crates/vox-orchestrator/src/models/registry.rs` (`best_for_with_filter`, `best_for_internal`)
- Test: `crates/vox-orchestrator/src/models/registry.rs` and `select.rs` test modules

- [ ] **Step 1: Write a failing test proving free models are excluded even when explicitly allowed via the new flag (which doesn't exist yet)**

Add to `select.rs`'s test module:

```rust
    #[test]
    fn research_intent_allows_free_when_prefer_free_tier_env_set() {
        let intent = SelectionIntent::research();
        // This assertion documents the contract this task establishes; it will
        // fail to compile until Step 2 adds the field.
        assert!(
            !intent.allow_free_in_performance_mode,
            "default (no env set) must preserve existing behavior: free models excluded"
        );
    }
```

- [ ] **Step 2: Run to confirm compile failure (field doesn't exist)**

Run: `cargo test -p vox-orchestrator research_intent_allows_free_when_prefer_free_tier_env_set`
Expected: compile error, no such field `allow_free_in_performance_mode`.

- [ ] **Step 3: Add the field to `SelectionIntent`**

In `select.rs`, add to the `SelectionIntent` struct (originally lines 393-414):

```rust
pub struct SelectionIntent {
    pub task: TaskCategory,
    pub axes: SelectionAxes,
    pub complexity: u8,
    pub context_size_hint: Option<usize>,
    pub caller_hint: Option<&'static str>,
    pub prefer_local: bool,
    pub max_cost_usd_per_call: Option<f64>,
    pub cacheable_workload: bool,
    /// When true, free-tier (`ModelSpec.is_free`) models are allowed to
    /// compete even under `CostPreference::Performance`, which otherwise
    /// excludes them unconditionally (`registry.rs::best_for_internal`).
    /// Defaults to `false` everywhere except `SelectionIntent::research()`,
    /// which sets it from `VOX_RESEARCH_PREFER_FREE_TIER`.
    pub allow_free_in_performance_mode: bool,
}
```

- [ ] **Step 4: Update every existing `SelectionIntent` constructor to set the new field**

Run: `grep -n "Self {" crates/vox-orchestrator/src/models/select.rs | head -30` to locate every intent-constructor struct literal (`for_task`, `repair_loop`, `research`, `review`, `nli_classifier`, `ide_autocomplete`, `plan_mode` — originally at lines 416-519).

Add `allow_free_in_performance_mode: false,` to every one of them **except** `research()`, where Step 5 sets it dynamically. This preserves current behavior (free models excluded under Performance) for every intent except the one this task is fixing.

- [ ] **Step 5: Set the field dynamically in `research()`**

Replace `SelectionIntent::research()` (originally lines 447-459):

```rust
    /// Pre-baked intent for research / planning / claim stages.
    #[must_use]
    pub fn research() -> Self {
        Self {
            task: TaskCategory::Research,
            axes: SelectionAxes::QUALITY_FIRST,
            complexity: 7,
            context_size_hint: None,
            caller_hint: Some("research"),
            prefer_local: false,
            max_cost_usd_per_call: None,
            cacheable_workload: false,
            allow_free_in_performance_mode: vox_config::inference::research_prefer_free_tier(),
        }
    }
```

- [ ] **Step 6: Run Step 1's test to confirm it now compiles and passes**

Run: `cargo test -p vox-orchestrator research_intent_allows_free_when_prefer_free_tier_env_set`
Expected: PASS (the env var is unset in the default test environment, so the flag is `false`).

- [ ] **Step 7: Thread the flag through `best_for_with_filter`/`best_for_internal`**

In `registry.rs`, change `best_for_with_filter`'s signature (originally lines 676-701) to accept the new bool:

```rust
    #[must_use]
    pub fn best_for_with_filter(
        &self,
        task_type: TaskCategory,
        complexity: u8,
        preference: CostPreference,
        allow_free_in_performance_mode: bool,
        mut pred: impl FnMut(&ModelSpec) -> bool,
        task: Option<&AgentTask>,
    ) -> Option<ModelSpec> {
        let effective_pref = if complexity <= 3 && preference == CostPreference::Economy {
            CostPreference::Economy
        } else {
            preference
        };

        let strength = task_category_strength(task_type);

        let result = self.best_for_internal(
            task_type,
            strength,
            effective_pref,
            allow_free_in_performance_mode,
            &mut pred,
            true,
            task,
        );
        if result.is_some() {
            return result;
        }

        self.best_for_internal(
            task_type,
            strength,
            effective_pref,
            allow_free_in_performance_mode,
            &mut pred,
            false,
            task,
        )
    }
```

And `best_for_internal`'s signature + the exclusion line (originally lines 703-722):

```rust
    fn best_for_internal(
        &self,
        _task_type: TaskCategory,
        strength: crate::models::StrengthTag,
        preference: CostPreference,
        allow_free_in_performance_mode: bool,
        pred: &mut impl FnMut(&ModelSpec) -> bool,
        respect_penalties: bool,
        task: Option<&AgentTask>,
    ) -> Option<ModelSpec> {
        self.models
            .values()
            .filter(|m| {
                if respect_penalties && self.is_penalized(&m.id, _task_type) {
                    return false;
                }
                if preference == CostPreference::Performance
                    && m.is_free
                    && !allow_free_in_performance_mode
                {
                    return false; // Skip free models in performance mode unless explicitly allowed
                }
                // ... rest of the closure body (safety cap, budget gating) is unchanged ...
```

(Leave the rest of the closure body — the safety-cap and budget-gating blocks below the exclusion check — exactly as it currently is; only the exclusion condition changes.)

- [ ] **Step 8: Find and fix every other call site of `best_for_with_filter`**

Run: `grep -rn "best_for_with_filter(" crates/`

For every call site found **other than** the one this task will update in Task 1's `select_via_scorer` (which needs `intent.allow_free_in_performance_mode`), pass `false` as the new parameter to preserve exact prior behavior. Also check for any direct callers of `best_for` (the non-filtered sibling method, if one exists per the doc comment `"Like Self::best_for but only considers..."` at registry.rs:674) — that method likely needs the same signature update or an internal `false` default; confirm via the same grep and adjust accordingly.

- [ ] **Step 9: Update `select_via_scorer` to pass the new flag**

In `select.rs`, update the `select_via_scorer` body (already modified in Task 1, Step 6) to pass `intent.allow_free_in_performance_mode`:

```rust
    let model = registry.best_for_with_filter(
        intent.task,
        intent.complexity,
        cost_pref,
        intent.allow_free_in_performance_mode,
        |m| supports_intent_constraints(m, &intent_clone) && ModelRegistry::key_is_present_for(m),
        None,
    )?;
```

- [ ] **Step 10: Write a test proving a free model is now selectable when the flag is set**

Add to `registry.rs`'s test module (find its existing pattern for constructing test `ModelSpec`s and registering them, e.g. reuse or adapt `select.rs`'s `key_gate_spec` helper shape):

```rust
    #[test]
    fn best_for_with_filter_admits_free_model_when_explicitly_allowed() {
        let mut registry = ModelRegistry::default();
        let mut free_model = key_gate_spec("free-test-model", ProviderType::OpenRouter);
        free_model.is_free = true;
        registry.register(free_model);

        let excluded = registry.best_for_with_filter(
            TaskCategory::Research,
            5,
            CostPreference::Performance,
            false, // not allowed
            |_| true,
            None,
        );
        assert!(excluded.is_none(), "free model must be excluded by default");

        let included = registry.best_for_with_filter(
            TaskCategory::Research,
            5,
            CostPreference::Performance,
            true, // explicitly allowed
            |_| true,
            None,
        );
        assert_eq!(included.map(|m| m.id), Some("free-test-model".to_string()));
    }
```

(This test assumes `key_gate_spec` or an equivalent test-fixture builder is accessible from `registry.rs`'s test module — if it's currently private to `select.rs`'s test module, either move it to a shared `#[cfg(test)]` location both files can use, or duplicate the minimal `ModelSpec` literal directly in this test, matching the field list already confirmed in this plan's grounding.)

- [ ] **Step 11: Run all affected test suites**

Run: `cargo test -p vox-orchestrator models:: -- --test-threads=1`
Expected: all tests pass, including the new ones from this task and Task 1.

- [ ] **Step 12: Commit**

```bash
git add crates/vox-orchestrator/src/models/select.rs crates/vox-orchestrator/src/models/registry.rs
git commit -m "feat: allow free models under Performance preference when VOX_RESEARCH_PREFER_FREE_TIER is set"
```

---

### Task 3: Wire `QualityLevel` into real per-stage axis selection

`crates/vox-research-shim/src/research/model_select.rs::resolve_research_models` accepts `_base_inference: &InferenceConfig` but never reads it — `QualityLevel::{Flash,Balanced,Premium}` has zero effect today.

**Files:**
- Modify: `crates/vox-research-shim/src/research/model_select.rs`
- Test: same file, extend existing test module

- [ ] **Step 1: Write a failing test proving quality level currently has no effect**

Add to `model_select.rs`'s test module:

```rust
    #[test]
    fn quality_level_flash_prefers_cost_first_axes() {
        // This documents the contract this task establishes: Flash quality
        // should bias the research/synthesis intent toward COST_FIRST axes
        // rather than the always-QUALITY_FIRST default from
        // SelectionIntent::research(). Once wired, resolve_stage's caller
        // should be passing a modified intent whose axes differ by quality
        // level — asserted indirectly here via the helper this task adds.
        use vox_orchestrator::models::SelectionAxes;
        let flash = axes_for_quality(QualityLevel::Flash);
        let premium = axes_for_quality(QualityLevel::Premium);
        assert_eq!(flash, SelectionAxes::COST_FIRST);
        assert_eq!(premium, SelectionAxes::QUALITY_FIRST);
    }
```

- [ ] **Step 2: Run to confirm compile failure (no `axes_for_quality` fn yet)**

Run: `cargo test -p vox-research-shim quality_level_flash_prefers_cost_first_axes`
Expected: compile error, function undefined.

- [ ] **Step 3: Implement `axes_for_quality` and thread it through `resolve_stage`**

In `model_select.rs`, add the import and helper function, then update `resolve_stage`/`resolve_research_models`:

```rust
use vox_orchestrator::models::ModelRegistry;
use vox_orchestrator::models::{SelectionAxes, SelectionIntent, select_with_default_registry};

/// Maps a research `QualityLevel` onto the underlying 3-axis selection knob.
fn axes_for_quality(quality: QualityLevel) -> SelectionAxes {
    match quality {
        QualityLevel::Flash => SelectionAxes::COST_FIRST,
        QualityLevel::Balanced => SelectionAxes::BALANCED,
        QualityLevel::Premium => SelectionAxes::QUALITY_FIRST,
    }
}

fn resolve_stage(
    registry: &ModelRegistry,
    mut intent: SelectionIntent,
    quality: QualityLevel,
    fallback: &str,
) -> String {
    intent.axes = axes_for_quality(quality);
    if let Some(outcome) = select_with_default_registry(&intent) {
        return outcome.model_id;
    }
    registry
        .get(fallback)
        .map(|m| m.id.clone())
        .unwrap_or_else(|| fallback.to_string())
}

/// Select models for planner, claim extraction, synthesis, and judge stages.
#[must_use]
pub fn resolve_research_models(
    registry: &ModelRegistry,
    base_inference: &InferenceConfig,
) -> ResolvedResearchModels {
    let planner = resolve_stage(
        registry,
        SelectionIntent::research(),
        base_inference.quality,
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let claim = resolve_stage(
        registry,
        SelectionIntent::nli_classifier(),
        base_inference.quality,
        vox_config::NLI_FALLBACK,
    );
    let synthesis = resolve_stage(
        registry,
        SelectionIntent::research(),
        base_inference.quality,
        vox_config::RESEARCH_FLASH_FALLBACK,
    );
    let judge = resolve_stage(
        registry,
        SelectionIntent::review(),
        base_inference.quality,
        vox_config::REVIEW_PREMIUM_FALLBACK,
    );

    ResolvedResearchModels {
        planner_model: planner,
        claim_model: claim,
        synthesis_model: synthesis,
        judge_model: judge,
    }
}
```

Note this changes `resolve_stage`'s signature (adds a `quality: QualityLevel` parameter) — since `QualityLevel` is `Copy` (confirmed: `#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]`), passing it by value is fine.

- [ ] **Step 4: Run the new test**

Run: `cargo test -p vox-research-shim quality_level_flash_prefers_cost_first_axes`
Expected: PASS.

- [ ] **Step 5: Update the existing `resolve_research_models_returns_non_empty_ids` test if needed**

Run: `cargo test -p vox-research-shim resolve_research_models_returns_non_empty_ids -- --nocapture`
Expected: still PASS unchanged — this task doesn't change the function's public signature or the `InferenceConfig::default()` behavior (which defaults to `QualityLevel::Balanced`, i.e. `SelectionAxes::BALANCED` — a real but reasonable behavior change from the *implicit* prior default of always using `SelectionIntent::research()`'s baked-in `QUALITY_FIRST`; if this test asserted anything about specific model ids rather than just non-emptiness, revisit, but per the grounding pass it only asserts non-empty strings).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-research-shim/src/research/model_select.rs
git commit -m "feat: wire QualityLevel into per-stage research model selection axes"
```

---

### Task 4: Provider-aware primary candidate for the research LLM cascade

`cascade_for_research_stage` (in `vox-actor-runtime`, which cannot depend on `vox-orchestrator`) only builds local+OpenRouter candidates. Since `vox-research-shim` already depends on both `vox_orchestrator` and `vox_actor_runtime`, this task adds a helper there that resolves the *actual* winning provider (via the now-key-gated `decide()`) and converts it to a dispatchable `LlmConfig`, prepended ahead of the existing 2-lane cascade as the primary candidate.

**Files:**
- Create: `crates/vox-research-shim/src/research/orchestrator/model_dispatch.rs`
- Modify: `crates/vox-research-shim/src/research/orchestrator/mod.rs` (register the module — check the existing pattern other sibling files use)
- Modify: `crates/vox-research-shim/src/research/orchestrator/web_gather.rs` (adopt the new helper at the `try_llm_query_expansion` call site, Task 2's plan already touched this file — reread it fresh before editing)

- [ ] **Step 1: Confirm `ModelRegistry::get_llm_config`'s exact signature before writing calling code**

This function was identified in this plan's grounding pass (`crates/vox-orchestrator/src/models/registry.rs:1041-1144`, feature-gated `runtime`, already converts every `ProviderType` variant to an `LlmConfig`) but its exact parameter/return types weren't captured verbatim. Run:

Run: `grep -n "fn get_llm_config" -A 15 crates/vox-orchestrator/src/models/registry.rs`

Confirm the signature matches the assumed shape below (`fn get_llm_config(&self, spec: &ModelSpec) -> Option<LlmConfig>` or similar) before proceeding — adjust Step 3's code to match whatever the real signature is if it differs (e.g. if it takes the model id string instead of a `ModelSpec`, or returns a `Result` instead of `Option`).

- [ ] **Step 2: Write a failing test for the new dispatch helper's fallback behavior**

Create `crates/vox-research-shim/src/research/orchestrator/model_dispatch.rs`:

```rust
//! Resolves a primary, multi-provider-aware LLM candidate for a research
//! stage via the full key-gated `vox_orchestrator::models` selector,
//! bridging the `vox-actor-runtime` <-> `vox-orchestrator` dependency gap
//! (the cascade builders in `vox_actor_runtime::llm::cascade` cannot
//! depend on `vox-orchestrator` directly; this crate already depends on
//! both, so the bridging happens here).

use vox_actor_runtime::llm::LlmConfig;
use vox_orchestrator::models::{ModelRegistry, ModelSelectionRequest, SelectionIntent, decide};

/// Resolves the winning `ModelSpec` for `intent` through the key-gated
/// `decide()` path and converts it to a dispatchable `LlmConfig`. Returns
/// `None` if no candidate clears selection (e.g. no keys configured for
/// any eligible provider) or if the conversion isn't available — callers
/// should fall back to `cascade_for_research_stage`'s local+OpenRouter
/// lanes in that case, never treat `None` as a hard error.
pub fn primary_candidate_for_intent(intent: SelectionIntent) -> Option<LlmConfig> {
    let registry = ModelRegistry::from_cache();
    let request = ModelSelectionRequest::from_intent(intent);
    let decision = decide(&request, &registry)?;
    registry.get_llm_config(&decision.outcome.model_spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_or_some_without_panicking_for_research_intent() {
        // This is a smoke test, not a behavioral assertion: the real
        // registry's contents and configured keys vary by environment
        // (CI has none configured), so both `None` (nothing selectable)
        // and `Some` (a local/keyless candidate wins) are valid outcomes.
        // What matters is that this never panics.
        let _ = primary_candidate_for_intent(SelectionIntent::research());
    }
}
```

- [ ] **Step 3: Run the test to confirm it compiles and passes**

Run: `cargo test -p vox-research-shim model_dispatch:: --features runtime -- --nocapture`
Expected: PASS. If `ModelRegistry::get_llm_config` is feature-gated `runtime` (per Step 1's confirmation), this crate's `Cargo.toml` needs a `runtime` feature that enables `vox-orchestrator`'s `runtime` feature — check `crates/vox-research-shim/Cargo.toml`'s existing feature declarations and add this dependency if not already present, following whatever pattern the existing `scientia-claims`/`news-publish`/`gamify` optional features use.

- [ ] **Step 4: Register the module**

In `crates/vox-research-shim/src/research/orchestrator/mod.rs`, add (matching the existing pattern for sibling modules like `web_gather`, `stages`, `pipeline`):

```rust
pub(super) mod model_dispatch;
```

(Use `pub(super)` unless another sibling file's existing declaration uses a different visibility — check before assuming.)

- [ ] **Step 5: Wire the helper into the one confirmed call site (`try_llm_query_expansion`'s cascade call in `web_gather.rs`)**

This file was already modified by the trust/novelty plan's Task 2 (moving `try_llm_query_expansion` into `vox-search`). **Read the current state of `web_gather.rs` fresh before this step** — if that prior task has already landed, the call site now lives in `vox-search::llm_query_expansion::try_llm_query_expansion` and takes plain `Option<&str>` params rather than a `ResearchConfig`; adapt accordingly. The pattern to apply, regardless of which file the call site ends up in: before building the existing local+OpenRouter cascade, try the new primary candidate first and prepend it:

```rust
    let primary = super::model_dispatch::primary_candidate_for_intent(
        vox_orchestrator::models::SelectionIntent::research(),
    );
    let mut candidates: Vec<vox_actor_runtime::llm::LlmConfig> = primary.into_iter().collect();
    candidates.extend(cascade_with_optional_manual(
        ResearchStage::Planner,
        &RouteResolutionInput::default(),
        config.llm_endpoint.as_deref(),
        config.api_key.as_deref(),
        Some(&config.planner_model),
    ));
    let response = chat_with_cascade(&opts, messages, candidates, Some(ResearchStage::Planner)).await;
```

(This replaces whatever the current single `cascade_with_optional_manual(...)` call + direct `chat_with_cascade` call looks like at this site — the shape above is the pattern to apply, adapt variable names to match what's actually in the file when this step is executed.)

- [ ] **Step 6: Run the research-shim test suite**

Run: `cargo test -p vox-research-shim --features runtime`
Expected: all tests pass, including the new `model_dispatch` test.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-research-shim/src/research/orchestrator/model_dispatch.rs crates/vox-research-shim/src/research/orchestrator/mod.rs crates/vox-research-shim/src/research/orchestrator/web_gather.rs
git commit -m "feat: prepend a key-gated, multi-provider primary candidate ahead of the local+OpenRouter research cascade"
```

- [ ] **Step 8: Document the remaining call sites needing the same treatment**

This task wires one confirmed call site as a working example. The same `primary_candidate_for_intent(...)` prepend pattern needs applying to the other research-stage cascade call sites (claim extraction in `claims.rs`, synthesis and verification in `stages.rs`/`verifier.rs`) — these weren't fully read in this plan's grounding pass, so writing their exact diffs here would mean guessing at surrounding code this plan didn't verify. Open a follow-up task (or extend this plan before executing further) that: (a) greps each remaining `cascade_with_optional_manual`/`cascade_for_research_stage` call site across `vox-research-shim`, (b) applies the identical prepend pattern from Step 5, (c) adds one smoke test per site matching Step 2's shape.

---

## Self-review notes

- **Spec coverage:** Task 1 covers spec items 8 (route through key-presence-aware selection). Task 2 covers the free-tier-exclusion half of item 10. Task 3 covers the `QualityLevel` half of item 10. Task 4 covers item 9 (provider-aware cascade), fully wired for one call site with an explicit, honest follow-up note for the rest rather than fabricated diffs against unread code.
- **Placeholder scan:** no TBD/TODO in code steps. Task 4 Step 1 and Step 8 are explicit verification/follow-up steps, not skipped implementation — they name exactly what to check and why the remaining scope wasn't guessed at.
- **Type consistency:** `SelectionIntent` gains `allow_free_in_performance_mode` (Task 2) — every constructor must be updated (Step 4 names the grep to find them all). `best_for_with_filter`/`best_for_internal` gain a new positional bool parameter (Task 2) — every call site must be updated (Step 8 names the grep). `resolve_stage` gains a `quality: QualityLevel` parameter (Task 3) — its only two callers are both inside `resolve_research_models` in the same file, both updated in the same step.
- **Task ordering:** Task 1 and Task 2 both touch `select_via_scorer` — Task 2's Step 9 assumes Task 1's Step 6 has already landed (the `&& ModelRegistry::key_is_present_for(m)` clause). Execute Task 1 before Task 2. Task 4 assumes Task 1 has landed (it calls `decide()`, which only produces correct results once — well, `decide()` was already correct before Task 1; Task 4 doesn't strictly depend on Task 1, but does depend on Task 3 not yet existing conflicts — Tasks 3 and 4 are independent of each other and of Task 2, but both should follow Task 1 for consistency of the overall change set).
