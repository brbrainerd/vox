---
title: "Vox Model Autonomic System — L1/L2/L3 Design (2026-Q2)"
description: "Continuous-discovery + auto-classification + selection-SSOT architecture replacing hand-curated model bootstrap."
category: "Architecture SSOTs"
status: "current"
---

# Vox Model Autonomic System

**Council-ratified 2026-05-15.** Companion to [`model-selection-2026-q2.md`](./model-selection-2026-q2.md) §8.

## 0. Problem

The current model pipeline mixes three concerns into one hand-edited file:
the bootstrap catalog is simultaneously (a) the cold-start fallback, (b) the
SSOT humans edit when a new model ships, and (c) the pin list for
reproducibility. Result: every new frontier model release blocks on a manual
PR to `model-catalog.bootstrap.v1.json`, and selection logic is scattered
across two parallel paths (`select()` + `resolve_model_with_registry_fallbacks`)
plus several provider-default constants.

## 1. The three loops

```
┌───────────────────────────────────────────────────────────────┐
│  L3  Council-review (quarterly + on alert)                     │
│      • approves Provisional → Confirmed tier promotions        │
│      • approves premium_alias rotations                        │
│      • reads council-report.md (auto-generated)                │
└───────────────────────────────────────────────────────────────┘
                  ▲ council-report.md
┌───────────────────────────────────────────────────────────────┐
│  L2  Continuous classification (NEW)                           │
│      • classifier LLM (Haiku/Flash-tier) consumes              │
│        (id, description, param_graph, sample_pricing)          │
│        → emits {tier, strengths[], confidence}                 │
│      • shadow-runs new model on eval panel for N samples       │
│      • DB scoreboard converges on success/cost/latency         │
│      • confidence ≥ threshold → Provisional → Confirmed        │
└───────────────────────────────────────────────────────────────┘
                  ▲ provisional ModelSpec + DB rows
┌───────────────────────────────────────────────────────────────┐
│  L1  Continuous discovery (extension of existing)              │
│      OpenRouter /models • LiteLLM pricing • Anthropic /models  │
│      • runs on a schedule (nightly cron), not just at startup  │
│      • diffs against registry → emits DiscoveryEvent           │
└───────────────────────────────────────────────────────────────┘
```

## 2. Existing pieces we reuse

| Surface | What it does today | How L1/L2/L3 uses it |
|---|---|---|
| `OpenRouterCatalog::refresh()` | One-shot `/models` fetch | L1: scheduled refresh |
| `LiteLLMCatalog::fetch()` | Pricing enrichment | L1: scheduled refresh |
| `AnthropicDirectCatalog::refresh()` | Key-gated Anthropic catalog | L1: scheduled refresh |
| `infer_strengths()` (catalog.rs) | Parameter-graph → strengths | L2: prior for classifier |
| `ModelRegistry::premium_alias_for()` | Task→pin lookup | L3: read from pins.yaml |
| `select(intent, registry)` | Multi-axis SSOT picker | runtime: unchanged |
| `vox-db model_scoreboard` | Per-model success/cost/latency | L2: convergence signal |
| `vox-db model_pricing_catalog` | Telemetry-confirmed pricing | L2: confidence promotion |

## 3. New contracts

### 3.1 `contracts/orchestration/model-pins.v1.yaml`

Council-reviewed, infrequently changed. Pin list separate from catalog:

```yaml
schema: vox.orchestration.pins/v1
premium_alias:
  codegen: anthropic/claude-opus-4.7
  research: google/gemini-3.1-pro
  review: anthropic/claude-sonnet-4.6
  planning: openai/gpt-5.5-pro
  # …
version_pins:
  # When CR-L0 eval-panel reproducibility matters, lock these.
  llm-panel.claude-sonnet: claude-sonnet-4-6
  llm-panel.gpt-frontier: gpt-5.4
council_signoff:
  rotation_id: 2026-Q2-rotation-2
  approved_by: [council]
  approved_at: 2026-05-15
```

### 3.2 `contracts/orchestration/catalog-fallback.v1.json`

