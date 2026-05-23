---
title: "Secrets SSOT"
description: "Canonical secret-management source of truth for Vox Secrets"
category: "Language Reference"
last_updated: "2026-05-08"
training_eligible: true

schema_type: "TechArticle"
---

## Secrets SSOT

`vox-secrets` is the canonical source of truth for managed secret metadata and resolution precedence.

Research and forward-looking analysis live in [Secrets env vars, and API key strategy research 2026](../archive/research-2026-q1/clavis-secrets-env-research-2026.md).
Threat and policy controls are documented in [Secrets Cloudless Threat Model V1](../archive/research-2026-q1/clavis-cloudless-threat-model-v1.md), with execution steps in [Secrets Cloudless Implementation Catalog](../archive/research-2026-q1/clavis-cloudless-implementation-catalog.md).

## Naming Convention

- `VOX_*`: Vox-owned platform contracts (mesh, runtime auth, DB, cloud orchestration, internal boundaries).

## Non-secret environment parsing

Use **`vox_config::env_parse`** for numeric defaults and operator tuning (e.g. HTTP retry caps, timeouts expressed as plain integers). Do **not** route API keys or other credentials through those helpers — use **`vox_secrets::resolve_secret`** (and the `SecretId` inventory below) so precedence and aliases stay consistent.

**`vox-gamify` free-tier AI:** when `FreeAiProvider::{Gemini,OpenRouter}` carries an empty `api_key`, resolution goes through Secrets (`GeminiApiKey`, `OpenRouterApiKey`) — same canonical + compat env names as the rest of the repo; do not read `GEMINI_API_KEY` / `OPENROUTER_API_KEY` directly in new Gamify codepaths.

- Provider-native names (for example `OPENROUTER_API_KEY`, `OPENAI_API_KEY`): upstream ecosystem names kept for compatibility.
- Optional `VOX_*` provider aliases are accepted as migration aids; canonical names remain stable.

## Secret Inventory (Phase 0)

