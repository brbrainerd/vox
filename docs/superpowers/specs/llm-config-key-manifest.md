---
title: "LLM/AI Config-Key Manifest (Phase 0 inventory)"
description: "Consolidated inventory of every LLM/AI setting key across the workspace, with band split and the four-registry reconciliation findings that reshape Band A Phase 1."
category: "Architecture SSOTs"
---

# LLM/AI Config-Key Manifest — Phase 0 Inventory

Produced by a 6-agent parallel read-only sweep (2026-06-15) seeding the Band A registry. Source plan: `docs/superpowers/plans/2026-06-15-llm-ai-settings-ssot-band-a.md`.

## CRITICAL FINDING — there are FOUR registries today, not three

The spec assumed three drifting registries (vox-config accessors / operator_registry / vox-gui FIELDS). The sweep found a **fourth, and most complete**, one:

1. **`vox-secrets/src/spec/registry/llm.rs`** — a declarative `SecretId` registry of **~70 LLM keys** (lines 8–748): canonical env name + aliases + secret classification + `resolve_secret(SecretId)` resolution with profiles (`DevLenient`/`CiStrict`/…) and cutover phases. This is the richest existing SSOT seed. Covers every provider (OpenRouter, OpenAI, Anthropic, Gemini, Groq, Mistral, DeepSeek, SambaNova, Together, Cerebras, HF, Ollama, RunPod, Vast, v0, OpenClaw), all `*_TUNING_*`, all routing/capability knobs, all `*_CHAT_COMPLETIONS_URL`.
2. **`vox-config` accessors** (~30 fns in `inference.rs`/`routing_policy.rs`) — typed wrappers; **many duplicate keys already in vox-secrets/llm.rs** (e.g. `OLLAMA_TUNING_*`, `OPENROUTER_API_KEY`, all tuning temps). Most accessors actually call `vox_secrets::resolve_secret` under the hood already.
3. **`vox-config/operator_registry.rs`** — partial CI-only metadata (infra knobs, not the LLM surface).
4. **`vox-gui/.../user_config.rs FIELDS`** — only **12 keys** surfaced (fewer than the spec's "~16" estimate).

**Implication for Phase 1 (decision needed — see 5-point analysis):** the SSOT should likely be **vox-secrets `SecretId` + `spec/registry/llm.rs`** (extended with the non-secret display metadata: group/kind/label/hint/options/default), with vox-config accessors and the GUI catalog generated as *views* — rather than minting a *fifth* registry in vox-config as the plan currently says.

## Other grounding facts (for later phases)

- **Egress is already centralized** for the sanctioned path: `vox-actor-runtime/src/llm/{chat,stream,embed}.rs` all funnel through `vox_http_client::client()` (chat.rs:55, stream.rs:47, embed.rs:76). The seal target is precise.
- **`vox-gamify` is the only real second egress**: builds its own `reqwest` requests (does NOT use `vox_http_client`) to Gemini (`generativelanguage.googleapis.com`, transport.rs:78,376) and OpenRouter (transport.rs:122,234). Refactor must preserve: 5-model OpenRouter free cascade, provider cascade (Ollama→Pollinations→Gemini→OpenRouter→deterministic), `x-response-cost` cost callback, retry-after callback, Pollinations GET, Ollama probe, structured `AiError`. **Pollinations + deterministic + Ollama-probe are NOT facade-covered** → keep as pre-cascade local fallbacks; route only Gemini + OpenRouter through the facade.
- **`EnvSecretShapeDetector` already exists** (`detectors/env_secret_shape.rs`, rule `vox/secret/env-get-shape`) and flags direct `env::var()` of secret-shaped keys. The planned `unregistered_llm_env` detector must complement, not duplicate it (non-secret LLM knobs only).
- **Detector facts** (grounded): registration at `detectors/mod.rs:178-179`; `Finding` struct fields confirmed (rules.rs:136); `SourceFile::new(PathBuf, String)` (rules.rs:102); langs Vox/Rust/TS.
- **Tauri wiring** (grounded for Phase 3): setup hook `vox-gui/src/main.rs:61-99`; `invoke_handler` registers user_config cmds at 171-173; existing emit pattern `spawn_orchestrator_status_stream` emits `vox://orch-status` via `tauri::Emitter` (`commands/orchestrator.rs`). Frontend `ui/src/components/surfaces/Settings/SettingsView.tsx` → `RuntimeConfigSection` (line 710); vitest `SettingsView.test.tsx`. SettingsView already has 12 tabs incl. "LLM & providers" and "Model routing".
- **arch-check schema** (grounded for Phase 4): use `[[forbidden_pattern]]` (with `exempt_files` + `allow_annotation`) — template at layers.toml:240-276 (`raw-git-exec`); or `[[forbidden_deps]]` (layers.toml:61-69). vox-gui is L5 binary, must NOT be excluded from sweeps but would be an `exempt_files` entry if it legitimately reaches a knob.

## Band A keys (provider / endpoint / model / tuning / budget) — the registry seed

Canonical source: vox-secrets/llm.rs unless noted. Secret keys resolve via Clavis (never written to config.toml).

| env | kind | secret | group | reads_via | notes |
|---|---|---|---|---|---|
| OPENROUTER_API_KEY (+VOX_ alias) | String | y | Models | resolve_secret / openrouter_api_key() | |
| OPENAI_API_KEY | String | y | Models | resolve_secret | |
| ANTHROPIC_API_KEY | String | y | Models | resolve_secret | |
| GEMINI_API_KEY (+VOX_ alias) | String | y | Models | resolve_secret | |
| GROQ_API_KEY / MISTRAL_API_KEY / DEEPSEEK_API_KEY / SAMBANOVA_API_KEY / TOGETHER_API_KEY / CEREBRAS_API_KEY / RUNPOD_API_KEY / VAST_API_KEY / V0_API_KEY / CUSTOM_OPENAI_API_KEY | String | y | Models | resolve_secret | per-provider keys |
| HF_TOKEN (+HUGGING_FACE_HUB_TOKEN, VOX_ aliases) | String | y | Models | huggingface_hub_token() | |
| OPENCLAW_TOKEN / OPENCLAW_API_KEY | String | y | Models | resolve_secret | skill publishing |
| OPENROUTER_BASE_URL | Url | n | Models | openrouter_base_url() | default https://openrouter.ai/api |
| VOX_OPENAI_BASE_URL (+legacy OPENAI_BASE_URL) | Url | n | Models | openai_compatible_base_url() | default https://api.openai.com/v1 |
| VOX_POPULI_LOCAL_OLLAMA_URL / POPULI_URL / OLLAMA_URL / OLLAMA_HOST | Url | mixed | Models | local_ollama_populi_base_url() | 3-tier precedence; first is secret |
| {OPENROUTER,OPENAI}_{CHAT_COMPLETIONS,EMBEDDINGS,MODELS_LIST}_URL | Url | n | Models | const fns inference.rs | hardcoded endpoint fallbacks |
| VOX_{GROQ,CEREBRAS,MISTRAL,DEEPSEEK,SAMBANOVA,ANTHROPIC}_CHAT_COMPLETIONS_URL | Url | n | Models | resolve_secret | per-provider endpoint overrides |
| HF_DEDICATED_CHAT_URL / VOX_HF_ROUTER_CHAT_COMPLETIONS_URL | Url | n | Models | hf_* accessors | |
| OPENROUTER_CHAT_MODEL / OPENROUTER_MODEL / OPENROUTER_GEMINI_MODEL / GEMINI_DIRECT_MODEL / HF_CHAT_MODEL / HF_DEDICATED_CHAT_MODEL / OLLAMA_MODEL / TOGETHER_FINETUNE_MODEL | String | n | Models | resolve_secret / accessors | preferred model ids |
| {OLLAMA,OPENAI,ANTHROPIC,GEMINI,TOGETHER}_TUNING_TEMPERATURE | Float | n | Tuning | *_tuning_temperature() | |
| {OLLAMA,OPENAI,ANTHROPIC,GEMINI,TOGETHER}_TUNING_TOP_P | Float | n | Tuning | *_tuning_top_p() | |
| OLLAMA_TUNING_NUM_CTX | Int | n | Tuning | ollama_tuning_num_ctx() | |
| OPENROUTER_HTTP_REFERER / OPENROUTER_APP_TITLE / OPENROUTER_ROUTE_HINT | String | n | Models | resolve_secret | attribution headers |
| vox_populi::inference_PROFILE | Enum | n | Models | inference_profile_from_env() | desktop_ollama/cloud/mobile*/lan |
| VoxConfig.model | String | n | General | VoxConfig | default model id |
| VoxConfig.daily_budget_usd / per_session_budget_usd | Float | n | General | VoxConfig | |
| VoxConfig.llm_max_concurrent_requests / llm_{openrouter,openai}_max_concurrent / llm_retry_max_attempts | Int | n | Tuning | VoxConfig | throttle/retry |

## Band B keys (orchestrator selection / routing / cascade / autonomic) — SEPARATE plan

Seeds the future Band B plan. Do NOT register in Band A.

- **Raw env::var in select/policy** (should move behind accessors): VOX_MODEL_FORCE, VOX_MODEL_AXES (`33:33:34`), VOX_ROUTING_ENABLE_EXPLORATION, VOX_EXPLORATION_BUDGET_EXHAUSTED, VOX_PROVIDER_UNAVAILABLE (select.rs/policy.rs).
- **Routing/capability secrets** (vox-secrets/llm.rs): VOX_AUTO_MODEL_STRATEGY, VOX_AUTO_ROUTING_PRIORITY, VOX_GEMINI_ROUTE_POLICY, VOX_ROUTING_PROFILE, VOX_ROUTING_EXPLORATION_EPSILON, VOX_ROUTING_MAX_SPEND_USD_PER_SESSION, VOX_ROUTING_PROVIDER_{ALLOW,DENY}LIST, VOX_ROUTING_HARD_PIN_MODEL, VOX_CAPABILITY_* (require/prefer reasoning/tool/vision/image/codegen/web_search + model pins), VOX_ANTHROPIC_DIRECT.
- **Scoring magic-number consts** (`models/scoring.rs:6-33`, ~25 of them): QUALITY_*, EFFICIENCY_COST_SCALER, COMPLEXITY_*, FIM_*, ECONOMY/PERFORMANCE bonuses, RATE_LIMITED_SCORE_FLOOR, BUDGET_*, THROUGHPUT_*, DEEPSEEK_OFFPEAK_*.
- **Tier cascade** (`tier_cascade.rs:101-103`): economy_max_complexity=3, standard_max_complexity=7, low_confidence_threshold=0.55.
- **Calibration/bandit** (`calibration.rs:72-74`): min_observations=10, drift_sigma_threshold=2.0.
- **SelectionAxes presets** (`select.rs:287-310`): COST_FIRST 70/15/15, BALANCED 33/33/34, QUALITY_FIRST 15/15/70, FAST 15/70/15.
- **YAML-backed** (`vox_config::load_model_routing_config`): latency_bands, quality_weights, premium_alias map, model pins.
- **Route capability policy env** (`vox-actor-runtime/route_capability_policy.rs`): VOX_ROUTE_POLICY_PROFILE, VOX_ROUTE_ALLOW_NET, VOX_ROUTE_ALLOW_PROVIDER_NETWORK, VOX_ROUTE_ALLOW_LOCAL_MODEL_HTTP.
- **mesh/donation** (`models/scoring.rs`, `registry.rs`): VoxRoutingPreferMesh, VOX_MESH_DONATION_POLICY_{PATH,JSON}, mesh control addr.
