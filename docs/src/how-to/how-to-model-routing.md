---
title: "Model Routing & Provider Cascade"
description: "Official documentation for Model Routing & Provider Cascade for the Vox language. Detailed technical reference, architecture guides, and "
category: "How-To Guides"
last_updated: "2026-04-29"
training_eligible: true

schema_type: "HowTo"
---

<!-- markdownlint-disable MD025 -->

# Model Routing & Provider Cascade

Vox uses a **dynamic OpenRouter catalog** as the primary cloud model source, with **provider policy** enforced in shipped surfaces via in-tree helpers (for example `vox doctor` under `--features codex`) and **MCP / `vox-orchestrator-d`** for full multi-agent routing. The **`vox-orchestrator`** crate is the routing SSOT and ships both the library used by MCP and the **`vox-orchestrator-d`** daemon binary (see [`crates/vox-orchestrator/Cargo.toml`](../../../crates/vox-orchestrator/Cargo.toml)).

Usage statistics and BYOK-style limits are persisted to **Codex** (Turso via `vox-package` / `vox-db`) where wired; legacy docs may say `vox-arca` for the same storage plane.

For full runtime architecture and operational rollout details, also read:

- `crates/vox-cli/src/dei_daemon.rs` — stable RPC **method id** SSOT used by the orchestrator daemon (filename retains historical `dei_` prefix; the daemon binary is `vox-orchestrator-d`)
- `crates/vox-actor-runtime/src/model_resolution.rs` — OpenAI-compatible chat route resolution in the shipped runtime
- `crates/vox-orchestrator/src/runtime.rs` — agent fleet, dispatch, and routing metadata in the live library

## Dynamic Catalog

Catalog refresh and normalization for CLI / MCP paths are owned by the **`vox-orchestrator-d` daemon + MCP stack** together with `vox-actor-runtime` / `vox_config` inference helpers. Conceptually the pipeline is:

1. **Fetches** models from `https://openrouter.ai/api/v1/models` (public fetch; API key optional but recommended for consistent provider policy behavior)
2. **Normalizes** each entry to capability metadata (vision, cost, strengths) in the consumer
3. **Caches** under `~/.vox/cache/` where applicable
4. **Falls back** to cache, then static allowlists where implemented

```text
API (if key) → Cache (if fresh) → Static fallback
```

## Provider Cascade

```text
┌─────────────────────────────────────────────────┐
│              Model Selection (catalog-driven)     │
├─────────────────────────────────────────────────┤
│  Layer 1: Google AI Studio (direct)             │
│  └── google/gemini-* from catalog (auto-selected)│
│                                                  │
│  Layer 2: OpenRouter (requires free API key)     │
│  └── :free models from catalog (Devstral, Qwen…)  │
│                                                  │
│  Layer 3: OpenRouter Paid (premium)              │
│  └── SOTA models from catalog                   │
│                                                  │
│  Layer 0: Ollama (always available, zero-auth)   │
│  └── any locally pulled model                   │
└─────────────────────────────────────────────────┘
```

## How Model Selection Works

### `vox chat` (CLI)

The minimal **`vox`** binary does not ship the historical interactive `vox chat` subtree. Use **Mens / MCP / `vox-orchestrator-d`** for chat-shaped flows, or wire a new chat module deliberately behind an explicit feature. When a chat stack is enabled, the cascade conceptually remains:

1. Refresh or load catalog / model list (daemon or runtime)
2. Check for Google AI Studio key → prefer Gemini-family routes where configured
3. Check for OpenRouter key → respect **`--free` / efficient** vs paid routing in the active implementation
4. Check for Ollama → fall back to local inference (`vox_config::inference::local_ollama_populi_base_url`)
5. No keys → guide the user to free-tier setup

### Mens / Ollama base URL

Local inference uses a single resolution order: **`OLLAMA_URL` → `POPULI_URL` →** default `http://localhost:11434`, exposed as **`vox_config::inference::local_ollama_populi_base_url()`** (SSOT in `crates/vox-config/src/inference.rs`). The Mens client (`vox_actor_runtime::mens::MensConfig::from_env`) uses the same precedence.

### Hugging Face Inference Providers (router)

For OpenAI-compatible chat against the HF **Inference Providers** router, use:

- **URL:** `https://router.huggingface.co/v1/chat/completions` (constant `vox_actor_runtime::inference_env::HF_ROUTER_CHAT_COMPLETIONS_URL`)
- **Token:** `HF_TOKEN` or `HUGGING_FACE_HUB_TOKEN` via **`vox_config::inference::huggingface_hub_token()`**
- **Descriptor:** `vox_actor_runtime::inference_env::resolve_huggingface_router("org/model")` returns model id, URL, and optional bearer token.
- **Dedicated endpoint:** `vox_actor_runtime::inference_env::resolve_huggingface_dedicated("https://….hf.space/v1/chat/completions", "model-id")` for pinned Inference Endpoints (same token env vars).
- **Env shortcut (policy resolver):** `HF_DEDICATED_CHAT_URL` + `HF_DEDICATED_CHAT_MODEL` (see `vox_config::inference::hf_dedicated_chat_completions_url` / `hf_dedicated_chat_model`) are read by [`vox_actor_runtime::model_resolution::RouteResolutionInput::default`] and take precedence over the shared router when an HF token is present.