| Secret | Scope | Tier | Primary consumer surfaces |
| --- | --- | --- | --- |
| `OPENROUTER_API_KEY` / `GEMINI_API_KEY` / `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` | LLM inference | Minimal cloud LLM | `vox-mcp`, `vox-actor-runtime`, `vox-cli doctor/status` |
| `HF_TOKEN` | LLM retrieval / HF router | Optional | `vox-config`, HF routes |
| `GROQ_API_KEY`, `CEREBRAS_API_KEY`, `MISTRAL_API_KEY`, `DEEPSEEK_API_KEY`, `SAMBANOVA_API_KEY`, `CUSTOM_OPENAI_API_KEY` | Alternative LLM providers | Optional power-user | provider-specific runtime/mcp paths |
| `VOX_RUNPOD_API_KEY`, `VOX_VAST_API_KEY` | Cloud GPU infra | Optional cloud GPU | `vox-populi` cloud providers |
| `TOGETHER_API_KEY` | Remote fine-tune API | Optional cloud training | `vox-cli train --provider together` |
| `GITHUB_TOKEN` | Publishing/review automation | Workflow-specific required | `vox-cli review/publish` |
| `VOX_NEWS_TWITTER_TOKEN`, `VOX_NEWS_OPENCOLLECTIVE_TOKEN`, `VOX_SOCIAL_REDDIT_*`, `VOX_SOCIAL_YOUTUBE_*` | Scientia/news syndication | Optional (per channel) | `vox-publisher` resolves via Secrets `SecretId` specs; GitHub syndication also accepts `VOX_NEWS_GITHUB_TOKEN` as an alias of `GITHUB_TOKEN` |
| `ZENODO_ACCESS_TOKEN`, `OPENREVIEW_EMAIL`, `OPENREVIEW_ACCESS_TOKEN`, `OPENREVIEW_PASSWORD`, `CROSSREF_PLUS_API_KEY`, `DATACITE_REPOSITORY`, `DATACITE_PASSWORD`, `ORCID_CLIENT_ID`, `ORCID_CLIENT_SECRET`, `TAVILY_API_KEY`, `TAVILY_PROJECT`, `X_TAVILY_API_KEY`, `VOX_ARXIV_ASSIST_HANDOFF_SECRET` (plus `VOX_*` aliases for DataCite, ORCID, Tavily where listed below) | Scholarly repository adapters | Optional (`Workflow::Publish` / `publish_review` bundle) | Zenodo / OpenReview / Crossref / DataCite / ORCID / Tavily clients resolve via Secrets; VOX-prefixed aliases accepted where listed |
| `VOX_DB_URL`, `VOX_DB_TOKEN` | Remote DB | Workflow-specific required | DB remote flows |
| `VOX_TELEMETRY_UPLOAD_URL`, `VOX_TELEMETRY_UPLOAD_TOKEN` | Optional telemetry ingest (explicit `vox telemetry upload`) | Optional | `vox-cli` resolves via `SecretId::VoxTelemetryUploadUrl` / `VoxTelemetryUploadToken`; see [ADR 023](../adr/023-optional-telemetry-remote-upload.md) |
| `VOX_SEARCH_QDRANT_API_KEY` | Qdrant HTTP `api-key` (optional RAG sidecar) | Optional | [`vox_search::vector_qdrant`](../../../crates/vox-search/src/vector_qdrant.rs) via `SecretId::VoxSearchQdrantApiKey` |
| `VOX_MESH_TOKEN` | Populi control-plane auth (legacy full-access token) | Workflow-specific required (any mesh-class token) | Mesh transport/auth |
| `VOX_MESH_WORKER_TOKEN` | Worker-scoped populi HTTP bearer | Optional (advance pools) | `POST` join/heartbeat/inbox/ack |
| `VOX_MESH_SUBMITTER_TOKEN` | Submitter-scoped populi HTTP bearer | Optional | `POST` A2A deliver only |
| `VOX_MESH_ADMIN_TOKEN` | Mesh admin bearer | Optional | Full HTTP surface when configured |
| `VOX_MESH_JWT_HMAC_SECRET` | HS256 key for mesh JWT bearer | Optional | JWT claims `role`, `jti`, `exp` |
| `VOX_MESH_WORKER_RESULT_VERIFY_KEY` | Ed25519 verify key (hex or Standard base64) | Optional | Signed `job_result` / `job_fail` payloads |
| `VOX_API_KEY`, `VOX_BEARER_TOKEN` | Runtime ingress auth | Optional hardening | `vox-actor-runtime` auth gate |
| `VOX_MCP_HTTP_BEARER_TOKEN`, `VOX_MCP_HTTP_READ_BEARER_TOKEN` | MCP HTTP gateway auth | Optional hardening | `vox-mcp` HTTP gateway auth surfaces |
| `V0_API_KEY`, `VOX_OPENCLAW_TOKEN` | Auxiliary tooling | Optional | island generation / OpenClaw |
| `*_TUNING_TEMPERATURE`, `*_TUNING_TOP_P` | LLM inference overrides (Gemini, Ollama, OpenAI, Anthropic, Together) | Optional | Resolution precedence: Request > Env > Tool Default |

## Managed Secret Env Names

