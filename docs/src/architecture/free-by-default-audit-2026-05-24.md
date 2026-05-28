---
title: "Free-by-Default Audit (2026-05-24)"
description: "Post-implementation audit of the 89 free_only / is_free call sites after the F-F sprint landed ModelTier::Free + Fast and flipped CostPreference default to Economy. Documents what changed, watchlists, and three follow-up gaps (Balanced→Economy, exploration parity, RoutingProfile docs) — all closed in commit 7f2edd8e7e."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-25"
training_eligible: true
sort_order: 37
---

# Free-by-Default Audit — 2026-05-24

**Status:** F-F track complete. Free-by-default is now live.

---

## 1. What Changed

| # | Change | File |
|---|--------|------|
| F-F-1 | Added `Free` and `Fast` variants to `ModelTier` enum (YAML → generated) | `contracts/orchestration/model-routing.v1.yaml` |
| F-F-2 | `CostPreference` derives `Default`; `Economy` is the `#[default]` variant | `vox-orchestrator/src/config/enums.rs` |
| F-F-3 | `default_cost_preference()` returns `Economy` (was `Performance`) | `vox-orchestrator/src/config/defaults.rs` |
| F-F-4 | Added `supports_file_input: bool` and `supports_jsonl: bool` to `ModelCapabilities` | `vox-orchestrator/src/models/spec.rs` |
| F-F-5 | Added `supports_web_search()`, `supports_file_input()`, `supports_jsonl()` methods on `ModelSpec` | `vox-orchestrator/src/models/spec.rs` |
| F-F-6 | New `RoutingProfile` enum (7 variants) in `vox-orchestrator::types` | `vox-orchestrator/src/types/routing.rs` |
| F-G   | Re-activated `selection/` in `vox-dei-shim` (7 files, 10 tests passing) | `vox-dei-shim/src/selection/` |
| Bonus | Reclassified 9 bootstrap catalog models to `Free`/`Fast` tier | `contracts/orchestration/model-catalog.bootstrap.v1.json` |

---

## 2. Call-Site Audit (89 `is_free` / `free_only` sites)

### Category A — Core routing logic (✅ consistent)

| Site | What it does | Assessment |
|------|--------------|------------|
| `registry.rs:680,778` | Skip free models when `CostPreference::Performance` | Correct — Performance mode intentionally excludes free |
| `registry.rs:841,858,868,877,894` | `list_free_*` registry query methods | First-class free API — correct |
| `scoring.rs:80` | Reduces quality_score for `is_free` models (QUALITY_FREE_PAID_COMPONENT=0.35 vs 0.95) | Intentional — free models score lower in quality; economy preference compensates |
| `registry_model_resolve.rs:71,91,128` | `free_only` filter in inference predicate | Correct — honours explicit free-only constraint |
| `selection/scorer.rs` | Economy: `economy_free_bonus +3.0`; Performance: `performance_free_penalty -1.0` | Correct |
| `selection/free_tier.rs` | `FreeTierRouter` filters to `is_free=true` only | Correct |

### Category B — MCP bridge (✅ consistent)

All 18 `free_only` sites in `vox-orchestrator-mcp` are propagating a free-tier decision from `resolve_chat_llm_model` / `resolve_mcp_chat_model` through to the infer layer. The consistency check at `infer.rs:265` warns if `routing.free_only != model.is_free`. No changes needed.

### Category C — Potential false positives (🔍 watchlist)

| Site | Risk | Mitigation |
|------|------|------------|
| `registry.rs:680` — `Performance && is_free` skip | Could starve high-quality free models in Performance sessions | Acceptable: Performance is an explicit user opt-in; free models still win under Economy (default) |
| `QualityLevel::Balanced → Performance` in `mode.rs:27` | Default InferenceConfig still resolves to Performance cost preference | ⚠️ Tracked below — see §3 |
| `scoring.rs:80` — quality score 0.35 for free models | Free models score ~60% less than paid on quality axis | Intentional; economy weight compensates; net result is free models win under Economy |

### Category D — Bootstrap catalog (✅ fixed)

9 models reclassified from `Light`/`Local` to `Free` or `Fast`:
- **Free (5):** `qwen/qwen3-coder-next-32b`, `qwen/qwen3-coder:free`, `meta-llama/llama-4-scout:free`, `google/gemini-2.0-flash-lite`, (1 existing)
- **Fast (4):** `anthropic/claude-haiku-4.5`, `openai/gpt-5-mini`, `google/gemini-3-flash`, `google/gemini-3.1-flash-lite`, `deepseek/deepseek-v4-flash`

---

## 3. Remaining Gap: `QualityLevel::Balanced → CostPreference::Performance`

In `vox-orchestrator/src/mode.rs`:
```rust
impl QualityLevel {
    pub fn to_cost_preference(self) -> CostPreference {
        match self {
            Self::Flash => CostPreference::Economy,
            Self::Balanced | Self::Premium => CostPreference::Performance,  // ← gap
        }
    }
}
```

`InferenceConfig` defaults to `QualityLevel::Balanced`, which maps to `Performance`. This means when the registry resolver uses `cfg.quality.to_cost_preference()` (not the `default_cost_preference()` path), the result is still `Performance`.

**Recommendation:** Change `Balanced → Economy` to complete free-by-default. This is a one-line change but has broader user-visible impact (existing sessions using "balanced" quality may see different model selection). Defer to a separate tracked item tagged `free-by-default-follow-up`.

**Workaround in place:** The `default_cost_preference()` path (OrchestratorConfig) now returns `Economy`, which covers the common session-start path. The InferenceConfig path is only reached when an explicit quality level is requested.

---

## 4. Exploration Bonus Gap

In `vox-orchestrator/src/routing/engine.rs:102-106`:
```rust
if self.policy.routing_objective.kind == "quality_first"
    && s + f == 0
    && matches!(m.capabilities.tier, ModelTier::Pro)
{
    base += 0.06;
}
```

New `Free` and `Fast` models also benefit from exploration (Thompson exploration covers them), but the hardcoded Pro-tier exploration bonus does not apply. Under quality_first routing, Free/Fast newcomers compete fairly on Thompson draws but don't get the Pro novelty boost.

**Recommendation (not urgent):** Extend to `ModelTier::Pro | ModelTier::Fast | ModelTier::Free` for equal exploration across all tiers that might be optimal. Tag `exploration-parity-follow-up`.

---

## 5. True Positives (healthy patterns)

- `FreeTierRouter` in `vox-dei-shim::selection` provides intelligent multi-candidate routing with FIM, latency, and vision constraint handling.
- `infer.rs:250` enforces free-only at runtime — no leakage of paid models when `free_only=true`.
- `list_free_models_for_strength()` and `list_free_models_for_strength_with_pred()` in registry.rs give callers clean first-class free-model APIs.
- `economy_free_bonus: 3.0` in `ScoringWeights` means free models out-compete paid by 3 points in Economy mode, making them the default winner when available.

---

## 6. Next Steps

| Item | Priority | Effort | Status |
|------|----------|--------|--------|
| `QualityLevel::Balanced → Economy` | Medium | 1 line | ✅ Done — `mode.rs` Flash+Balanced→Economy, Premium→Performance |
| Exploration bonus parity (Pro/Fast/Free) | Low | 3 lines | ✅ Done — `engine.rs` novelty bonus extended to Fast+Free |
| Update where-things-live.md to document `RoutingProfile` | Low | 1 row | ✅ Already present (vox-dei-shim + vox-orchestrator rows) |
