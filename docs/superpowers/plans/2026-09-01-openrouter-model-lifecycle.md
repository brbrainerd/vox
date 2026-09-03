---
title: "OpenRouter Model Lifecycle — Discovery, Auto-Benchmark, and Routing Adoption"
description: "Plan for detecting newly-published OpenRouter models, benchmarking them automatically on three axes (efficiency, responsiveness, intelligence) with context-fitted Vox prompts, and promoting them into the GUI's auto modes only once evidence exists."
category: "Plans"
status: "draft"
training_eligible: false
---

# OpenRouter Model Lifecycle Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans.
>
> **Depends on:** [benchmark v2](2026-09-01-vox-efficacy-benchmark-v2.md) Phases 0–E (the scorer and verifier). **Audit context:** [adversarial audit](../../src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md).

**Goal:** When a model is published on OpenRouter, Vox notices within hours, benchmarks it on Vox code generation with a prompt fitted to its context window, scores it on efficiency / responsiveness / intelligence, and — **only if the evidence supports it** — makes it available to the GUI's automatic modes, preferring newer models of a family.

**Architecture:** Three insights from the code audit make this far cheaper than it looks.

1. **The confidence state machine already exists and already gates routing.** `ModelConfidence::{Provisional, Shadowed, Confirmed, Deprecated}` with `eligible_for_routing() == Confirmed` (`crates/vox-orchestrator/src/models/autonomic.rs:44-68`). It is inert only because `confidence_state_for_model` passes a hardcoded `None` where the scoreboard row belongs (`select.rs:277`). Passing the real row is a **one-line change** that turns "benchmark before you trust it" on.
2. **Measured quality is already read in two places** — `scoreboard_feedback_boost` (`scoring.rs:60-108`) and `get_effective_cost` (`registry.rs:795`, `base_cost * (2.0 - quality_score)`). It changes nothing today only because `model_scoreboard.quality_score` is `COALESCE(AVG(llm_feedback.rating)/5.0, 1.0)` over an empty table (`ops_scientia.rs:191`), so every model reads a flat `1.0`. **Writing a real number lights up both readers with no selection-code change.**
3. **OpenRouter already publishes everything needed** — verified against the live catalog (419 models, 2026-09-01): `created` (Unix publication timestamp), `canonical_slug` (dated, the reproducibility key), `knowledge_cutoff`, `expiration_date`, `context_length`, `supported_parameters` (317 support `seed`), a `reasoning` object (292 models), an `alias_target` on 12 `~…-latest` pseudo-models, and even a `benchmarks` block carrying Artificial Analysis indices on 238 models.

So this plan is mostly **wiring existing mechanisms to real evidence**, not building new machinery.

**Tech Stack:** Rust, `reqwest`, `serde`, `clap`, `vox_actor_runtime::llm`, React/TS (GUI).

## Global Constraints

Inherits every constraint from [benchmark v2](2026-09-01-vox-efficacy-benchmark-v2.md), plus:

- **Never benchmark a `~…-latest` id.** Resolve `alias_target` and benchmark the concrete slug; the alias moves under you. Record both.
- **Never benchmark `:nitro` / `:floor` / `:exacto`.** These change endpoint selection per request, so the measurement is not attributable to a model.
- **Pin the provider or the measurement is void.** `provider: {order:[…], allow_fallbacks:false, require_parameters:true, quantizations:[…]}`. The same model id is served by endpoints with a 3.6× price spread, different quantizations, and different `supported_parameters`. Record `provider_name` from the generation endpoint and **discard any attempt served by a different provider than pinned**.
- **`canonical_slug` is the reproducibility key**, not `id` — an undated `id` silently re-points when a revision ships. But **key the discovery diff on `id`**: 72 canonical_slugs serve multiple ids.
- **Dedupe benchmark targets by `canonical_slug`** — `:free` and `:batch` are the same weights and must not be paid for twice.
- **Respect `expiration_date`** — preview models genuinely vanish; never schedule past it.
- **No LLM judge in the correctness path.** The judge scores *quality among already-verified-correct solutions* and is reported as its own axis. It never converts a fail to a pass or vice versa, and it must be a **different model family** than the system under test (self-preference bias, arXiv:2410.21819).

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/vox-orchestrator/src/catalog.rs` | **Modify.** Parse the dropped OpenRouter fields. |
| `crates/vox-orchestrator/src/models/spec.rs` | **Modify.** Carry them on `ModelSpec`. |
| `crates/vox-orchestrator/src/models/lifecycle.rs` | **Create.** Variant/alias/recency logic. |
| `crates/vox-orchestrator/src/orchestrator/catalog_refresh.rs` | **Modify.** Run discovery in the background loop. |
| `crates/vox-cli/src/commands/model/discover.rs` | **Modify.** Add `--json`. |
| `crates/vox-corpus/src/humaneval_runner/probe.rs` | **Create.** The deterministic triage probe. |
| `crates/vox-corpus/src/humaneval_runner/context_fit.rs` | **Create.** Context-window-fitted prompt assembly. |
| `crates/vox-cli/src/commands/model/probe.rs` | **Create.** `vox model probe`. |
| `crates/vox-orchestrator/src/models/select.rs` | **Modify.** Pass the real scoreboard row (one line). |
| `crates/vox-db/src/store/ops_scientia.rs` | **Modify.** Source `quality_score` from benchmark evidence. |
| `crates/vox-gui/ui/src/components/surfaces/Settings/` | **Modify.** Surface evidence; stop hardcoding presets. |

---

# Phase G — Catalog metadata and discovery

### Task 1: Parse the OpenRouter fields currently dropped

`OpenRouterCatalog`'s deserialize struct (`catalog.rs:36-55`) omits `created`, upstream `canonical_slug`, `knowledge_cutoff`, `expiration_date`, `alias_target`, `reasoning`, `benchmarks`, and pricing `overrides`. Without `created` and `knowledge_cutoff` there is no new-model detection and no contamination window.

**Files:** Modify `crates/vox-orchestrator/src/catalog.rs`, `crates/vox-orchestrator/src/models/spec.rs`.

**Interfaces:**
- Produces on `ModelSpec`: `pub created_unix: Option<i64>`, `pub upstream_canonical_slug: Option<String>`, `pub knowledge_cutoff: Option<String>`, `pub expiration_date: Option<String>`, `pub alias_target: Option<String>`, `pub external_benchmarks: Option<ExternalBenchmarks>`.

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator/src/catalog.rs`'s test module:

```rust
    /// Real shape from the live /api/v1/models response (2026-09-01).
    const LIVE_SAMPLE: &str = r#"{"data":[{
      "id":"anthropic/claude-fable-5.1",
      "canonical_slug":"anthropic/claude-fable-5.1-20260831",
      "name":"Anthropic: Claude Fable 5.1",
      "created":1788285838,
      "context_length":1000000,
      "knowledge_cutoff":"2026-02-16",
      "expiration_date":"2098-12-31",
      "pricing":{"prompt":"0.000003","completion":"0.000015"},
      "architecture":{"input_modalities":["text"],"output_modalities":["text"]},
      "top_provider":{"max_completion_tokens":64000,"is_moderated":true},
      "supported_parameters":["max_tokens","temperature","seed","reasoning"],
      "reasoning":{"mandatory":false,"default_enabled":true,"supported_efforts":["high","low"]},
      "benchmarks":{"artificial_analysis":{"intelligence_index":60.9,"coding_index":77.4,"agentic_index":57.8}}
    },{
      "id":"~anthropic/claude-fable-latest",
      "canonical_slug":"~anthropic/claude-fable-latest",
      "name":"Anthropic: Claude Fable (latest)",
      "created":1788285838,
      "context_length":1000000,
      "pricing":{"prompt":"0.000003","completion":"0.000015"},
      "architecture":{"input_modalities":["text"],"output_modalities":["text"]},
      "top_provider":{"max_completion_tokens":64000,"is_moderated":true},
      "supported_parameters":["max_tokens"],
      "alias_target":{"name":"Claude Fable 5.1","slug":"anthropic/claude-fable-5.1"}
    }]}"#;

    #[test]
    fn parses_publication_timestamp_and_contamination_window() {
        let specs = parse_openrouter_models(LIVE_SAMPLE).expect("parses");
        let m = specs.iter().find(|s| s.id == "anthropic/claude-fable-5.1").unwrap();
        assert_eq!(m.created_unix, Some(1788285838), "publication timestamp drives new-model detection");
        assert_eq!(m.knowledge_cutoff.as_deref(), Some("2026-02-16"), "contamination window input");
        assert_eq!(m.upstream_canonical_slug.as_deref(), Some("anthropic/claude-fable-5.1-20260831"),
            "dated slug is the reproducibility key; the bare id silently re-points");
        assert_eq!(m.expiration_date.as_deref(), Some("2098-12-31"));
    }

    #[test]
    fn parses_external_benchmark_priors() {
        let specs = parse_openrouter_models(LIVE_SAMPLE).expect("parses");
        let m = specs.iter().find(|s| s.id == "anthropic/claude-fable-5.1").unwrap();
        let b = m.external_benchmarks.as_ref().expect("238/419 live models carry this");
        assert!((b.coding_index.unwrap() - 77.4).abs() < 1e-6);
    }

    #[test]
    fn parses_alias_target_for_latest_pseudo_models() {
        let specs = parse_openrouter_models(LIVE_SAMPLE).expect("parses");
        let alias = specs.iter().find(|s| s.id.starts_with('~')).unwrap();
        assert_eq!(alias.alias_target.as_deref(), Some("anthropic/claude-fable-5.1"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator catalog 2>&1 | tail -20`
Expected: FAIL — fields do not exist.

- [ ] **Step 3: Write minimal implementation**

Add to the deserialize struct in `catalog.rs` and to `ModelSpec`:

```rust
/// Third-party benchmark indices OpenRouter publishes for 238/419 models.
/// Used ONLY as a sanity prior — if Vox's own measurement disagrees wildly
/// with the external index, that is a signal the harness is measuring
/// something unintended, not that the model is unusual.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ExternalBenchmarks {
    pub intelligence_index: Option<f64>,
    pub coding_index: Option<f64>,
    pub agentic_index: Option<f64>,
}
```

