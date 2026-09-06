---
title: "Model Orchestration SSOT — Audit & Convergence Plan (2026-04-20)"
description: "Audit of Vox model selection, orchestration, telemetry, discovery, and mesh-secret distribution; proposes a single source of truth and a concrete backlog of ~70 improvements."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Core orchestration architecture reference; names all files touching the model-routing surface."
---

# Model Orchestration SSOT — Audit & Convergence Plan

**Scope.** MENS (local), Populi (GPU mesh), OpenRouter, direct-provider cloud backends (Anthropic, Google, Groq, DeepSeek, Cerebras, Mistral, SambaNova, HuggingFace), plus the vox-secrets secret plane that feeds all of them. This document lists what exists today, where it drifts, and exactly what to change, file-by-file.

**How to read this.** Every "FIX" item below is a mechanical operation keyed to a file path (and line numbers where stable). Each item can be handed to an agent with no further context.

---

## Part 1 — Executive summary

**What is good today.**

- `vox-orchestrator::models::ModelRegistry` is the one selector used across the workspace (`crates/vox-orchestrator/src/models/registry.rs:14`). All task-to-model decisions flow through `best_for()` / `best_for_task()`.
- `vox-secrets` is a credible secret plane with a documented resolver chain, `doctor`, `parity`, and `secret-env-guard` (`crates/vox-secrets/src/resolver.rs:1`, `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/secrets.rs`).
- A live catalog refresh against `https://openrouter.ai/api/v1/models` already exists with a min-interval and jitter guard (`crates/vox-orchestrator/src/catalog.rs:200`; `crates/vox-orchestrator/src/models/registry.rs:49`).
- Telemetry lands in a typed `llm_interactions` table (v59) with a validation contract including `context_utilization_pct` and `cache_read_tokens`.
- Observed pricing is stabilized in `model_pricing_catalog` (v59), enabling empirical cost overrides for routing decisions.
- Mesh-node identity uses Ed25519 with challenge/response (`crates/vox-identity/src/identity.rs:20`) and mesh bearer auth uses constant-time compare (`crates/vox-populi/src/transport/auth.rs:5`).
- `.voxignore` is the declared SSOT for AI context exclusion (`AGENTS.md:37`).

**What is broken or drifting.**

1. **Model-selection logic is split across 5 crates with two different `ModelTier` enums** (`crates/vox-orchestrator/src/models/spec.rs:14`, `crates/vox-orchestrator/src/models/routing_table.rs:6`) and two different `ChatRouteBackend`-like enums (`vox-orchestrator` vs `vox-actor-runtime/src/model_resolution.rs:22`).
2. **Strength tags are free-form strings materialized from three independent heuristics** (`spec.rs:230`, `catalog.rs:107`, `routing_table.rs:30`). No enum, no parity check.
3. **10 model IDs are hardcoded as defaults** in `spec.rs:273-482` with their own cost/context data, duplicating whatever OpenRouter returns live. Drift is silent.
4. **No model scoreboard.** `eval_runs` and `llm_feedback` exist but are never aggregated per `(model_id, task_category)` and never fed back to `best_for()`.
5. **No distributed trace ID.** `journey_id`, `session_id`, `run_id` are local to each subsystem; there is no OpenTelemetry-style GenAI span with `gen_ai.request.model`, `gen_ai.usage.input_tokens`, etc.
6. **Automatic model discovery is one shot per process start.** No scheduled nightly refresh; no Populi mesh catalog aggregation; no HF Hub auto-registration into the routing registry.
7. **Direct env reads for secret-ish values leak outside vox-secrets.** Confirmed violation in `crates/vox-schola/src/curator.rs` (`OPENAI_API_KEY`) and suspected drift for `TOGETHER_FINETUNE_MODEL` (`crates/vox-cli/src/commands/ai/train.rs`), `GEMINI_DIRECT_MODEL`/`OPENROUTER_GEMINI_MODEL` (`crates/vox-config/src/routing_policy.rs`).
8. **No cross-node secret sync.** `A2ADeliverRequest.jwe_payload` is plumbed but never populated (`crates/vox-populi/src/transport/mod.rs:76`). `vox-crypto` has ChaCha20-Poly1305 and Ed25519 but **no X25519 KEM** for wrapping secrets to another node.
9. **No device-pairing flow.** A user with 3 mesh nodes must install `OPENROUTER_API_KEY` three times by hand.
10. **Retired-surface drift.** `vox_dei::model_route` is still used as the `tracing` target in `crates/vox-actor-runtime/src/model_resolution.rs:183-246` (harmless in theory, but violates the retired-symbol policy in `AGENTS.md:140`).

**What this document proposes.**

- Elevate **`crates/vox-orchestrator/src/models/`** to the single-source-of-truth crate for everything routing-related. Move, delete, or alias anything that currently duplicates it.
- Declare **`contracts/orchestration/model-routing.v1.yaml`** as the machine-readable SSOT for task-→-strength mapping, tier definitions, scoring weights, and fallback chains. Generate Rust enums from it.
- Declare **`contracts/orchestration/model-telemetry.v1.yaml`** aligned with OpenTelemetry GenAI semconv v1.37 (`gen_ai.*`). Every LLM call on every provider emits a span with the same attribute names.
- Build a **`ModelScoreboard`** table keyed by `(model_id, task_category, strength_tag)` populated from `eval_runs` + `llm_feedback`. Make `best_for()` read it.
- Add **`vox secrets sync`** with X25519-sealed-box pairing so secrets installed on one mesh node propagate to a user's other nodes without re-entry.
- Add **`vox mens models discover`** and **`vox populi models inventory`** scheduled jobs so MENS checkpoints and mesh-node capabilities register into the routing catalog automatically.

---

## Part 2 — Proposed SSOT layout

This is what "converged" looks like. Every bullet below is also a "FIX" in Part 3.

### 2.1 File authority map (post-convergence)

| Concern | SSOT file | Consumers read via |
|---|---|---|
| Model spec, capabilities, pricing | `contracts/orchestration/model-catalog.v1.json` (generated nightly from live OpenRouter + HF Hub + Ollama + Populi mesh) | `vox_orchestrator::models::Registry::load()` |
| Task-category → strength mapping, preferred tier, context floor | `contracts/orchestration/model-routing.v1.yaml` | codegen → `crates/vox-orchestrator/src/models/routing_table.rs` (generated) |
| Scoring weights (efficiency/precision/latency/availability/balance/mobile) | `contracts/orchestration/model-routing.v1.yaml` `[scoring]` | `crates/vox-orchestrator/src/models/scoring.rs` |
| Provider enum, secret-id mapping | `contracts/orchestration/providers.v1.yaml` | codegen → `crates/vox-orchestrator/src/models/spec.rs::ProviderType`, `crates/vox-orchestrator/src/models/key_guard.rs` |
| Telemetry event attributes (GenAI) | `contracts/orchestration/model-telemetry.v1.yaml` (mirrors OTel GenAI semconv v1.37) | `crates/vox-actor-runtime/src/routing_telemetry.rs`, `crates/vox-db/src/research_metrics_contract.rs` |
| Secrets & env var names | `crates/vox-secrets/src/spec/**` (unchanged authority) | `vox_secrets::resolve_secret(...)` |
| Env-variable allowlist (non-secret tuning) | `crates/vox-secrets/src/lib.rs::OPERATOR_TUNING_ENVS` (extend) | `secret-env-guard` |
| `.voxignore` derived ignore files | `.voxignore` (unchanged) | `vox ci sync-ignore-files` |

