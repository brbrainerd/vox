# Orchestrator Model Agnosticism Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ModelRegistry::select()` the single source of truth for all LLM model selection across the entire workspace — closing the `FreeAiClient` bypass in `AiTaskProcessor`, removing the redundant 8-step resolver in `vox-actor-runtime`, and making `ModelConfidence::Confirmed` a hard routing gate.

**Architecture:** Introduce a `ModelSelector` trait in `vox-orchestrator-core`. `ModelRegistry` implements it. `vox-actor-runtime` accepts an injected `Arc<dyn ModelSelector>` instead of performing its own resolution. The daemon's `AgentFleet` wires the real `ModelRegistry` in; tests wire a `StubModelSelector`. `ModelConfidence` becomes a hard filter inside `ModelRegistry::select()` so non-confirmed models can never be returned.

**Tech Stack:** Rust, `async_trait`, `Arc<dyn Trait>`, `cargo test`

**Prerequisite:** Plan 1 (Foundation) must be complete. `vox-orchestrator-core` must exist and contain `src/models/`.

---

## File Map

| Action | Path |
|---|---|
| CREATE | `crates/vox-orchestrator-core/src/models/selector_trait.rs` |
| MODIFY | `crates/vox-orchestrator-core/src/models/mod.rs` — export trait + stub |
| MODIFY | `crates/vox-orchestrator-core/src/models/registry.rs` — impl `ModelSelector` |
| MODIFY | `crates/vox-orchestrator-core/src/models/select.rs` — add `Confirmed`-only filter |
| MODIFY | `crates/vox-actor-runtime/src/lib.rs` — accept injected selector |
| MODIFY | `crates/vox-actor-runtime/src/llm/mod.rs` (or `chat.rs`) — use injected selector |
| DELETE | `crates/vox-actor-runtime/src/model_resolution.rs` — remove own resolver |
| MODIFY | `crates/vox-orchestrator/src/runtime.rs` — `AiTaskProcessor` drops `FreeAiClient` |
| MODIFY | `crates/vox-orchestrator/src/runtime.rs` — `AgentFleet` injects `Arc<dyn ModelSelector>` |

---

## Task 1: Define the `ModelSelector` trait

This is the single interface that separates "who chooses the model" from "who calls the model."
All code that previously picked a model independently will be replaced with a call to this trait.

**Files:**
- Create: `crates/vox-orchestrator-core/src/models/selector_trait.rs`
- Modify: `crates/vox-orchestrator-core/src/models/mod.rs`

- [ ] **Step 1: Write the failing test first**

Add to `crates/vox-orchestrator-core/src/models/selector_trait.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelCapabilities, ModelTier, ProviderType, StrengthTag};

    fn dummy_spec(id: &str) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: id.to_string(),
            provider: "test".to_string(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 8192,
            cost_per_1k: 0.001,
            cost_per_1k_input: 0.001,
            cost_per_1k_output: 0.002,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::PricingSource::Bootstrap,
            is_free: false,
            strengths: vec![StrengthTag::Generalist],
            capabilities: ModelCapabilities {
                max_context: 8192,
                tier: ModelTier::Pro,
                ..Default::default()
            },
            supported_parameters: vec![],
        }
    }

    #[tokio::test]
    async fn stub_selector_returns_fixed_model() {
        let spec = dummy_spec("test/model-1");
        let selector = StubModelSelector { fixed_model: Some(spec.clone()) };
        let intent = SelectionIntent::default();
        let result = selector.select(intent).await;
        assert_eq!(result.unwrap().id, "test/model-1");
    }

    #[tokio::test]
    async fn stub_selector_returns_none_when_no_fixed_model() {
        let selector = StubModelSelector { fixed_model: None };
        let intent = SelectionIntent::default();
        let result = selector.select(intent).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn stub_record_outcome_is_noop() {
        let selector = StubModelSelector { fixed_model: None };
        // Should not panic
        selector.record_outcome(
            "test/model-1",
            vox_orchestrator_types::TaskCategory::Codegen,
            true,
            150,
            0.001,
        ).await;
    }
}
```

