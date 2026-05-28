---
title: "Clavis as a one-stop secrets manager: research findings 2026"
description: "Comprehensive research synthesis on secret sprawl, env-var taxonomy, user-facing feature requirements, AI-agent credential flows, A2A delegation, and the roadmap for evolving Vox Clavis into a full-lifecycle secrets management platform."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Synthesizes architecture constraints, security research, and feature gaps for Clavis implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Clavis as a one-stop secrets manager: research findings 2026

> **Companion documents**
>
> - [Clavis secrets, env vars, and API key strategy research 2026](clavis-secrets-env-research-2026.md) — the original SSOT research dossier; this document extends and completes it.
> - [Clavis Cloudless Threat Model V1](clavis-cloudless-threat-model-v1.md) — threat actor matrix, allowed source policy, break-glass governance.
> - [Clavis Cloudless Implementation Catalog](clavis-cloudless-implementation-catalog.md) — ordered implementation tasks.
> - [Clavis SSOT reference](../reference/clavis-ssot.md) — canonical secret inventory and resolution precedence.

This document is a research dossier focused on the **product-level and architectural gaps** between Vox Clavis today and the feature surface needed for a world-class, AI-era secrets management platform. It departs from the base research doc by adding extensive field evidence, an env-var taxonomy, user-facing feature requirements derived from the open-source and commercial ecosystem, MCP/A2A credential delegation patterns, and a structured feature roadmap.

---

## 1. The scale of the problem: industry evidence

The following statistics ground the urgency of this research in concrete, current data.

### Secret sprawl metrics (2024–2025, GitGuardian State of Secrets Sprawl)

- **23.8 million** new hardcoded secrets detected in **public** GitHub repositories in 2024 — a **25% year-over-year increase**.
- **4.6%** of all public repositories contain at least one secret; **35%** of private repositories do.
- **70%** of secrets leaked in 2022 remained active (unrevoked) in 2024.
- AI coding assistants (Copilot, etc.) correlate with **40% higher** secret leakage rates in public repositories.
- **15% of commit authors** leaked at least one secret.
- Container images: 100,000 valid secrets found in 15 million public Docker images; **65%** of these from `ENV` instructions.
- Generic secrets (hardcoded passwords, custom keys without standard patterns) account for **58%** of all leaks — the category hardest to detect with pattern-based scanners.

### What this means for Vox Clavis

Vox's own workspace already has 100+ environment variable names managed or audited through Clavis. The workspace-wide secret-env-guard CI policy is a leading-edge control — but the evidence shows that scanning *alone* is insufficient. Active lifecycle management (rotation, expiry tracking, metadata tagging, and agent-boundary controls) is necessary to close the remaining risk surface.

archived_date: 2026-04-18
---

## 2. Taxonomy of Vox environment variables

The current Clavis inventory spans multiple semantic classes that should be governed differently. This taxonomy maps each class to recommended lifecycle controls.

### Class 1: Platform identity and bootstrap secrets

| Canonical form | Description |
| --- | --- |
| `VOX_DB_URL`, `VOX_DB_TOKEN` | Remote database credentials |
| `VOX_CLAVIS_VAULT_URL`, `VOX_CLAVIS_VAULT_TOKEN`, `VOX_CLAVIS_VAULT_PATH` | Vault backend bootstrap |
| `INFISICAL_TOKEN`, `INFISICAL_SERVICE_TOKEN`, `VAULT_ADDR`, `VAULT_TOKEN` | External vault access |
| `VOX_CLAVIS_KEK_REF`, `VOX_CLAVIS_KEK_VERSION` | Key encryption key references |
| `VOX_ACCOUNT_ID`, `VOX_CLAVIS_PROFILE`, `VOX_CLAVIS_BACKEND` | Resolver and profile selectors |

**Lifecycle controls required:** Immediate rotation on any suspected compromise. Short TTL where dynamic issuance is available. Stored only in keyring or vault, not in env for strict profiles. Break-glass procedure enforced.

### Class 2: LLM provider API keys (BYOK model)

| Canonical form | Provider |
| --- | --- |
| `OPENROUTER_API_KEY` / `VOX_OPENROUTER_API_KEY` | OpenRouter (primary gateway) |
| `OPENAI_API_KEY` / `VOX_OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` / `VOX_ANTHROPIC_API_KEY` | Anthropic Claude |
| `GEMINI_API_KEY` / `VOX_GEMINI_API_KEY` | Google Gemini |
| `GROQ_API_KEY` / `VOX_GROQ_API_KEY` | Groq |
| `CEREBRAS_API_KEY` / `VOX_CEREBRAS_API_KEY` | Cerebras |
| `MISTRAL_API_KEY` / `VOX_MISTRAL_API_KEY` | Mistral |
| `DEEPSEEK_API_KEY` / `VOX_DEEPSEEK_API_KEY` | DeepSeek |
| `SAMBANOVA_API_KEY` / `VOX_SAMBANOVA_API_KEY` | SambaNova |
| `CUSTOM_OPENAI_API_KEY` / `VOX_CUSTOM_OPENAI_API_KEY` | Custom OpenAI-compatible endpoint |
| `HF_TOKEN` / `VOX_HF_TOKEN` | Hugging Face Hub |