### 2.2 The single `ModelCatalogEntry` schema (proposal)

```yaml
# contracts/orchestration/model-catalog.v1.json — one entry
model_id: "anthropic/claude-sonnet-4.6"
family: "anthropic"
revision: "4.6"
provider_route:
  primary: "OpenRouter"            # one of providers.v1.yaml enum
  fallback: ["Anthropic"]
context_length_tokens: 200000
input_modalities: ["text", "image"]
output_modalities: ["text"]
pricing:
  input_per_1k: 3.00
  output_per_1k: 15.00
  cache_read_per_1k: 0.30          # if provider reports it
  unit: "USD"
supports:
  tools: true
  json_mode: true
  streaming: true
  reasoning: false
strengths: ["codegen", "review", "debugging", "security"]  # from enum in model-routing.v1.yaml
tier: "Pro"
availability:
  openrouter_uptime_30d: 0.993     # from OpenRouter endpoints API
  measured_p50_ms: 1820            # from our own eval_runs
  measured_p99_ms: 7700
scoreboard:
  codegen_success_rate_30d: 0.86
  review_success_rate_30d: 0.91
  last_scored_at: "2026-04-19T04:00:00Z"
discovered_from: "openrouter-v1-catalog@2026-04-20T00:00:00Z"
```

The **only** hand-maintained file after convergence is `contracts/orchestration/model-routing.v1.yaml` (strength enum, task→strength table, scoring weights, tier definitions, hard overrides). Everything else regenerates.

### 2.3 User-visible single login → mesh-wide secrets (proposal)

```
┌──────────────┐      ┌─────────────────────┐       ┌──────────────┐
│ Node A (desk)│      │ SecretsSync gossip  │       │ Node B (laptop)
│ identity (Ed │─────>│   over mesh         │<──────│ identity (Ed)│
│ 25519 pair)  │      │ - pairing → trust  │       │              │
└──────┬───────┘      │ - X25519 KEM wrap  │       └──────┬───────┘
       │              │   of secret value  │              │
       │              │ - Ed25519 sig on   │              │
       │              │   wrapped envelope │              │
       │              └─────────────────────┘              │
       │                                                   │
       v                                                   v
  vox-secrets local vault (ChaCha20-Poly1305 KDF from   vox-secrets local vault
  user-pairing passphrase or OS keyring)                (same)
```

- `vox secrets pair` on Node A prints a one-time QR / 5-word pairing code.
- `vox secrets pair --accept <code>` on Node B performs X25519 ECDH, attests via Ed25519, enrolls into `TrustedNodeRegistry` (`crates/vox-identity/src/storage.rs`).
- `vox secrets sync` pushes every `shareable=true` secret (in `SecretSpec`) to every trusted peer, wrapped with the peer's X25519 public key, signed with the sender's Ed25519 private key, delivered over Populi's A2A channel.
- No secret value ever leaves the user's mesh.
- Operators opt a secret *out* by setting `shareable: false` in the spec (applies by default to registry/local-only things like `VOX_IDENTITY_KEY_PATH`).

---

## Part 3 — Backlog (~70 numbered improvements)

Every item starts with **FIX-NN**. When executing, treat title, problem, operation, and success criteria as a self-contained ticket.

### A. Single source of truth — data model & codegen

**FIX-01. [FIXED] Define `contracts/orchestration/model-routing.v1.yaml` as the routing SSOT.**
- *Problem.* Routing table, scoring weights, and tier enum live as hand-edited Rust in `crates/vox-orchestrator/src/models/routing_table.rs:30-122`, `.../scoring.rs:6-31`, `.../spec.rs:14-24`.
- *Operation.* Create `contracts/orchestration/model-routing.v1.yaml` with top-level keys `schema_version`, `tiers[]`, `strengths[]`, `task_categories[]`, `scoring.weights`, `scoring.latency_bands`, `premium_alias{}`, `economy_cost_ceiling_usd_per_1k`.
- *Success.* File exists, JSON-Schema-validates against a new `contracts/orchestration/model-routing.v1.schema.json`. CI check added in `crates/vox-cli/src/commands/ci/run_body_helpers/` under a new `routing-ssot-validate` guard.

**FIX-02. [FIXED] Codegen `ModelTier`, `StrengthTag`, `TaskCategory` from the YAML.**
- *Problem.* Two `ModelTier` enums exist (`spec.rs:14`, `routing_table.rs:6`). Strength tags are free-form strings with no enum. `TaskCategory` is defined in `crates/vox-orchestrator/src/types/tasks.rs` independently of the routing table.
- *Operation.* Introduce `crates/vox-orchestrator/build.rs` that reads `contracts/orchestration/model-routing.v1.yaml` and emits `src/models/generated.rs` containing enums. Delete `routing_table.rs::ModelTier` (FIX-02a) and replace `spec.rs::ModelTier` imports with the generated one.
- *Success.* `cargo build -p vox-orchestrator` regenerates on YAML change. `rg "enum ModelTier"` returns one hit.

**FIX-03. [FIXED] Replace the `infer_strengths()` triple-path with a single table.**
- *Problem.* Three independent heuristics: parameter-graph (`catalog.rs:118-142`), provider family (`catalog.rs:143-148` and `spec.rs:230-255`), name heuristic (`catalog.rs:151-188`).
- *Operation.* In the YAML add a `strength_inference` section with ordered rules (parameter_graph / provider_family / name_regex / default). Generate `infer_strengths(entry) -> Vec<StrengthTag>` from it. Delete the duplicate in `spec.rs:230-255`.
- *Success.* `rg 'fn infer_strengths|fn provider_family_strengths'` shows exactly one definition (in generated code).

**FIX-04. [FIXED] Declare `contracts/orchestration/providers.v1.yaml` and regenerate `ProviderType` + `key_guard`.**
- *Problem.* `ProviderType` enum is hardcoded (`spec.rs:80-106`). `provider_secret_is_available()` is hand-mapped (`key_guard.rs:8-27`).
- *Operation.* New YAML: for each provider `{name, base_url, secret_id, supports_openai_compat, default_route_kind, fallback_kind}`. Codegen both.
- *Success.* Adding a new provider is a YAML edit only.

**FIX-05. [FIXED] Declare `contracts/orchestration/model-catalog.v1.json` as the runtime catalog.**
- *Problem.* 10 models are baked into `spec.rs:273-482` as fallback. Live OpenRouter data is merged at runtime but never persisted; restart loses it. Two sources of truth coexist silently.
- *Operation.* Move the 10 bootstrap entries into `contracts/orchestration/model-catalog.bootstrap.v1.json`. At runtime, `Registry::load()` reads bootstrap, then merges from `~/.vox/cache/model-catalog.v1.json` (persisted by the discovery job — FIX-30). Delete the literal `ModelSpec::new(...)` calls at `spec.rs:301, 318, 335, 353, 370, 387, 405, 428, 447`.
- *Success.* `rg 'ModelSpec::new\(' crates/vox-orchestrator` returns zero hits; bootstrap lives in JSON; cache auto-refreshes.