- [ ] **Step 2: Run the test to verify it fails (type not defined yet)**

```powershell
cargo test -p vox-orchestrator-core models::selector_trait 2>&1 | Select-String "^error|FAILED|not found" | Select-Object -First 10
```

Expected: compile error `use of undeclared type StubModelSelector` (or similar). Good — the test correctly fails because the trait doesn't exist yet.

- [ ] **Step 3: Write the trait and stub implementation**

Write `crates/vox-orchestrator-core/src/models/selector_trait.rs`:

```rust
//! Injectable model selection interface.
//!
//! `ModelRegistry` implements `ModelSelector` for production use.
//! `StubModelSelector` is provided for tests.

use async_trait::async_trait;
use vox_orchestrator_types::TaskCategory;

use super::select::SelectionIntent;
use super::spec::ModelSpec;

/// The single interface through which all LLM model selection happens.
///
/// Callers receive an `Arc<dyn ModelSelector>` and call `select()`.
/// They never construct their own resolver or call OpenRouter directly.
#[async_trait]
pub trait ModelSelector: Send + Sync + 'static {
    /// Select the best available, fully-confirmed model for the given intent.
    ///
    /// Returns `None` if no confirmed model matches — callers MUST handle
    /// this by degrading gracefully (e.g. queuing the task, returning an
    /// error to the user) rather than panicking.
    async fn select(&self, intent: SelectionIntent) -> Option<ModelSpec>;

    /// Feed call outcome into the scoreboard so the registry can update
    /// Thompson arm stats and observed pricing.
    ///
    /// Called after every LLM call, success or failure.
    async fn record_outcome(
        &self,
        model_id: &str,
        category: TaskCategory,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    );
}

/// No-op selector for unit tests and dry runs.
///
/// Always returns `fixed_model` regardless of intent. `record_outcome` is
/// a no-op — no scoreboard or DB writes.
pub struct StubModelSelector {
    pub fixed_model: Option<ModelSpec>,
}

#[async_trait]
impl ModelSelector for StubModelSelector {
    async fn select(&self, _intent: SelectionIntent) -> Option<ModelSpec> {
        self.fixed_model.clone()
    }

    async fn record_outcome(
        &self,
        _model_id: &str,
        _category: TaskCategory,
        _success: bool,
        _latency_ms: u64,
        _cost_usd: f64,
    ) {
        // no-op in tests
    }
}
```

- [ ] **Step 4: Export from `models/mod.rs`**

Add to `crates/vox-orchestrator-core/src/models/mod.rs`:

```rust
pub mod selector_trait;
pub use selector_trait::{ModelSelector, StubModelSelector};
```

- [ ] **Step 5: Run the tests to verify they pass**

```powershell
cargo test -p vox-orchestrator-core models::selector_trait 2>&1 | tail -10
```

Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-core/src/models/selector_trait.rs
git add crates/vox-orchestrator-core/src/models/mod.rs
git commit -m "feat(orchestrator-core): add ModelSelector trait and StubModelSelector"
```

---

## Task 2: Make `ModelConfidence` a hard routing gate in `select()`

Currently `select()` may return models that are `Shadowed` or `Provisional`. The autonomic
`ModelConfidence` state machine exists but is a soft heuristic, not a hard filter.
This task makes `Confirmed` the mandatory gate.

**Files:**
- Modify: `crates/vox-orchestrator-core/src/models/select.rs`
- Modify: `crates/vox-orchestrator-core/src/models/tests.rs` (or inline tests)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator-core/src/models/tests.rs`:

```rust
#[test]
fn select_excludes_non_confirmed_models() {
    use crate::models::spec::PricingSource;
    use crate::models::{ModelCapabilities, ModelTier, ProviderType, StrengthTag};
    use crate::models::select::{SelectionIntent, SelectionAxes};

    // A shadowed model (OpenRouter = Shadowed per discovery_pipeline heuristic)
    let shadowed = ModelSpec {
        id: "acme/shadowed-model".to_string(),
        canonical_slug: "acme/shadowed-model".to_string(),
        provider: "openrouter".to_string(),
        provider_type: ProviderType::OpenRouter,
        max_tokens: 8192,
        cost_per_1k: 0.001,
        cost_per_1k_input: 0.001,
        cost_per_1k_output: 0.002,
        observed_cost_per_1k: None,
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        // OpenRouter source => Shadowed confidence
        pricing_source: PricingSource::OpenRouter,
        is_free: false,
        strengths: vec![StrengthTag::Generalist],
        capabilities: ModelCapabilities {
            max_context: 8192,
            tier: ModelTier::Pro,
            ..Default::default()
        },
        supported_parameters: vec![],
    };

    let mut registry = ModelRegistry::default();
    registry.register(shadowed);

    let intent = SelectionIntent {
        axes: SelectionAxes {
            cost: 50,
            responsiveness: 50,
            intelligence: 50,
        },
        ..Default::default()
    };

    // A registry with only shadowed models must return None.
    let outcome = select(&intent, &registry);
    assert!(outcome.is_none(), "select() must not return Shadowed models");
}
```

- [ ] **Step 2: Run the test to verify it currently FAILS (select returns the shadowed model)**

```powershell
cargo test -p vox-orchestrator-core select_excludes_non_confirmed_models 2>&1 | tail -10
```

Expected: `FAILED` — the test fails because the current `select()` does not filter on confidence.

- [ ] **Step 3: Add the confidence filter to `select()`**

In `crates/vox-orchestrator-core/src/models/select.rs`, find the function that builds the
candidate list (it will be a `filter` or `retain` call on registry models). Add a confidence
filter before scoring:

```rust
use crate::models::discovery_pipeline::resolve_eligibility;
use crate::models::autonomic::ModelConfidence;

// Inside the candidate-building section of select():
let candidates: Vec<&ModelSpec> = registry
    .all_models()
    .filter(|m| {
        // Hard gate: only route to fully-confirmed models.
        // Shadowed/Provisional models have not yet earned scoreboard evidence.
        let confidence = resolve_eligibility(
            m,
            registry.scoreboard_snapshot().get(&m.id),
            registry.catalog_median_p50_ms(),
        );
        confidence == ModelConfidence::Confirmed
    })
    .filter(/* existing key/provider/capability filters */)
    .collect();
```

If `ModelRegistry` doesn't already have a `catalog_median_p50_ms()` method, add it:

```rust
// In registry.rs
pub fn catalog_median_p50_ms(&self) -> f64 {
    let latencies: Vec<f64> = self.models.values()
        .filter_map(|m| m.capabilities.latency_p50_ms.map(|l| l as f64))
        .collect();
    if latencies.is_empty() { return 1000.0; }
    let mut sorted = latencies.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[sorted.len() / 2]
}
```

- [ ] **Step 4: Run the test to verify it passes**

```powershell
cargo test -p vox-orchestrator-core select_excludes_non_confirmed_models 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 5: Verify no existing tests broke**

```powershell
cargo test -p vox-orchestrator-core 2>&1 | tail -15
```

If existing tests set up a `ModelRegistry` with only OpenRouter (Shadowed) models and expect
`select()` to return them, those tests now need to be updated to use `PricingSource::Telemetry`
or `PricingSource::UserConfig` (both map to `Confirmed`). Update each such test.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-core/src/models/select.rs
git add crates/vox-orchestrator-core/src/models/registry.rs
git add crates/vox-orchestrator-core/src/models/tests.rs
git commit -m "feat(orchestrator-core): make ModelConfidence::Confirmed a hard routing gate in select()"
```

---

## Task 3: Implement `ModelSelector` on `ModelRegistry`

**Files:**
- Modify: `crates/vox-orchestrator-core/src/models/registry.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator-core/src/models/tests.rs`:

