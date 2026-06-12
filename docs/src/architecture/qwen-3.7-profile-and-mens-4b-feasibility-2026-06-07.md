---
title: "Qwen3.7 profile + MENS 4B-on-RTX-4080 feasibility (revised)"
description: "Verified June-2026 research on the Qwen3.7 release (closed-weight, API-only) and a codebase-grounded revision of whether a 4B model can be QLoRA-fine-tuned on a 16 GB RTX 4080 in the MENS pipeline."
category: "Architecture SSOTs"
---

# Qwen3.7 profile + MENS 4B feasibility (revised) — 2026-06-07

Two questions drove this note:

1. What is Qwen3.7 (released ~May 2026) and is any part of it usable as a local fine-tuning target?
2. The user's first MENS pipeline run at **4B** spiked above 16 GB and crashed on an **RTX 4080 (16 GB)**. Is that fixable at a reasonable sequence length, or does the earlier "4B fits comfortably" advice need revising?

Both were researched with the deep-research harness (adversarial 2-of-3 verification) and, for Q2, cross-checked against the actual MENS codebase. **The short answer to both: Qwen3.7 is closed and undownloadable, and 4B is not viable for QLoRA on a 16 GB 4080 — the pipeline already encodes this and auto-retreats to 2B.**

---

## Part 1 — Qwen3.7 release profile (verified, June 2026)

### Bottom line
Qwen3.7 is a **two-model, proprietary, API-only family**. There are **no open weights, no small/distilled variants, and no quantizations of any kind** on Hugging Face or ModelScope. It cannot be downloaded, self-hosted, or fine-tuned. It is irrelevant as a local training target.

### The family
| Model | Modality | Role |
|-------|----------|------|
| **Qwen3.7-Max-Preview** (API id `qwen3.7-max`) | Text-only (no image input) | Flagship reasoning / agentic / coding model |
| **Qwen3.7-Plus-Preview** (API id `qwen3.7-plus`) | Multimodal (text + image) | Cheaper vision-capable sibling |

No separate coder / thinking / omni 3.7 SKU — agentic and coding strength is built into Max. Both carry "Preview" status. Announced at the Alibaba Cloud Summit **May 20, 2026** (DashScope API live May 19); Plus reached GA ~June 1–3, 2026.

### Capabilities
- **Context window: 1,000,000 tokens** (vendor-stated; ~991.8K input + 65.5K output), up from 256K on Qwen3.6 Max. No independent effective-context benchmark exists yet.
- **Max strengths:** coding (frontend prototyping → complex SWE), office/productivity & workflow automation via MCP + multi-agent orchestration, sustained long-horizon autonomous execution, explicit prompt caching. Alibaba markets a 35-hour kernel-optimization run (1,158 tool calls, 10× speedup).
- **Leaderboard placement:** Max ~#13 text arena; Plus ~#16 vision arena (Alibaba claimed #6 lab in Text, #5 in Vision). **Head-to-head benchmark numbers (SWE-bench/LiveCodeBench, AIME/MATH, GPQA, MMLU-Pro) were NOT verifiable** — only arena placements survived adversarial checking.

### Availability & access (all closed)
- **Closed-weight / proprietary.** No QwenLM/Qwen3.7 GitHub repo (newest open line is Qwen3.6). Nothing labeled "3.7" in the official Qwen HF org or Qwen3 collection. No GGUF/AWQ/GPTQ/MLX from Qwen or third parties (unsloth, bartowski, mradermacher, DavidAU — all their 3.x quants are 3.5/3.6).
- **The only "3.7" artifact on Hugging Face** is a third-party community **dataset**, `armand0e/qwen3.7-max-pi-traces` (model output traces, *not* weights).
- **Access paths:** Alibaba Cloud Model Studio / DashScope (both OpenAI-compatible and Anthropic-compatible endpoints — e.g. `ANTHROPIC_BASE_URL=https://dashscope-intl.aliyuncs.com/apps/anthropic` for Claude Code), OpenRouter (`qwen/qwen3.7-max`, `qwen/qwen3.7-plus`), and reportedly Together AI.

### Pricing (per 1M tokens; promotional, time-sensitive)
| Model | Input | Output | Cached input |
|-------|-------|--------|--------------|
| Qwen3.7-Max | **$1.25** (50% off $2.50 list) | **$3.75** (50% off $7.50) | ~$0.25 |
| Qwen3.7-Plus | **$0.40** | **$1.60** | ~$0.08 |

