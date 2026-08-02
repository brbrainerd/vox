---
title: "Gap-Fill: Aider/Zed/Cursor Local-Model Mechanisms — Verified Research 2026-07-30"
description: "Closes the tool-comparison research's remaining gaps: Aider's ollama_chat/ prefix convention, Zed's language_models config block with unconfirmed exact field names, and the corrected finding that Cursor has no built-in Ollama support — only an OpenAI-compatible base-URL workaround. Recovered from a workflow run whose synthesis stage failed, reconstructed directly from per-claim verification votes."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Gap-Fill: Aider / Zed / Cursor Local-Model Mechanisms (2026-07-30)

> **Provenance and a methodology note worth reading first.** `deep-research` run
> `wf_f87b3356-fbc` (101 of 103 agents completed; 2 errored on transient rate-limiting) reached
> **17 confirmed / 8 refuted** across 25 verified claims — but its **synthesis stage failed**
> and returned a literal placeholder (`"claim":"test claim"`) instead of the real findings,
> while the `refuted`, `sources`, and `stats` fields carried real content. This document was
> **hand-reconstructed from the raw per-claim verification votes in the run's journal**, not
> from the (broken) synthesized output — every claim below is traced to its exact vote.
>
> This is disclosed in full because it's an unusual failure mode worth naming: a workflow can
> complete "successfully" (no agent errors reported) while its final synthesis step silently
> produces garbage. **Always spot-check a synthesized `findings` array against the raw
> `logs`/journal before trusting it** — this run is the concrete example of why.

Closes the gap left by [`coding-agent-local-model-ux-comparison-2026-07-30.md`](coding-agent-local-model-ux-comparison-2026-07-30.md)
§6 (items 1, 2, 4) and the companion security gap-fill run — see
[`skill-marketplace-security-and-provenance-research-2026-07-30.md`](skill-marketplace-security-and-provenance-research-2026-07-30.md)
for the sibling batch, which synthesized cleanly.

---

## 0. Headline correction — Cursor does not have built-in Ollama support

The earlier tool-comparison document (§4) left Cursor entirely unresearched. This run found and
**refuted (0-3)** the natural assumption:

> **⚠ Refuted 0-3:** "Cursor has built-in support for local models via Ollama, requiring no
> extension."

What's actually confirmed instead: Cursor's local-model setup does **not** use a dedicated
`ollama/`-style model-name prefix. It relies on **generic custom model naming plus an
OpenAI-compatible base-URL override** — Cursor treats a local endpoint as an OpenAI-API-compatible
provider, not as a first-class local-model concept. Concretely, per a Cursor forum thread
(corroborating, not primary-doc-sourced): enable "Override OpenAI Base URL," point it at the
local endpoint with a `/v1` suffix.

**This means Cursor's actual local-model posture is closer to Claude Code's (none, natively) than
to Continue.dev's (first-class).** It gets local models working only via the generic
OpenAI-compatible-endpoint escape hatch every one of these tools has, not via anything purpose-
built. No hardware/VRAM check was found (unsurprising, given no dedicated local-model UI exists
to gate).

---

## 1. Aider — the `ollama_chat/` prefix convention, confirmed (this closes the earlier gap directly)

The tool-comparison doc (§2.2) flagged Aider's local-model mechanism as genuinely unresolved
after the `--openai-api-base` guess was refuted. **This run resolves it:**

### 1.1 Model-name prefix (confirmed 3-0, corroborated 3-0 by a second independent claim)

> "Aider uses the `ollama_chat/` model name prefix (a LiteLLM-style prefix), not a dedicated
> `--ollama` flag, and recommends it over the older `ollama/` prefix."

A second, differently-worded extraction of the same fact also confirmed 3-0:

> "Aider uses the `ollama_chat/` prefix (with plain `ollama/` also supported) as the model-name
> mechanism for configuring local Ollama models, matching LiteLLM's provider-prefix convention
> rather than a dedicated `--ollama` flag."