**Lifecycle controls required:** These are the most impactful vector for AI-era leakage — an agent accessing model context leaks these first. Provider-side: scoped to minimum required capabilities (read vs. read-write, project scoping). Consumer-side: resolved to `secrecy::SecretString`, never logged, and instrumented for usage alerting. Rotation cadence: 90 days or immediately on leakage detection. OpenRouter as primary gateway reduces the number of provider keys that must be present at runtime.

### Class 3: Cloud GPU and training infrastructure

| Canonical form | Provider |
| --- | --- |
| `VOX_RUNPOD_API_KEY` | RunPod |
| `VOX_VAST_API_KEY` | Vast.ai |
| `TOGETHER_API_KEY` / `VOX_TOGETHER_API_KEY` | Together AI |

**Lifecycle controls required:** These are high-blast-radius credentials (unlimited compute spend potential). Scope restrictions at provider level (project/budget limits) are essential. Rotation cadence: 60 days maximum.

### Class 4: Publication and scholarly adapter credentials

| Canonical form | Service |
| --- | --- |
| `GITHUB_TOKEN` / `VOX_FORGE_TOKEN` | GitHub/Forge publishing |
| `ZENODO_ACCESS_TOKEN` / `VOX_ZENODO_ACCESS_TOKEN` | Zenodo scholarly publishing |
| `OPENREVIEW_EMAIL`, `OPENREVIEW_ACCESS_TOKEN`, `OPENREVIEW_PASSWORD` | OpenReview |
| `CROSSREF_PLUS_API_KEY` / `VOX_CROSSREF_PLUS_API_KEY` | Crossref reference API |
| `DATACITE_REPOSITORY` / `DATACITE_PASSWORD` | DataCite |
| `ORCID_CLIENT_ID` / `ORCID_CLIENT_SECRET` | ORCID OAuth |
| `TAVILY_API_KEY` / `X_TAVILY_API_KEY` / `VOX_TAVILY_API_KEY` | Tavily search |
| `VOX_ARXIV_ASSIST_HANDOFF_SECRET` | arXiv assist handoff token |

**Lifecycle controls required:** Platform-specific OAuth scoping where available (ORCID, GitHub). Expiry alerting critical — many of these expire on provider-defined schedules without notification. Password-based credentials (OpenReview) are the weakest link; prefer token alternatives.

### Class 5: Social and syndication credentials

| Canonical form | Platform |
| --- | --- |
| `VOX_NEWS_TWITTER_TOKEN`, `VOX_NEWS_OPENCOLLECTIVE_TOKEN` | Twitter/X, OpenCollective |
| `VOX_SOCIAL_REDDIT_CLIENT_ID`, `VOX_SOCIAL_REDDIT_CLIENT_SECRET`, `VOX_SOCIAL_REDDIT_REFRESH_TOKEN` | Reddit OAuth2 |
| `VOX_SOCIAL_YOUTUBE_CLIENT_ID`, `VOX_SOCIAL_YOUTUBE_CLIENT_SECRET`, `VOX_SOCIAL_YOUTUBE_REFRESH_TOKEN` | YouTube OAuth2 |
| `VOX_SOCIAL_MASTODON_TOKEN`, `VOX_SOCIAL_MASTODON_DOMAIN` | Mastodon |
| `VOX_SOCIAL_LINKEDIN_ACCESS_TOKEN` | LinkedIn |
| `VOX_SOCIAL_DISCORD_WEBHOOK_URL` | Discord webhook |

**Lifecycle controls required:** OAuth refresh token rotation should be tracked in Clavis metadata. Platform access tokens expire; expiry state should be observable via `vox clavis doctor`. Discord webhook URL is an indirect credential (bearer URL) and must not appear in logs.

### Class 6: Platform service mesh and transport tokens

| Canonical form | Usage |
| --- | --- |
| `VOX_MESH_TOKEN` | Mesh control-plane (full access) |
| `VOX_MESH_WORKER_TOKEN` | Worker-scoped mesh bearer |
| `VOX_MESH_SUBMITTER_TOKEN` | Submitter-scoped bearer |
| `VOX_MESH_ADMIN_TOKEN` | Admin bearer |
| `VOX_MESH_JWT_HMAC_SECRET` | HS256 JWT signing key |
| `VOX_MESH_WORKER_RESULT_VERIFY_KEY` | Ed25519 result verification key |
| `VOX_MESH_BOOTSTRAP_TOKEN` | Bootstrap token (one-time) |
| `VOX_API_KEY`, `VOX_BEARER_TOKEN` | Runtime ingress auth |
| `VOX_MCP_HTTP_BEARER_TOKEN`, `VOX_MCP_HTTP_READ_BEARER_TOKEN` | MCP HTTP gateway auth |

**Lifecycle controls required:** These are `transport` class secrets — the highest-risk category for lateral movement. JWT HMAC secrets and Ed25519 keys require short rotation schedules. Bootstrap tokens must be invalidated immediately after use. No raw value should ever appear in logs or diagnostic output.

### Class 7: Telemetry and search infrastructure

| Canonical form | Usage |
| --- | --- |
| `VOX_TELEMETRY_UPLOAD_URL`, `VOX_TELEMETRY_UPLOAD_TOKEN` | Optional telemetry sink |
| `VOX_SEARCH_QDRANT_API_KEY` | Qdrant vector store API key |

**Lifecycle controls required:** Optional keys; disable-by-default in strict profiles. Telemetry upload token must not appear in telemetry payloads (circular leakage risk).

