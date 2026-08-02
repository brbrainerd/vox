---
title: "Deep Research: Multi-Provider Model Routing, GUI Key Wiring, Skills Packaging & Publication Handoff (2026-08-01)"
description: "Code-audited finding that Vox's key-presence-aware multi-provider gating (key_guard.rs/decide()) and 9-provider GUI settings surface already exist but the research pipeline bypasses both via an older selection path and a 2-lane (local+OpenRouter) LLM egress cascade; surveys free-tier APIs beyond OpenRouter; sketches deep-research skill packaging and SCIENTIA/VoxGiantia publication handoff."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Normative code audit of the multi-provider routing split-brain plus free-tier survey and skills/publication design considerations; direct input to the deep-research enhancement implementation plan."
---

# Deep Research: Multi-Provider Model Routing, GUI Key Wiring, Skills Packaging & Publication Handoff

**Date:** 2026-08-01
**Scope:** Whether Vox's deep-research pipeline can already route intelligently across multiple LLM providers based on configured keys (it can't, despite the machinery existing elsewhere); a survey of free/cheap LLM API tiers beyond OpenRouter; and design considerations for packaging deep research as an ad-hoc skill and handing findings off to SCIENTIA/VoxGiantia for auto-publication. Companion to [deep-research-fundamentals-2026-08-01.md](deep-research-fundamentals-2026-08-01.md), [deep-research-competitive-landscape-2026-08-01.md](deep-research-competitive-landscape-2026-08-01.md), and [deep-research-trust-novelty-scoring-landscape-2026-08-01.md](deep-research-trust-novelty-scoring-landscape-2026-08-01.md).

---

## Part A — Code audit: multi-provider routing is a split-brain

This is the single most consequential finding in this research phase: **Vox already built the machinery for key-presence-aware multi-provider routing, and it works — but the deep-research pipeline calls a different code path that never uses it.**

### A1. Credential entry already exists and is generic

`contracts/orchestration/providers.v1.yaml` declares 11 providers with `secret_id` mappings: `GoogleDirect` (`GeminiApiKey`), `OpenRouter` (`OpenRouterApiKey`), `Groq` (`GroqApiKey`), `Cerebras` (`CerebrasApiKey`), `Mistral` (`MistralApiKey`), `DeepSeek` (`DeepSeekApiKey`), `SambaNova` (`SambaNovaApiKey`), `Anthropic` (`AnthropicApiKey`), `HuggingFaceRouter` (`HuggingFaceToken`), plus keyless `Ollama`/`PopuliMesh`/`VoxLocal`. `crates/vox-orchestrator-types/build.rs` codegens the `ProviderType` enum from this file at build time. `crates/vox-secrets/src/spec/ids.rs` registers every one of those as a `SecretId`; `CHAT_CLOUD_PRIMARY` is `&[SecretId::OpenRouterApiKey]` **only** — OpenRouter is the sole key treated as blocking/primary, every direct-provider key is merely optional/observed.

The GUI already surfaces all of them generically: `crates/vox-gui/src/commands/secrets.rs::list_secret_status()` enumerates every registered `SecretSpec`, and `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`'s `KeysSecretsSection` renders them dynamically under a "Keys & Secrets" tab — write-only, redacted-preview only. A user can already paste in a Gemini, Groq, Cerebras, Mistral, DeepSeek, SambaNova, HuggingFace, Anthropic, or OpenRouter key through this one table today. The known gap (matching a standing memory note): `crates/vox-gui/ui/src/lib/federatedSearchIndex.ts` has no references to secrets/API keys, so the Omnibar settings search can't find "add my Groq key."

**Conclusion:** credential entry for 9 cloud providers is already built and data-driven. The gap is entirely downstream — whether anything *uses* those keys for research routing.

### A2. Key-presence gating exists — in a path the research pipeline doesn't call

Two independent, non-communicating layers exist:

**Layer 1 (works, but unused by research):** `crates/vox-orchestrator/src/models/key_guard.rs::provider_secret_is_available(&ProviderType) -> bool` maps every provider to its `SecretId` and checks `vox_secrets::resolve_secret(...).expose().is_some()`. This is consumed by `crates/vox-orchestrator/src/models/select.rs::decide()`, which rejects any candidate whose key isn't present (tests confirm: a keyless Anthropic-direct candidate is rejected, an Ollama candidate wins; setting `ANTHROPIC_API_KEY` flips the outcome). `crates/vox-actor-runtime/src/model_resolution.rs` documents `decide()` as "the single exercised selection path" for **chat** routes.