```rust
#[tokio::test]
async fn model_registry_implements_model_selector() {
    use crate::models::selector_trait::ModelSelector;
    use crate::models::select::{SelectionIntent, SelectionAxes};
    use crate::models::spec::PricingSource;

    let mut registry = ModelRegistry::default();
    // Add a confirmed model (Telemetry = Confirmed)
    let spec = ModelSpec {
        id: "google/gemini-2.5-flash".to_string(),
        canonical_slug: "google/gemini-2.5-flash".to_string(),
        provider: "google".to_string(),
        provider_type: ProviderType::Google,
        max_tokens: 100_000,
        cost_per_1k: 0.0,
        cost_per_1k_input: 0.0,
        cost_per_1k_output: 0.0,
        observed_cost_per_1k: None,
        cache_creation_cost_per_1k: 0.0,
        cache_read_cost_per_1k: 0.0,
        supports_prompt_caching: false,
        pricing_source: PricingSource::Telemetry,  // => Confirmed
        is_free: true,
        strengths: vec![StrengthTag::Generalist],
        capabilities: ModelCapabilities {
            max_context: 100_000,
            tier: ModelTier::Pro,
            ..Default::default()
        },
        supported_parameters: vec![],
    };
    registry.register(spec);

    // Use via trait object — this is the key test: registry is usable as dyn ModelSelector
    let selector: &dyn ModelSelector = &registry;
    let intent = SelectionIntent::default();
    let result = selector.select(intent).await;
    assert!(result.is_some(), "ModelRegistry as ModelSelector must return the confirmed model");
    assert_eq!(result.unwrap().id, "google/gemini-2.5-flash");
}
```

- [ ] **Step 2: Run test to verify it fails**

```powershell
cargo test -p vox-orchestrator-core model_registry_implements_model_selector 2>&1 | tail -10
```

Expected: compile error — `ModelRegistry` does not implement `ModelSelector` yet.

- [ ] **Step 3: Implement the trait on `ModelRegistry`**

Add to `crates/vox-orchestrator-core/src/models/registry.rs`:

```rust
use super::selector_trait::ModelSelector;
use super::select::{SelectionIntent, select};
use async_trait::async_trait;
use vox_orchestrator_types::TaskCategory;

#[async_trait]
impl ModelSelector for ModelRegistry {
    async fn select(&self, intent: SelectionIntent) -> Option<crate::models::spec::ModelSpec> {
        // Delegates to the existing pure select() function.
        // That function now has the Confirmed-only filter from Task 2.
        select(&intent, self).map(|outcome| outcome.spec)
    }

    async fn record_outcome(
        &self,
        model_id: &str,
        category: TaskCategory,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    ) {
        // Update in-memory arm stats for Thompson sampling.
        // This is the feedback loop from actual call outcomes.
        let mut registry = self.clone();  // NOTE: if ModelRegistry is behind Arc<RwLock<>>,
                                          // use the write guard instead.
        let key = (model_id.to_string(), category);
        let (successes, failures) = registry.arm_stats.entry(key.0.clone()).or_insert((0, 0));
        if success {
            *successes = successes.saturating_add(1);
        } else {
            *failures = failures.saturating_add(1);
        }
        tracing::debug!(
            model_id, %category, success, latency_ms, cost_usd,
            "ModelSelector::record_outcome"
        );
    }
}
```

> **Note on mutability:** If `ModelRegistry` is currently used via `Arc<RwLock<ModelRegistry>>`,
> the `impl ModelSelector for Arc<RwLock<ModelRegistry>>` pattern works better than implementing
> on the struct directly. Check how `ModelRegistry` is held in `Orchestrator` and use the same
> pattern. The test above should guide the final shape.

- [ ] **Step 4: Run test to verify it passes**

```powershell
cargo test -p vox-orchestrator-core model_registry_implements_model_selector 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 5: Run full suite**

```powershell
cargo test -p vox-orchestrator-core 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-core/src/models/registry.rs
git commit -m "feat(orchestrator-core): implement ModelSelector trait on ModelRegistry"
```

---

## Task 4: Update `vox-actor-runtime` to accept injected `ModelSelector`

`vox-actor-runtime` currently resolves models via `model_resolution.rs` — its own 8-step waterfall.
This task replaces that with an injected `Arc<dyn ModelSelector>`. The actor runtime drops all
model-picking logic and becomes a pure executor.

**Files:**
- Modify: `crates/vox-actor-runtime/Cargo.toml` — add `vox-orchestrator-core` dep
- Modify: `crates/vox-actor-runtime/src/lib.rs` — export `ModelSelector` re-export
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs` (or wherever LLM calls are assembled)
- Delete: `crates/vox-actor-runtime/src/model_resolution.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-actor-runtime/tests/llm_selector_injection.rs` (new file):