### Class 8: Auxiliary and tooling secrets

| Canonical form | Usage |
| --- | --- |
| `V0_API_KEY` / `VOX_V0_API_KEY` | v0.dev island generation |
| `VOX_OPENCLAW_TOKEN` | OpenClaw tool access |
| `VOX_WEBHOOK_INGRESS_TOKEN`, `VOX_WEBHOOK_SIGNING_SECRET` | Webhook signing/auth |
| `OPENROUTER_MODEL`, `OPENAI_MODEL`, `OPENAI_BASE_URL`, `GEMINI_MODEL`, `OLLAMA_URL`, `OLLAMA_MODEL` | Provider configuration (non-secret but Clavis-managed) |

**Lifecycle controls required:** Webhook signing secrets require the dual-key overlap rotation pattern (old+new simultaneously valid during rotation window). Model selection env vars are non-secret configuration; stored in `OPERATOR_TUNING_ENVS` but not in secret stores.

### Class 9: CI and guard configuration (operator tuning, not secrets)

These are operational levers in `OPERATOR_TUNING_ENVS`, not credentials. They belong in documentation and configuration management — not in secret stores. Examples: `VOX_CLAVIS_CUTOVER_PHASE`, `VOX_SECRET_GUARD_GIT_REF`, `VOX_BUILD_TIMINGS_BUDGET_WARN`, `SKIP_CUDA_FEATURE_CHECK`.

**Key insight:** A significant source of confusion in the codebase is that operator tuning env vars and actual secrets coexist in `OPERATOR_TUNING_ENVS`. The classes above clarify which should flow through `resolve_secret` versus `vox_config::env_parse`.

---

## 3. What users and teams need: feature requirements analysis

Based on synthesis of the commercial secrets management landscape (Doppler, Infisical, 1Password Secrets Automation, Pulumi ESC, HashiCorp Vault) and the OWASP Secrets Management Cheat Sheet, the following feature categories define a complete secrets management platform. Each section maps to Clavis's current state.

### 3.1 Centralization and single registry

**Industry standard:** All secrets flow through one control plane. Metadata (name, class, purpose, owner, scope, rotation cadence) is co-located with the secret value reference.

**Vox Clavis today:** `spec.rs` provides centralized metadata. Resolution precedence is deterministic. CI enforces against direct env reads. **Gap:** `vox-db::secrets` operates as a partial parallel surface. The `OPERATOR_TUNING_ENVS` list conflates configuration with secrets.

**Feature requirement:** A canonical secret-vs-config split, enforced in CI and documented explicitly. All product secrets — and only product secrets — flow through `resolve_secret`.

### 3.2 Secret lifecycle metadata

**Industry standard:** Every secret has: creation time, last-rotated time, expiry target, owner (human or system), scope (environment, profile, service), sensitivity class, and rotation cadence. Platforms like TokenTimer and Infisical's lifecycle model expose this metadata via API and CLI.

**Vox Clavis today:** `SecretSpec` contains `rotation_policy: RotationPolicy` and `class: SecretClass` but no runtime tracking of actual rotation timestamps or operational metadata.

**Feature requirement:**
- Extend `SecretSpec` with `rotation_schedule` (optional cron-like cadence), `last_rotated_hint` (operator-supplied metadata, not stored value), and `expiry_warning_days`.
- Expose metadata via `vox clavis doctor --show-metadata` and a forthcoming structured JSON output.
- Track `ResolutionStatus::DeprecatedAliasUsed` already; add `ResolutionStatus::NearingExpiry` and `ResolutionStatus::StaleRotation`.

### 3.3 Import wizard and migration tooling

**Industry standard:** Both Doppler and Infisical provide CLI-driven import flows. Modern flows: detect `.env` files or shell environment dumps, validate format, classify by pattern matching, preview import plan, then apply with optional dry-run.

**Vox Clavis today:** `vox clavis import-env` exists (based on conversation history). **Gap:** dry-run support, structured preview output, and conflict detection for existing secrets are not confirmed complete.

**Feature requirement:**
- `vox clavis import-env --dry-run` must produce a structured diff of what would be imported without modifying any state.
- Detect known env var patterns (LLM API keys, OAuth tokens, known service credentials) and pre-classify before prompting.
- Warn on non-canonical naming (e.g., `GEMINI_KEY` vs. `GEMINI_API_KEY`) and suggest canonical form.
- Detect secrets already present in the keyring or vault before overwriting.

### 3.4 Audit logging and observability

**Industry standard:** Doppler and Infisical log every read and write with timestamp, identity, source, and resolution path. This is table-stakes for SOC 2 and HIPAA compliance. The log must be tamper-evident.

**Vox Clavis today:** No structured audit log exists. `tracing` events fire for doctor/status but there is no persistent audit trail.

**Feature requirement:**
- Structured audit log for `resolve_secret` calls in non-dev profiles. Minimum fields: `timestamp_utc`, `secret_id`, `resolution_status`, `source`, `profile`, `caller_crate` (derived from compile-time location).
- Logs must be written to an append-only structured sink (JSON file or VoxDB append-only table) when enabled.
- `vox clavis audit-log [--since <time>] [--secret <id>]` CLI surface for inspection.
- Logs must never contain resolved secret values — only resolution metadata.

### 3.5 Secret health dashboard (`vox clavis doctor` evolution)