**FIX-06. [FIXED] Delete the duplicate `ChatRouteBackend` in `vox-actor-runtime`.**
- *Problem.* `crates/vox-actor-runtime/src/model_resolution.rs:22-32` redefines `ChatRouteBackend`; `vox-orchestrator/src/models/spec.rs::ProviderType` is the canonical one. Intentional decoupling exists to avoid a cycle but produces drift.
- *Operation.* Extract `ProviderType`, `ChatRouteBackend`, `ChatProviderRouteKind` into a new tiny leaf crate `crates/vox-orchestrator-types/` (generated from `providers.v1.yaml`). Both `vox-orchestrator` and `vox-actor-runtime` depend on it; cycle broken.
- *Success.* `rg 'enum ChatRouteBackend|pub enum ProviderType' crates/` returns exactly one hit each.

**FIX-07. [FIXED] Kill `vox_dei::model_route` tracing targets.**
- *Problem.* Retired crate name still appears as `tracing` span target at `crates/vox-actor-runtime/src/model_resolution.rs:183,203,219,232,246`. Violates `AGENTS.md:140` and confuses log aggregation.
- *Operation.* Replace `target: "vox_dei::model_route"` with `target: "vox_orchestrator::model_route"`. Add a lint in `vox ci run` guards to fail if `vox_dei::` appears anywhere outside comments or tombstone archive.
- *Success.* `rg '"vox_dei::'` returns zero code hits.

**FIX-08. [FIXED] Resolve the `ModelSpec` vs. `ModelRegistryEntry` vs. `ModelCatalogEntry` name collision.**
- *Problem.* Three structs (`spec.rs::ModelSpec`, `vox-actor-runtime/src/llm/types.rs::ModelRegistryEntry`, proposed `ModelCatalogEntry`) will exist simultaneously during migration.
- *Operation.* Keep `ModelCatalogEntry` as the wire/file type, have `ModelSpec` derive `From<&ModelCatalogEntry>`, then remove `ModelRegistryEntry` by inlining its two useful fields into `ModelSpec`.
- *Success.* Two structs remain (`ModelCatalogEntry` for serde, `ModelSpec` for in-memory).

### B. Intelligent selection — scoring, feedback, and self-tuning

**FIX-09. [FIXED] Add `ModelScoreboard` table and `record_llm_outcome()` helper.**
- *Problem.* We store per-call latency and tokens, but never roll up per `(model_id, task_category, strength_tag)`. `best_for()` selects purely on strength + cost (`crates/vox-orchestrator/src/models/registry.rs:240-276`).
- *Operation.* New SQL migration under `crates/vox-db/src/schema/domains/scientia.rs` adding `model_scoreboard` with columns `(model_id, task_category, strength_tag, window_days, n_calls, success_rate, p50_latency_ms, p99_latency_ms, cost_per_success_usd, quality_score, updated_at)`. Helper `vox_db::record_llm_outcome(ModelOutcome)` writes to both `llm_interactions` and an aggregation buffer. Nightly job (FIX-31) recomputes windows using true P50/P99 window functions.
- *Success.* `SELECT * FROM model_scoreboard` returns rows; `cargo test -p vox-db model_scoreboard` green. P50/P99 verified with window functions.

**FIX-10. [FIXED] Make `best_for()` read the scoreboard when available.**
- *Problem.* Selection is cost-first, not evidence-first (`registry.rs:265-267`).
- *Operation.* Inject `Option<&ModelScoreboard>` into `best_for()`. When present and `n_calls >= 30`, multiply the candidate cost by `(1 - quality_score)` before the cost sort. When absent or warming up, fall back to current behavior. Add `--force-cost` and `--force-model` CLI flags.
- *Success.* Unit test shows a historically-bad-at-codegen cheap model loses to a proven model; `vox config routing explain --task codegen` prints the ranking.

**FIX-11. [FIXED] Reconcile `llm_interactions` and `llm_feedback` joins.**
- *Problem.* Aggregation can inflate `n_calls` if one interaction has multiple feedbacks.
- *Operation.* Use a subquery/CTE for `llm_feedback` to ensure 1:1 join in the rollup query. On `insert_gamify_ai_feedback()`, also update a `llm_outcome_hints` table with `(interaction_id, signed_score)` where thumbs_up=+1 / thumbs_down=-1. The nightly aggregator joins this into `model_scoreboard.quality_score`.
- *Success.* `n_calls` in scoreboard matches `COUNT(DISTINCT interaction_id)`. A thumbs-down lowers that model's score visible via `vox model scoreboard show`.

**FIX-12. [FIXED] Wire `RiskDecision::Abstain` back into routing.**
- *Problem.* `SocratesSurfaceTelemetry.risk_decision` records abstain events (`crates/vox-db/src/socrates_telemetry.rs:142`) but never feeds re-selection.
- *Operation.* On `Abstain` with `confidence_estimate < 0.5`, orchestrator marks the `(model_id, task_category)` in a short-lived in-memory penalty map (10-minute TTL). `best_for()` skips penalized entries unless they are the only option. Penalty map is persisted as `model_penalty_events` for audit.
- *Success.* Forcing abstain in tests causes the next invocation to pick a different model.

**FIX-13. [DONE] Emit `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.response.finish_reasons` on every LLM call.**
- *Problem.* No OpenTelemetry GenAI semconv emitted. `llm_interactions.token_count` was one integer, conflating input and output.
- *Operation.* In `crates/vox-actor-runtime/src/llm/types.rs::ModelMetric::from_response`, populate the six required GenAI span attributes per OTel GenAI v1.37. Extended `llm_interactions` to accept detailed metrics. Split `token_count` into `input_tokens` and `output_tokens` columns.
- *Success.* `llm_interactions` (v59) rows contain `gen_ai`-aligned attributes for every call.

**FIX-14. [DONE] Add a trace ID that follows user-request → orchestrator → provider.**
- *Problem.* `journey_id`, `session_id`, `run_id` are subsystem-local; no causal chain.
- *Operation.* Generate a single `trace_id` (UUIDv7) at the top of `vox_orchestrator::handle_task()`. Propagate via `AgentTask.trace_id`; include in every telemetry row; set outbound HTTP `traceparent` header (OTel W3C) in `crates/vox-actor-runtime/src/http` for OpenRouter / Anthropic / Google calls.
- *Success.* `SELECT trace_id, count(*) FROM research_metrics WHERE trace_id IS NOT NULL GROUP BY 1` shows every user turn as one trace.

**FIX-75. [DONE] Implement observed-cost feedback loop via `model_pricing_catalog`.**
- *Problem.* Model pricing in `ModelSpec` was static and hardcoded, leading to stale budget estimates and inaccurate routing.
- *Operation.* Created `model_pricing_catalog` (v59) to track ground-truth costs derived from provider-reported usage. Implemented a nightly rollup that calculates `observed_blended_per_1k` from telemetry. Injected these observed prices into the `ModelRegistry` at runtime when sample confidence is high (>= 100 samples) or medium (>= 20 samples).
- *Success.* `vox model pricing show` displays real-time price corrections, and `ModelRegistry` automatically adjusts routing for providers that change their pricing dynamically.

**FIX-15. [FIXED] Track context-window utilization per call.**
- *Problem.* We know `ModelSpec.context_length_tokens` and per-call tokens but never store `utilization = (input+output)/context`.
- *Operation.* Add `context_utilization_pct` column to `llm_interactions`; compute in `ModelMetric::from_response`. When utilization > 0.8 for a `(model_id, task_category)` three times in a window, escalate selection to the next-larger context tier (planning hint into `best_for_task()`).
- *Success.* Scoreboard reports utilization; escalation occurs in tests.