```rust
//! Verifies that the actor runtime uses an injected ModelSelector rather than
//! performing its own model resolution.

use std::sync::Arc;
use vox_orchestrator_core::models::{ModelSelector, StubModelSelector, ModelSpec};
use vox_orchestrator_core::models::select::SelectionIntent;
use vox_orchestrator_types::TaskCategory;
use vox_actor_runtime::LlmCallContext;

#[tokio::test]
async fn actor_runtime_uses_injected_selector() {
    let spec = ModelSpec {
        id: "stub/model".to_string(),
        provider: "stub".to_string(),
        // fill minimum required fields with defaults
        ..ModelSpec::test_default("stub/model")
    };
    let selector: Arc<dyn ModelSelector> = Arc::new(StubModelSelector {
        fixed_model: Some(spec.clone()),
    });

    let ctx = LlmCallContext::builder()
        .model_selector(selector)
        .build();

    // Resolving a model through context must return the stub's fixed model
    let resolved = ctx.resolve_model(SelectionIntent::default()).await;
    assert_eq!(resolved.unwrap().id, "stub/model");
}
```

- [ ] **Step 2: Run to verify it fails**

```powershell
cargo test -p vox-actor-runtime actor_runtime_uses_injected_selector 2>&1 | tail -10
```

Expected: compile errors — `LlmCallContext`, `resolve_model`, `ModelSelector` not available in actor-runtime yet.

- [ ] **Step 3: Add `vox-orchestrator-core` dep to actor-runtime**

In `crates/vox-actor-runtime/Cargo.toml`, add:

```toml
vox-orchestrator-core = { path = "../vox-orchestrator-core" }
```

- [ ] **Step 4: Create `LlmCallContext` in actor-runtime**

Create `crates/vox-actor-runtime/src/llm/call_context.rs`:

```rust
//! Context for a single LLM call, carrying the injected model selector.

use std::sync::Arc;
use vox_orchestrator_core::models::{ModelSelector, ModelSpec};
use vox_orchestrator_core::models::select::SelectionIntent;

/// Carries everything needed to make one LLM call: the model selector,
/// tenant config, trace context. Constructed by AgentFleet when spawning
/// an actor; actors receive it as part of their ProcessContext.
pub struct LlmCallContext {
    selector: Arc<dyn ModelSelector>,
}

impl LlmCallContext {
    pub fn builder() -> LlmCallContextBuilder {
        LlmCallContextBuilder::default()
    }

    /// Resolve the best model for this call. Returns `None` if no confirmed
    /// model is available — the actor must handle this gracefully.
    pub async fn resolve_model(&self, intent: SelectionIntent) -> Option<ModelSpec> {
        self.selector.select(intent).await
    }

    /// Feed outcome back to the registry's scoreboard.
    pub async fn record_outcome(
        &self,
        model_id: &str,
        category: vox_orchestrator_types::TaskCategory,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    ) {
        self.selector.record_outcome(model_id, category, success, latency_ms, cost_usd).await;
    }
}

#[derive(Default)]
pub struct LlmCallContextBuilder {
    selector: Option<Arc<dyn ModelSelector>>,
}

impl LlmCallContextBuilder {
    pub fn model_selector(mut self, s: Arc<dyn ModelSelector>) -> Self {
        self.selector = Some(s);
        self
    }

    pub fn build(self) -> LlmCallContext {
        LlmCallContext {
            selector: self.selector.expect("model_selector is required"),
        }
    }
}
```

Add `pub mod call_context;` and `pub use call_context::LlmCallContext;` to
`crates/vox-actor-runtime/src/llm/mod.rs`.

Also add `ModelSpec::test_default` helper to `vox-orchestrator-core` (in `spec.rs`):