**Industry standard:** "Secret health" visible in CLI. Infisical and Doppler both provide health overviews: missing required secrets, secrets nearing expiry, rotation overdue alerts, and integration-level status checks (can we actually authenticate with this token?).

**Vox Clavis today:** `vox clavis doctor` evaluates blocking requirement groups. **Gap:** no expiry-aware status, no rotation overdue detection, no per-class health view, no integration probe (i.e., does the resolved `OPENROUTER_API_KEY` actually work?).

**Feature requirement:**
- `vox clavis doctor --health` → structured health report per secret class:
  - `present` / `missing` / `stale-rotation` / `nearing-expiry` / `deprecated-alias`
  - For optional secrets: `unlocked` (present, enables capability) vs. `locked` (absent, capability unavailable)
- Optional integration probe: `vox clavis probe --secret OPENROUTER_API_KEY` → HTTP handshake to verify the key is still valid (opt-in only, requires explicit consent, network probe).
- Expiry warning threshold configurable per secret class (default 14 days for OAuth tokens, 30 days for API keys).

### 3.6 Secret rotation support

**Industry standard:** Rotation is the most-requested feature by security teams. Zero-downtime rotation requires supporting dual-key validity during the transition window. Infisical uses a rolling lifecycle model (active → inactive → revoked). Doppler supports both API-based and agent-proxied rotation.

**Vox Clavis today:** No rotation orchestration. `vox clavis set` supports manual value update; backend stores new value but old value is not tracked.

**Feature requirement (phased):**

**Phase 1 — Rotation awareness (metadata only):**
- `SecretSpec` gains `rotation_policy: RotationPolicy` fields for: `scheduled_days` (rotation cadence), `dual_validity_window_mins` (overlap period).
- `vox clavis rotate <secret_id> --new-value <val>` command that atomically updates value and records `last_rotated_hint` timestamp.
- Doctor shows stale rotation warnings.

**Phase 2 — Webhook-triggered rotation:**
- Provider-specific rotation hooks registered in Clavis (e.g., "when GitHub PAT expires, alert and guide user to recreate").
- `vox clavis rotation-status` → human-readable rotation calendar.

**Phase 3 — Programmatic rotation (future):**
- Provider APIs that support programmatic rotation (RunPod, Vast.ai) could be wired to `vox clavis rotate --auto <provider>`.
- GitHub: transition recommendations to GitHub Apps (which generate short-lived installation tokens programmatically) rather than PATs.

### 3.7 Version history and rollback

**Industry standard:** Infisical supports point-in-time recovery. Doppler keeps version history with diff views. Both enable rollback to previous values on rotation failure.

**Vox Clavis today:** No version history. Keyring overwrites previous value silently.

**Feature requirement:**
- VoxDB-backed vault: store encrypted value history with `version_index` and `created_at`. Maximum history depth: configurable, default 5 versions.
- `vox clavis history <secret_id>` → show creation timestamp per version (no values exposed).
- `vox clavis rollback <secret_id> --to-version <n>` → restore a previous version.
- Rollback must require reason code and produce an audit log entry.

### 3.8 Environment and profile namespacing

**Industry standard:** Doppler and Infisical organize secrets by `workspace → project → environment`. This allows the same logical secret name to hold different values in `dev`, `staging`, and `prod`, with promotion workflows.

**Vox Clavis today:** `ResolveProfile` (DevLenient, CiStrict, ProdStrict, HardCutStrict) provides profile-aware resolution semantics. **Gap:** no per-profile overrides for secret values; a secret has one value regardless of profile.

**Feature requirement:**
- Profile-scoped value overrides: `vox clavis set <id> --profile ci --value <val>` stores a profile-specific override.
- `resolve_secret(id)` checks for profile-specific override before falling back to global value.
- Prevents manual `.env` file management per environment.

### 3.9 Status sync and drift detection

**Industry standard:** Configuration drift between environments is a leading cause of outages. Doppler highlights when secrets differ between environments. Pulumi ESC uses environment imports for composable, DRY configuration.

**Vox Clavis today:** `clavis-parity` CI guard catches docs drift against the managed-env-names manifest. **Gap:** no cross-environment drift detection; no parity check between local keyring and expected CI values.

**Feature requirement:**
- `vox clavis diff --env-file .env` → compare a local `.env` file against the Clavis-expected managed set. Output: missing from Clavis, present in file but unmanaged, canonical name mismatches.
- CI: extend `clavis-parity` to validate that all managed secrets are resolvable (at least via env) in CI context.

archived_date: 2026-04-18
---

## 4. AI-era and agent-specific requirements

This section covers the uniquely new requirements posed by AI agent workflows. These are not adequately addressed by any existing Clavis documentation.

### 4.1 The OWASP NHI Top 10 (2025): Clavis alignment

The OWASP Non-Human Identities Top 10 (2025) directly maps to Vox's agent architecture. Each risk has a corresponding Clavis control.