**Layer 2 (what research actually calls, and never touches the gate):** `crates/vox-research-shim/src/research/model_select.rs::resolve_stage()` calls `select_with_default_registry(&intent)` → `select()` → `select_inner()` → `select_via_premium_alias()` or `select_via_scorer()`. **Neither checks key presence anywhere.** `select_via_scorer` bottoms out in `registry.rs::best_for_internal()`, whose filter closure checks cost penalties and budget caps but never `key_is_present_for`. Net effect: `resolve_research_models()` can and does return a model ID for a provider whose key is not configured — nothing filters it out.

**A third, independent gap — the LLM egress cascade the research stages actually call has only two lanes, period:** `crates/vox-actor-runtime/src/llm/cascade.rs::cascade_for_research_stage()` builds candidates from exactly two sources: (1) local Ollama/Mens, gated by `inference_profile_allows_local_ollama_http()`; (2) OpenRouter, gated by `openrouter_api_key().is_some()`, via `LlmConfig::openrouter(model_id)`. **There is no third branch for `GoogleDirect`/`Groq`/`Cerebras`/`Mistral`/`DeepSeek`/`SambaNova`/`Anthropic`/`HuggingFaceRouter` direct dispatch**, regardless of what `provider_secret_is_available()` reports elsewhere. A user who configures a Groq or Gemini-direct key but has no OpenRouter key gets **zero** cloud candidates from research — it silently degrades to local-only, or fails outright ("no LLM candidates available for research cascade") even with a perfectly good key sitting unused. The resolved model IDs from Layer 2 *are* threaded into every stage (planner, claim extraction, synthesis, verifier) — they're just forced through the OpenRouter branch of a two-lane cascade regardless of which provider the model conceptually belongs to.

### A3. Even the scorer path structurally excludes free models for research

`SelectionIntent::research()` uses `SelectionAxes::QUALITY_FIRST` (15/15/70), which derives `CostPreference::Performance`. `registry.rs`'s filter has:

```rust
if preference == CostPreference::Performance && m.is_free {
    return false; // Skip free models in performance mode unless they are explicitly mapped
}
```

So even with keys and routing fixed, today's research intent would still never select a free-tier model via the scorer path by construction. Separately, `QualityLevel` (`Flash | Balanced | Premium` in `model_select.rs`) is **dead plumbing** — its sole consumer, `resolve_research_models()`, takes it as an underscore-prefixed unused parameter. The one working "prefer free" knob, `vox_config::inference::research_prefer_free_tier()` (env var `VOX_RESEARCH_PREFER_FREE_TIER`), only reorders candidates *within* the OpenRouter lane — it cannot express "prefer my free Groq key over my paid OpenRouter model," and isn't exposed in the GUI.

On the memory note's "defaults to `openrouter/auto`" claim: the fallback constants have moved on (`RESEARCH_FLASH_FALLBACK = "google/gemini-3-flash"`, `REVIEW_PREMIUM_FALLBACK = "anthropic/claude-sonnet-4.6"` in `bootstrap_inference.rs`) but both are still paid, non-free identifiers — the underlying "research defaults to paid" gap persists with different specifics than the 2026-06-17 audit saw.

### What this means for the design

Two concrete, well-scoped fixes, independent of each other:

1. **Route research stage selection through `decide()` instead of the bare `select()`/`select_with_default_registry` cascade**, or port `key_guard::available_inference_providers()` into `select_via_scorer`'s filter closure — either closes the Layer 2 gap.
2. **Make `cascade_for_research_stage` provider-aware** using `ModelSelectionDecision.provider_route` (`ModelRouteBackend`) and each provider's `ChatRouteBackend`, which the chat path (`model_resolution.rs`) already has plumbing for — `vox-actor-runtime::llm::cascade` just doesn't reuse it. This is the fix that actually lets a configured Groq/Gemini/Mistral key get used at all for research.

Both are prerequisites for any "intelligent, key-aware, free-tier-preferring" routing to take effect — today the selection layer can nominally "want" a non-OpenRouter model for a stage, but the egress layer has no way to call it.

## Part B — Free/low-cost LLM API tiers beyond OpenRouter (surveyed 2026-08-01)