```rust
#[cfg(test)]
impl ModelSpec {
    pub fn test_default(id: &str) -> Self {
        use crate::models::{ModelCapabilities, ModelTier, ProviderType, StrengthTag};
        use crate::models::spec::PricingSource;
        Self {
            id: id.to_string(),
            canonical_slug: id.to_string(),
            provider: "test".to_string(),
            provider_type: ProviderType::OpenRouter,
            max_tokens: 8192,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Telemetry,
            is_free: true,
            strengths: vec![StrengthTag::Generalist],
            capabilities: ModelCapabilities {
                max_context: 8192,
                tier: ModelTier::Pro,
                ..Default::default()
            },
            supported_parameters: vec![],
        }
    }
}
```

- [ ] **Step 5: Run the test to verify it passes**

```powershell
cargo test -p vox-actor-runtime actor_runtime_uses_injected_selector 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Commit the passing test + LlmCallContext**

```powershell
git add crates/vox-actor-runtime/
git add crates/vox-orchestrator-core/src/models/spec.rs
git commit -m "feat(actor-runtime): add LlmCallContext with injected ModelSelector"
```

---

## Task 5: Remove `model_resolution.rs` from `vox-actor-runtime`

`model_resolution.rs` in actor-runtime has its own 8-step resolution waterfall (env pins, Mens GPU
probe, HF router, OpenRouter, local fallback). Now that `LlmCallContext` delegates to an injected
`ModelSelector`, this file is dead weight.

**Files:**
- Delete: `crates/vox-actor-runtime/src/model_resolution.rs`
- Modify: `crates/vox-actor-runtime/src/lib.rs` — remove `mod model_resolution` declaration

- [ ] **Step 1: Check if `model_resolution.rs` is referenced from other crates**

```powershell
rg "model_resolution" crates/ --files-with-matches
```

Expected: only `crates/vox-actor-runtime/src/lib.rs` and `crates/vox-actor-runtime/src/model_resolution.rs` itself.
If other crates reference it, migrate them to use `LlmCallContext` first.

- [ ] **Step 2: Check which public symbols from `model_resolution` are used**

```powershell
rg "resolve_chat_provider_route\|model_resolution::" crates/ --files-with-matches
```

For each use site, replace with an `LlmCallContext::resolve_model()` call passed through from the actor's `ProcessContext`.

- [ ] **Step 3: Move env-pin logic into `ModelRegistry::select()`**

The env-pin behavior (`VOX_SELECTOR_MODEL`, `VOX_MODEL`) currently lives in `model_resolution.rs`.
It must move to `ModelRegistry::select()` as the first step, before scoring:

In `crates/vox-orchestrator-core/src/models/select.rs`, at the top of the `select()` function:

```rust
pub fn select(intent: &SelectionIntent, registry: &ModelRegistry) -> Option<SelectionOutcome> {
    // 1. Env-pin override (highest priority, short-circuit)
    if let Ok(pinned_id) = std::env::var("VOX_SELECTOR_MODEL")
        .or_else(|_| std::env::var("VOX_MODEL"))
    {
        if let Some(spec) = registry.get(&pinned_id) {
            return Some(SelectionOutcome {
                spec: spec.clone(),
                reason: SelectionReason::EnvPin,
                score: ScoreBreakdown::default(),
                decision: None,
            });
        }
        // If pinned model is not in registry, fall through to normal selection.
        tracing::warn!(pinned_id, "VOX_MODEL pin not found in registry; falling through");
    }

    // 2. Rest of selection logic (Confirmed-only filter, scoring, etc.)
    // ... existing code ...
}
```

- [ ] **Step 4: Delete `model_resolution.rs`**

```powershell
Remove-Item crates/vox-actor-runtime/src/model_resolution.rs
```

Remove the module declaration from `crates/vox-actor-runtime/src/lib.rs`:

```rust
// Remove this line:
pub mod model_resolution;
```

- [ ] **Step 5: Verify compilation**

```powershell
cargo check -p vox-actor-runtime 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 6: Run all actor-runtime tests**