| NHI Risk | Risk Description | Clavis Mitigation (current/needed) |
| --- | --- | --- |
| NHI1: Improper Offboarding | NHI credentials not revoked when services retire | Needed: `vox clavis revoke <id>` linked to service lifecycle |
| NHI2: Secret Leakage | Secrets in code, logs, or output | Current: secret-env-guard, `#[serde(skip_serializing)]`, `secrecy::SecretString` |
| NHI3: Vulnerable Third-Party NHI | 3rd-party integrations with excessive permissions | Needed: per-integration scope documentation in `SecretSpec.capabilities` |
| NHI4: Insecure Authentication | Weak/deprecated auth mechanisms | Current: Clavis targets keyring + vault; env is deprecated in strict mode |
| NHI5: Overprivileged NHI | Broad permissions exceeding functional need | Needed: scope-width metadata per SecretSpec (`SecretScope::MinimalRequired`) |
| NHI6: Insecure Cloud Deployment | Misconfigured CI/cloud IAM | Current: `secret-env-guard` CI policy |
| NHI7: Long-Lived Secrets | Static, non-expiring credentials | Needed: expiry metadata + rotation cadence per SecretSpec |
| NHI8: Environment Isolation | dev ↔ prod credential sharing | Needed: profile-scoped overrides (§3.8) |
| NHI9: NHI Reuse | Same credential used across multiple services | Needed: `SecretSpec.consumers[]` tracking to detect shared use |
| NHI10: Human Use of NHI | Admins using service accounts for interactive access | Current: break-glass governance in threat model |

### 4.2 Secret isolation boundaries for AI agents

AI agents — including the Vox DEI orchestrator, MCP tool servers, and all `vox-skills` consumers — constitute non-human identities (NHIs) with ambient access to any secrets loaded at process start. The threat model must distinguish:

**Four boundaries for agent credential isolation:**

1. **Process boundary:** Secrets resolved from Clavis into the orchestrator process are visible to all code in that process. There is no per-agent sandboxing at this layer.

2. **Model context boundary:** The most critical boundary. Any secret value that enters a `system_prompt`, `user_message`, `tool_call arguments`, or `tool_call result` becomes visible to the LLM backend — and potentially to its provider logs. This boundary is enforced today by `#[serde(skip_serializing)]` on `api_key` fields and the `model-context-secret-material` CI detector.

3. **MCP tool output boundary:** MCP tool results are serialized to JSON and returned to the calling agent. `WebhookSignature`, `api_key` fields, and resolved secret values must never appear in tool results. The `secret_dataflow_leak_categories` CI check enforces this for code patterns but not at runtime.

4. **Agent-to-agent (A2A) delegation boundary:** When an orchestrator agent spawns a sub-agent for a specialized task, it must not pass raw secret values as task parameters. Instead, it should pass scoped capability references that the sub-agent resolves independently.

**Implementation requirements for each boundary:**

- **Process:** Continue current approach. No per-agent memory isolation at process level.
- **Model context:** Runtime `ResolvedSecret` must never implement `Display`, `Debug` (without `[redacted]`), or be used in format strings in tool/prompt paths. Enforce via linting rule.
- **MCP tool output:** All MCP tool results that include agent state must pass through a `redact_secrets(value: &Value, known_ids: &[SecretId]) -> Value` scrubber before serialization.
- **A2A delegation:** Defined in §4.4 below.

### 4.3 MCP authentication: OAuth 2.1 as the target

The MCP specification (2025/2026) mandates or strongly recommends OAuth 2.1 for remote MCP server authentication. Key requirements:

- **PKCE required** for all clients, including public clients (`vox-mcp` acting as MCP client).
- **Client ID Metadata Documents** (not Dynamic Client Registration) as the preferred client registration model.
- **Protected Resource Metadata (PRM)** for authorization endpoint discovery — prevents confused deputy attacks.
- **Resource Indicators (RFC 8707)** — tokens bound to specific audiences/resources.
- Short-lived access tokens (minutes, not hours); refresh tokens rotated on use.

**Clavis implications:**

- `vox-mcp` HTTP gateway currently uses static bearer tokens (`VOX_MCP_HTTP_BEARER_TOKEN`). This is appropriate for local stdio MCP but insufficient for remote MCP.
- For remote MCP deployment: Clavis must manage OAuth 2.1 client credentials (`client_id`, `client_secret`) and the authorization server discovery metadata as managed secrets.
- New secret class needed: `SecretClass::McpClientCredential` to represent OAuth client registration material.
- `vox clavis mcp-auth-status` — verify OAuth 2.1 configuration completeness for remote MCP deployment.

### 4.4 Agent-to-agent (A2A) credential delegation

When DEI orchestrates multi-agent workflows, secret delegation must follow the **OAuth 2.0 Token Exchange pattern (RFC 8693)** rather than passing raw secrets between agents.

**The problem:** If orchestrator A resolves `OPENROUTER_API_KEY` and passes it to sub-agent B as a string parameter, B now holds the full credential even if it only needs to make a single API call. A prompt injection attack on B can exfiltrate the key.

**The solution: scoped capability tokens**

1. **Orchestrator resolves credential** → gets `ResolvedSecret`.
2. **Orchestrator creates scoped delegation record** in VoxDB: `{parent_agent_id, child_agent_id, secret_id, scope, ttl_seconds, issued_at}`.
3. **Sub-agent receives a delegation reference** (opaque token ID), not the raw secret.
4. **Sub-agent calls `resolve_secret_for_delegation(ref_token)`** which validates the scope, checks TTL, and returns the resolved value only within the allowed scope.
5. **After TTL expiry**, delegation record is invalidated; sub-agent can no longer resolve the secret through that reference.

