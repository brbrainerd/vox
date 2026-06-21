---
title: "VoxMens Fine-Tuning Boundaries — Research Findings (2026-06-21)"
description: "Deep-research evidence on where fine-tuning a resource-scalable open-weight LLM is worth it vs not, for the VoxMens hub-and-spoke design: hub base-model choices, the harness/tool-use generalization boundary, low-resource DSL adaptation, multi-adapter serving, and whether a fine-tuned ~32B can reach Flash/Sonnet tier."
category: "Architecture SSOTs"
status: "current"
---

# VoxMens Fine-Tuning Boundaries — Research Findings (2026-06-21)

Evidence base for deciding **what VoxMens should fine-tune (spokes), what it should start from (hubs), and where the fine-tuning boundary lies** now that we are no longer constrained to a single RTX 4080 SUPER / 32B ceiling. This document is research findings only; the design decision and execution plan follow in brainstorm → plan.

## Method & verification status (read first)

Produced by the `deep-research` workflow (6 angles → 25 primary/secondary sources → 100 extracted claims → top 25 surfaced). **The adversarial-verification phase was 100% rate-limited** (108 concurrent agents tripped the server-side throttle; every vote returned `0-0 abstain`, which the harness mislabeled as "refuted"). Per the known failure mode, **these are treated as UNVERIFIED, not refuted.** The decision-critical claims were then **re-verified via targeted single fetches in small clusters** (which did not trip the limit).

Legend: ✅ **verified** (re-fetched 2026-06-21) · 🟡 **unverified** (extracted from a credible primary source; rate-limited before adversarial check — treat as a lead, confirm before betting the design on it) · 🟠 **partial** (core number verified, an inference around it not).

> Re-running the full 108-agent verify into the same wall is not worth it. Remaining 🟡 claims should be confirmed with 2–3 targeted fetches each if/when they become load-bearing.

## Headline synthesis — the fine-tuning boundary

The evidence points to a clean split that should drive the spoke architecture:

- **Fine-tune for *capability/syntax the base model lacks*** (a novel DSL, a low-resource language, domain idiom) — here fine-tuning/RL **beats retrieval**, because retrieval can only surface context the model still can't *internalize* (✅ 2503.18760). This is the case for **Vox** and (to a lesser degree) **Rust** spokes.
- **Do *not* fine-tune for *the catalog of tools/skills/plugins*** — tool-call orchestration should generalize to **new tools without retraining** via schema-grounding + tool **retrieval/RAG** (🟡 2509.20415; 🟠 2512.17052 shows retrieval alone is worth +23–104%). Fine-tune the *harness skill* (how to call tools, plan, recover) **once**, on the tool-calling *behavior*, not per-tool.
- **A fine-tuned small/mid model can reach or beat frontier-tier on a *narrow* domain** (✅ xLAM #1 on BFCL over GPT-4/Claude-3; ✅ Agnostics lifts Qwen3-4B to 16-70B-rival on low-resource code) — validating "Flash/Sonnet-tier at ~32B" as realistic **per-domain**, not as a general-intelligence claim.

## 1. Hubs — candidate base models by capability type

- 🟡 **Qwen3 offers a resource-scalable dense ladder** (0.6B, 1.7B, 4B, 8B, 14B, 32B) plus MoE (30B-A3B, 235B-A22B), each evaluated on BFCL in both native function-calling (FC) and prompt modes. *(source: Qwen3-Coder repo SUPPORTED_MODELS.md — primary)* → strongest candidate for a **single shared dense ladder** that lets intelligence scale up/down by local VRAM.
- 🟡 **Qwen3-Coder exposes a native FC mode** (built-in tool/function calling via dedicated definitions) distinct from prompt mode. *(Qwen3-Coder repo — primary)* → relevant to whether the harness can lean on the base model's native tool-call formatting.
- 🟡 For low-resource/domain languages, the literature's **fine-tuning approaches predominantly use open-weight families (LLaMA, DeepSeek, StarCoder)**; prompting-only approaches lean on proprietary GPT. *(2410.03981 — primary)*

> Note: blog rankings of "best agentic-coding open models 2026" were retrieved (benchlm.ai, mindstudio.ai, seldo.com) but are low-trust; do not cite for model selection without a primary BFCL/SWE-bench check.

## 2. The harness / tool-use generalization boundary

- 🟠 **Retrieval-based tool selection is independently worth +23–104%** over static retrievers (DTDR). *(✅ number verified, 2512.17052)* — the abstract does **not** itself assert "new tools without retraining"; that premise is sourced below.
- 🟡 **Tool use can be driven by RAG (embed query ↔ tool descriptions) with no change to the LLM**, and **a retrieval-based selector supports dynamic tool inventories — new tools added/selected without retraining.** *(2509.20415 — primary; the core "no-retrain for new tools" lead)*
- 🟡 **On-device/local agents use a retrieval module to select relevant tools** (improves accuracy, cuts context length) rather than packing all tools in context or fine-tuning per tool. *(2512.17052 — primary)*
- 🟡 **Decoupling tool-calling into (tool-selection) + (argument-generation), each a LoRA adapter, improved a 7B's tool-calling by 46% on MCP-Bench**, beating same-size and most 2× models. *(2510.00229 — primary)* → if we DO fine-tune the harness, decomposing it is the lever.
- 🟡 **Untuned open-weight models consistently underperform frontier on tool-calling**, failing specifically at (a) selection from large tool sets and (b) complex argument structures. *(2510.00229 — primary)* → these two failure modes are exactly what retrieval (a) and a thin harness fine-tune (b) target.

**Implication for "harness" spoke:** keep tools/skills/plugins out of the weights; serve them via a retrieval+schema layer. Reserve harness fine-tuning for the *behavior* (selection discipline + argument fidelity + recovery), which generalizes across catalogs.

## 3. SoTA — fine-tuning for function-calling / agentic generalization

- ✅ **xLAM (fine-tuned, open-weight, 1B–8x22B) reached #1 on the Berkeley Function-Calling Leaderboard, outperforming GPT-4 and Claude-3 on tool use.** *(2409.03215, abstract re-verified 2026-06-21)* — strongest evidence that a purpose-built tool-use spoke can exceed frontier on the narrow domain.
- 🟡 **A 7B/14B trained with pure RL (binary reward) can outperform GPT-4o on major function-calling benchmarks**, and **pure RL need not underperform SFT-then-RL** (distilled SFT trajectories may be unnecessary). *(2505.00024 — primary)* → RL-from-verifier is a viable harness training path, not just SFT.

## 4. Low-resource / novel-language (Vox, Rust) adaptation

- ✅ **Fine-tuning on synthetic textbook-quality demonstrations beats standard RAG** for an unfamiliar low-resource language (Excel Formulas); retrieval gives "only modest improvement" because the model can't *internalize* novel-domain knowledge from retrieved context alone. *(2503.18760, re-verified)* — **the core reason Vox needs a spoke, not just retrieval.**
- ✅ **Agnostics (language-agnostic RL post-training) lifts Qwen3-4B to rival 16B–70B models on 5 low-resource languages (Lua, Julia, R, OCaml, Fortran) without per-language data/engineering.** *(2508.04865, re-verified)* — a fine-tuned *small* model punches far above its class on narrow coding, and a **single RL environment can cover many languages** (cuts per-language curation).
- 🟡 **MultiPL-T**: generate synthetic low-resource training data by translating + test-validating from a high-resource language, filtering faulty/low-coverage items; fine-tuned StarCoderBase/Code-Llama then beat other open models on MultiPL-E. *(2308.09895 — primary)* → concrete data-synthesis recipe for Vox/Rust.
- 🟡 **Measurable Python≫Rust/R capability gap** for most models, and **scarce training data for Rust/R + DSLs (Ansible, Verilog)** motivating synthesis/transfer. *(2410.03981 — primary)*

## 5. Multi-adapter serving topology

*(sources retrieved but not yet re-verified — 🟡 throughout; LoRAX, vLLM LoRA docs, AWS SageMaker multi-LoRA, 2311.03285, 2310.18547 are all primary/secondary)*

- 🟡 **Shared-base + hot-swappable LoRA adapters** is a supported, cost-effective multi-tenant serving pattern (LoRAX; vLLM dynamic LoRA; SageMaker multi-tenant LoRA). → favors **one hub + many spoke adapters** when spokes share a base, with per-domain adapter hot-swap rather than N full servers.
- Open question for the plan: this only works if Vox/Rust/harness/conversation spokes **share a base**. If a spoke needs a *different* base (e.g., a code-specialized vs chat-specialized hub), it needs a separate server — a real cost the topology decision must weigh.

## 6. Can a fine-tuned ~32B reach Flash / Sonnet tier?

- **Narrow-domain: yes (evidence-backed).** ✅ xLAM (#1 BFCL > GPT-4/Claude-3) and ✅ Agnostics (4B → 16-70B-rival on low-resource code) both show fine-tuned small/mid models exceeding much larger/frontier models *on the targeted domain*.
- **General-purpose: not supported here.** No retrieved source claims a fine-tuned 32B matches Flash/Sonnet across the board. The realistic framing is **per-spoke parity on its domain**, with intelligence scaled by picking a larger hub rung when local VRAM allows.
- **Eval anchor:** 🟡 **BFCL (UC Berkeley Gorilla), V4 (≈2026-04), holistic/multi-turn agentic** is the primary tool-use benchmark; MultiPL-E for low-resource code. *(gorilla.cs.berkeley.edu — primary)* → use BFCL for the harness spoke and MultiPL-E (+ our own Vox/Rust compile-rate gates) for code spokes.

## Sources (25; quality as classified by the harness)

Primary: `gorilla.cs.berkeley.edu/leaderboard.html`; Qwen3-Coder `SUPPORTED_MODELS.md`; arXiv 2509.20415, 2410.03981, 2510.00229, 2505.00024, 2512.17052, 2409.03215, 2308.09895, 2503.18760, 2508.04865, 2311.03285, 2310.18547, 2508.09883; `github.com/predibase/lorax`; `docs.vllm.ai/.../lora`; arXiv 2511.22880; `thinkingmachines.ai/blog/on-policy-distillation`. Secondary: themoonlight.io, emergentmind.com (BFCL), AWS SageMaker multi-LoRA. Low-trust (excluded from load-bearing use): benchlm.ai, mindstudio.ai, seldo.com, premai.io.

## Open items to confirm before the plan bets on them

1. 🟡 Qwen3 dense ladder + native-FC claim (re-fetch Qwen3 repo) — drives the hub choice.
2. 🟡 The "new tools without retraining via retrieval" premise (re-fetch 2509.20415 full text) — the keystone of the no-fine-tune-harness thesis.
3. 🟡 Multi-adapter serving viability on our stack (LoRAX/vLLM) — drives shared-base-vs-separate-servers.
4. Whether Rust truly needs a *separate* spoke from Vox, or a shared code base + two adapters suffices (Agnostics suggests one RL env may cover both).
