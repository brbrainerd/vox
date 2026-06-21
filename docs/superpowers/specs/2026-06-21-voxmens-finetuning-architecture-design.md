# VoxMens Fine-Tuning Architecture & Spot Pipeline — Design Spec

**Status:** Approved + adversarially reviewed (brainstorm 2026-06-21; review rev 2026-06-21). **Research basis:** `docs/src/architecture/voxmens-finetuning-boundaries-research-2026-06-21.md`. **Implementation plan:** `docs/superpowers/plans/2026-06-21-voxmens-finetuning-pipeline.md`.

## Goal

Make VoxMens a **resource-scalable hub-and-spoke** system that fine-tunes **only where the base model lacks the capability** and serves everything else (tools, skills, plugins, facts) through a **dynamic discovered layer** that needs **no retraining when capabilities are added**. Training runs on **cloud spot GPUs** (RunPod default), scaling from a CPU/dev tier up to 96 GB VRAM.

**V1 success criterion (reframed after review):** the pipeline runs end-to-end **and each adapter beats its own Qwen3 base rung on its gate metric** by a documented margin. **Per-spoke Gemini-3-Flash / Claude-Sonnet-4.6 parity is a tracked north-star, reported as a gap — not a v1 pass/fail gate** (v1 is SFT-only; the RL/distillation levers that close the last gap are v2). This prevents a *working* pipeline from reading as "failed."

## The boundary (the core decision)

- **Fine-tune** = *capability or syntax the base model cannot be prompted into.* → Vox DSL, Rust idiom, and tool-call **behavior**. Evidence: fine-tuning beats RAG for a novel/low-resource language (2503.18760 ✅); fine-tuned small models top BFCL over GPT-4/Claude-3 (xLAM 2409.03215 ✅) and rival 16-70B on low-resource code (Agnostics 2508.04865 ✅).
- **Dynamic / discovered** = *anything enumerable or changing.* → the tool/skill/command/plugin catalog, facts, DB/API knowledge. Served via retrieval + schema, never weights. New skill = a registry row → retrieval surfaces it → the trained harness calls it. Zero retraining.

## Architecture

### Hub (one dense lineage + one small embedder)
One **Qwen3 dense ladder** lineage; all spokes are LoRA adapters on it, enabling multi-adapter hot-swap. **The base must be pinned by HF revision/commit, not a floating tag** (reproducibility). A **second, tiny hub artifact — a ~0.6B embedder — is a first-class declared dependency** (`hub.embedder` in `domain-profiles.yaml`), co-resident with the base; it powers the retrieval layer and is **not** fine-tuned.

**Per-tier base table (committed SSOT; tune after first runs).** Preference order is *(larger rung, then less quantization)*; at 48 GB spend headroom on **un-quantizing the 14B (LoRA over QLoRA) before climbing to 32B-QLoRA** — quantization error usually costs more on a narrow domain than two extra B of params, and it keeps the serve rung stable for adapter provenance.

| VRAM | Train rung | Method | Serve rung | Note |
|---|---|---|---|---|
| CPU / ≤8 GB **dev** | Qwen3-0.6B/1.7B | QLoRA r8 | same | pipeline smoke only — **no quality gate** |
| 16 GB | Qwen3-8B | QLoRA r16 | 8B | 4080 tier |
| 24 GB | Qwen3-14B | QLoRA r32 | 14B | single 3090/4090 |
| 48 GB | Qwen3-14B | **LoRA** r32 (un-quantized) | 14B | un-quantize before climbing |
| 96 GB | Qwen3-32B | QLoRA r64 | 32B | not full-rank |

**Scale-down floor:** the resolver fails closed below the dev rung's QLoRA minimum. **Dense, not MoE:** MoE (Qwen3-30B-A3B / 235B-A22B) is a *separate base lineage* — its adapters can't serve on the dense base and it needs its own server, breaking the cheap hot-swap invariant. Deferred to v2; justified only if measured dense-32B parity lags.