```powershell
cargo test -p vox-actor-runtime 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 7: Commit**

```powershell
git add crates/vox-actor-runtime/src/
git commit -m "refactor(actor-runtime): remove model_resolution.rs; move env-pin logic to ModelRegistry::select()"
```

---

## Task 6: Replace `FreeAiClient` in `AiTaskProcessor`

`AiTaskProcessor` in `crates/vox-orchestrator/src/runtime.rs` constructs a `vox_gamify::ai::FreeAiClient`
directly. This bypasses the model registry entirely. Replace it with an injected `Arc<dyn ModelSelector>`.

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator/tests/runtime_selector_injection.rs` (new file):

```rust
//! Verifies AiTaskProcessor uses the injected ModelSelector, not FreeAiClient.

use std::sync::Arc;
use vox_orchestrator_core::models::{StubModelSelector, ModelSpec};
use vox_orchestrator::runtime::AiTaskProcessor;
use vox_orchestrator::events::EventBus;

#[tokio::test]
async fn ai_task_processor_constructed_with_selector() {
    let stub_spec = ModelSpec::test_default("stub/task-model");
    let selector: Arc<dyn vox_orchestrator_core::models::ModelSelector> =
        Arc::new(StubModelSelector { fixed_model: Some(stub_spec) });

    let bus = EventBus::new();
    // Construction must succeed with an injected selector.
    // It must NOT call network (FreeAiClient::auto_discover does network I/O).
    let _processor = AiTaskProcessor::with_selector(bus, selector);
}
```

- [ ] **Step 2: Run to verify it fails**

```powershell
cargo test -p vox-orchestrator ai_task_processor_constructed_with_selector 2>&1 | tail -10
```

Expected: compile error — `AiTaskProcessor::with_selector` does not exist yet.

- [ ] **Step 3: Rewrite `AiTaskProcessor` in `runtime.rs`**

Find `AiTaskProcessor` in `crates/vox-orchestrator/src/runtime.rs` (lines 73–98 approximately).

Replace:

```rust
// OLD:
pub struct AiTaskProcessor {
    client: vox_gamify::ai::FreeAiClient,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<Orchestrator>,
    provider: String,
    model: String,
}

impl AiTaskProcessor {
    pub async fn new(event_bus: crate::events::EventBus, orchestrator: Arc<Orchestrator>) -> Self {
        let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
        let (provider, model) = client.active_provider_info();
        Self { client, event_bus, orchestrator, provider, model }
    }
```

With:

```rust
// NEW:
pub struct AiTaskProcessor {
    /// Injected model selector — the single source of truth for model choice.
    selector: Arc<dyn vox_orchestrator_core::models::ModelSelector>,
    event_bus: crate::events::EventBus,
    orchestrator: Arc<Orchestrator>,
}

impl AiTaskProcessor {
    /// Construct with an injected model selector. Does not perform network I/O.
    pub fn with_selector(
        event_bus: crate::events::EventBus,
        orchestrator: Arc<Orchestrator>,
        selector: Arc<dyn vox_orchestrator_core::models::ModelSelector>,
    ) -> Self {
        Self { selector, event_bus, orchestrator }
    }
```

Update `run_phase_stream` (the method that actually calls the LLM) to resolve the model at call time:

```rust
async fn run_phase_stream(&self, task: &AgentTask, phase: &str) -> anyhow::Result<String> {
    use vox_orchestrator_core::models::select::{SelectionIntent, SelectionAxes};

    // Resolve model for this specific task.
    let intent = SelectionIntent {
        axes: SelectionAxes { cost: 50, responsiveness: 70, intelligence: 50 },
        task_category: Some(task.category),
        ..Default::default()
    };
    let spec = self.selector.select(intent).await
        .ok_or_else(|| anyhow::anyhow!("No confirmed model available for task {:?}", task.category))?;

    // Build egress request from the resolved spec.
    // (Use the existing vox-llm-egress / vox-actor-runtime chat path,
    //  which expects a provider URL + model id.)
    let provider_url = spec.capabilities.base_url.clone()
        .unwrap_or_else(|| vox_config::OPENROUTER_BASE_URL.to_string());

    // ... rest of the streaming call using provider_url and spec.id ...
    todo!("wire to existing vox_llm_egress::chat_once / vox_actor_runtime::llm::chat_once")
}
```

