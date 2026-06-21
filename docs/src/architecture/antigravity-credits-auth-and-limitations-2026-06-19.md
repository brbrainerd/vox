---
title: "Antigravity (agy) Credits, Auth & Limitations"
description: "Persistent SSOT for how Vox interfaces with Google Antigravity's Gemini credits via the agy CLI: the OAuth-only auth model (no headless API key — antigravity-cli#78 open), per-project quota/rate limits, the un-queryable credit balance, the agy-credits-vs-GEMINI_API_KEY split, and how this plugs into Vox Clavis (vox-secrets) and credential-aware model selection."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Antigravity (`agy`) Credits, Auth & Limitations

**Purpose.** Single source of truth for *how Vox uses Antigravity's Gemini credits* and *what it cannot do*. Read this before wiring anything that assumes an `agy` API key, a queryable credit balance, or `agy` as a chat-completion model provider — all three are false. Companion to the delegation plan [`docs/superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md`](../../superpowers/plans/2026-06-19-agy-delegation-wedge1-2.md).

## TL;DR
- `agy` (Antigravity CLI) authenticates with **interactive Google OAuth** (or a GCP project). There is **no headless API-key auth** for `agy` yet — [antigravity-cli#78](https://github.com/google-antigravity/antigravity-cli/issues/78) requests it and is **open**.
- `agy` stores **its own** OAuth tokens in its config dir. **Vox Clavis cannot hold a usable `agy` key** — there isn't one. Clavis support for Antigravity = a *provider-availability record* (the doctor) + optional non-secret GCP-project hint, **not** a stored API key.
- **Two distinct Gemini egresses exist in Vox** and must not be conflated:
  1. **`agy` CLI → Antigravity credits** (OAuth, local, *agentic* — edits files, runs tools). Used by `vox_agy_delegate`. Billed in **Antigravity credits**, not USD.
  2. **GoogleDirect → `generativelanguage.googleapis.com`** via **`GEMINI_API_KEY`** (a Clavis secret; *inference* HTTP completions). Used by the model-selection/egress path. Billed in **USD per token**.
- **Credit balance is not programmatically queryable.** `/stats` is an interactive TUI command; there is no headless balance/quota API. ⇒ Vox **cannot meter `agy` credit spend in dollars**; it treats `agy` as a non-USD credit pool, gated by the doctor's `Ready` state and request-level retry, not by a USD budget.

## Auth model (verified 2026-06)
- First `agy` run launches **Google Sign-In** (OAuth) or binds a **GCP project**. One-time, interactive, human-in-the-loop. We **never store Google credentials** in Vox.
- Headless/CI: the Gemini *API* and *gemini-cli* support `GOOGLE_API_KEY` / `GOOGLE_APPLICATION_CREDENTIALS` (service account). **`agy` does not yet** ([#78](https://github.com/google-antigravity/antigravity-cli/issues/78)). Track that issue; if it lands, revisit storing an agy-scoped key in Clavis.
- Rate limits are enforced **per project, not per API key**, across RPM / TPM / RPD / IPM.

## Credits & quota limits (verified 2026-06)
| Dimension | Free tier | Tier 1 (paid) |
|---|---|---|
| RPM | ~5–15 (model-dependent) | ~150–300 |
| Also limited by | TPM, RPD, IPM | TPM, RPD, IPM |

- During the Antigravity preview, the CLI ships with "generous rate limits at no cost"; tiers auto-upgrade as spend/usage rises.
- **GCP credit caveat:** Google Cloud accounts opened **after 2026-03-02** cannot use GCP credits to pay for Gemini API / AI Studio usage (only other GCP products). Relevant if you fund the GoogleDirect path via a GCP project.
- **Quota cutoff mid-run is expected** under heavy fan-out. Vox classifies `quota` / `rate limit` / `resource_exhausted` in `agy_exec::classify_failure` and applies bounded backoff retry (`should_retry`).

## Hard limitations (do not design around these)
1. **No headless API key for `agy`** (#78 open) → OAuth only; first login is interactive.
2. **No queryable credit/quota balance headlessly** (`/stats` is TUI) → no USD metering of `agy` credits; budget treats it as an opaque credit pool.
3. **Sandbox bypass under auto-accept** ([#36](https://github.com/google-antigravity/antigravity-cli/issues/36)) → `--sandbox` + `--dangerously-skip-permissions` is escapable; Vox isolates via a git worktree instead and never passes `--sandbox`.
4. **No reliable JSON output** → parse exit code + stderr + `git diff`.
5. **Internal sub-agents are TUI-only** (`/agents` panel) → unmanageable under `-p`; Vox's parallelism is N concurrent `agy -p` workers, not agy's internal sub-agents.
6. **`agy` is an agent, not an inference endpoint** → it must NOT be modeled as a `ProviderType` in `vox-orchestrator::models`. It is a *delegation provider* surfaced via the doctor.

## How this plugs into Vox Clavis & credential-aware selection
Vox Clavis is the `vox-secrets` crate (vault `.vox/clavis_vault.db`). The "interceptor" people refer to is the **resolution chokepoint**: `vox_secrets::resolve_secret(SecretId)` and `vox-config::resolve_egress`. Model selection is **already credential-aware**: `vox-orchestrator::models::key_guard::provider_secret_is_available(ProviderType)` checks each inference provider's Clavis key and selection filters out providers with no key (OpenRouter is one of many — GoogleDirect, Groq, Mistral, DeepSeek, SambaNova, Cerebras, Anthropic, HuggingFaceRouter, local/Ollama/PopuliMesh).

Antigravity joins this picture as a **delegation provider**, not an inference provider:
- **Availability** comes from `agy_doctor::detect()` (`Missing` / `PresentUnauthed` / `Ready`), not a Clavis key lookup.
- **The Clavis-keyed Gemini path** (`GEMINI_API_KEY` → `GoogleDirect`) remains the *inference* egress and is already wired.
- A unified **credentials-status surface** (`vox_credentials_status`, see the plan's Wedge C) reports, in one place, every inference provider whose key is present (via the redaction-safe `vox_secrets::list_secret_status()`) **plus** the agy delegation provider's doctor state — so the system, and the operator, can see exactly which models/providers are payable right now.

## Update triggers (keep this doc current)
- antigravity-cli **#78** closes (headless agy API key) → add an agy Clavis secret + revisit auth.
- antigravity-cli **#36** closes (sandbox bypass) → revisit whether `agy --sandbox` can be trusted (still keep the worktree jail).
- Google publishes a headless quota/balance endpoint → wire real USD/credit metering into `BudgetManager`.

## Sources
- [Antigravity CLI #78 — headless Gemini API key auth (open feature request)](https://github.com/google-antigravity/antigravity-cli/issues/78)
- [Antigravity CLI #36 — sandbox bypass with --dangerously-skip-permissions](https://github.com/google-antigravity/antigravity-cli/issues/36)
- [Gemini API — Billing](https://ai.google.dev/gemini-api/docs/billing)
- [Gemini API — Rate limits](https://ai.google.dev/gemini-api/docs/rate-limits)
- [Gemini API — API keys vs OAuth](https://ai.google.dev/gemini-api/docs/api-key)
