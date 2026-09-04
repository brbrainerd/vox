---
title: "Model Catalog SSOT — Architecture & Implementation Plan 2026"
description: "Evidence-based audit of split-brain problems in the model catalog pipeline, and the complete plan to achieve a single source of truth with automatic model adoption."
category: "Architecture SSOTs"
sort_order: 42
status: "current"
---

# Model Catalog SSOT — Audit & Implementation Plan (2026)

> **Scope:** `crates/vox-orchestrator/src/{catalog.rs,models/}` · `contracts/orchestration/model-catalog.bootstrap.v1.json` · `contracts/orchestration/model-routing.v1.yaml`

---

## Part 1 — Confirmed Split-Brain Bugs (True Positives)

Every item below is proven with exact file + line references.

---

### Bug 1 — Refresh-Interval Key Name Mismatch (Silent No-Op Throttle)

| | File | Line |
|---|---|---|
| **Reads** | `models/registry.rs` | 279 |
| **Writes** | `models/registry.rs` | 386 |

The throttle check reads `"openrouter_catalog_refresh"` but writes `"catalog_refresh"`. The check never finds its own timestamp. Every process startup triggers a full OpenRouter + LiteLLM network fetch regardless of the configured `VoxOpenRouterCatalogMinRefreshIntervalSecs`. Wasted bandwidth, slower startup, rate-limit risk.

**Fix:** Unify to a single constant `MODEL_CATALOG_LAST_REFRESH_KEY = "model_catalog_last_refresh"`.

---

### Bug 2 — `register()` Silently Overwrites Telemetry-Calibrated Pricing

| File | Lines |
|---|---|
| `models/registry.rs` | 574–576 (unconditional insert) |
| `models/registry.rs` | 443–446 (call site during refresh) |

`inject_pricing_catalog()` correctly guards `PricingSource::Telemetry` (lines 116–117), but the background refresh loop calls the unguarded `register()` for every model afterwards. A model whose cost was calibrated from real observed spend (e.g. `$0.0045/1k`) is overwritten by the stale OpenRouter catalog price on the next startup.

**Fix:** `register()` must merge-but-preserve pricing fields when `existing.pricing_source == Telemetry`.

---

### Bug 3 — AnthropicDirect Registers Expensive Models as `is_free: true`

| File | Lines |
|---|---|
| `catalog.rs` | 602–623 |

```
let (c_in, c_out) = (0.0_f64, 0.0_f64);
let pricing_unknown = c_in == 0.0 && c_out == 0.0;
// ...
is_free: pricing_unknown,   // Claude Opus → is_free = true
```

When LiteLLM is unreachable, newly discovered Anthropic models enter as `is_free: true`. The `Economy` preference picker ranks them highest (best quality score, best latency score, zero apparent cost). A $75/M token model gets invoked. `BudgetManager` cannot catch it because `cost_per_1k = 0.0`.

**Fix:** Never set `is_free: true` for Anthropic models. Mark as `PricingSource::Unknown`. The routing gate (Wave 1) blocks `Unknown` from autonomous dispatch.

---

### Bug 4 — Bootstrap JSON Has `"max_context": 0` for Every Single Model

| File | Lines |
|---|---|
| `contracts/orchestration/model-catalog.bootstrap.v1.json` | 19, 39, 59, 79, 99, 119, 139, 159, 179, 199 |

Every capabilities block in the bootstrap JSON has `"max_context": 0` despite `max_tokens` being correctly set (e.g. `128000`). Any routing path that reads `capabilities.max_context` for long-context filtering receives a broken signal on cold-start.

**Fix:** Populate `max_context` to match `max_tokens` in the bootstrap file, or add normalization in `ModelConfig::default()` that sets `cap.max_context = spec.max_tokens` when zero.

---

### Bug 5 — `premium_alias` Defined in Two Places With No Sync

| Location | |
|---|---|
| `models/spec.rs:249–265` | Rust: `built_in_premium_alias()` |
| `contracts/orchestration/model-routing.v1.yaml:84–92` | YAML: `premium_alias:` block |

Both define the exact same model-ID-to-task mappings. The YAML is never loaded at runtime — `ModelRegistry::new()` always uses the Rust defaults. Editing the YAML has zero effect on routing.