Extend the response struct with `created: Option<i64>`, `canonical_slug: Option<String>`, `knowledge_cutoff: Option<String>`, `expiration_date: Option<String>`, `alias_target: Option<AliasTarget>`, `benchmarks: Option<BenchmarksBlock>`, and map each onto the new `ModelSpec` fields. Extract `parse_openrouter_models(json: &str) -> Result<Vec<ModelSpec>>` as a pure function so the tests above need no network.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator catalog 2>&1 | tail -20`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/catalog.rs crates/vox-orchestrator/src/models/spec.rs
git commit -m "feat(orchestrator): parse OpenRouter created, cutoff, alias, and benchmark metadata"
```

---

### Task 2: Variant, alias, and recency logic

There is no variant parser in the repo today: `is_free` is inferred from zero pricing (`catalog.rs:139-147`), `:thinking`/`:nitro` appear nowhere, and nothing prefers a newer snapshot of a family.

**Files:** Create `crates/vox-orchestrator/src/models/lifecycle.rs`.

**Interfaces:** `pub enum Variant { Base, Free, Batch, DynamicRouting }`; `pub fn parse_variant(id: &str) -> Variant`; `pub fn is_benchmarkable(spec: &ModelSpec, today: &str) -> Option<String>`; `pub fn dedupe_benchmark_targets(specs: &[ModelSpec]) -> Vec<&ModelSpec>`; `pub fn resolve_latest(specs: &[ModelSpec]) -> HashMap<String, String>`.

- [ ] **Step 1: Write the failing test**

```rust
//! Model variant, alias, and recency rules for the benchmark lifecycle.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_parsing_covers_the_live_suffix_vocabulary() {
        assert_eq!(parse_variant("anthropic/claude-opus-5"), Variant::Base);
        assert_eq!(parse_variant("qwen/qwen3-coder:free"), Variant::Free);
        assert_eq!(parse_variant("z-ai/glm-5.3-flash:batch"), Variant::Batch);
        // Dynamic routing shortcuts change endpoint selection per request, so a
        // score cannot be attributed to a model.
        for id in ["x/y:nitro", "x/y:floor", "x/y:exacto"] {
            assert_eq!(parse_variant(id), Variant::DynamicRouting, "{id}");
        }
    }

    #[test]
    fn dynamic_routing_and_alias_models_are_not_benchmarkable() {
        assert!(is_benchmarkable(&spec_with_id("x/y:nitro"), "2026-09-01").is_none());
        assert!(is_benchmarkable(&spec_with_id("~anthropic/claude-opus-latest"), "2026-09-01").is_none(),
            "the alias target moves; benchmark the concrete slug instead");
    }

    #[test]
    fn expired_models_are_not_benchmarkable() {
        let mut s = spec_with_id("nex-agi/nex-n2-pro");
        s.expiration_date = Some("2026-08-01".to_string());
        assert!(is_benchmarkable(&s, "2026-09-01").is_none(), "expired");
        s.expiration_date = Some("2098-12-31".to_string()); // the no-expiry sentinel
        assert!(is_benchmarkable(&s, "2026-09-01").is_some());
    }

    #[test]
    fn dedupe_collapses_variants_sharing_one_canonical_slug() {
        // 72 canonical_slugs serve multiple ids live; benchmarking each id would
        // pay three times for the same weights.
        let mut base = spec_with_id("z-ai/glm-5.3-flash");
        base.upstream_canonical_slug = Some("z-ai/glm-5.3-flash-20260826".into());
        let mut batch = spec_with_id("z-ai/glm-5.3-flash:batch");
        batch.upstream_canonical_slug = Some("z-ai/glm-5.3-flash-20260826".into());
        let out = dedupe_benchmark_targets(&[base, batch]);
        assert_eq!(out.len(), 1, "same weights benchmarked once");
        assert_eq!(out[0].id, "z-ai/glm-5.3-flash", "prefer the base variant");
    }

    #[test]
    fn resolve_latest_maps_family_alias_to_concrete_slug() {
        let mut alias = spec_with_id("~x-ai/grok-latest");
        alias.alias_target = Some("x-ai/grok-4.6".into());
        let map = resolve_latest(&[alias, spec_with_id("x-ai/grok-4.6")]);
        assert_eq!(map.get("~x-ai/grok-latest").map(String::as_str), Some("x-ai/grok-4.6"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator lifecycle 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

```rust
use std::collections::HashMap;
use crate::models::spec::ModelSpec;

/// OpenRouter id suffix classes. Only `Free` and `Batch` appear as distinct
/// ids in `/api/v1/models`; the dynamic-routing suffixes are appended at
/// request time and change which endpoint serves the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant { Base, Free, Batch, DynamicRouting }

#[must_use]
pub fn parse_variant(id: &str) -> Variant {
    match id.rsplit_once(':').map(|(_, v)| v) {
        Some("free") => Variant::Free,
        Some("batch") => Variant::Batch,
        Some("nitro" | "floor" | "exacto" | "online" | "thinking" | "extended") => Variant::DynamicRouting,
        _ => Variant::Base,
    }
}