And a third, narrower framing (2-1): Aider does **not** use plain `ollama/` *by default* — the
docs specifically recommend `ollama_chat/<model>`.

**Net: `ollama_chat/<model-name>` is Aider's documented, recommended convention.** This is the
exact same prefix convention LiteLLM itself uses (routing doc §1.1 —
`model: ollama_chat/llama3.1`) — worth noting because it means **Aider adopted LiteLLM's naming
scheme rather than inventing its own**, which is itself a small piece of evidence that this
prefix pattern is becoming a de facto standard worth Vox adopting too, alongside the
Continue.dev-style `provider`/`apiBase` array (parity plan Phase 2.1).

### 1.2 Connection endpoint (confirmed 2-1)

> "Aider configures the local Ollama endpoint via the `OLLAMA_API_BASE` (and optionally
> `OLLAMA_API_KEY`) environment variables rather than a CLI flag."

Quoted example: `export OLLAMA_API_BASE=http://127.0.0.1:11434 # Mac/Linux`.

> **⚠ Two near-duplicate framings of this same fact were refuted (1-2, 1-2)** — both used the
> phrase "not a dedicated flag/config file field," which verifiers apparently judged
> overstated (Aider *does* have a `.aider.model.settings.yml` file where a fixed context size
> can be pinned, so "not a config file field" is too absolute even though "not a CLI flag" is
> accurate for the endpoint itself). **Cite the endpoint-via-env-var fact; don't cite the
> "no config file involvement at all" framing.**

### 1.3 No hardware/VRAM check — confirmed, but with a real nuance (confirmed 3-0)

> "Aider does not perform any hardware/VRAM/capability check before offering a local Ollama
> model; instead it addresses only the **context-window** limitation via automatic sizing or
> manual YAML/env config."

Quoted: *"Ollama defaults to only 2k tokens, which is insufficient. Aider automatically adjusts
the context window to accommodate your request plus 8k tokens for responses."*

> **⚠ A more absolute framing — "Aider's official docs do not describe any hardware… check" —
> was refuted 0-3.** The distinction that survived verification: Aider's docs **do** address a
> pre-flight consideration (context-window sizing), just not a *hardware/capability* one. Don't
> claim the docs are silent on pre-flight concerns generally — only on hardware/VRAM
> specifically.
>
> **⚠ Also refuted (0-3):** the specific claim that Aider auto-sets Ollama's context window to
> exactly "8k tokens by default." The mechanism (auto-adjustment) is confirmed; the specific
> number is not.

Per-model fixed sizing is available via `.aider.model.settings.yml`:

```yaml
- name: ollama/qwen2.5-coder:32b-instruct-fp16
  extra_params:
    num_ctx: 65536
```

### 1.4 LM Studio has its own dedicated docs page (confirmed 3-0)

> "Aider has dedicated documentation for LM Studio support at
> `aider.chat/docs/llms/lm-studio.html`."

Corroborated by a second confirmed claim (2-1): the docs site has separate dedicated pages per
local-model provider, not one generic "local models" page.

---

## 2. Zed — architecture confirmed, specific field names NOT confirmed

This is the most important nuance in the whole gap-fill run: **several plausible, specific
settings.json field-name claims sourced from third-party blogs were explicitly refuted (0-3).**
Only the architectural shape survived at high confidence.

### 2.1 What IS confirmed (3-0 each)

- Zed configures local model providers (**Ollama, LM Studio, llama.cpp**) via a
  **`language_models` block** in `settings.json`.
- Zed has a **built-in Ollama provider** that also works with llama.cpp and other
  OpenAI-compatible local servers.
- For Ollama specifically, Zed sends context length as a **`num_ctx`** parameter (2-0, one
  verifier errored rather than dissented — effectively confirmed).