**Fix:** Delete `built_in_premium_alias()`. Parse `model-routing.v1.yaml` at startup via `vox_config::ModelRoutingConfig`. Use the YAML as the only source. Add a `vox ci model-routing-check` guard.

---

### Bug 6 — Scoring Constants Duplicated From YAML (Resolved 2026-09-03: all four are read)

| Constant | `scoring.rs` | `model-routing.v1.yaml` | Loaded at runtime? |
|---|---|---|---|
| `LATENCY_EXCELLENT_MS = 500.0` | line 25 | line 81 | ✅ read at `crates/vox-orchestrator/src/models/scoring.rs:76-78` (via `latency_bands.excellent_ms`) |
| `LATENCY_POOR_MS = 8_000.0` | line 27 | line 82 | ✅ read at `crates/vox-orchestrator/src/models/scoring.rs:76-78` (via `latency_bands.poor_ms`) |
| `exploration.budget_usd_per_day: 50.0` | not present | line 21 | ✅ read at `crates/vox-orchestrator/src/runtime.rs:708-710` |
| `safety.max_cost_usd_per_request: 5.0` | not present | line 25 | ✅ read at `crates/vox-orchestrator/src/models/registry.rs:838-840` (`safety_cap` in `best_for_task_with_filter`), enforced `:848-849` |

As of 2026-09-03 all four are read through `vox_config::load_model_routing_config()`; this table
previously rotted because nothing mechanically checks it against source.

---

### Bug 7 (RESOLVED) — `quality_weights` in YAML is consumed

| File | Lines |
|---|---|
| `contracts/orchestration/model-routing.v1.yaml` | 8–13 |
| `models/scoring.rs` | 58–108 |

```yaml
quality_weights:
  socrates_factuality: 0.25
  contradiction_inverse: 0.15
  success_rate: 0.25
  p50_latency_inverse: 0.15
  cost_inverse: 0.2
```

This section previously claimed the block was "dead config — declared but never
consumed". That is no longer true. `scoreboard_feedback_boost` takes a
`&vox_config::model_routing::QualityWeightsConfig` and applies all five weights
to the scoreboard signals, normalising by their sum:

```text
(socrates_factuality * quality_score
 + contradiction_inverse * (1 - min(1 - success_rate, 1))
 + success_rate        * success_rate
 + p50_latency_inverse * latency_band_score(p50_latency_ms)
 + cost_inverse        * 1/(1 + 100 * cost_per_success_usd))
/ sum(weights) * 0.15
```

`AutoRoutingPriority::from_env()` (the `VOX_ROUTE_*` vars) still governs the
*separate* priority axis; it is not a substitute for these weights.

**Caveat that does still hold:** the `quality_score` this blend reads is
`COALESCE(AVG(llm_feedback.rating) / 5.0, 1.0)` over a table with zero rows, so
the `socrates_factuality` term currently contributes a constant. Fixing that is
Task M2 of the chat-harness-unification plan; the weight plumbing itself is
correct.

---

### Bug 8 — `HuggingFaceCatalog` Returns Three Hardcoded Models, One Deprecated

| File | Lines |
|---|---|
| `catalog.rs` | 399–403 |

```rust
let known_models = vec![
    "Qwen/Qwen2.5-72B-Instruct",
    "meta-llama/Llama-3.1-70B-Instruct",
    "mistralai/Mixtral-8x7B-Instruct-v0.1",  // deprecated late 2024
];
```

All three are registered with `is_free: true` and `$0.0` cost regardless of the user's HF account tier. The HF Inference Providers API (`/api/models?inference=warm`) provides dynamic discovery and is not used.

**Fix:** Replace the static list with a call to HF's warm-inference endpoint. Pass results through `ModelAdmissionFilter` (Wave 2).

---

### Bug 9 — DeepSeek Off-Peak Bonus Misses OpenRouter-Routed DeepSeek Models

| File | Lines |
|---|---|
| `models/scoring.rs` | 289–299 |

```rust
if matches!(m.provider_type, ProviderType::DeepSeek) && is_deepseek_off_peak()
```