This is analogous to OAuth 2.0 Token Exchange where a subject token (orchestrator's credential) exchanges for an actor token (sub-agent's downscoped credential). RFC 8693 provides the standard shape.

**Minimum viable implementation:**
- VoxDB table: `agent_credential_delegations(id, parent, child, secret_id, scope_bits, issued_at, expires_at, revoked_at)`.
- `resolve_secret_for_delegation(delegation_id: &str) -> ResolvedSecret` in `vox-clavis`.
- Delegation revocation: `vox clavis revoke-delegation <id>`.
- CI: agents must not accept raw secret values as task parameters (linting rule).

**For the current architecture (pre-A2A credential exchange):** The minimum safe practice is ensuring sub-agent processes resolve secrets from Clavis independently using the same `SecretId` inventory, rather than receiving values from the orchestrator via IPC parameters.

### 4.5 Secret redaction pipeline for agent outputs

Any pipeline stage that collects agent outputs (tool results, traces, structured logs, telemetry) needs a scrubbing pass before the data leaves the process or is stored.

**Pattern library:**

The `secret_dataflow_leak_categories` CI check tests for static patterns in source code. A complementary runtime scrubber is needed for dynamic values.

```rust
// Conceptual API (not yet implemented):
/// Scrub known managed secret values from an arbitrary JSON value.
/// Uses a compact Bloom-filter-style membership test against all currently
/// resolved secrets to avoid false positives and O(n*m) string scanning.
pub fn redact_secrets_from_value(
    value: &serde_json::Value,
    resolved_ids: &[SecretId],
) -> serde_json::Value;

/// Check whether a string slice contains any resolved secret value.
pub fn contains_secret_material(text: &str, resolved_ids: &[SecretId]) -> bool;
```

**Implementation constraints:**
- The scrubber must itself not hold resolved secret values in its data structures — use hashed membership test or `secrecy::Secret<Bytes>` for the reference material.
- Apply automatically in: MCP tool result serialization path, structured telemetry events, VoxDB row writes, and agent trace commits.
- Opt-in for performance-critical paths; mandatory in telemetry upload and MCP output.

---

## 5. Envelope encryption and key hierarchy

This section formalizes the cryptographic model for the Clavis Cloudless vault.

### 5.1 KEK / DEK hierarchy (code-grounded)

The current Clavis vault backend (`crates/vox-clavis/src/backend/vox_vault.rs`) uses AES-GCM encryption backed by a master key stored in the OS keyring or derived from a passphrase. This is a single-level key model.

For account-level persistence with proper lifecycle controls, a two-level envelope encryption model is required:

```
Master Key (KEK)
  ├── Stored in OS keyring (local-first) or external KMS (cloud)
  └── Used only to wrap/unwrap Data Encryption Keys (DEKs)

Data Encryption Key (DEK)
  ├── One per secret class or per secret ID (configurable)
  ├── Wrapped by KEK; stored in VoxDB as ciphertext
  └── Used to encrypt/decrypt secret values (AES-256-GCM)

Secret Value
  └── Encrypted with DEK, stored in VoxDB
```

**Properties:**
- KEK rotation does not require re-encrypting secret values — only the wrapped DEKs need rewrapping.
- Compromising one DEK exposes only the secrets encrypted under that DEK.
- DEKs are never stored in plaintext; they exist only briefly in memory during encrypt/decrypt operations and are `zeroize`d immediately after use.
- KEK version (`VOX_CLAVIS_KEK_VERSION`) is stored alongside the wrapped DEK to support key versioning during rotation.

### 5.2 Existing implementation anchors

The `VOX_CLAVIS_KEK_REF` and `VOX_CLAVIS_KEK_VERSION` secrets in spec.rs already anticipate this model. The break-glass runbook covers KEK rotation. The implementation catalog should be updated to include DEK management as a separate step from KEK management.

### 5.3 Local-first operating model

For developers running Clavis without a remote vault:

1. KEK is derived from OS keyring entry (`vox-clavis-vault / master`).
2. DEKs are generated per-session (or per-secret-class) and wrapped by the KEK.
3. Wrapped DEKs and encrypted secret values are stored in a local SQLite file (`~/.vox/clavis.db`).
4. Remote VoxDB sync is opt-in: wrapped DEKs and ciphertext can sync to Turso; KEK remains local-only.

This model ensures: **the cloud never has the key**, only encrypted ciphertext. Users retain full sovereignty. Matches the "Hybrid (Keyring + VoxDB ciphertext)" tier from the base research document.

archived_date: 2026-04-18
---

## 6. Competitive feature gap analysis

This table maps features from leading secrets managers against Clavis's current state.