### Spokes — target = 4 QLoRA adapters; v1 sequences mono-harness first
| Spoke | Capability the base lacks | Gate metric (beat-base) |
|---|---|---|
| `vox-lang` | Novel Vox DSL syntax/idiom | vox parse/compile rate |
| `rust` | Lower-resource Rust idiom | `rust_compile_rate` |
| `tool-selection` | Right tool from a large catalog | selection accuracy |
| `argument-generation` | Schema-valid args for complex params | `tool_call_valid_json_rate` |

`tool-selection` + `argument-generation` = the **harness**, decomposed per 2510.00229 (+46% on MCP-Bench vs *monolithic*). **V1 trains a single combined `harness` adapter first** (union corpus) as the end-to-end smoke + the measured baseline, **then** trains the two decomposed adapters and keeps whichever wins against *base + retrieval + constrained-decoding* (B3+B4 may already capture much of the selection/arg gains). **Retired as fine-tuned spokes** (→ hub + dynamic layer): `chat`, `research`, `research-expert`, `rocks`, `populi-meta`. The existing **`lane:vox_rust_review` routes to the base model (no adapter)** — an authoring-tuned adapter hurts review; a dedicated review spoke is a v2 decision, evidenced by a base-only review eval.

### Dynamic / discovered layer (NOT fine-tuned)
- **Vocabulary (exists):** `tool-registry.canonical.yaml`, `SkillRegistry`, `input_schemas.rs` (schemars JSON schemas).
- **Gap 1 — semantic tool RETRIEVAL** via the `hub.embedder`: embed task ↔ tool descriptions, inject top-K (today all tools are dumped in context). BM25 is an explicitly **warned degraded mode**, not the design. Research: +23–104% (2512.17052 🟠) and scales the catalog without retraining (2509.20415 🟡).
- **Gap 2 — schema-CONSTRAINED decoding at generation:** today only post-hoc `JsonPrefix`/`StrictJson`. Map each tool schema to vLLM `guided_json` (backend **configurable**, not a hard-coded magic string). Do not hand-roll a grammar engine.

### Serving (kept, but not on the v1 critical path)
**v1 acceptance validates adapters OFFLINE** (load adapter + run the held-out eval pack). vLLM multi-LoRA hot-swap (`VLLM_ALLOW_RUNTIME_LORA_UPDATING` + LRU, `DomainRouter`→adapter) is wired but **gated behind a real (non-mocked) compatibility spike** that pins the vLLM version and **matches serve quantization to the QLoRA training quantization** (mismatch silently degrades/!loads). New adapters register as **challenger**, promote to **champion** only on beat-the-incumbent, with one-call rollback to last-known-good. Routing emits telemetry (which adapter served what, fallback rate, load-evictions, latency).

### Cloud spot fine-tuning pipeline
Extend the existing trait-based `crates/vox-populi/src/mens/cloud/` (`CloudProvider::dispatch/poll_status`, `CloudJobSpec`, `BudgetLedger`, estimator, watchdog, `CloudResolver`). **RunPod default**; Vast.ai opt-in. Complete: job-submission, log streaming + **retention**, checkpoint sync **with resume-from-checkpoint**, **idempotent re-submit**, **orphaned-pod cleanup**, **budget enforced against cumulative spend incl. retries**, **provider-side max-price auto-terminate**, **secrets via `vox_secrets::resolve_secret`** (env fallback only). Each run emits a `training_manifest.json` (base HF id + revision, rung/preset, rank/alpha, seed, corpus hash, metrics, cost, provider, git SHA). One command (real flag is `--cloud`):

```
vox mens train --cloud runpod --spoke <name> [--dry-run] [--apply]
  → estimate → (cumulative budget gate) → sync corpus up → train QLoRA
  → stream+retain logs → sync checkpoints down (resumable) → pull adapter
  → eval-gate (beat-base, on held-out pack) → register as challenger w/ manifest
```
Money-spending tasks require an explicit **machine-enforceable flag/sentinel** the executor cannot set itself.