The bonus only fires for `ProviderType::DeepSeek`. When DeepSeek R1 is accessed via OpenRouter (the common path — `provider_type = OpenRouter`), the off-peak bonus is never applied. R1 scores identically at 3am and 3pm, defeating the discount window entirely.

**Fix:** Match on `m.id.to_ascii_lowercase().contains("deepseek")` instead of `provider_type`, or introduce a `time_of_day_discount: f32` field that the LiteLLM oracle populates.

---

### Bug 10 — Exploration Budget Contract Is Never Enforced

| File | Line |
|---|---|
| `contracts/orchestration/model-routing.v1.yaml` | 21 |

```yaml
budget_usd_per_day: 50.0
```

No Rust code enforces a daily USD cap on exploration calls. If Thompson bandit scoring is miscalibrated or a novel model is mislabeled as excellent, the system can spend unboundedly on exploration.

**Fix:** Add `exploration_usd_today: AtomicU64` to `BudgetManager`. Gate `record_novel_routing_explore()` against the ceiling from `ModelRoutingConfig`.

---

## Part 2 — Eliminated False Positives (from Prior Plan)

| Prior Claim | Verdict | Reason |
|---|---|---|
| "Six catalog implementations is fragmented" | False positive | Each provider has different auth/discovery APIs. Multiplicity is intentional. The bugs are *within* them, not in their count. |
| "Anthropic tier-by-name is a hack" | False positive | Tier display is harmless. Bug 3 (pricing=0.0 → is_free=true) is the real danger. |
| "Bootstrap JSON format should be YAML" | False positive | Format doesn't matter. Content (max_context=0, stale prices) is the real issue. |
| "Pricing Oracle Risk is catastrophic" | Overstated | LiteLLM failure degrades to Bootstrap pricing, not zero. The specific path to $0 cost is only via Bug 3 (AnthropicDirect). |

---

## Part 3 — Target Architecture

### Design Principle

> **OpenRouter is the discovery authority. `model-routing.v1.yaml` is the behavioral authority. VoxDb telemetry is the cost authority. These three must be wired together — the current system treats them as independent.**

### Data Flow (Target State)

```
DISCOVERY (Who exists?)
  OpenRouter /api/v1/models ──►
  AnthropicDirect /v1/models ──► ModelAdmissionFilter ──► ModelRegistry
  HuggingFace /api/models ────►   ├─ capability inference
  MensCatalog (local) ─────────►  ├─ strength inference (from YAML rules)
                                  └─ initial tier + PricingSource assignment

PRICING (What do they cost?)
  LiteLLM oracle ──► apply_litellm_pricing()         [Oracle]
  VoxDb telemetry ──► inject_pricing_catalog()        [Telemetry — highest]
  Bootstrap seed ──► ModelConfig::default()           [Fallback — lowest]
  PricingSource::Unknown ──► BLOCKED from autonomous routing

SCORING (How good are they?)
  auto_score_model() reads from:
    • model-routing.v1.yaml (latency_bands, weights, exploration budget)
    • ModelSpec.capabilities (live from discovery)
    • ModelScore from VoxDb scoreboard (success_rate, p50_latency_ms)
    • BudgetManager (exploration quota, doom-loop gate)

ROUTING (Who gets the task?)
  ModelRegistry.best_for_task()
    Filter: PricingConfidence gate (Unknown → blocked)
    Filter: ExplorationBudget gate (daily cap enforced)
    Filter: CircuitBreaker / penalty_map
    Rank: auto_score_model() + Thompson bandit arm stats
    premium_alias → read from model-routing.v1.yaml ONLY
```

### Automatic Model Adoption Pipeline

New models released on OpenRouter are automatically discovered, classified, scored, and routed without manual intervention:

```
1.  OpenRouter refresh (hourly) returns N new model IDs
2.  ModelAdmissionFilter runs each new model through:
      a. infer_strengths()   — from YAML strength_inference rules
      b. infer_tier()        — context window size + provider family
      c. LiteLLM match       — pricing lookup (Bootstrap→LiteLLM if found)
      d. Set PricingSource::Unknown if no LiteLLM match
3.  New model enters registry with PricingSource set
4.  If PricingSource == Unknown:
      - Allowed in manual/interactive sessions
      - BLOCKED in autonomous task dispatch
      - Written to VoxDb model_admission_queue
5.  On first N=3 successful calls:
      - Actual cost recorded via BudgetManager.record_cost()
      - PricingSource promoted to Telemetry
      - Model fully admitted to autonomous routing
6.  Thompson bandit tracks success_rate, quality_score, p50_latency
7.  auto_score_model() ranks model against peers continuously
8.  ModelRegistry.best_for_task() routes to it when it wins
```

---

## Part 4 — Implementation Tasks

### Wave 0 — Critical Bug Fixes (Regressions Today, No Architecture Change)

| Task | File | Bug Fixed |
|---|---|---|
| W0-1: Unify refresh key name constant | `models/registry.rs:279,386` | Bug 1 |
| W0-2: Guard `register()` against Telemetry overwrite | `models/registry.rs:574` | Bug 2 |
| W0-3: Normalize `max_context=0` in bootstrap | `models/spec.rs:294` | Bug 4 |
| W0-4: Remove `is_free: pricing_unknown` from AnthropicDirect | `catalog.rs:623` | Bug 3 |
| W0-5: Fix DeepSeek off-peak provider-type check | `models/scoring.rs:289` | Bug 9 |

### Wave 1 — SSOT Wiring (Behavioral Authority from YAML)

| Task | Files |
|---|---|
| W1-1: Add `PricingSource::Unknown` variant | `models/spec.rs` |
| W1-2: Add Pricing Confidence Gate to `best_for_internal()` | `models/registry.rs` |
| W1-3: Expose `model-routing.v1.yaml` at runtime via `vox_config::ModelRoutingConfig` | `crates/vox-config/` |
| W1-4: Delete `built_in_premium_alias()`, read from YAML | `models/spec.rs`, `registry.rs` |
| W1-5: Replace `const` scoring values with `ModelRoutingConfig` reads | `models/scoring.rs` |
| W1-6: Resolve `quality_weights` / `scoring.weights` ambiguity | `model-routing.v1.yaml`, `scoring.rs` |
| W1-7: Enforce exploration daily budget in `BudgetManager` | `budget/mod.rs` |

### Wave 2 — Automatic Model Adoption

| Task | Files |
|---|---|
| W2-1: `ModelAdmissionFilter` struct | New: `models/admission.rs` |
| W2-2: Wire admission to `maybe_refresh_catalogs()` | `models/registry.rs` |
| W2-3: Persist `PendingPricing` models to VoxDb `model_admission_queue` | `budget/persistence.rs` |
| W2-4: `vox model catalog status` CLI command | `crates/vox-cli/src/commands/model.rs` |
| W2-5: Replace `HuggingFaceCatalog` static list with HF warm API | `catalog.rs` |

### Wave 3 — Enforcement & Maintainability

| Task | |
|---|---|
| W3-1: `vox ci model-routing-check` guard | Verify no hardcoded aliases, constants within 1% of YAML, no max_context=0 in bootstrap |
| W3-2: Bootstrap JSON as `@generated` artifact | Regenerate from YAML + admission filter output; block manual edits in CI |
| W3-3: Doc-sync `PricingSource` priority ladder | Code comment must exactly mirror enforced priority order in `register()` and `best_for_internal()` |

---

## Part 5 — Traceability Matrix

| Bug | Fixed by | Priority |
|---|---|---|
| Refresh key mismatch | W0-1 | P0 |
| `register()` overwrites Telemetry | W0-2 | P0 |
| AnthropicDirect `is_free` lie | W0-4 + W1-1 + W1-2 | P0 |
| `max_context: 0` in bootstrap | W0-3 | P1 |
| DeepSeek off-peak provider check | W0-5 | P1 |
| Exploration budget unenforced | W1-7 | P1 |
| `premium_alias` duplication | W1-4 | P1 |
| Scoring constants not from YAML | W1-5 | P2 |
| `quality_weights` dead config | W1-6 | P2 |
| HF hardcoded model list | W2-5 | P2 |
| Automatic new model adoption | W2-1 → W2-3 | P2 |