| Feature | Doppler | Infisical | Pulumi ESC | Vault OSS | Clavis today | Clavis gap |
| --- | --- | --- | --- | --- | --- | --- |
| Centralized metadata registry | ✓ | ✓ | ✓ | ✓ | ✓ (`spec.rs`) | None |
| CLI secret resolution | ✓ | ✓ | ✓ (`esc run`) | ✓ | ✓ (`vox clavis doctor`) | Needs `vox clavis run <cmd>` wrapper |
| Import wizard | ✓ | ✓ | ✓ | Partial | Partial | dry-run, conflict detection |
| Secret versioning | ✓ | ✓ | ✓ | ✓ | ✗ | VoxDB version history |
| Automatic rotation | ✓ (managed) | ✓ (rolling) | ✓ (scheduled) | ✓ (dynamic) | ✗ | Phase 1–3 rotation (§3.6) |
| Expiry alerting | ✓ | ✓ | ✓ | ✓ | ✗ | Metadata + doctor warning |
| Audit logging | ✓ | ✓ | ✓ | ✓ | ✗ | Append-only log |
| Profile/environment namespacing | ✓ | ✓ | ✓ | ✓ | Partial (profiles) | Per-profile value overrides |
| Self-hosted option | ✗ | ✓ | Partial | ✓ | ✓ (local-first) | Strength; maintain |
| Agent/NHI lifecycle | ✗ | Partial | ✗ | Partial | ✗ | A2A delegation (§4.4) |
| AI-specific secret redaction | ✗ | ✗ | ✗ | ✗ | Partial (CI static) | Runtime scrubber (§4.5) |
| MCP OAuth 2.1 integration | ✗ | ✗ | ✗ | ✗ (general) | ✗ | McpClientCredential class (§4.3) |
| BYOK KEK model | ✓ (enterprise) | ✓ (enterprise) | ✓ (CSEK) | ✓ | Partial (KEK ref) | Full KEK/DEK separation (§5) |
| Drift detection | ✓ | ✓ | ✓ | ✗ | Partial (`clavis-parity`) | Cross-env diff (§3.9) |
| Secret health probe | Partial | Partial | ✗ | ✗ | ✗ | Optional integration probe (§3.5) |
| OWASP NHI alignment | ✗ | Partial | ✗ | Partial | Partial | Full NHI control mapping (§4.1) |

**Unique Clavis advantages vs. the comparison set:**
1. Fully local-first, cloudless-native from day one — Doppler requires a SaaS backend.
2. Integrated with AI agent (MCP/DEI) architecture — none of the comparison tools have AI-agent-native credential isolation.
3. CI-enforced policy guards at compile-time (`secret-env-guard`) — unique to this codebase.
4. Zero vendor lock-in for core functionality — all secret storage is open.
5. TOESTUB-compliant Rust implementation — memory safety, no CVE inheritance from Python/Node supply chains.

---

## 7. Feature roadmap (Clavis V2)

This section synthesizes all findings into an ordered roadmap. Sequencing reflects dependency order: metadata before rotation, rotation before delegation.

### Wave 0: Secret taxonomization and documentation (no code changes)

- Publish this taxonomy document as the authoritative env-var classification guide.
- Annotate each `SecretSpec` in `spec.rs` with the taxonomy class from §2.
- Label operator tuning envs explicitly in `OPERATOR_TUNING_ENVS` with their non-secret status.
- Update `clavis-ssot.md` with class assignments and lifecycle policy per class.

### Wave 1: Metadata enrichment

- `SecretSpec` additions: `rotation_cadence_days: Option<u32>`, `expiry_warning_days: Option<u32>`, `consumers: Vec<&'static str>`, `scope_description: &'static str`.
- `ResolutionStatus` additions: `NearingExpiry`, `StaleRotation`, `RotationOverdue`.
- `vox clavis doctor` shows per-class health with rotation warnings.
- `vox clavis history <id>` surface (even if only showing "no history tracked yet").

### Wave 2: Audit logging

- Append-only audit log: JSON lines written to `~/.vox/clavis-audit.log` (or VoxDB table).
- Fields: timestamp, secret_id, resolution_status, source, profile, caller module, resolved_value_present (bool only).
- `vox clavis audit-log` CLI reader.
- CI: validate audit log schema has not changed in a breaking way.

### Wave 3: Import and migration hardening

- `vox clavis import-env --dry-run` with conflict detection.
- Pattern-based classification pre-analysis (detect provider keys from name patterns).
- Canonical name suggestion for non-standard env var names.

### Wave 4: Secret versioning

- VoxDB vault backend gains `secret_versions` table.
- `vox clavis rotate <id> --new-value <val>` records version history.
- `vox clavis rollback <id> --to-version <n>` restores previous value.

### Wave 5: Profile-scoped overrides

- Per-profile value overrides in VoxDB vault.
- `vox clavis set <id> --profile <profile> --value <val>`.
- `resolve_secret` checks profile-specific value first.

### Wave 6: AI agent secret boundaries

- Runtime `redact_secrets_from_value` scrubber (§4.5).
- Apply scrubber at MCP tool result serialization path.
- `McpClientCredential` secret class for OAuth 2.1 client material.
- `vox clavis mcp-auth-status` CLI surface.

### Wave 7: A2A credential delegation

- VoxDB `agent_credential_delegations` table.
- `resolve_secret_for_delegation` API.
- TTL-bounded delegation with revocation.
- Delegation audit events.

### Wave 8: Rotation orchestration (Phase 1)

- Provider-specific rotation guidance registry.
- `vox clavis rotation-calendar` — shows upcoming rotation due dates.
- Programmatic rotation for providers with APIs (RunPod, Vast.ai).

archived_date: 2026-04-18
---

## 8. Security invariants (additions to V1 threat model)

These extend the invariants in [Clavis Cloudless Threat Model V1](clavis-cloudless-threat-model-v1.md).