Manual model pins and task overrides still win over automatic routing (see precedence below).

### Hugging Face Hub catalog (text-generation)

`vox_actor_runtime::inference_env::fetch_hf_hub_text_generation_models(limit)` calls the Hub **`/api/models`** listing (`pipeline_tag=text-generation`, sorted by downloads) and normalizes rows with `parse_hf_hub_models_array`. Use this for adapters and tooling that need a fresh allowlist without hardcoding model ids in business logic.

### Runtime SSOT resolver (OpenAI-compatible chat)

`vox_actor_runtime::model_resolution::resolve_chat_provider_route` applies fixed precedence: **manual** → **Mens (GPU-prefer)** → **HF dedicated** (token + dedicated env) → **HF router** (token + `HF_CHAT_MODEL`) → **OpenRouter** (key) → **any Mens** → **OpenRouter bootstrap** (`OPENROUTER_AUTO`). Map the result with `chat_route_to_llm_config` before `vox_actor_runtime::llm::llm_chat`.

### Unified four-lane backend semantics (orchestrator / MCP / runtime chat)

Registry-backed work (`vox-orchestrator` `ModelSpec` + `route_backend_for_model`) and HTTP chat routing share **four** normalized backend lanes for telemetry and dashboards:

| Lane | Orchestrator (`ModelRouteBackend`) | Runtime chat (`ChatRouteBackend`) | Telemetry `(family, choice)` |
| --- | --- | --- | --- |
| Google direct | `GeminiDirect` | `GeminiDirect` when manual `base_url` contains `generativelanguage.googleapis.com`; registry `ProviderType::GoogleDirect` maps here in MCP | `("google", "direct")` |
| OpenRouter | `OpenRouter` | `OpenRouter` for `ChatProviderRouteKind::OpenRouter` and manual model id without base (OpenRouter id) | `("openrouter", "openrouter")` |
| Local Ollama / Mens | `Ollama` | `Ollama` for `PopuliLocal` | `("mens", "populi_local")` |
| Cascade / other | `CascadeFallback` (and Groq/Mistral/… per `route_backend_for_model` rules) | `CascadeFallback` for HF router/dedicated, BYOK OpenAI-compatible manual URLs (non-Google), and other non-native HTTP lanes | `("custom", "cascade")` |

**SSOT for telemetry strings:** `vox_actor_runtime::model_resolution::backend_telemetry_labels`. MCP `mcp_provider_telemetry_labels` delegates to it so labels cannot drift.

**Residual divergence (by design):**

- **Precedence vs lane:** Runtime chat resolution still prefers HF dedicated/router when an HF token is present (see precedence above); those routes are labeled **cascade** for backend-family purposes, not as separate HF enum variants.
- **Gemini without Generative Language URL:** A pinned Gemini model delivered only through OpenRouter (OpenRouter-shaped URL/model id) is labeled **openrouter**, not **google/direct**, until the chat stack uses a Google direct endpoint URL.
- **Orchestrator `route_backend_for_model` nuance:** Non-OpenRouter third-party `ProviderType`s map to `OpenRouter` vs `CascadeFallback` based on model id heuristics (e.g. `org/model` → OpenRouter lane); runtime chat has no equivalent until a concrete `ChatProviderRouteKind` is built for that call.

Helpers: `route_backend_for_chat_route`, `route_telemetry_labels` (derived from the backend). Structured logs from routers may still use different `tracing` targets; filter `RUST_LOG` by the binary you run.

### Mens capability probe (GPU / health)

`vox_actor_runtime::inference_env::probe_populi_capabilities(base_url)` (and `PopuliClient::probe_capabilities`) call Ollama-compatible **`/api/tags`** and **`/api/version`**. `gpu_capable` is `Some(true)` only when version JSON (string match) suggests CUDA, ROCm, or Metal; otherwise `None` if unknown.

### Multi-agent registry (orchestrator daemon)

Full **multi-agent model registry** behavior (task categories, complexity bands, economy vs performance, research stage picks) lives in the **`vox-orchestrator-d`** / MCP plane. The in-tree **`vox-orchestrator`** crate handles affinity, routing metadata, registry lookup, and session layout for MCP and the `vox live` demo bus.

### Task inference (precedence)

For orchestrator-attached tasks, treat precedence as **task override → per-agent config → mode profile / env / `Vox.toml` → MCP model override**, matching the semantics documented for MCP `vox_submit_task` / `vox_set_model_override`.

### MCP chat / inline / ghost override

Tools `vox_set_active_model` and `vox_get_active_model` pin the model used by `vox_chat_message`, `vox_inline_edit`, and `vox_ghost_text` to a **registry** id (must exist in `vox_list_models`). Pass an **empty** `model_id` to `vox_set_active_model` to clear the override and restore automatic `best_for_config` resolution (same path as chat when no override is set).

