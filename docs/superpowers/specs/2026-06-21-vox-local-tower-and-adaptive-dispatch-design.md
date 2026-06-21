---
title: "Vox Local Tower + Resource-Adaptive Dispatch — Design"
description: "Sweet-spot local tower (Threadripper 24c + 2x RTX 3090, ~$6.3k) plus a Vox resource-adaptive model-dispatch architecture: probe hardware, pick a precision-ladder VoxMens variant, route latency-bound work local and quality-bound work to cloud/OpenRouter."
category: "architecture"
status: "research"
---

# Vox Local Tower + Resource-Adaptive Dispatch — Design

> Terminal of a brainstorming session. Two halves: a **hardware** half (the tower — a buy/build,
> not code) and a **software** half (resource-adaptive dispatch in Vox — the part that flows to
> `writing-plans` → implementation). Grounded in measured build numbers and verified 2026 prices.

## 1 · Goals & constraints
- **Speed up Rust builds** (the 116-crate workspace) and **run VoxMens locally** for low-latency harness work.
- **Offload routine LLM work off the cloud** (Gemini Flash / Sonnet tier) to a local model → cut OpenRouter spend; escalate to Opus 4.8 / Fable 5 / Claude Code only for the hard slice.
- **Run on a variety of systems** — the same VoxMens artifacts must scope/scale from a laptop → this tower → cloud.
- **Budget ≤ $15k (expandable);** the chosen tower is **~$6.3k**, leaving headroom.

## 2 · Hardware — the sweet-spot tower (web-verified prices, 2026-06-21)
Two CPU tiers: **value (24c)** and **"fastest at this price" (32c)**. Real currently-purchasable listings (Newegg/Micro Center/eBay), not historical averages.

| Part | Pick | Real price |
|---|---|---|
| CPU (value) | **Threadripper 9960X** 24c/48t (sTR5) | ~$1,499 |
| CPU (fastest) | **Threadripper 9970X** 32c/64t — swap in for ~15–20% faster full builds | $2,299 |
| Motherboard | **ASRock TRX50 WS** (PCIe 5.0, multi-GPU, ECC) — or Gigabyte TRX50 Aero D $599 / ASUS SAGE $899 | ~$799 |
| RAM | **128 GB DDR5-5600 ECC RDIMM** (Kingston Fury Renegade Pro 4×32) | ~$850 |
| GPU | **2× used RTX 3090 24 GB → 48 GB** (best $/VRAM) | ~$2,300 |
| Boot + Dev Drive NVMe | Crucial T705 2 TB Gen5 | ~$240 |
| Models/bulk NVMe | Samsung 990 PRO 4 TB | ~$350 |
| PSU | 1300 W Platinum, ATX 3.1 (mandatory for 2×3090) | ~$250 |
| Cooling | sTR5 360 mm AIO | ~$200 |
| Case | high-airflow full tower (dual-GPU) | ~$250 |
| **Total (24c value)** | | **~$6,740** |
| **Total (32c fastest)** | | **~$7,540** |

> Was ~$6,289 in v1 — corrected up by used-3090 price rise (~$950→~$1,150 ea) + real board/RAM. **Tip:** Micro Center sells CPU+TRX50+128 GB *bundles* that can shave the platform cost.

- **Power:** ~900 W under full build+inference load (2× ~350 W GPU + ~250 W CPU + rest).
- **GPU choice (verified):** 2×3090 (48 GB, ~$2,300) beats a single **RTX 5090** (32 GB, $2,909–4,329 — pricier *and* less VRAM) and **RTX 6000 Ada** (48 GB, $6,800) on value. If you find **used RTX 4090s ~$1,100–1,400**, **2×4090** is the upgrade (48 GB, faster, similar price, newer).
- **CPU choice:** for the stated *"fastest Rust build"* goal, the **9970X 32c** is the honest pick (more cores = faster full builds); the 9960X 24c is the value floor. The 64c 9980X ($4,999) is fastest but breaks "middling."

**Expansion (corrected):** **TRX50 cleanly runs 2 GPUs at full lanes + headroom for a 3rd at reduced lanes** (it is the non-PRO platform). For **4+ GPUs or the future 96 GB Blackwell serving card**, the right platform is **WRX90 (Threadripper PRO, 128 lanes)** — i.e. the ~$18k build. So: build on TRX50 if 2–3 GPUs is the ceiling; start on WRX90 if you *know* you'll scale to 4 GPUs / public serving.

**Durability & risk (real):** used 3090s = no warranty, possible mining wear, known GDDR6X memory-heat (budget a spare and a repad/repaste); ~900 W heat/noise. **Mitigations:** buy 1 newer + 1 used, prefer 2×4090 if cheap, or **go prebuilt** — a warrantied dual-GPU Threadripper from **Custom Lux / AVADirect / Puget runs ~$7,500–9,000** (+15–30% over DIY) and removes used-card + assembly + validation risk (burn-in tested, multi-year warranty). The DIY value build wins on cost; the prebuilt wins on reliability/support.

## 3 · Software — resource-adaptive dispatch (the implementable part)
Extends existing Vox machinery; **no new parallel system**:
- reuse the **model-agnostic facade** `vox_actor_runtime::llm` (`infer_with_retry`/`llm_chat`/`llm_stream`/`llm_embed`),
- the scorer `vox-orchestrator::models::{registry,select,autonomic}`,
- **ADR-043** quantized-safetensors on-disk format,
- the planned **`spokes.yaml`** SSOT (per-spoke base/method/adapter/router).