Refuted (do not use): flat $2.50/$7.50 as the *current* Max price (that's the pre-discount list); any context-tiered pricing for Plus.

### License / open-source plans
No license for downloadable weights exists (there are none). **No announced plan to open-source the 3.7 weights.** Pattern: Qwen3.6 and earlier stayed Apache-2.0 open; the Max/Plus flagship tier went proprietary.

---

## Part 2 — Can the MENS pipeline train a 4B model on a 16 GB RTX 4080? (REVISED)

### Revision of prior advice
An earlier statement that "4-bit QLoRA on a 4B base is comfortably feasible in 16 GB" was **wrong for this setup**. It came from generic web guides and contradicts both the user's measured crash and the pipeline's own calibration. **Corrected conclusion: 4B is not viable for QLoRA training on a 16 GB 4080 in the current pipeline.**

### Why — it's the resident weight footprint, not sequence length
The OOM is already baked into the pipeline as a hardware-calibrated constant in
[`crates/vox-populi/src/mens/tensor/memory_budget.rs`](../../../crates/vox-populi/src/mens/tensor/memory_budget.rs):

> *"Calibrated from hardware: a Qwen3.5-4B QLoRA run OOMed on a 16 GiB RTX 4080 Super **even at seq 128**, so resident(4B) must exceed ~15.5 GiB."* (lines 27–31)

- `RESIDENT_GIB_PER_B_PARAMS = 3.5` ⇒ resident(4B) ≈ **15.6 GiB** (rejected on 16 GiB; fits 24 GiB); resident(2B) ≈ **8.6 GiB** (comfortable on 16 GiB).
- `FIXED_OVERHEAD_GIB = 1.6` (CUDA context, cuBLAS, allocator slack).
- Because it **OOMed even at seq 128**, the binding constraint is the resident footprint of the base model, not transient activation/attention spikes. **Reducing sequence length cannot rescue 4B** — base weights + overhead alone (~17.2 GiB) already exceed 16 GB before a single token of activations.
- Sequence length *is* the dominant lever, but only for models whose resident footprint leaves headroom (e.g. 1.5B/2B). See `ACT_GIB_PER_KTOK_PER_SQRTB = 9.5` (memory_budget.rs:40–46).

The pipeline handles this correctly today: it **auto-retreats 4B → 2B** on 16 GB cards via `QWEN35_LADDER` / `plan_qwen35()`.

### What knobs exist (and what's missing)
**Present:** `--seq-len`, `--batch-size`, `--grad-accum`, `--vram-limit-fraction`, `VOX_MENS_VRAM_SAFETY` (0.5–0.98, default 0.88), `VOX_MENS_NO_WEIGHT_CACHE=1` (drops BF16 weight cache, saves ~2 GiB, ~1 step/s slower), NF4 base weights + double-quant (default on), CUDA pool trimming, `vox mens probe`, conservative `qwen_4080_16g` preset (seq 384 / batch 1 / grad_accum 8), and the auto-retreat ladder. Crates: `vox-plugin-mens-candle-cuda` (trainer), `vox-populi` (budget/probe), `vox-ml-cli` (`vox mens train` / `vox mens probe`).

**Missing — the one feature that could matter for 4B:** **gradient checkpointing / activation recompute** is *not* implemented (activations stay F32 and are retained across all layers for backward). Also absent: fp16/bf16 activations, paged/8-bit optimizers, flash-attention/SDPA selection, runtime OOM retry, and a forward-backward preflight "dry run".

**Even with gradient checkpointing, 4B stays borderline** because checkpointing reduces *activation* memory, while the killer here is *resident* (~15.6 GiB). Making 4B fit would require both gradient checkpointing **and** resident-footprint surgery (e.g. never materializing a BF16 weight cache, more aggressive quant) — a feature project with marginal payoff on 16 GB.

### Recommendation
1. **Use `Qwen/Qwen3.5-2B`** as the realistic upgrade from the old 1.5B. Resident ≈ 8.6 GiB — comfortable on the 4080 with real seq-len headroom, and a genuine capability jump over 1.5B. The pipeline already retreats to it automatically.
2. **Do not target 4B locally** on the 4080 unless someone first lands gradient checkpointing in `vox-plugin-mens-candle-cuda` (and even then it's borderline).
3. **For 4B specifically**, the clean path is a **24 GB card** (4090 / A5000) — the budget already says 4B fits at 24 GiB.
4. **Qwen3.7 is not an option** for any local training — closed, undownloadable, API-only.

---

## Caveats & open questions
- Qwen3.7 primary surfaces (qwen.ai blog, Model Studio catalog) are JS-rendered and weren't directly scrapable; specs (1M context, dates) rest on convergent secondary reporting + the fetchable Alibaba Cloud blog "Qwen3.7: The Agent Frontier". The **negative findings** (no weights / no quants / no repo) are the most reliable here — direct primary-source absence checks — but "absence" is as-of June 2026.
- No verified cross-competitor benchmark numbers for 3.7; only arena placements.
- Unconfirmed: whether Qwen ever open-sources a distilled 3.7-small; whether Plus uses context-tiered pricing on Model Studio directly; ModelScope was not independently confirmed beyond HF.