### Route telemetry

Structured logs for route telemetry are emitted from the **daemon / MCP** implementation; use `RUST_LOG` filters documented for the binary you run (`vox-mcp`, `vox-orchestrator-d`, etc.).

```text
# Pseudocode shape (concrete types live in the orchestrator daemon and MCP)
registry.resolve_for_task(task_category, complexity, cost_preference, inference_config)
```

## Escalation Chain

If a model fails (rate limit, error), chat-shaped surfaces **escalate** using catalog-driven fallback lists in the orchestrator routing layer. The chain is **catalog-driven**, not a hardcoded short list in `vox-cli`:

| Provider | Source |
| --- | --- |
| Google | `google/gemini-*` models from catalog, ordered by capability |
| OpenRouter | Free codegen models from catalog |
| Ollama | Local model (e.g. llama3.2) |

## Catalog Refresh

Force-refresh the OpenRouter catalog (e.g. after new models are added):

```bash
vox status --refresh-catalog   # Refresh before showing provider status
```

The orchestrator-side registry also performs periodic refresh merges using:

- `VOX_OPENROUTER_CATALOG_MIN_REFRESH_INTERVAL_SECS`
- `VOX_OPENROUTER_CATALOG_REFRESH_JITTER_MS`

with a refresh marker in the Vox config directory to avoid excessive fetch churn.

## Key Management

Keys are managed via the unified `vox auth` system:

```bash
vox auth login --registry google YOUR_KEY      # Google AI Studio
vox auth login --registry openrouter YOUR_KEY  # OpenRouter

# Keys stored in ~/.vox/auth.json
# Also reads from env vars: GEMINI_API_KEY, OPENROUTER_API_KEY
```

## Cost Tracking

When using paid models, Vox tracks costs in **Codex**. You can check your current usage and estimated costs for the day:

Quota rollups that depended on the excluded in-tree DeI crate are **not** shipped in the default `vox` binary; inspect provider dashboards or Codex tables directly until a daemon-backed quota API is wired.

Cost data may still be persisted as provider-specific usage rows in Codex (Arca schema on Turso) where integrations exist.

## Repository Context Controls (Rollout)

Add these keys under `[dei]` in `Vox.toml` for repo-aware chat/index/A2A behavior.
(Legacy: `[orchestrator]` is also supported for backward compatibility.)

```toml
[dei]
context_window_soft_ratio = 0.80
context_window_hard_ratio = 0.95
repo_index_max_files = 12000
repo_index_max_file_bytes = 262144
provider_tool_calls_enabled = true
provider_tool_calls_max_per_turn = 5
provider_tool_calls_read_only_mode = false
repo_index_incremental = false   # set true for monorepos (vox repo enables it)
context_window_chars_per_token = 4
a2a_context_packet_enabled = true
```

Equivalent environment variables (prefer `vox_orchestrator_*`; `VOX_DEUS_*` and `VOX_ORCHESTRATOR_*` are legacy):

- `vox_orchestrator_CONTEXT_WINDOW_SOFT_RATIO`
- `vox_orchestrator_CONTEXT_WINDOW_HARD_RATIO`
- `vox_orchestrator_REPO_INDEX_MAX_FILES`
- `vox_orchestrator_REPO_INDEX_MAX_FILE_BYTES`
- `vox_orchestrator_PROVIDER_TOOL_CALLS_ENABLED`
- `vox_orchestrator_PROVIDER_TOOL_CALLS_MAX_PER_TURN`
- `vox_orchestrator_PROVIDER_TOOL_CALLS_READ_ONLY_MODE`
- `vox_orchestrator_A2A_CONTEXT_PACKET_ENABLED`

Operational MCP tools for rollout verification:

- `vox_repo_index_status` / `vox_repo_index_refresh`
- `vox_context_sources`
- `vox_context_budget_snapshot` / `vox_compaction_history`

## Migration and environment compatibility

| Concern | Guidance |
| --- | --- |
| **Agent `model:`** | Optional in `.vox/agents/*.md`. Use a catalog id (`openrouter/...`, `google/gemini-...`). MCP task submit refreshes inference from the file each time so you do not need to respawn agents after edits. |
| **Efficient / free-only** | `vox_orchestrator_MODE_PROFILE=efficient` or MCP `mode_profile: efficient` keeps `free_only` routing; OpenRouter defaults stay on free/auto when the usage tracker runs with `free_only`. |
| **Local Ollama URL** | `vox_config::inference::local_ollama_populi_base_url()` — `OLLAMA_URL` → `POPULI_URL` → `http://localhost:11434`. |
| **OpenRouter key** | `vox_config::inference::openrouter_api_key()` (env `OPENROUTER_API_KEY`). |
| **Hugging Face token** | `vox_config::inference::huggingface_hub_token()` (`HF_TOKEN` / `HUGGING_FACE_HUB_TOKEN`). |
| **Research stage models** | Defaults come from `ModelRegistry::best_for_config` per stage (`research::model_select::resolve_research_models`). Last-resort string fallbacks exist only if the registry returns no candidate. |