{{#include ../../../contracts/secrets/managed-env-names.md}}

## Resolution Precedence

For each managed secret ID:

1. canonical env name
2. non-deprecated aliases (including opt-in `VOX_*` aliases)
3. deprecated aliases (returns `DeprecatedAliasUsed` status)
4. configured external backend (`infisical` or `vault`, when enabled)
5. secure local store
6. compatibility file stores (`~/.vox/auth.json`, legacy `~/.vox/auth_token`, `.vox/populi/mesh.env` where applicable)

## Required vs Optional Model

- `vox secrets doctor` evaluates **blocking requirement groups** (`AnyOf`/`AllOf`) per workflow/profile.
- `Chat`/`Mcp` blocking model in cloud mode is **OpenRouter-first** (`OPENROUTER_API_KEY` / `VOX_OPENROUTER_API_KEY`); alternate providers are optional capability keys.
- `local` mode requires no cloud key; `auto` resolves from `VOX_INFERENCE_PROFILE`.
- Optional keys are reported separately as capability unlocks (not startup blockers).
- OpenRouter does not replace RunPod/Vast keys: LLM gateway credentials and cloud GPU credentials are distinct domains.

## Canonical Bundles

- `minimal_local_dev`: zero required cloud keys.
- `minimal_cloud_dev`: OpenRouter only.
- `gpu_cloud`: RunPod or Vast key (plus Together optional).
- `publish_review`: GitHub token required; Zenodo / OpenReview / Crossref / arXiv-assist secrets optional (see inventory table).
- `mesh_roles`: worker or submitter mesh token (see `SecretBundle::MeshRoles` / SSOT mesh section).

## Transition and Deprecation Window Policy

1. Add alias support first (no breakage).
2. Emit `DeprecatedAliasUsed` in doctor for legacy aliases.
3. Keep legacy aliases for at least two release trains after warning lands.
4. Remove legacy aliases from docs examples first; remove runtime support only after explicit release note and CI parity update.

## Command Surfaces

- `vox secrets doctor --workflow <...> --profile <dev|ci|mobile|prod> --mode <auto|local|cloud> [--bundle <minimal-local-dev|minimal-cloud-dev|gpu-cloud|publish-review>]`
- `vox secrets set <registry> <token> [--username <name>]`
- `vox secrets get <registry>`
- `vox secrets backend-status`
- `vox secrets migrate-auth-store`
- FORGE_TOKEN
- GH_TOKEN
- GITLAB_TOKEN
- GL_TOKEN
- GOOGLE_AI_STUDIO_KEY
- HUGGING_FACE_HUB_TOKEN
- POPULI_API_KEY
- TURSO_AUTH_TOKEN
- TURSO_URL
- VOX_ANTHROPIC_API_KEY
- VOX_CEREBRAS_API_KEY
- VOX_CROSSREF_PLUS_API_KEY
- VOX_CUSTOM_OPENAI_API_KEY
- VOX_DEEPSEEK_API_KEY
- VOX_FORGE_TOKEN
- VOX_GEMINI_API_KEY
- VOX_GROQ_API_KEY
- VOX_HF_TOKEN
- VOX_MISTRAL_API_KEY
- VOX_OPENAI_API_KEY
- VOX_OPENREVIEW_EMAIL
- VOX_OPENREVIEW_PASSWORD
- VOX_POPULI_API_KEY
- VOX_SAMBANOVA_API_KEY
- VOX_SOCIAL_REDDIT_CLIENT_ID
- VOX_SOCIAL_REDDIT_CLIENT_SECRET
- VOX_SOCIAL_REDDIT_REFRESH_TOKEN
- VOX_SOCIAL_REDDIT_USER_AGENT
- VOX_SOCIAL_YOUTUBE_CLIENT_ID
- VOX_SOCIAL_YOUTUBE_CLIENT_SECRET
- VOX_SOCIAL_YOUTUBE_REFRESH_TOKEN
- VOX_TOGETHER_API_KEY
- VOX_TURSO_TOKEN
- VOX_TURSO_URL
- VOX_V0_API_KEY
- VOX_WEBHOOK_INGRESS_TOKEN
- VOX_WEBHOOK_SIGNING_SECRET
- VOX_ZENODO_ACCESS_TOKEN
- VOX_SOCIAL_MASTODON_TOKEN
- VOX_SOCIAL_MASTODON_DOMAIN
- VOX_SOCIAL_LINKEDIN_ACCESS_TOKEN
- VOX_SOCIAL_DISCORD_WEBHOOK_URL
- GEMINI_TUNING_TEMPERATURE
- GEMINI_TUNING_TOP_P
- OLLAMA_TUNING_TEMPERATURE
- OLLAMA_TUNING_TOP_P
- OPENAI_TUNING_TEMPERATURE
- OPENAI_TUNING_TOP_P
- ANTHROPIC_TUNING_TEMPERATURE
- ANTHROPIC_TUNING_TOP_P
- TOGETHER_TUNING_TEMPERATURE
- TOGETHER_TUNING_TOP_P