| Provider | 2026 free-tier limits | Quality tier | Best-fit research stage |
|---|---|---|---|
| **Google AI Studio (Gemini API)** | Permanent free tier for Flash/Flash-Lite; 2.5 Pro capped ~50 req/day; Flash-family ~5–15 RPM, 100–1,000 req/day. Google may train on free-tier prompts/outputs. | Flash-Lite = cheap classifier-grade; Flash = solid mid-tier; Pro rate-starved on free. | Flash-Lite → NLI/claim classification; Flash → query planning. |
| **Groq** | 30 RPM, ~6,000 TPM, 1,000–14,400 req/day; org-level; no card required. | Open models (Llama, Kimi) at very high tok/s via LPU hardware — speed over frontier reasoning. | Planner/query-decomposition and claim-extraction where latency matters more than max depth. |
| **Cerebras** | Figures vary by source/account: recent docs cite 5 RPM/30K TPM/1M tokens-per-day on `gpt-oss-120b`/`GLM-4.7`; older trackers cite 30 RPM/60–100K TPM with an 8K context cap. No card required; verify per-account. | Very fast wafer-scale inference; open-model roster similar to Groq. | Same profile as Groq; 8K context cap (if applicable) rules out synthesis over long evidence sets. |
| **Mistral (La Plateforme)** | "Experiment" tier: free, all models including Mistral Large/Codestral, ~1B tokens/month soft cap, "for evaluation not production." | Mistral Large = strong general reasoning. | Plausible for synthesis/judge given reasoning quality, if cap/RPM acceptable. |
| **Together AI** | Not rate-limited — sign-up **credit** (up to $100, depleting). | 100+ open models, OpenAI-compatible API. | Burst/overflow lane once Groq/Cerebras free RPM exhausted; treat as "cheap," not "free." |
| **HuggingFace Inference Providers** | Free accounts: well under $0.10/month effective spend; ZeroGPU Spaces ~3.5 min/day H200, not the router API. | Router re-exposes third-party providers' open models; quality varies. | Too thin for any recurring pipeline stage; manual testing only. |
| **Cohere** | Trial key: 1,000 free calls/month, 20 RPM chat/5 RPM embed; barred from production/commercial use. | Command R+ generalist/RAG-oriented; Rerank 3.5/Embed 4 useful for retrieval-quality steps. | Lowest-volume stage (LLM-judge), or Rerank endpoint augmenting evidence ranking. |
| **Ollama (local)** | No rate limit — bounded by local hardware; zero marginal cost, works offline. | Whatever open-weights model is pulled locally. | Already Vox's first-choice fallback lane; universal floor for planner/claim-extraction, can serve synthesis when no cloud key is configured. |

**Design implication:** query-planning/claim-extraction is well-served by Groq or Cerebras free tiers (fast, tolerant of tight RPM); synthesis and the judge pass benefit from a stronger tier like Gemini Flash or Mistral Experiment rather than a rate-starved open-model lane. This per-stage tiering only becomes actionable once Part A's egress-cascade fix lands — today the selection layer can't get a non-OpenRouter model dispatched at all.

## Part C — Packaging deep research as an ad-hoc skill

Vox already has a skill marketplace (`crates/vox-skills/`: `SkillRegistry`, `SkillManifest`/`SkillCategory`/`SkillPermission`, `VoxSkillBundle`/`SKILL.md` parser, `PluginManager`, container-sandboxed execution) and an advisory-only local mining/dedup engine (`crates/vox-skill-discovery/`) that flags MCP↔skill drift without auto-installing. This is the natural home for exposing "run a deep research task" as an ad-hoc, discoverable capability rather than only a `vox research run` CLI invocation or an internal orchestrator stage.

Packaging considerations (design-level; exact `SkillManifest` field names should be verified against `crates/vox-skills/src/skill_manifest.rs` — re-exported from `vox_plugin_host` — during implementation, not assumed here):

- **Permissions**: a research skill needs outbound network access (web search/fetch), LLM invocation (already model-agnostic via `vox_actor_runtime::llm`), and read access to prior findings for novelty scoring. It should *not* need arbitrary filesystem write — output should flow through the existing `ResearchEvent`/`FindingCandidateV1` event pipeline into `vox-db`, not raw file writes, so the sandboxed execution model isn't weakened for a documentation-adjacent capability.
- **Invocation shape**: the skill's `SKILL.md` would wrap the existing `vox research run`/`run_research_with_context` entry points (`crates/vox-cli-research`), giving it a natural-language-triggerable surface (`/research <question>`) consistent with how other skills in this registry are invoked, without duplicating the pipeline logic.
- **Trust/provenance**: per the skills-registry-trust-and-curation research already on file (`docs/src/architecture/skill-registry-trust-and-curation-research-2026-07-30.md`), any first-party skill wrapping a core capability like this should ship signed/first-party rather than going through the same provenance bar as a third-party-contributed skill — this needs reconciling with that doc during implementation rather than assumed here.