/// `None` when the model must not be benchmarked, with the reason; `Some(id)` otherwise.
#[must_use]
pub fn is_benchmarkable(spec: &ModelSpec, today: &str) -> Option<String> {
    if spec.id.starts_with('~') { return None; }
    if parse_variant(&spec.id) == Variant::DynamicRouting { return None; }
    if let Some(exp) = &spec.expiration_date {
        // "2098-12-31" is OpenRouter's no-expiry sentinel; ISO dates compare as strings.
        if exp.as_str() < today { return None; }
    }
    Some(spec.id.clone())
}

/// One benchmark target per set of weights, preferring the base variant.
#[must_use]
pub fn dedupe_benchmark_targets(specs: &[ModelSpec]) -> Vec<&ModelSpec> {
    let mut best: HashMap<String, &ModelSpec> = HashMap::new();
    for s in specs {
        let key = s.upstream_canonical_slug.clone().unwrap_or_else(|| s.id.clone());
        let entry = best.entry(key).or_insert(s);
        if parse_variant(&s.id) == Variant::Base && parse_variant(&entry.id) != Variant::Base {
            *entry = s;
        }
    }
    let mut out: Vec<&ModelSpec> = best.into_values().collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// `~family-latest` → concrete slug, so routing can prefer "latest" while
/// scores stay attached to the concrete model that was actually measured.
#[must_use]
pub fn resolve_latest(specs: &[ModelSpec]) -> HashMap<String, String> {
    specs.iter()
        .filter_map(|s| s.alias_target.as_ref().map(|t| (s.id.clone(), t.clone())))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator lifecycle 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/models/lifecycle.rs crates/vox-orchestrator/src/models/mod.rs
git commit -m "feat(orchestrator): OpenRouter variant, alias, expiry, and dedupe rules"
```

---

### Task 3: Run discovery in the background loop, and emit machine-readable output

**The automation gap:** `run_catalog_refresh_loop` (6-hourly, `catalog_refresh.rs:30-48`) calls `refresh_once`, which **never calls `diff_and_emit_discovery`**. New-model discovery fires only when a human runs `vox model discover`. And `DiscoverArgs` has no `--json`, so nothing downstream can consume it.

**Files:** Modify `crates/vox-orchestrator/src/orchestrator/catalog_refresh.rs`, `crates/vox-cli/src/commands/model/discover.rs`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn discover_json_is_machine_consumable_and_stable() {
        let report = DiscoverReport {
            new_discovery_ids: vec!["x/y".into()],
            pending_eval_ids: vec!["a/b".into()],
            benchmarkable: vec!["x/y".into()],
            skipped: vec![("~a/latest".into(), "alias target moves".into())],
            total_models: 419,
        };
        let v: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["new_discovery_ids"][0], "x/y");
        assert_eq!(v["benchmarkable"][0], "x/y");
        assert!(v["skipped"][0]["reason"].is_string(), "skips must be explained, not silent");
    }

    #[test]
    fn background_refresh_emits_discovery() {
        // Regression guard: the 6-hour loop must run the diff, or "as models are
        // released" never happens without a human at a terminal.
        let src = include_str!("catalog_refresh.rs");
        assert!(
            src.contains("diff_and_emit_discovery"),
            "the background refresh path must call diff_and_emit_discovery"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli discover 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

Add `--json` to `DiscoverArgs`, a `DiscoverReport` with `to_json()` (including `schema_version: 1`), and call `diff_and_emit_discovery` from `refresh_once` so the background loop performs discovery. Persist the discovery high-water mark alongside the existing cache at `~/.vox/cache/model-catalog.v1.json`.

Keep the diff keyed on `id` (72 canonical_slugs serve multiple ids), and use `created` only for ordering and reporting — a pure `created > last_seen` filter misses backfilled entries.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli discover 2>&1 && cargo test -p vox-orchestrator catalog_refresh 2>&1 | tail -20`

- [ ] **Step 5: Sync the registry and commit**

```bash
cargo fmt -p vox-cli && cargo fmt -p vox-orchestrator
cargo run -q -p vox-cli -- ci command-sync --write
git add crates/vox-cli/src/commands/model/discover.rs crates/vox-orchestrator/src/orchestrator/catalog_refresh.rs contracts/cli/command-registry.yaml
git commit -m "feat(models): background discovery + machine-readable vox model discover --json"
```

---

# Phase H — The deterministic probe

### Task 4: Context-fitted prompt assembly

Live context windows span **4,095 → 2,000,000 tokens** (median 262,144), so no single prompt fits every model. Nothing today sizes a prompt to the target model: the history budget is a hardcoded 4,000 tokens (`conversation.rs:37`) and compaction defaults to 128,000 (`compaction.rs:73`), neither derived from `capabilities.max_context`.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/context_fit.rs`.

**Interfaces:** `pub enum ContextTier { S, M, L, Xl }`; `pub fn tier_for(context_length: u32) -> ContextTier`; `pub fn assemble(tier: ContextTier, repo_root: &Path) -> anyhow::Result<PromptContext>`; `pub fn completion_budget(spec_max_completion: Option<u32>, is_reasoning: bool) -> u32`.

- [ ] **Step 1: Write the failing test**

```rust
//! Fit the Vox reference context to the target model's window.
//!
//! Live context lengths span 4,095 to 2,000,000 tokens (median 262,144), so a
//! fixed prompt either wastes capacity or overflows. Tiers are cumulative:
//! each adds material to the one below.
//!
//! IMPORTANT for comparability: the tier is a CAPACITY fit, not the
//! experimental variable. Cross-model comparisons must hold the *condition*
//! constant (see conditions.rs); tiers are reported as a separate "best-fit"
//! arm for practical guidance.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_partition_the_live_context_range() {
        assert_eq!(tier_for(4_095), ContextTier::S);
        assert_eq!(tier_for(131_072), ContextTier::M);
        assert_eq!(tier_for(262_144), ContextTier::L);
        assert_eq!(tier_for(2_000_000), ContextTier::Xl);
    }

    #[test]
    fn tiers_are_cumulative_and_monotonically_larger() {
        let root = repo_root();
        let sizes: Vec<usize> = [ContextTier::S, ContextTier::M, ContextTier::L, ContextTier::Xl]
            .iter().map(|t| assemble(*t, &root).unwrap().context_text.len()).collect();
        for w in sizes.windows(2) {
            assert!(w[1] >= w[0], "each tier must include the one below: {sizes:?}");
        }
    }

    #[test]
    fn smallest_tier_fits_the_smallest_live_window() {
        // 4,095-token models exist. The S tier plus a task must fit, with room
        // for the answer — otherwise those models are untestable by construction.
        let ctx = assemble(ContextTier::S, &repo_root()).unwrap();
        assert!(ctx.context_text.len() / 4 < 1_500, "S tier must stay under ~1.5k tokens");
    }

    #[test]
    fn reasoning_models_get_a_larger_completion_budget() {
        // 292/419 models are reasoning models; 91 have mandatory reasoning. A
        // 1024-token cap truncates them mid-thought and scores a harness
        // artifact as a model failure.
        assert!(completion_budget(Some(64_000), true) > completion_budget(Some(64_000), false));
        assert!(completion_budget(None, true) >= 8_192, "reasoning needs headroom");
        // Never exceed what the provider accepts.
        assert!(completion_budget(Some(4_096), true) <= 4_096);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus context_fit 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

```rust
use anyhow::Result;
use std::path::Path;
use super::conditions::PromptContext;

/// Context-capacity band for the target model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextTier { S, M, L, Xl }

#[must_use]
pub fn tier_for(context_length: u32) -> ContextTier {
    match context_length {
        0..=32_767 => ContextTier::S,
        32_768..=199_999 => ContextTier::M,
        200_000..=999_999 => ContextTier::L,
        _ => ContextTier::Xl,
    }
}

/// Assemble cumulative Vox reference material for `tier`.
///
/// S  — compact grammar only (~780 tokens).
/// M  — + worked syntax example.
/// L  — + the harness/skills overview and the project's VOX.md.
/// Xl — + the full syntax reference.
pub fn assemble(tier: ContextTier, repo_root: &Path) -> Result<PromptContext> {
    let read = |rel: &str| std::fs::read_to_string(repo_root.join(rel)).unwrap_or_default();
    let mut parts = vec![vox_grammar_export::emit_compact_llm_prompt()];
    if matches!(tier, ContextTier::M | ContextTier::L | ContextTier::Xl) {
        parts.push(format!("## Worked example\n\n{}", read("examples/golden/ref_syntax.vox")));
    }
    if matches!(tier, ContextTier::L | ContextTier::Xl) {
        parts.push(format!("## Project conventions\n\n{}", read("VOX.md")));
    }
    if tier == ContextTier::Xl {
        parts.push(format!("## Syntax reference\n\n{}", read("docs/src/reference/ref-syntax.md")));
    }
    let text = parts.join("\n\n");
    super::conditions::context_from_text(&format!("{tier:?}"), text)
}

/// Output-token budget for one generation.
///
/// Reasoning models spend most of their budget before emitting an answer; a
/// budget sized for the answer alone truncates them mid-thought and records a
/// harness artifact as a model failure. Reference solutions max at ~320 tokens,
/// so the answer is never the constraint.
#[must_use]
pub fn completion_budget(spec_max_completion: Option<u32>, is_reasoning: bool) -> u32 {
    let want = if is_reasoning { 32_768 } else { 4_096 };
    spec_max_completion.map_or(want, |cap| want.min(cap))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus context_fit 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/context_fit.rs
git commit -m "feat(corpus): context-window-fitted prompt tiers and reasoning-aware budgets"
```

---

### Task 5: The three-axis probe

The repeatable, deterministic triage that runs on every newly-discovered model: cheap enough for every model, decisive enough to gate promotion.

**Design.** Fixed 5 held-out fixtures, `n=1`, greedy (`temperature=0.0`), condition C1 (grammar in prompt, fits every window). Yields all three axes in one run:

| Axis | Source | Judge-free? |
|---|---|---|
| **Intelligence (correctness)** | pass@1 by compile + assert exit codes | yes — symbolic |
| **Intelligence (quality)** | LLM judge over *already-correct* solutions only, different family | no — reported separately, never gates |
| **Efficiency** | `total_cost` from `GET /api/v1/generation?id=` | yes |
| **Responsiveness** | `generation_time` + `latency` from the same endpoint | yes |

`/api/v1/generation` is authoritative and beats wall-clock: it returns `total_cost`, `generation_time`, `latency` (TTFT), `moderation_latency`, and `provider_name` — the last of which verifies the pinned provider actually served the request.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/probe.rs`, `crates/vox-cli/src/commands/model/probe.rs`.

**Interfaces:** `pub struct ProbeResult { pub model_id: String, pub canonical_slug: Option<String>, pub provider_name: Option<String>, pub correctness: f64, pub quality: Option<f64>, pub cost_usd: Option<f64>, pub generation_ms: Option<i64>, pub ttft_ms: Option<i64>, pub n_fixtures: usize, pub verdict: ProbeVerdict }`; `pub enum ProbeVerdict { Promote, Hold, Reject }`; `pub const PROBE_FIXTURE_IDS: [&str; 5]`; `pub fn classify(correctness: f64, compile_rate: f64) -> ProbeVerdict`.

- [ ] **Step 1: Write the failing test**

```rust
//! The deterministic triage probe run against every newly-discovered model.
//!
//! Deliberately tiny: 5 held-out fixtures, n=1, greedy. It is a gate, not a
//! ranking — 5 fixtures cannot resolve model-vs-model differences (the exact
//! McNemar floor at n=5 is near 100 percentage points). Its only job is to
//! answer "can this model write Vox at all, at what cost and speed", which is
//! exactly what promotion out of Provisional requires.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_uses_a_fixed_held_out_set_so_runs_are_comparable() {
        assert_eq!(PROBE_FIXTURE_IDS.len(), 5);
        let mut sorted = PROBE_FIXTURE_IDS;
        sorted.sort_unstable();
        assert_eq!(sorted, PROBE_FIXTURE_IDS, "ids must be sorted for a stable probe hash");
    }

    #[test]
    fn verdict_requires_correctness_not_merely_compilation() {
        // A model that emits syntactically valid but wrong Vox must not promote.
        assert_eq!(classify(0.0, 1.0), ProbeVerdict::Reject, "compiles but never correct");
        assert_eq!(classify(0.8, 1.0), ProbeVerdict::Promote);
        assert_eq!(classify(0.4, 0.8), ProbeVerdict::Hold, "borderline -> full corpus decides");
    }

    #[test]
    fn total_failure_to_compile_is_a_reject_not_a_hold() {
        assert_eq!(classify(0.0, 0.0), ProbeVerdict::Reject);
    }

    #[test]
    fn quality_is_optional_and_never_affects_the_verdict() {
        // The judge is a separate axis. A missing or hostile judge score must
        // not change promotion, or an LLM re-enters the correctness path.
        let a = classify(0.8, 1.0);
        let b = classify(0.8, 1.0);
        assert_eq!(a, b);
        assert_eq!(a, ProbeVerdict::Promote, "verdict depends only on symbolic signals");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus probe 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

```rust
use serde::{Deserialize, Serialize};

/// Fixed held-out probe set. Changing this invalidates cross-model
/// comparability, so treat it as a versioned contract.
pub const PROBE_FIXTURE_IDS: [&str; 5] = ["041", "043", "044", "045", "049"];

/// What the probe recommends for a newly-discovered model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeVerdict {
    /// Correct often enough to earn a full-corpus run.
    Promote,
    /// Ambiguous; run the full corpus before deciding.
    Hold,
    /// Cannot write working Vox; do not spend a full corpus run.
    Reject,
}

/// One probe run's three axes plus provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub model_id: String,
    /// The dated slug actually measured — the reproducibility key.
    pub canonical_slug: Option<String>,
    /// Who served it. If this differs from the pinned provider, discard the run.
    pub provider_name: Option<String>,
    /// pass@1 over the probe set (symbolic).
    pub correctness: f64,
    pub compile_rate: f64,
    /// LLM-judged quality over already-correct solutions only. Never gates.
    pub quality: Option<f64>,
    /// `total_cost` from /api/v1/generation, summed.
    pub cost_usd: Option<f64>,
    /// `generation_time`, median.
    pub generation_ms: Option<i64>,
    /// `latency` (time to first token), median.
    pub ttft_ms: Option<i64>,
    pub n_fixtures: usize,
    pub verdict: ProbeVerdict,
}

/// Verdict from symbolic signals only.
///
/// Thresholds are deliberately coarse: with 5 fixtures the confidence interval
/// on any rate is enormous, so this distinguishes "clearly works" from
/// "clearly does not" and defers everything else to the full corpus.
#[must_use]
pub fn classify(correctness: f64, compile_rate: f64) -> ProbeVerdict {
    if correctness >= 0.6 { ProbeVerdict::Promote }
    else if correctness <= 0.0 && compile_rate <= 0.5 { ProbeVerdict::Reject }
    else if correctness <= 0.0 { ProbeVerdict::Reject }
    else { ProbeVerdict::Hold }
}
```

Then implement `vox model probe --model <id>` in `crates/vox-cli/src/commands/model/probe.rs`, reusing v2's `verify_program` and `score_corpus`, pinning the provider, and enriching each attempt from `/api/v1/generation?id=`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus probe 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-corpus && cargo fmt -p vox-cli
cargo run -q -p vox-cli -- ci command-sync --write
git add crates/vox-corpus/src/humaneval_runner/probe.rs crates/vox-cli/src/commands/model/probe.rs contracts/cli/command-registry.yaml
git commit -m "feat(models): three-axis deterministic probe for newly-discovered models"
```

---

# Phase J — Routing adoption (the payoff)

### Task 6: Make measured quality reach the router

Two readers already consume `model_scoreboard.quality_score` and both are currently identities because the column is `COALESCE(AVG(llm_feedback.rating)/5.0, 1.0)` over an empty table (`ops_scientia.rs:191`).

**Files:** Modify `crates/vox-db/src/store/ops_scientia.rs`.

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn quality_score_prefers_benchmark_evidence_over_the_empty_feedback_table() {
        let db = test_db().await;
        // No llm_feedback rows, but a benchmark row exists for this model.
        upsert_benchmark_quality(&db, "x/y", "vox-codegen", 0.72).await.unwrap();
        let rows = db.refresh_model_scoreboard().await.unwrap();
        let row = rows.iter().find(|r| r.model_id == "x/y").expect("row present");
        assert!((row.quality_score - 0.72).abs() < 1e-9,
            "measured pass@1 must win over the 1.0 placeholder");
    }

    #[tokio::test]
    async fn unbenchmarked_models_keep_the_neutral_placeholder() {
        let db = test_db().await;
        let rows = db.refresh_model_scoreboard().await.unwrap();
        for r in &rows {
            assert!((r.quality_score - 1.0).abs() < 1e-9,
                "without evidence the score stays neutral, never 0 — an unmeasured \
                 model must not be penalised as though it had failed");
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-db quality_score 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

Change the `quality_score` expression to prefer a benchmark-derived value, falling back to feedback, then to the neutral `1.0`:

```sql
COALESCE(
  (SELECT quality_score FROM model_scoreboard b
    WHERE b.model_id = m.model_id AND b.task_category = 'vox-codegen'
      AND b.strength_tag = 'benchmark' AND b.n_calls > 0),
  AVG(f.rating) / 5.0,
  1.0
)
```

**Neutral-not-zero is deliberate:** an unmeasured model must not be scored as though it had failed, or the router would prefer any benchmarked model over every new one regardless of merit. That is what the confidence gate (Task 7) is for.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-db quality_score 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-db
git add crates/vox-db/src/store/ops_scientia.rs
git commit -m "feat(db): source model quality_score from benchmark evidence"
```

---

### Task 7: Activate the confidence gate

`confidence_state_for_model` passes a hardcoded `None` where the scoreboard row belongs (`select.rs:277`), so `resolve_eligibility` short-circuits to pricing-derived confidence and the evidence-based state machine (`should_promote`, `autonomic.rs:235-266`) is unreachable.

**Files:** Modify `crates/vox-orchestrator/src/models/select.rs`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn an_unbenchmarked_model_is_not_routable() {
        // The safety property behind "auto-adopt new models": a model nobody has
        // measured must not silently start serving user traffic.
        let spec = provisional_spec("brand-new/model");
        assert!(!confidence_state_for_model(&spec, None).eligible_for_routing());
    }

    #[test]
    fn evidence_promotes_a_model_to_routable() {
        let spec = provisional_spec("proven/model");
        let row = scoreboard_row(/* n_calls */ 50, /* quality */ 0.8, /* p50_ms */ 900);
        assert!(confidence_state_for_model(&spec, Some(&row)).eligible_for_routing());
    }

    #[test]
    fn a_benchmarked_but_failing_model_stays_out_of_routing() {
        let spec = provisional_spec("bad/model");
        let row = scoreboard_row(50, 0.05, 900);
        assert!(!confidence_state_for_model(&spec, Some(&row)).eligible_for_routing(),
            "measured-and-bad must not route just because it was measured");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator confidence 2>&1 | tail -20`

- [ ] **Step 3: Write minimal implementation**

Change `confidence_state_for_model` to take `Option<&ModelScoreboardRow>` and pass it through to `resolve_eligibility` instead of the hardcoded `None` at `select.rs:277`. Thread the row from the registry's scoreboard map at the call site.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator 2>&1 | tail -20`
Expected: PASS. Re-run the full orchestrator suite — this changes routing eligibility and may surface tests that assumed every model was routable.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/models/select.rs
git commit -m "feat(orchestrator): gate routing on benchmark evidence via the confidence state machine"
```

---

### Task 8: Prefer the latest member of a family

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn latest_alias_resolves_to_the_concrete_scored_model() {
        // "Prefer latest" must not route to the ~alias itself: the alias moves,
        // so its score would be attributed to whatever it points at next.
        let map = resolve_latest(&catalog_with_alias());
        let target = map.get("~x-ai/grok-latest").expect("alias resolves");
        assert!(!target.starts_with('~'), "must resolve to a concrete slug");
    }

    #[test]
    fn a_newer_family_member_is_preferred_only_once_it_has_evidence() {
        // The whole safety property: newer does not mean adopted until measured.
        let older = scored_spec("x-ai/grok-4.5", 0.70, /* confirmed */ true);
        let newer = scored_spec("x-ai/grok-4.6", 0.00, /* confirmed */ false);
        assert_eq!(prefer_latest_confirmed(&[older.clone(), newer]), "x-ai/grok-4.5");

        let newer_proven = scored_spec("x-ai/grok-4.6", 0.85, true);
        assert_eq!(prefer_latest_confirmed(&[older, newer_proven]), "x-ai/grok-4.6");
    }
```

- [ ] **Step 2–4:** Implement `prefer_latest_confirmed` in `lifecycle.rs` — among `Confirmed` models sharing a family, pick the one with the greatest `created_unix`. Run the tests; expect PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/models/lifecycle.rs
git commit -m "feat(orchestrator): prefer the newest confirmed member of a model family"
```

---

# Phase K — GUI and automation

### Task 9: Surface the evidence, stop hardcoding presets

The GUI's four emphasis presets are hardcoded in TypeScript (`SettingsView.tsx:1301-1306`) rather than imported from the Rust `ClutchProfile` (`mode.rs:109-115`) — so they can drift silently. Note also that `Free` and `Efficiency` currently resolve to **identical axes** `(70,15,15)` (`mode.rs:133-172`), differing only by `force_free_pool`; the GUI presents them as distinct choices.

- [ ] Expose `ClutchProfile` and its axis triples through an existing Tauri command so the GUI reads one SSOT; remove the hardcoded TS array.
- [ ] Add a benchmark-evidence column to `ModelsView` sourced from `get_model_scoreboard` (`crates/vox-gui/src/commands/models.rs:302-328`): measured pass@1, cost per solved fixture, p50 generation time, confidence state, and last-measured date.
- [ ] Show unconfirmed models as **visible but not selectable in auto modes**, with the reason ("awaiting benchmark") — silently hiding them makes the gate look like a bug.
- [ ] Resolve the `Free`/`Efficiency` duplication: either differentiate the axes or merge the presets.

### Task 10: Schedule it

Per the CI-track findings: `schedule:`-triggered workflows are **exempt** from `workflow-concurrency-guard` (it matches only `push`/`pull_request`/`pull_request_target`), and `runs-on: [self-hosted, linux, x64]` needs **no** exception row (`runner_policy_check` scans for hosted labels as substrings — do not even mention them in comments).

- [ ] **Daily** — `vox model discover --json`. Zero LLM spend. If `new_discovery_ids` is non-empty, `vox model probe` each benchmarkable, deduped target; promote/hold/reject.
- [ ] **Weekly** — full-corpus run for every `Hold` and every newly `Promote`d model; refresh the leaderboard.
- [ ] Sanity gate before anything publishes: the oracle sweep must be 100% and a majority of models must have completed, or the previous board keeps serving.
- [ ] Add `contracts/reports/vox-efficacy/**` to `docs-deploy.yml`'s path filter, and dispatch the deploy explicitly (`gh workflow run docs-deploy.yml`) — a `GITHUB_TOKEN`-authored push triggers no workflows. **No `[skip ci]`.**
- [ ] Orchestrate from a `.vox` script per the VoxScript-first rule; let Rust own JSON, HTTP, and secrets.

---

## Safety property

The point of the confidence gate is that **"automatically adopt better models" never means "automatically trust unmeasured models."**

```
discovered (OpenRouter `created`)
  → Provisional        — visible, NOT routable
  → probe (5 fixtures, cents, minutes)
      Reject → stays out; recorded with its reason
      Hold   → full corpus decides
      Promote → Shadowed
  → full corpus (164 fixtures, conditions C0–C3)
  → Confirmed          — eligible for auto-mode routing
  → prefer-latest applies only among Confirmed family members
```

A model published an hour ago cannot serve user traffic until it has passed a probe whose correctness signal is compiler and test exit codes. That is the property that makes automatic adoption safe rather than reckless.

---

## What this plan deliberately does not claim

- **The probe is a gate, not a ranking.** Five fixtures cannot resolve model-vs-model differences; the exact McNemar floor at n=5 is near 100 percentage points. Probe numbers must never be published as a leaderboard.
- **Judged quality is not correctness.** It is reported beside pass@1, never folded into it, and is produced by a different model family than the one under test.
- **External `benchmarks` indices are a prior, not a result.** OpenRouter's Artificial Analysis figures (238/419 models) are useful for sanity-checking that Vox's own measurement is not measuring something unintended. They are not Vox evidence and must not be republished as such.
- **The contamination window works today and decays.** All 183 models declaring a `knowledge_cutoff` predate the corpus (max 2026-02-16 vs corpus 2026-05-27), so the corpus is clean for them. But 236 models declare no cutoff, and the corpus is public — so ongoing fixture authorship is required, not optional.