6. No secret class `transport` or `account` credential may be passed as a string parameter in A2A task descriptors. Agent delegation must use opaque delegation references only.
7. All MCP tool results must pass through `redact_secrets_from_value` before serialization when the result contains fields resolved from external state.
8. OAuth 2.1 client credentials for remote MCP must be stored as `SecretClass::McpClientCredential` and must never appear in `VOX_MCP_HTTP_BEARER_TOKEN` directly in production profiles.
9. Any `SecretSpec` with `rotation_cadence_days` set must produce a `ResolutionStatus::RotationOverdue` warning after twice the configured cadence has elapsed without a recorded rotation event.
10. Delegation tokens have a hard maximum TTL of 1 hour. No perpetual delegation references.
11. The `redact_secrets_from_value` scrubber must be applied before any write to: VoxDB `agent_events`, MCP tool response payloads, telemetry upload batches, or structured log sinks.

---

## 9. Open research questions (feeding Wave 6–8 implementation plans)

1. **DEK granularity:** Should DEKs be per-secret-ID, per-secret-class, or per-profile? Finer granularity increases blast-radius isolation but adds overhead and key management complexity.
2. **Delegation reference format:** Should delegation references be opaque random tokens, signed JWTs, or content-addressed tokens? JWTs allow offline validation; opaque tokens require a DB lookup but support revocation without coordination.
3. **Provider-specific expiry metadata:** How do we retrieve and cache provider-reported expiry dates (e.g., GitHub PAT expiry from the API response) without having to rotate manually?
4. **Scrubber performance:** The `redact_secrets_from_value` scrubber must not become a bottleneck on high-frequency tool call paths. What is the right combination of Bloom filter + AhoCorasick string scanner for this use case?
5. **Human-in-the-loop for delegation approvals:** For high-blast-radius credentials (GPU providers, DB tokens), should delegation require an explicit HITL approval step before the delegation record is created?
6. **Cross-device sync of `NearingExpiry` alerts:** If a user's Clavis instance detects a nearing-expiry credential, how should this propagate to a second device without syncing the credential value itself?

archived_date: 2026-04-18
---

## 10. Bibliography and sources

### Standards and specifications
- [OWASP Secrets Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html)
- [OWASP Non-Human Identities Top 10 (2025)](https://owasp.org/www-project-non-human-identities-top-10/2025/)
- [OWASP LLM Top 10 for LLM Applications (2025)](https://owasp.org/www-project-top-10-for-large-language-model-applications/)
- [OWASP LLM Prompt Injection Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/LLM_Prompt_Injection_Prevention_Cheat_Sheet.html)
- [RFC 8693: OAuth 2.0 Token Exchange](https://rfc-editor.org/rfc/rfc8693)
- [RFC 8707: Resource Indicators for OAuth 2.0](https://rfc-editor.org/rfc/rfc8707)
- [RFC 7591: OAuth 2.0 Dynamic Client Registration](https://rfc-editor.org/rfc/rfc7591)
- [MCP Specification (2025/2026)](https://modelcontextprotocol.io/specification/latest/basic)
- [MCP Authorization Documentation](https://modelcontextprotocol.io/docs/concepts/authentication)
- [NIST SP 800-57 Part 1 Rev. 6 — Key Management Recommendation](https://csrc.nist.gov/pubs/sp/800/57/pt1/r6/ipd)

### Industry research and statistics
- [GitGuardian: State of Secrets Sprawl 2025](https://www.gitguardian.com/state-of-secrets-sprawl)
- [Infisical: Dynamic secrets and just-in-time credentials](https://infisical.com/docs/documentation/platform/dynamic-secrets/overview)
- [Doppler: Secrets management best practices](https://www.doppler.com/blog/environment-variables-secrets-management)
- [Pulumi ESC: Environment secrets and configuration](https://www.pulumi.com/product/esc/)
- [Aembit: AI agent security and NHI governance (2025)](https://aembit.io)
- [Akeyless: Dynamic secrets in 2025](https://akeyless.io)
- [Cloud Security Alliance: NHI governance](https://cloudsecurityalliance.org)

### Competitive platform documentation
- [Infisical: Self-hosted deployment and open-source comparison](https://infisical.com/docs/self-hosting/overview)
- [Doppler: Automatic rotation docs](https://docs.doppler.com/docs/rotations)
- [1Password: Secrets Automation](https://developer.1password.com/docs/secrets-automation)
- [HashiCorp Vault: Dynamic credentials](https://developer.hashicorp.com/vault/tutorials/db-credentials/database-secrets)
- [OpenBao: Vault-compatible open-source fork](https://openbao.org/)
- [SOPS + age](https://github.com/getsops/sops)

### AI agent security
- [Microsoft: Defense-in-depth for prompt injection](https://learn.microsoft.com/en-us/security/ai-infrastructure/)
- [Red Hat: Zero trust for AI agents (2025)](https://www.redhat.com)
- [paloaltonetworks.com: MCP security analysis](https://www.paloaltonetworks.com)
- [Datadog: MCP attack surface](https://www.datadoghq.com)

### Rust ecosystem
- [`secrecy` crate](https://docs.rs/secrecy/latest/secrecy/)
- [`zeroize` crate](https://docs.rs/zeroize/latest/zeroize/)
- [`aes-gcm` crate](https://docs.rs/aes-gcm/latest/aes_gcm/)
- [`blake3` crate](https://docs.rs/blake3/latest/blake3/)
- [`vaultrs` crate](https://crates.io/crates/vaultrs)
- [`keyring` crate](https://docs.rs/keyring/latest/keyring/)