This is scoped intentionally light — the skills-packaging question deserves its own focused pass once the underlying pipeline gaps (novelty scoring, confidence gate, multi-provider routing) are actually closed, since packaging a stub-heavy pipeline as a discoverable skill would just surface the stubs to more callers.

## Part D — Handoff to SCIENTIA / VoxGiantia for auto-publication

Vox's self-publication system (SCIENTIA) already defines the lifecycle a research finding needs to pass through before publication: `draft` → `publication-prepare` → discovery/evidence refresh → preflight/approvals → scholarly submit → status sync (per `docs/src/reference/scientia-ssot-handbook.md`), backed by `vox-db` tables (`publication_manifests`, `publication_approvers`, `publication_attempts`/`external_submission_jobs`) and channel adapters (RSS, Twitter, GitHub, Reddit, HN, YouTube, arXiv/Zenodo/OpenReview via scholarly adapters) — this is the "write once, publish many" model documented in the archived VoxGiantia publication-architecture doc.

The natural handoff point is exactly the trust/novelty gating this research phase already maps in [deep-research-trust-novelty-scoring-landscape-2026-08-01.md](deep-research-trust-novelty-scoring-landscape-2026-08-01.md): a `FindingCandidateV1` that clears `WorthinessSignalsV2`'s hard/soft gates (once populated per that doc) is the correct trigger for promotion into a `publication_manifest` draft, rather than every research run attempting to auto-publish. This keeps the "reputational firewall" SCIENTIA's finalization plan already commits to intact — auto-publication should be gated on the *same* trust/novelty signals this program is building, not a separate, weaker check. Concretely: closing the novelty-scoring and confidence-gate stubs (Part of the trust/novelty doc) is a **prerequisite** for safely wiring automatic publication, not a parallel, independent workstream — publishing un-verified, non-novel, or low-trust findings automatically would be strictly worse than the current human-gated status quo.

## Sources (Part B)

- [Gemini API Free Tier 2026: Limits, Quotas, and What You Actually Get](https://pecollective.com/tools/gemini-free-tier-guide/)
- [Gemini API Rate Limits 2026: Complete Per-Tier Guide with All Models](https://www.aifreeapi.com/en/posts/gemini-api-rate-limits-per-tier)
- [Groq API Free Tier Limits in 2026 - Grizzly Peak Software](https://www.grizzlypeaksoftware.com/articles/p/groq-api-free-tier-limits-in-2026-what-you-actually-get-uwysd6mb)
- [Groq Free Tier Limits 2026 - TokenMix Blog](https://tokenmix.ai/blog/groq-free-tier-limits-2026)
- [Rate Limits - Cerebras Inference (official docs)](https://inference-docs.cerebras.ai/support/rate-limits)
- [Cerebras Free Tier 2026 | Price Per Token](https://pricepertoken.com/endpoints/cerebras/free)
- [Mistral AI Free Tier 2026 | Price Per Token](https://pricepertoken.com/endpoints/mistral/free)
- [Mistral API Free Tier & Pricing in 2026 | Perkstack](https://perkstack.co/blog/mistral-api-free-tier)
- [Together AI Free Credits 2026 | Get AI Perks](https://www.getaiperks.com/en/ai/together-ai-free-credits-2026)
- [Hugging Face pricing explained: what you actually pay in 2026 | eesel AI](https://www.eesel.ai/blog/hugging-face-pricing)
- [Cohere Trial Key Pricing and Limits Summary](https://codenote.net/en/posts/cohere-trial-api-key-pricing-and-limits/)
- [Different Types of API Keys and Rate Limits | Cohere (official docs)](https://docs.cohere.com/docs/rate-limits)
- [OpenRouter Rate Limits (official)](https://openrouter.zendesk.com/hc/en-us/articles/39501163636379-OpenRouter-Rate-Limits-What-You-Need-to-Know)