- The Zed local-model docs page contains **no mention of any hardware/VRAM/capability check.**
- Zed itself performs **no automatic hardware/VRAM capability check** — where such a check
  exists at all (e.g. LM Studio's own UI showing whether a model fits available VRAM), it
  happens in the **external tool**, not in Zed.

### 2.2 What is NOT confirmed — refuted specific field names (0-3 each)

Three specific, plausible-sounding settings.json schemas — each sourced to a different
third-party blog post — were **explicitly refuted**:

> ⚠ Refuted: Zed auto-detects a running Ollama instance and writes
> `agent.default_model.provider = "ollama"` / `agent.default_model.model = "devstral:latest"`
> into `~/.config/zed/settings.json`.

> ⚠ Refuted: the schema uses a `"language_models"` section with `"api_url"` and
> `"low_speed_timeout_in_seconds"` fields, plus a separate `"assistant"` section with
> `"provider"`, `"model"`, `"version"`, `"enabled"`, `"dock"` fields.

**Read this as: three different bloggers described three different (and mutually
inconsistent) schemas, none of which survived verification against Zed's actual current
docs.** This is itself informative — it suggests Zed's local-model config schema has changed
across versions faster than blog coverage has kept up, which is exactly the kind of
config-schema volatility the routing research flagged as a real risk (routing doc §5, citing
Continue.dev's `config.json`→`config.yaml` migration). **Do not cite any specific Zed field name
beyond `language_models` (the block name) and `num_ctx` (the context-length parameter) — both
independently confirmed 3-0/2-0. Everything more granular needs a fresh direct fetch of Zed's
current docs before being cited anywhere, including in Vox's own design work.**

### 2.3 One confirmed, narrowly-scoped claim worth flagging

> "Zed does not use a dedicated settings.json Ollama config scheme documented in **this [specific
> blog] post**; instead it recommends pointing LM Studio's local server at
> `http://localhost:1234/api/v0`." (confirmed 2-1)

This is a claim about **one specific source's content**, not a general claim about Zed's
capabilities — it happens to corroborate that LM Studio integration works via the same
generic local-HTTP-server pattern Aider and Continue.dev use, rather than a bespoke integration.

---

## 3. Windsurf — still entirely unresearched

No claims about Windsurf survived extraction in this run either (the `windsurf-primary-docs`
search angle returned results, but only 1 of 6 was judged novel, and it did not survive to the
verified set). **Windsurf's local-model story remains completely open** across three research
attempts now. If this matters for the parity plan, it needs a dedicated, narrowly-scoped run
against Windsurf's own docs specifically — the general and semi-general passes have both failed
to surface anything on it.

---

## 4. What this changes in the parity plan

Cross-referencing [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md)
Phase 2.1:

- **Adopt `ollama_chat/<model>`-style prefixing as an *alternative* addressing scheme**, not a
  replacement for the Continue.dev-style `provider`/`model`/`apiBase` array already
  recommended. Aider and LiteLLM both converged on this prefix convention independently of
  Continue.dev's array-of-objects approach — supporting a prefix form (e.g. accepting
  `ollama_chat/qwen3:8b` as a valid model identifier anywhere a model id is expected) costs
  little and matches an emerging convention two tools already share.
- **Do not model Vox's local-model UX on Cursor's** — its "support" is the generic
  OpenAI-compatible-endpoint workaround every tool has by default, not a dedicated feature.
  Continue.dev remains the strongest prior art (tool-comparison doc §3).
- **When implementing hardware/VRAM gating (parity plan Phase 2.6), Zed's own pattern is worth
  copying for the boundary of responsibility**: Zed does no hardware checking itself and
  explicitly defers to the local-model tool's own UI (LM Studio) for that signal. Vox, wiring
  `vox-plugin-nvml-probe` directly, can do better than every tool surveyed — but should still
  design the check as advisory (deprioritize, don't hard-block) unless a model provably cannot
  load, matching the soft/hard distinction from the routing research (routing doc §2.2).