**FIX-50. [FIXED] Track task complexity clamping.**
- *Operation.* In `AgentTask::complexity()`, clamp `estimated_complexity` to 1-10.
- *Success.* Complexity is always in bounds for routing table lookup.

**FIX-51. [FIXED] Support model preference hint.**
- *Operation.* Add `model_preference: Option<String>` to `AgentTask`.
- *Success.* Users can suggest specific families or aliases.

**FIX-52. [FIXED] Implement research intent routing.**
- *Operation.* If `research_hints` are present, force `TaskCategory::Research`.
- *Success.* Search-heavy tasks pick reasoning-capable models.

**FIX-53. [FIXED] Implement tool-hint floor.**
- *Operation.* If `tool_hints.len() >= 2`, force `complexity >= 7`.
- *Success.* Multi-tool tasks skip lightweight models.

**FIX-16. [DONE] Track retry / fallback chains.**
- *Problem.* Only the final result lands in `llm_interactions`; retries are invisible.
- *Operation.* New table `llm_attempt` with `(trace_id, attempt_number, model_id, provider, outcome, latency_ms, error_class)`; `llm_interactions` retains one row per final outcome. `vox-actor-runtime` writes `llm_attempt` rows during its fallback loop (`crates/vox-actor-runtime/src/model_resolution.rs:162`).
- *Success.* A forced OpenRouter 5xx triggers a row with attempt_number=1 (failed) and a row in `llm_interactions` referencing the successful retry.

**FIX-17. [DONE] Track OpenRouter cache-hit savings.**
- *Problem.* OpenRouter returns `cache_tokens` in pricing (`crates/vox-orchestrator/src/catalog.rs:60`) but we never persist hits per call.
- *Operation.* Parse `usage.cache_creation_input_tokens`, `usage.cache_read_input_tokens` from OpenRouter and Anthropic responses; store in `llm_interactions.cache_read_tokens`. Add `cache_savings_usd` to scoreboard computation.
- *Success.* `vox model scoreboard show anthropic/claude-sonnet-4.6 --with-cache` prints non-zero savings when prompt-caching is active.

**FIX-18. [DONE] Add budget-pre-check to `best_for()`.**
- *Problem.* `scoring.rs:196-198` scores down rate-limited models but does not gate by explicit budget.
- *Operation.* Add `AgentTask.budget: Option<Budget{ max_cost_usd, max_latency_ms }>`. In `best_for()`, after sorting, drop candidates whose `expected_cost > budget.max_cost_usd`. `expected_cost = spec.cost_per_1k * estimated_token_count(task)`. Reuse `estimated_token_count` helper (already exists near `scoring.rs`).
- *Success.* `vox chat --max-usd 0.10` never picks claude-mythos-preview.

**FIX-19. [DONE] Normalize strength tags to the enum at ingestion.**
- *Problem.* `catalog.rs::infer_strengths()` returns `Vec<String>`; consumers match on exact string.
- *Operation.* Return `Vec<StrengthTag>` (generated enum from FIX-02). Any unknown inference result maps to `StrengthTag::Unknown`, logged once per unique string via `tracing::warn!`.
- *Success.* `rg '"codegen"|"logic"|"review"' crates/vox-orchestrator/src | wc -l` drops by ~80% (becomes `StrengthTag::Codegen` etc.).

**FIX-20. [DONE] Publish `vox model explain` CLI.**
- *Problem.* No way for a user to see why a given model was picked.
- *Operation.* `vox model explain "<task description>" [--category codegen]` prints: (a) matched strength, (b) tier, (c) ranked candidate list with per-criterion scores, (d) final pick, (e) trace_id of the most recent real call for the same category. Lives at `crates/vox-cli/src/commands/model/explain.rs`.
- *Success.* Command exists; regression test in `crates/vox-cli/tests/` asserts the top candidate for `codegen` at `complexity=9` matches the premium alias.

### C. Automatic model discovery

**FIX-21. [DONE] Introduce the `ModelCatalog` trait as the discovery plugin surface.**
- *Problem.* `ModelCatalog` exists as a trait today but only `OpenRouterCatalog` implements it; no enumeration, no plugin registry.
- *Operation.* In `crates/vox-orchestrator/src/catalog.rs`, keep `trait ModelCatalog { async fn refresh(&self) -> Result<Vec<ModelCatalogEntry>>; fn name(&self) -> &'static str; }`. Add `CatalogRegistry { sources: Vec<Box<dyn ModelCatalog>> }`. Register sources in one place: `CatalogRegistry::default_sources()`.
- *Success.* Adding a new source (e.g., `GroqCatalog`) is a single `register(Box::new(GroqCatalog::new()))` call.

**FIX-22. [DONE] Add `OllamaCatalog`.**
- *Problem.* Local Ollama models are resolved ad-hoc via `VoxPopuliModel` secret (`model_resolution.rs:136`). No catalog entry exists.
- *Operation.* New `crates/vox-orchestrator/src/catalog/ollama.rs` that calls `GET {OLLAMA_URL}/api/tags`, parses `models[]`, maps each to a `ModelCatalogEntry` with `provider_route.primary = Ollama`, `strengths = ["generalist"]`, `cost_per_1k = 0.0`, and `context_length_tokens` parsed from `/api/show`.
- *Success.* After `ollama pull llama3.2`, `vox model list --source Ollama` shows it.

**FIX-23. [DONE] Add `HuggingFaceCatalog`.**
- *Problem.* `fetch_hf_hub_text_generation_models()` (`crates/vox-actor-runtime/src/inference_env.rs`) fetches models for display but never writes them to the registry.
- *Operation.* New `crates/vox-orchestrator/src/catalog/hf_hub.rs`. Pages through `/api/models?filter=text-generation&sort=downloads&direction=-1&limit=200`. Marks entries `provider_route.primary = HuggingFaceRouter`. Stores a fingerprint to avoid full re-ingest.
- *Success.* `vox model list --source HuggingFaceRouter | head` shows top 20 most-popular HF text-gen models; dedup works across refreshes.

**FIX-24. [DONE] Add `PopuliMeshCatalog`.**
- *Problem.* Each mesh node advertises capability hints (`VOX_MESH_ADVERTISE_GPU`, etc., via `PopuliEnv`) but there is no aggregate view of "what models can my mesh serve right now."
- *Operation.* New endpoint `GET /v1/populi/models` on the Populi control plane returns the union of each peer's `~/.vox/cache/mens/local-registry.json`. `PopuliMeshCatalog` calls it. Each entry gets `provider_route.primary = PopuliMesh`, `node_id`, `labels`.
- *Success.* After a second node joins the mesh and MENS finishes training on node A, `vox model list --source PopuliMesh` on node B shows the new checkpoint.

**FIX-25. [DONE] Add `MensCatalog` for local MENS checkpoints.**
- *Problem.* `mens/runs/<run_id>/training_manifest.json` exists but is never ingested into the routing registry (`crates/vox-populi/`).
- *Operation.* New `crates/vox-orchestrator/src/catalog/mens.rs` (or integrated in `catalog.rs`) that globs `mens/runs/*/training_manifest.json`, parses each, emits `ModelCatalogEntry`.
- *Success.* A newly-trained MENS checkpoint appears in `vox model list --source Mens` after running `vox model discover`.