- [ ] **Step 4: Update `AgentFleet::new()` to inject the registry**

In `crates/vox-orchestrator/src/runtime.rs`, find where `AiTaskProcessor` is constructed inside
`AgentFleet::new()`. Change it to inject the registry as the selector:

```rust
// AgentFleet::new() or wherever AiTaskProcessor is created:
let selector: Arc<dyn vox_orchestrator_core::models::ModelSelector> =
    Arc::clone(&orchestrator.model_registry);  // ModelRegistry must be Arc-wrapped and impl ModelSelector

let processor = Arc::new(AiTaskProcessor::with_selector(
    event_bus.clone(),
    Arc::clone(&orchestrator),
    selector,
));
```

Check how `Orchestrator` holds the `ModelRegistry` — if it's `Arc<Mutex<ModelRegistry>>`,
implement `ModelSelector` on `Arc<Mutex<ModelRegistry>>` instead:

```rust
#[async_trait]
impl ModelSelector for Arc<tokio::sync::Mutex<ModelRegistry>> {
    async fn select(&self, intent: SelectionIntent) -> Option<ModelSpec> {
        let registry = self.lock().await;
        select(&intent, &registry).map(|o| o.spec)
    }
    async fn record_outcome(&self, model_id: &str, category: TaskCategory, success: bool, latency_ms: u64, cost_usd: f64) {
        let mut registry = self.lock().await;
        let (s, f) = registry.arm_stats.entry(model_id.to_string()).or_insert((0, 0));
        if success { *s = s.saturating_add(1); } else { *f = f.saturating_add(1); }
        let _ = (category, latency_ms, cost_usd); // used for future scoreboard writes
    }
}
```

- [ ] **Step 5: Run the test**

```powershell
cargo test -p vox-orchestrator ai_task_processor_constructed_with_selector 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Run full orchestrator suite**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -15
```

Expected: `test result: ok`.

- [ ] **Step 7: Commit**

```powershell
git add crates/vox-orchestrator/src/runtime.rs
git add crates/vox-orchestrator/tests/runtime_selector_injection.rs
git commit -m "feat(orchestrator): replace FreeAiClient in AiTaskProcessor with injected ModelSelector

- AiTaskProcessor no longer performs network I/O at construction time
- AgentFleet injects Arc<dyn ModelSelector> backed by ModelRegistry
- FreeAiClient removed from orchestrator runtime path"
```

---

## Task 7: Final integration check

- [ ] **Step 1: Full workspace compile**

```powershell
cargo check --workspace 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 2: Search for any remaining direct `FreeAiClient` instantiations**

```powershell
rg "FreeAiClient::auto_discover\|FreeAiClient::new" crates/ --files-with-matches
```

Expected: no results (or only in `vox-gamify` itself, which is the definition).

- [ ] **Step 3: Search for any remaining direct `model_resolution` uses**

```powershell
rg "model_resolution\|resolve_chat_provider_route" crates/ --files-with-matches
```

Expected: no results outside of deleted/updated files.

- [ ] **Step 4: Run full test suite for affected crates**

```powershell
cargo test -p vox-orchestrator-core -p vox-actor-runtime -p vox-orchestrator 2>&1 | tail -20
```

Expected: all three report `test result: ok`.

- [ ] **Step 5: Commit final tag**

```powershell
git commit --allow-empty -m "feat: Plan 2 complete — registry-owns-all model selection

- ModelSelector trait defined in vox-orchestrator-core
- ModelConfidence::Confirmed is now a hard routing gate
- ModelRegistry implements ModelSelector
- vox-actor-runtime model_resolution.rs removed
- AiTaskProcessor uses injected Arc<dyn ModelSelector>
- Env-pin logic (VOX_MODEL, VOX_SELECTOR_MODEL) moved to ModelRegistry::select()"
```