What `model-catalog.bootstrap.v1.json` becomes after the rename: a minimal
**emergency-offline** subset (~5–10 models) covering each tier, used only
when L1 discovery has never succeeded. Not the SSOT.

### 3.3 Confidence states

Each `ModelSpec` carries `confidence: Confidence`:

```
Provisional  // discovered, classifier-tagged, no scoreboard data yet
Shadowed     // running on eval panel; not eligible for production routing
Confirmed    // scoreboard data passes thresholds; eligible everywhere
Deprecated   // failing thresholds OR retired by council
```

## 4. New telemetry events

```rust
// fired by select() on every selection
SelectionDecisionEvent {
    intent_caller: Option<&'static str>,   // "repair-loop", "research", …
    task: TaskCategory,
    axes: (u8, u8, u8),                    // (cost, responsiveness, intelligence)
    chosen_model: String,
    reason: SelectionReason,
    timestamp_ms: u64,
}

// fired by L1 when a model id appears that wasn't in the prior catalog
DiscoveryEvent {
    source: DiscoverySource,               // OpenRouter | LiteLLM | Anthropic | Mesh
    model_id: String,
    seen_at_ms: u64,
}

// fired by L2 when classifier completes
ClassificationEvent {
    model_id: String,
    classifier_model: String,              // which LLM classified it
    tier: ModelTier,
    strengths: Vec<StrengthTag>,
    confidence: f32,                       // 0.0–1.0
    timestamp_ms: u64,
}

// fired when confidence crosses a state boundary
ConfidencePromotionEvent {
    model_id: String,
    from: Confidence,
    to: Confidence,
    evidence: PromotionEvidence,           // ScoreboardThreshold | CouncilApproval
    timestamp_ms: u64,
}
```

These feed the L3 council report and CR-L8 corpus-feedback flywheel.

## 5. New CLI surfaces

```
vox models discover        # run L1 refresh manually
vox models classify <ID>   # run L2 classifier on a model id
vox models shadow <ID>     # run eval-panel against a Provisional model
vox models council-report  # generate the L3 quarterly markdown
```

## 6. Roll-out phases

| Phase | What | Status |
|---|---|---|
| **A** | `SelectionDecisionEvent` emit from `select()` | landed 2026-05-15 |
| **B** | Migrate `registry_model_resolve` to wrap `select()` | landed 2026-05-15 |
| **C** | Migrate `vox-code-audit::default_*_model()` to `select()` | landed 2026-05-15 |
| **D** | Split pinning from cataloging: `model-pins.v1.yaml` + rename bootstrap | landed 2026-05-15 |
| **E** | `vox models classify` scaffold + classifier prompt schema | landed 2026-05-15 |
| **F** | Nightly catalog-diff infrastructure + `DiscoveryEvent` | landed 2026-05-15 |
| **G** | Shadow-eval hook into llm-panel for Provisional models | landed 2026-05-15 |

Phases E/F/G land as **scaffolds** — the surfaces, types, and entry points
exist and are testable, but real LLM-classifier calls and the cron scheduler
are gated behind feature flags until council approves go-live.

## 7. Backwards compatibility

- `bootstrap_inference::*` constants kept as last-resort fallbacks; not in
  the hot path.
- `model-catalog.bootstrap.v1.json` kept at its filename for one release
  with a deprecation pointer to `catalog-fallback.v1.json`.
- `resolve_model_with_registry_fallbacks` retained as a thin wrapper over
  `select()` so older callers don't break.

## 8. Open questions

- **Classifier model choice.** Haiku 4.5 vs Gemini 3.1 Flash-Lite. Both
  support structured-output JSON schema. Lean Haiku for the prompt-cache
  TTL during shadow runs.
- **Promotion threshold.** Currently proposed: 30 successful calls + p50
  latency < 2× catalog median + cost telemetry confidence `High`. Council
  to ratify before turning F on.
- **Mesh peer classification.** PopuliMesh entries don't have an OpenRouter
  description. Treat them as `Confirmed`+`Local` at registration time and
  let scoreboard drive demotion.

---

*Document dated 2026-05-15. SSOT for the model-autonomic system; supersedes
the bootstrap-as-SSOT model.*