### 3.1 Units
| Unit | Responsibility | Interface |
|---|---|---|
| **CapabilityProbe** | Detect at runtime: CPU cores, GPU model(s), total VRAM, RAM, CUDA present | `probe() -> HwProfile` |
| **VariantCatalog** | The precision-ladder artifacts produced by cloud fine-tune (see §3.2), each tagged with min-VRAM + tokens/s | reads a manifest (registry entry per variant) |
| **VariantSelector** | Given `HwProfile` + task, pick the largest VoxMens variant that fits; else "no local" | pure fn → `Choice{local_variant?|cloud}` |
| **DispatchRouter** | Decide local vs cloud/OpenRouter per task class (see §3.3); inject VoxScript RAG context on cloud offload | wraps the facade |
| **(reuse) Scorer/Registry** | Cost/latency/quality scoring across candidates | existing |

### 3.2 The precision ladder (cloud fine-tune output)
Cloud fine-tuning (RunPod/Vast, Axolotl/QLoRA — Python tooling, independent of Vox being Rust) emits **one VoxMens base (32B) + per-spoke LoRA adapters**, exported at multiple precisions so the same model runs across the hardware spectrum:
| Variant | Fits | Use |
|---|---|---|
| 32B **FP16** | ≥ ~70 GB (cloud / 96 GB box) | max quality |
| 32B **FP8** | ~40 GB (48 GB tower) | tower default — near-lossless |
| 32B **AWQ-4bit** (~20 GB) | 24 GB cards / 4080 | laptops, fallback, many-users |
| 7B/14B spoke adapters | tiny | light spokes; sub-100 ms micro-tasks |
Served via **vLLM multi-LoRA** (one base + many spokes on one GPU); ADR-043 is the on-disk format.

### 3.3 Dispatch policy (local ↔ cloud)
- **Latency-bound micro-tasks** (route / classify / retrieve-augment / quick VoxScript gen) → **local** smallest-fitting variant (sub-100 ms round-trip; no network).
- **VoxScript-specific work** → **local 32B VoxMens** (it natively knows VoxScript).
- **Quality-bound / hard tasks** → escalate: local 32B → **Sonnet 4.6** → **Opus 4.8 / Fable 5 / Claude Code**, *with retrieved VoxScript context injected* so frontier models are competent without retraining.
- **No GPU / low VRAM host** → route everything to cloud/OpenRouter (the ladder degrades gracefully).
- The **scorer** picks among candidates on latency × cost × quality × locality (existing axes).

### 3.4 Train-time vs retrieval-time (so you never retrain to add a skill)
- **Bake into training (spokes):** VoxScript syntax/idioms, Rust-on-our-codebase patterns, agentic/tool-use *style*. Stable, high-value, slow-changing.
- **Retrieve dynamically (no retrain):** the **skills/tools catalog** (which tools exist + their schemas), codebase facts, and any fast-changing capability. Surface via the existing `vox skill list/search` + tool registry + `vox-search` (tantivy + semantic + RRF) as **context injection / tool-RAG** at inference — adding a skill is a registry/index update, not a fine-tune.

## 4 · Build acceleration (on the tower)
Linux + **mold** + **sccache** + `CARGO_HOME`/Dev set on Gen5 NVMe. Measured/estimated:
| | current i9 (Win, no mold) | this tower (24c, Linux+mold) |
|---|---|---|
| Full workspace | 243 s (measured) | ~150 s (est, ~40% faster) |
| Cascade (core crate) | 110 s (measured) | ~55 s (est, ~50% faster) |
| Incremental | 4.5 s (measured) | ~3.5 s |

## 5 · Economics (verified OpenRouter prices)
Local 32B absorbs **all Gemini Flash ($0.50/$3) + Haiku ($1/$5) + ~70–80% of Sonnet ($3/$15)** code tasks; escalate only to Opus 4.8 ($5/$25) / Fable 5 ($10/$50) / Claude Code.
| Your offloadable Flash+Sonnet $/mo | Saved/mo (~80%) | Tower payback (~$6.3k) |
|---|---|---|
| $300 | $240 | ~26 months |
| $800 | $640 | ~10 months |
| $2,000 | $1,600 | ~4 months |
Plus ~$100–200/mo-equiv in saved build/dev time. **Pays back < 2 yr at ≥ ~$500/mo offloadable spend.**

## 6 · Testing
- CapabilityProbe: unit tests with mocked `HwProfile` (no-GPU, 24 GB, 48 GB, cloud-only).
- VariantSelector: table tests (each VRAM tier → expected variant).
- DispatchRouter: tests that micro-tasks route local, hard tasks escalate, VoxScript offload injects RAG context.
- Integration: a small VoxMens variant served via vLLM multi-LoRA on the tower; latency + tokens/s smoke.
- Build-accel: `vox ci build-bench` baseline on the tower vs the measured i9 numbers.

## 7 · Risks
- Used-3090 reliability (mitigate: spare card / warranty-checked sellers).
- 32B-FP8 quality on hardest VoxScript tasks (mitigate: per-spoke eval gate; escalate to cloud).
- Power/heat/noise (~900 W) — desktop only, not portable.
- GPU price volatility (96 GB card jumped to $13,250) — defer the big card until serving justifies it.

## 8 · Recommendations & open questions
- **Build ②** now; keep fine-tuning + frontier in the cloud; revisit a 96 GB card only at public-serving scale.
- Implement the **software** half (CapabilityProbe → VariantSelector → DispatchRouter) on top of the existing facade/scorer — this is the `writing-plans` target.
- **Open:** your actual monthly OpenRouter spend (sets exact payback); whether to NVLink the 3090s; FP8-vs-AWQ default per spoke (decide by eval gate).