### Local-first / backwards compatibility (preserved)
Cloud is the *default for scale*, **not a replacement for local**. The existing local path must keep working for testing on the user's **RTX 4080 SUPER (16 GB)**:
- The current `vox mens train` (no `--cloud`, i.e. `--cloud local`) flow, the **CandleQlora plugin backend**, and the existing **`qwen_4080_16g` preset** are retained unchanged. New `qwen3_*` presets are added *alongside*, never replacing the old ones (the YAML↔Rust parity contract stays green).
- The 16 GB tier (Qwen3-8B QLoRA) is the local test rung; the CPU/dev tier (Qwen3-0.6B) is the no-GPU smoke. Either can run the full train→gate→register loop locally.
- **Provenance is uniform:** the local path emits the same `AdapterCard` + `training_manifest.json` as cloud, so a locally-trained 4080 adapter registers, validates, and serves identically to a cloud one (it just carries `provider: local`).
- Every new contract (`AdapterCard`-bearing `DomainRouter::register`, `bfcl_accuracy` gate, retrieval/schema layer) is **additive** — no existing local-training invocation breaks.

### Evals (baseline-first, leakage-guarded)
1. **Baseline capture FIRST:** run the untrained base rung + the Flash/Sonnet reference on the held-out packs → `baseline_report.json` (per spoke: metric, pass@k **with k**, **sample size**, **bootstrap CI**, judge identity). 2. **Leakage guard:** corpora split by tool/skill **identity** (not row), `split_manifest.json`; B7 asserts train↔eval fingerprint disjointness + near-dup dedup via existing `vox-similarity`. 3. **Gates:** beat-base per spoke (vox parse-rate, `rust_compile_rate`, `bfcl_accuracy`, `tool_call_valid_json_rate`); **per-rung thresholds** (a 4B spoke isn't held to a 32B bar); **regression guard** vs the prior registered adapter. 4. **Harness safety:** all harness evals run against a **mocked/dry-run tool executor** — no real side-effecting tool calls. 5. **Planning eval** (base-only) to evidence whether a v2 planning spoke is warranted.

## V1 scope & deferrals

- **V1 = QLoRA + SFT only**; 3 adapters trained (vox-lang, rust, mono-harness) with the decomposed harness split as a measured arm; dynamic layer (retrieval + schema-guided); offline eval acceptance; RunPod default.
- **Data-sufficiency spike gates all spend:** no cloud GPU is provisioned until the synth generators are proven to produce ready corpora at scale (the true critical path is data, not plumbing).
- **Deferred to V2:** RLVR/GRPO (cargo build / vox compile / BFCL rewards), on-policy distillation, DPO/ORPO/FullSft (fail-closed today), the decomposed-harness split if it doesn't beat mono+B3+B4, a Vox/Rust review spoke, a planning spoke, MoE lineage, live vLLM serving promotion.

## What we are explicitly NOT doing
No separate chat spoke; no per-tool/skill fine-tuning; no custom multi-LoRA merge engine; no new training backend (reuse CandleQlora plugin; cloud runs the same container); no frontier-teacher outputs in v1 SFT data (rule-based-from-surface only — asserted in B1).

## Risks (post-review)
1. **Data volume is the real critical path** — gated by the B2.5 sufficiency spike before any spend.
2. **Adapter↔base provenance (rung + quantization)** — silent corruption; fail-closed at register and load.
3. **Parallel-write conflicts** in execution — mitigated by worktree-per-track + integration task.
4. **vLLM/quantization/version compatibility** — gated by a real compat spike; serving off the v1 critical path.
5. **Spot interruption** — resume-from-checkpoint + watchdog + RunPod default.
6. **Research base is largely 🟡 unverified** — the 4 load-bearing open items are confirmed in B0 before building on them.