**FIX-26. [DONE] Add `AnthropicDirectCatalog` and `GoogleDirectCatalog`.**
- *Problem.* Direct provider calls use hardcoded model IDs (`spec.rs:447` for `claude-mythos-preview-20260407`); no auto-refresh of Anthropic's model list.
- *Operation.* Implement `AnthropicDirectCatalog` hitting `https://api.anthropic.com/v1/models` (uses `AnthropicApiKey`). Implement `GoogleDirectCatalog` hitting `https://generativelanguage.googleapis.com/v1beta/models` (uses `GeminiApiKey`). Both emit `ModelCatalogEntry` with pricing from their known tables (kept in `contracts/orchestration/provider-pricing-overlay.v1.yaml` because Anthropic & Google don't publish per-model pricing via their models API).
- *Success.* New Anthropic model launched by vendor shows up after nightly refresh without code change to `spec.rs`.

**FIX-27. [DONE] Persist discovery results to disk.**
- *Problem.* Catalog refresh lives only in memory; restart = re-fetch = rate limit risk.
- *Operation.* `CatalogRegistry::refresh()` writes every source's output to `~/.vox/cache/model-catalog.v1.json` (atomic rename). `Registry::load()` reads it at startup. TTL embedded per-source.
- *Success.* Second run within TTL emits zero network traffic for discovery.

**FIX-28. Throttle and jitter discovery.**
- *Problem.* The current jitter is OpenRouter-only (`registry.rs:41-47`).
- *Operation.* Move `min_refresh_interval_secs` and `jitter_ms` into `contracts/orchestration/model-routing.v1.yaml::[discovery]` per-source. Enforce in `CatalogRegistry`.
- *Success.* Env-based overrides still work; YAML is the default.

**FIX-29. [DONE] Enforce a catalog freshness SLO.**
- *Problem.* No alert if the catalog goes stale (provider outage, auth expired).
- *Operation.* `vox model discover --dry-run --check-freshness` returns non-zero if any source's `last_refresh > max_age` from YAML. Wire into `vox doctor` via `Model Scoreboard` freshness check.
- *Success.* Deliberately expiring the cache causes `vox doctor` to report `WARN: Model Scoreboard stale`.

**FIX-30. [DONE] Add a `vox model discover` CLI front-end.**
- *Problem.* Discovery is implicit; users cannot force-refresh. Also requires pricing joins for `cost_per_success_usd`.
- *Operation.* Implement `vox model discover` and `vox model rollup`. Add `cost_usd` to `llm_interactions` for historical accuracy.
- *Success.* `vox model discover --source OpenRouter --force` prints counts; `vox model rollup` calculates accurate costs.

**FIX-31. [FIXED] Schedule a nightly discover + scoreboard roll-up.**
- *Problem.* No cron, no persistent scheduled task surface.
- *Operation.* Use the existing `scheduled-tasks` MCP / skill surface referenced in this repo's ops tooling to create:
  1. `vox-model-discover-nightly` — runs `vox run scripts/orchestrator/model_discover.vox` daily at 03:00 local.
  2. `vox-scoreboard-rollup-nightly` — runs `vox run scripts/orchestrator/scoreboard_rollup.vox` daily at 03:15.
- *Operation cont.* Both scripts are `.vox` files (per `AGENTS.md` VoxScript-First policy). Write them to `scripts/orchestrator/`.
- *Success.* `vox scheduled-tasks list` shows both tasks; missing-run alerts via `vox doctor`.

**FIX-32. [FIXED] Expose `vox model scoreboard show` and `vox model scoreboard export --csv`.**
- *Problem.* Scoreboard invisible to users.
- *Operation.* New CLI under `crates/vox-cli/src/commands/model/scoreboard.rs`.
- *Success.* CSV round-trip parses; dashboard can consume.

### D. Telemetry hardening

**FIX-33. Introduce `contracts/orchestration/model-telemetry.v1.yaml`.**
- *Problem.* Event names and field sets are declared ad-hoc across `crates/vox-db/src/*_telemetry.rs`.
- *Operation.* Enumerate every event `(vox.model.request, vox.model.response, vox.model.error, vox.model.attempt, vox.model.discover, vox.model.score_update)` with attributes mapping to OTel GenAI semconv names. Generate validators in `research_metrics_contract.rs`.
- *Success.* `vox ci telemetry-validate` passes; unknown events rejected.

**FIX-34. Add session-prefix enforcement.**
- *Problem.* Prefixes `bench:`, `mcp:`, `workflow:` are by convention only.
- *Operation.* In `validate_research_metric_row()` (`crates/vox-db/src/research_metrics_contract.rs`), require `session_id` to start with one of the registered prefixes from the YAML. Fail-closed in strict profile.
- *Success.* Tests with wrong prefix reject at insert.

**FIX-35. Replace `target: "vox_dei::*"` tracing targets, repo-wide (pairs with FIX-07).**
- *Problem.* Legacy span names make dashboards diverge from module names.
- *Operation.* `rg -l 'target: "vox_dei'` — rewrite each occurrence to `target: "vox_orchestrator::<file_stem>"`.
- *Success.* `rg 'target: "vox_dei'` returns zero.

**FIX-36. Delete `detect_constructs()` in `vox-eval`.**
- *Problem.* Deprecated since 0.4.0 (`crates/vox-eval/src/lib.rs:194`).
- *Operation.* Remove function and its callers (use `ast_eval()`); bump minor version; update `CHANGELOG.md`.
- *Success.* `rg 'detect_constructs'` returns zero.

**FIX-37. Make the Socrates double-write transactional.**
- *Problem.* `record_socrates_surface_event()` and `record_socrates_eval_summary()` are separate writes; partial-failure loses rollup (`crates/vox-db/src/socrates_telemetry.rs:142`).
- *Operation.* Wrap both in a single libsql/turso transaction; return `Result<()>` that fails if either side errors; add unit test with a mock-failing connection.
- *Success.* Simulated failure leaves zero partial rows.

**FIX-38. Emit telemetry via OTel OTLP when `VoxTelemetryUploadUrl` is set.**
- *Problem.* Telemetry sinks only to local `research_metrics`; remote upload exists (`docs/src/adr/023-optional-telemetry-remote-upload.md`) but isn't OTel-shaped.
- *Operation.* Add `vox-actor-runtime/src/telemetry/otlp.rs` exporter that mirrors each `gen_ai.*` span to OTLP HTTP when the upload URL is configured. Respect `VoxTelemetryUploadToken` (vox-secrets).
- *Success.* `vox telemetry test` delivers a span to a local Jaeger/OTel-collector.

**FIX-39. Document and enforce the `trace_id` contract.**
- *Problem.* `trace_id` added in FIX-14 has no written contract.
- *Operation.* Extend `docs/src/reference/telemetry-metric-contract.md` with a "Trace ID" section: UUIDv7 required, propagated via `traceparent` outbound, stored as `trace_id` on every event row.
- *Success.* `vox ci telemetry-validate` rejects rows lacking `trace_id` for events from the `vox.model.*` family.

**FIX-40. Add `vox.model.attempt` event emission in the retry loop.**
- *Problem.* `llm_attempt` rows (FIX-16) need a matching telemetry event for live dashboards.
- *Operation.* Every attempt fires `vox.model.attempt` with `gen_ai.request.model`, `attempt_number`, `outcome`, `error_class`.
- *Success.* Dashboards can compute per-provider failure rates without joining SQL.

### E. Secrets & decentralized secret distribution

**FIX-41. Fix `OPENAI_API_KEY` violation in `vox-schola`.**
- *Problem.* `crates/vox-schola/src/curator.rs` reads `OPENAI_API_KEY` directly via `std::env::var`. Violates `AGENTS.md:58`.
- *Operation.* Replace with `vox_secrets::resolve_secret(SecretId::OpenaiApiKey)?`. Delete the `env::var` line. Add a unit test verifying the call fails open in `Profile::Dev` when the secret is missing.
- *Success.* `vox ci secret-env-guard` passes.

**FIX-42. [DONE] Migrate `TOGETHER_FINETUNE_MODEL` to vox-secrets or config.**
- *Problem.* `crates/vox-cli/src/commands/ai/train.rs` reads directly.
- *Operation.* Decide: if secret, add `SecretId::TogetherFinetuneModel` to `crates/vox-secrets/src/spec/registry/llm.rs` and migrate. If it is non-secret model name, add to `OPERATOR_TUNING_ENVS` in `crates/vox-secrets/src/lib.rs` (line ~59).
- *Success.* `secret-env-guard` passes.

**FIX-43. [DONE] Migrate `GEMINI_DIRECT_MODEL` and `OPENROUTER_GEMINI_MODEL` to the config domain.**
- *Problem.* `crates/vox-config/src/routing_policy.rs` reads them directly for routing choice.
- *Operation.* Add both to `OPERATOR_TUNING_ENVS` (they are *routing preference*, not secrets). Documented in new `docs/src/reference/routing-env.md`.
- *Success.* `secret-env-guard` passes; routing table generator reads the names from one place.

**FIX-44. [DONE] Add `POPULI_URL` to vox-secrets spec (as non-secret config) or rename.**
- *Problem.* `crates/vox-config/src/inference.rs:68-72` reads `POPULI_URL` → falls back to `OLLAMA_URL`. Neither is in vox-secrets. Confusing name: this is the local Ollama base URL used by Populi, not an auth key.
- *Operation.* Add to `OPERATOR_TUNING_ENVS`. Add deprecation alias: prefer `VOX_POPULI_LOCAL_OLLAMA_URL` going forward; keep `POPULI_URL` and `OLLAMA_URL` as deprecated aliases with a doctor warning.
- *Success.* Doctor prints the canonical name; older names still work.

**FIX-45. [DONE] Add `shareable: bool` to `SecretSpec` and default per-secret.**
- *Problem.* Foundation for FIX-46–FIX-50. Today the spec has no "share across my own mesh" flag.
- *Operation.* Extend `crates/vox-secrets/src/spec/mod.rs::SecretSpec` with `shareable: bool` and `sensitivity: Sensitivity { UserMeshOnly, UserMeshAndExternalVault }`. Default true for LLM API keys (`OpenRouterApiKey`, etc.), default false for `VoxIdentityKeyPath`, `VoxMeshJwtHmacSecret`, `VoxIdentityMasterPwd`.
- *Success.* `cargo test -p vox-secrets shareable_defaults` green.

**FIX-46. [DONE] Implement X25519 sealed-box in `vox-crypto`.**
- *Problem.* We have ChaCha20-Poly1305 (symmetric) and Ed25519 (signing) but no X25519 KEM (asymmetric encryption). Required for wrapping a secret for a specific peer without a pre-shared key.
- *Operation.* Add `x25519-dalek` dependency (pure-Rust, already in the Rust crypto ecosystem allowlist; no cmake/nasm). Expose in `crates/vox-crypto/src/facades.rs`: `fn seal(recipient_pub: &X25519PublicKey, plaintext: &[u8]) -> SealedBox` and `fn unseal(recipient_priv: &X25519PrivateKey, sealed: &SealedBox) -> Result<Vec<u8>>` using libsodium-style `crypto_box_seal` semantics (ephemeral sender key + ChaCha20-Poly1305).
- *Success.* Unit test: Alice seals → Bob unseals, round trip in 1ms.

**FIX-47. Add X25519 keypair to `NodeIdentity`.**
- *Problem.* `NodeIdentity` has Ed25519 only (`crates/vox-identity/src/identity.rs:20-31`).
- *Operation.* Add `x25519_signing_key` and `x25519_public_key`; store alongside the Ed25519 keypair. Derive deterministically from the same seed via HKDF-BLAKE3(ed25519_seed, "vox-x25519-v1").
- *Success.* Node advertises `x25519_pub` in its capability record; `vox populi nodes` shows it.

**FIX-48. Implement `vox secrets pair` device-pairing flow.**
- *Problem.* No user journey for "install my key once, use on any node."
- *Operation.* On Node A: `vox secrets pair` generates a 128-bit nonce, prints a 5-word mnemonic and a QR encoding `{node_a_x25519_pub, nonce, expires_unix_ms}`. On Node B: `vox secrets pair --accept <mnemonic|QR>` performs X25519 ECDH with A's public key, constructs `PairingRequest { node_b_x25519_pub, signed=Ed25519(nonce) }`, sends to A via the Populi control plane (FIX-49). A verifies Ed25519, prompts the user to approve `"pair with <nickname> (x25519_pub fingerprint)"`, writes both peers into each side's `TrustedNodeRegistry` (`crates/vox-identity/src/storage.rs`).
- *Success.* End-to-end test: two in-process mesh nodes complete pairing in <2s; replay of the same mnemonic fails.

**FIX-49. Implement `SecretsSync` gossip.**
- *Problem.* Secrets are per-node today.
- *Operation.* New crate `crates/vox-secrets-sync/` (or submodule of `vox-secrets`):
  1. On a local secrets write (`set`, `import-env`), iterate `TrustedNodeRegistry`. For each peer:
     - Read current value for each `SecretSpec { shareable: true }`.
     - Seal via `vox_crypto::seal(peer.x25519_pub, value)` (FIX-46).
     - Wrap in envelope `SecretsSyncEnvelope { sender_node_id, secret_id, sealed_ciphertext, version_counter, signed_at, signature: Ed25519 }`.
     - Deliver over `A2ADeliverRequest.jwe_payload` (`crates/vox-populi/src/transport/mod.rs:76`).
  2. On receive: verify signature, verify peer is trusted, unseal, write to local vox-secrets with source = `SyncedFrom(peer_node_id)`, bump local version counter.
  3. Last-writer-wins on conflicts, tagged by `signed_at`.
- *Success.* Setting `OPENROUTER_API_KEY` on Node A via `vox secrets set` causes Node B to have it within ~500ms; `vox secrets status` on B shows source `SyncedFrom(A)`.

**FIX-50. Add `vox secrets sync --now` and `--dry-run`.**
- *Problem.* Need manual control for initial bootstrap and audit.
- *Operation.* `vox secrets sync --now` pushes current local state to all trusted peers. `--dry-run` lists what would be pushed without sending.
- *Success.* Users can force sync after connectivity drops.

**FIX-51. Add `vox secrets rotate <secret>` (per-peer re-encryption).**
- *Problem.* Key rotation today means editing on every node.
- *Operation.* Rotation bumps the local version, triggers a `SecretsSync` push with the new value; peers replace on receipt. Old value archived locally for 24h for rollback.
- *Success.* Rotation audit trail in `secrets_audit_log`.

**FIX-52. Populate `A2ADeliverRequest.jwe_payload` end-to-end.**
- *Problem.* Field exists but is always empty (`crates/vox-populi/src/transport/mod.rs:76`).
- *Operation.* `SecretsSync` is the first consumer. Document that the field is free-form encrypted bytes; it is agnostic to the cipher — vox-secrets uses `seal()` output, other callers may use OpenPGP or JWE. Gate in handler to reject oversized payloads (> 64 KiB).
- *Success.* Integration test: `jwe_payload` non-empty on a sync delivery; handler unseals successfully.

**FIX-53. Add structured log redaction middleware.**
- *Problem.* No central redactor; a developer adding `debug!("{:?}", secret)` could leak.
- *Operation.* `crates/vox-actor-runtime/src/telemetry/redact.rs`: a `tracing` layer that scans each event's fields for known patterns (regex: `sk-[A-Za-z0-9_]{20,}`, `xoxp-…`, `AIza[0-9A-Za-z\\-_]{35}`, etc.) and replaces matches with `<REDACTED:<kind>>`. Install in the default subscriber.
- *Success.* `tracing::debug!("{}", "sk-live-abcdefghijklmnopqrstuvwx")` emits `<REDACTED:openai>`.

**FIX-54. Format-string leak audit.**
- *Problem.* `crates/vox-container/src/docker.rs` and `.../podman.rs` use `format!("{key}={val}")` for `--build-arg`; values could contain secrets (identified as moderate-risk).
- *Operation.* Rewrite the flag construction to use `--build-arg <key>` with the value placed via a tempfile (or `--secret`). Prohibit passing any `SecretSpec`-managed value as a build arg; reject in the container builder with a clear error.
- *Success.* Attempting to pass a vox-secrets secret as a docker build arg fails fast with remediation.

**FIX-55. `vox secrets import-env` UX pass.**
- *Problem.* Import is fire-and-forget; users don't know what happened.
- *Operation.* Add summary table: `(secret_id, source_env_name, action: imported|skipped|exists)`. `--interactive` prompts before each. After successful import, offer `vox secrets sync --now`.
- *Success.* First-time-user journey ends in a working multi-node setup in <2 minutes.

**FIX-56. Break-glass: `vox secrets unpair <node_id>`.**
- *Problem.* No revocation path.
- *Operation.* Removes peer from `TrustedNodeRegistry`, bumps a revocation counter, broadcasts `UNPAIR` signed message so other peers also drop. Future sync deliveries from the removed peer are rejected.
- *Success.* A revoked node stops receiving sync updates within one round trip.

### F. Vox.toml / operator surfaces / dead code

**FIX-57. Remove duplicate `ModelTier` definition.**
- *Problem.* (Part of FIX-02 but listed separately for the PR).
- *Operation.* Delete `crates/vox-orchestrator/src/models/routing_table.rs:6-17`; reroute imports.
- *Success.* `cargo build` green; `rg 'enum ModelTier'` = 1.

**FIX-58. Collapse `premium_alias` layering.**
- *Problem.* Two layers (`spec.rs:206-223` hardcoded, `registry.rs:162-166` TOML override). Both survive convergence but layering is implicit.
- *Operation.* Document layering explicitly in `contracts/orchestration/model-routing.v1.yaml::[premium_alias]`. TOML override under `~/.vox/models.toml` is the operator escape hatch, YAML is the repo default, built-in is retired.
- *Success.* `vox model explain --show-layers` prints exactly which layer won.

**FIX-59. Deduplicate Gemini model IDs.**
- *Problem.* `spec.rs:336` uses `google/gemini-2.0-flash-lite` (free tier); `crates/vox-config/src/routing_policy.rs:127` defaults to `gemini-2.5-flash`; `spec.rs:388` uses `google/gemini-2.5-pro-preview`; no preview suffix convention.
- *Operation.* Centralize in `contracts/orchestration/model-catalog.bootstrap.v1.json`; use live catalog as source of truth post-bootstrap. Remove hardcoded IDs from `routing_policy.rs` (read from registry via `Registry::best_of_family("google")`).
- *Success.* One text grep for each Gemini ID.

**FIX-60. Remove `research_eval_runs.tier_distribution_json` if unused.**
- *Problem.* Column populated but no consumer (`crates/vox-db/src/schema/domains/scientia.rs`).
- *Operation.* Add a dashboard consumer (scoreboard.rollup tier distribution by provider) OR drop the column in a migration.
- *Success.* Either it is consumed or it is gone.

**FIX-61. Replace `gpt-4o-mini` and `gpt-4o` bootstrap fallbacks.**
- *Problem.* Hardcoded legacy fallbacks (`crates/vox-config/src/bootstrap_inference.rs:6-20`) may be deprecated by OpenAI in 2026; will cause silent selection of unavailable models.
- *Operation.* Replace with `openrouter/auto` for generic fallback; replace `RESEARCH_FLASH_FALLBACK` with `google/gemini-2.5-flash` resolved via registry (FIX-59 machinery); replace `REVIEW_PREMIUM_FALLBACK` with the premium_alias for `review`.
- *Success.* `rg 'gpt-4o-mini|gpt-4o' crates/vox-config` returns zero code hits.

**FIX-62. Turn `models.toml` into an override-only surface.**
- *Problem.* `registry.rs:139-154` lazily creates `~/.vox/models.toml` with all defaults; users then hand-edit, so the file and the live catalog diverge silently.
- *Operation.* Change semantics: `models.toml` contains ONLY `[premium_alias]` and `[overrides.<model_id>]` sections; no `[[models]]` arrays. Migration: on first boot after this change, rewrite existing `models.toml` stripping the `[[models]]` table and leaving a comment pointer.
- *Success.* A pristine install has a minimal `models.toml`; overrides survive.

**FIX-63. Retired naming — archive legacy orchestrator DEI identifiers in logs / docs / symbols.**
- *Problem.* Mixed references; developers expect the retired orchestrator codename to exist somewhere.
- *Operation.* Grep for `vox_dei`, `dei_`, and hyphenated legacy spellings across `crates/**/src/**.rs` and either rename or add `#[allow(dead_code)] mod dei_shim` with a doc comment pointing to the new home. (One known non-code hit is `docs/src/reference/` — leave archival mentions in `docs/src/archive`.)
- *Success.* Only the archived directory and the retirement table in `AGENTS.md` mention the legacy orchestrator codename.

**FIX-64. Gate MENS training scripts behind `vox ci secret-env-guard`.**
- *Problem.* Training scripts under `scripts/mens/` may read envs directly.
- *Operation.* Add a `.vox` script linter that fails on `env.get(...)` for anything whose name matches a `SecretSpec` regex.
- *Success.* PR CI fails if a `.vox` script reads a managed secret directly.

**FIX-65. Track cost-per-success on MENS vs. remote.**
- *Problem.* Operators cannot answer "was MENS cheaper than OpenRouter this week?"
- *Operation.* `vox model scoreboard show --group-by provider` reports `cost_per_success_usd` per provider. Add a weekly rollup report to `vox doctor --report weekly`.
- *Success.* Report present and human-readable.

**FIX-66. Fix the `ProviderType::Custom(String)` lint hole.**
- *Problem.* `Custom(String)` variant (`spec.rs:80-106`) short-circuits strength inference and scoreboard keys because the string varies per install.
- *Operation.* Canonicalize via `host_of(base_url)` when storing in telemetry & scoreboard; keep the raw string for display only.
- *Success.* Two installs with the same custom provider share scoreboard rows.

**FIX-67. Validate `Vox.toml` against a schema.**
- *Problem.* `Vox.toml` parsed ad-hoc per-section; no JSON Schema.
- *Operation.* Add `contracts/Vox.v1.schema.json`; validate via a new `vox ci vox-toml-validate` guard.
- *Success.* Unknown keys in `Vox.toml` fail CI.

**FIX-68. Document every operator knob in `docs/src/reference/routing-env.md`.**
- *Problem.* `VOX_AUTO_MODEL_STRATEGY`, `VOX_AUTO_ROUTING_PRIORITY`, `VOX_GEMINI_ROUTE_POLICY`, `VOX_OPENROUTER_CATALOG_MIN_REFRESH_INTERVAL_SECS`, `VOX_OPENROUTER_CATALOG_REFRESH_JITTER_MS` have no single docs home.
- *Operation.* Write the reference doc, link from `AGENTS.md` "Related Operational Surfaces" and from `docs/src/reference/cli.md`.
- *Success.* Every env in `OPERATOR_TUNING_ENVS` appears in the doc.

**FIX-69. [DONE] `vox doctor` reports catalog source health.**
- *Problem.* Users don't know which sources failed to refresh.
- *Operation.* Add a `CatalogSourceHealth` check that prints `OpenRouter: 312 models (fresh 02h ago) | Ollama: 8 models (fresh 05m ago) | PopuliMesh: 3 models (fresh 00m ago) | HFHub: STALE (38h ago)`.
- *Success.* Doctor output visually obvious.

**FIX-70. [FIXED] Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.**
- *Problem.* New research/plan docs must be discoverable.
- *Operation.* Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
- *Success.* Page listed by the Starlight sidebar.

---

## Part 4 — Industry alignment notes

### 4.1 OpenTelemetry GenAI semconv v1.37

The OpenTelemetry project standardizes `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.response.finish_reasons`, and related attributes across LLM providers. Adopting this surface makes every Vox model call legible to any OTel collector (Datadog, Jaeger, Honeycomb, Grafana Tempo). See FIX-13, FIX-33, FIX-38. ([OpenTelemetry GenAI spec](https://opentelemetry.io/docs/specs/semconv/gen-ai/)).

### 4.2 Model-router landscape

RouteLLM (LMSYS) is open source routing logic that pairs a classifier with a two-model setup (cheap + strong) and reports up to ~85% cost reduction at ~95% benchmark retention. LiteLLM is a unified proxy for 100+ providers with fallbacks, budgets, and an admin UI. Martian classifies prompts with a small local model to pick the optimal destination. OpenRouter itself is a gateway plus catalog. The Vox orchestrator can borrow RouteLLM's "trained classifier" pattern (future work — FIX-10 lays the data foundation) while keeping our own gateway instead of LiteLLM, because we need secret-plane integration and mesh-node routing that third-party gateways do not provide. ([LLM router comparison 2026](https://inworld.ai/resources/best-llm-router-ai-gateway)).

### 4.3 OpenRouter models API

`GET /api/v1/models` returns `data[].pricing.{prompt,completion,request,image,reasoning}`, `data[].context_length`, `data[].architecture.{input_modalities,output_modalities}`, and `data[].supported_parameters[]`. `GET /api/v1/models/{author}/{slug}/endpoints` returns per-endpoint uptime and rate limits. These are exactly the inputs Vox needs for `ModelCatalogEntry`. ([OpenRouter models endpoint](https://openrouter.ai/docs/api/api-reference/models/get-models)).

### 4.4 Ollama API

`GET /api/tags` returns local models; `POST /api/show` returns parameter count, context length, quantization, and template. This covers everything `OllamaCatalog` (FIX-22) needs. ([Ollama list models](https://docs.ollama.com/api/tags)).

### 4.5 Cryptography for mesh-secret sync

`age` / `rage` is a pure-Rust file-encryption tool built around X25519 recipients and ChaCha20-Poly1305 — same primitives we already allow (`AGENTS.md:76`). We will not depend on `age` directly but will use the same pattern with `x25519-dalek` and our own ChaCha20-Poly1305 bindings. This keeps the bans in `AGENTS.md` intact (no `ring`, no AEGIS, no cmake/nasm). See FIX-46. ([rage documentation](https://docs.rs/age/latest/age/)).

---

## Part 5 — Staged rollout (high-level)

**Stage 1 — SSOT scaffolding (FIX-01..08, FIX-33, FIX-70).** Land the YAML contracts + codegen, collapse the enum duplicates, retire `vox_dei::*` targets. No runtime behavior change.

**Stage 2 — Telemetry v1 (FIX-13..17, FIX-34..40).** OTel GenAI attributes on every call; trace IDs; retry/attempt table; cache savings.

**Stage 3 — Discovery v1 (FIX-21..32).** Plugin trait, per-source catalogs, disk cache, nightly schedule. Users gain `vox model discover` and `vox model list --source <name>`.

**Stage 4 — Scoreboard + self-tuning (FIX-09..12, FIX-18..20).** Write the scoreboard table, roll up, feed into `best_for()`, expose `vox model explain` / `vox model scoreboard show`. This is where "the code learns which models to use."

**Stage 5 — Secrets v2 (FIX-41..56).** Secret-env-guard fixes, X25519 primitives, device pairing, `SecretsSync` gossip. This is where the single-login-across-mesh user journey completes.

**Stage 6 — Cleanup & docs (FIX-57..69).** Bootstrap-model rename, operator-knob documentation, dead-code removal, doctor polish.

---

## Part 6 — Success metrics

- **Zero hardcoded model IDs** outside `contracts/orchestration/model-catalog.bootstrap.v1.json` (grep-able in CI).
- **`rg 'enum (ModelTier|ProviderType|StrengthTag|ChatRouteBackend)'` returns one match each.**
- **Every LLM call emits `gen_ai.request.model` and `trace_id`.**
- **`model_scoreboard` has ≥1 row per `(model_id, task_category)` pair seen in the last 30 days.**
- **A user installs `OPENROUTER_API_KEY` once, `vox secrets pair`s a second node, and that node completes `vox chat` without re-entering the key.**
- **`vox ci secret-env-guard` returns zero violations.**
- **`vox model discover --all --force` succeeds** and writes a cache with ≥5 sources.
- **`vox ci ssot-audit` passes**, confirming parity between scoreboard telemetry and routing decisions.
- **Telemetry-driven cost accounting and verified fail-over routing loops are enabled by default for the next development sprint (Production Transition Complete).**

---

## Sources

- [OpenRouter — List all models and their properties](https://openrouter.ai/docs/api/api-reference/models/get-models)
- [OpenRouter — Models overview](https://openrouter.ai/docs/guides/overview/models)
- [OpenRouter — List endpoints for a model](https://openrouter.ai/docs/api/api-reference/endpoints/list-endpoints)
- [OpenTelemetry — GenAI semantic conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/)
- [OpenTelemetry — GenAI metrics](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-metrics/)
- [OpenTelemetry — GenAI client spans](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/)
- [Ollama — List models API](https://docs.ollama.com/api/tags)
- [Ollama — Model management API](https://deepwiki.com/ollama/ollama/3.1-model-management-api)
- [LMSYS — RouteLLM](https://github.com/lm-sys/RouteLLM)
- [LLM router comparison 2026](https://inworld.ai/resources/best-llm-router-ai-gateway)
- [LiteLLM alternatives 2026](https://toolsinfo.com/c/litellm-alternatives/guide)
- [age/rage encryption — X25519 recipients](https://docs.rs/age/latest/age/)
- [Vox `AGENTS.md` (this repo)](../../../AGENTS.md)
- [Vox Secrets SSOT (this repo)](../reference/secrets-ssot.md)
