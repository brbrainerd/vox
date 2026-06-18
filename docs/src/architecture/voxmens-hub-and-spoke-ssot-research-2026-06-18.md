---
title: "VoxMens Hub-and-Spoke: SSOT, Per-Spoke Model Selection, and Generalization Beyond QLoRA — Research & Audit"
description: "Audit of the existing lane/mix architecture plus researched best-practice for a config-only single source of truth that declares spokes, selects a base model and training method per spoke, and routes at inference — gesturing toward a future implementation plan."
category: "Architecture SSOTs"
training_eligible: true
---

# VoxMens Hub-and-Spoke: SSOT + Per-Spoke Model Selection + Generalization Beyond QLoRA

> **Status:** Research & audit (no implementation). Terminal of a brainstorming session; feeds a future `writing-plans` cycle.
> **Date:** 2026-06-18
> **Method:** Live codebase audit (this session) + deep-research web fan-out (6 angles, 25 sources fetched, 108 claims, 25 adversarially verified).
> **Structure:** Option A (spoke-major) — shared-architecture audit first, then one deep section per spoke, then cross-cutting research, then a sequencing gesture.

## 0. Confidence & provenance (read this first)

This report draws on two evidence classes that must not be conflated:

- **Live codebase audit** — direct inspection of `mens/config/`, `crates/vox-ml-cli`, `crates/vox-corpus` this session. High reliability for "what our code does today."
- **Deep-research web findings** — fan-out search with 3-vote adversarial verification. **The verification pass was heavily rate-limited** (server-side, not usage cap): of 25 claims, only **5 were confirmed** (all SSOT/config-pattern), and **20 came back 0-0 "abstain"** — meaning *not adversarially confirmed*, **not disproven**. The abstained claims came from primary sources (arXiv, GitHub) but could not be voted on before the rate limit hit.

Consequence: the **SSOT / config-pattern axis is well-grounded**; the **model-selection, router/serving, and agentic-data axes are sourced but unverified** and are carried below as **open research** rather than recommendations. The time-sensitive model-landscape claims (Q2) are exactly the ones lacking verification and **must be re-researched live before any base-model is committed to the SSOT.**

---

## 1. Shared architecture audit (the hub)

### 1.1 What exists today

VoxMens is **already a nascent hub-and-spoke**, expressed as per-domain **mix configs** under `mens/config/`:

| Mix config | De-facto spoke | Maturity |
|---|---|---|
| `mix-vox-lang.yaml` | VoxScript authoring | Mature |
| `mix-rust.yaml` | Rust (but cross-translation only) | Partial |
| `mix-agents.yaml` | Harness / agentic tooling | Skeleton, no corpus |
| `mix-research.yaml`, `mix-research-expert.yaml`, `mix-rocks.yaml`, `mix-populi-meta.yaml` | Research / meta | Varies |

The real backbone is the **`lane` field** stamped on every training record (`vox_codegen`, `vox_lang_tier_b`, `vox_rust_expert_cross`, `vox_tooling`, `vox_dogfood_agent`, `vox_research_expert`, …). Records flow through a staged pipeline (`crates/vox-ml-cli/src/commands/mens/pipeline.rs`): `Generate → ResearchGen → Extract → HealToDpo → Replay → Review* → Validate → Pairs → Eval → Mix → Train`. The `Mix` stage resolves a mix config and weight-merges lane sources into `train_mixed_*.jsonl`; `Train` runs QLoRA (`run_train`) with a single preset (`qwen_4080_16g`) on one RTX 4080 SUPER (16GB).

### 1.2 Confirmed structural gaps (code)

1. **No SSOT — the spoke definition is scattered.** A "spoke" today is implicitly defined across: the `mix-*.yaml` file (data + weights), hard-coded lane strings in `crates/vox-corpus`, the `eval-gates.yaml` thresholds, and the `run_train` call-site arguments in `pipeline.rs` (base model, preset, method are all positional args, not declared per spoke). There is **no single artifact that says "spoke X = these lanes + this base + this method + this eval gate + this router policy."** Adding a spoke is a multi-file, multi-crate edit, not a config-only operation.
2. **No inference-time router and no adapter-swap serve path.** `adapter_tag` exists only at *train* time. The chosen router+adapters (MoE-ish) topology is **entirely unbuilt.**
3. **Eval is monocultural.** `eval-gates.yaml` measures `vox_parse_rate`, `pass@k`, `anti_stub`, `construct_coverage` — all VoxScript. **No Rust-authoring eval, no tool-use/harness eval.** You cannot gate spokes you cannot measure.
4. **Silent-optional data sources mask missing corpora.** `mix-agents.yaml` points at `a2a_traces.jsonl`, `workflow_traces.jsonl`, `dogfood_converted.jsonl` — **none exist on disk** — and because they are `optional: true`, the mix silently produces an empty/degenerate agentic dataset with no error. (This is the same "declared-but-unwired config" and "silent drop" anti-pattern catalogued in the pipeline-gap and config-audit work.)
5. **Single training method baked in.** `pipeline.rs::Train` only calls `PopuliTrainBackendCli::Qlora`. DPO data is *produced* (`run_heal_to_dpo`, `vox_heal_dpo` lane) but there is no per-spoke method selector that would actually train it as preference data.

### 1.3 External best-practice for the SSOT (VERIFIED)

The strongest verified precedent is the **single declarative YAML config as the SSOT for the whole pipeline**:

- **Axolotl** — one `config.yml` drives *preprocessing, training, evaluation, quantization, and inference* (same file across all CLI subcommands). It declares **multiple heterogeneous datasets as a list** (`datasets:`, each with `path` + `type`, mixable across formats) and **selects the training method per run** via `adapter:` / `rl:` (full FT, LoRA, QLoRA, GPTQ/QAT, DPO/IPO/KTO/ORPO/SimPO, GRPO/GDPO). *(3-0 and 2-0 verified; sources: [axolotl repo](https://github.com/axolotl-ai-cloud/axolotl), [config-reference](https://docs.axolotl.ai/docs/config-reference.html), [dataset-formats](https://docs.axolotl.ai/docs/dataset-formats/).)*
- **torchtune** — datasets declared **entirely in config** as a list of `_component_` dotpaths to builder functions (e.g. `torchtune.datasets.alpaca_dataset` with sibling params), aggregated by `ConcatDataset`. Adding a source is a declarative edit. **Caveat: torchtune is plain concatenation, not weighted mixing** — VoxMens already has weighted mixing, so keep your mix-weight layer. *(3-0 verified; sources: [ConcatDataset](https://meta-pytorch.org/torchtune/0.5/generated/torchtune.datasets.ConcatDataset.html), [datasets tutorial](https://meta-pytorch.org/torchtune/0.3/tutorials/datasets.html).)*

**The `_component_` dotpath + registry convention is the recommended SSOT mechanism:** a spoke field points at a *named* builder/base/method, and validation/exhaustiveness is enforced over the registry of known names. This is exactly the drift-proofing VoxMens needs (and mirrors the codebase's existing SSOT-registry pattern used for env vars, config, and the crate graph).

### 1.4 Recommended SSOT shape (design sketch, not a plan)

A single `mens/config/spokes.yaml` (or a `spokes/` dir, one file per spoke) where each spoke is a self-contained, validated record:

```yaml
# illustrative schema — names indicative, not final
spoke:
  id: rust_authoring            # ← lane-family key; must be unique
  description: "Idiomatic Rust authoring & review of our own codebase"
  base:
    model_id: <registry-key>    # validated against a known-bases registry
    method: qlora               # qlora | full_sft | dora | dpo | orpo | kto | rag_only | prompt_only
    preset: qwen_4080_16g       # validated against gpu-specs.yaml presets
  data:
    mix: mix-rust.yaml          # existing weighted-mix file (reused as-is)
    require_sources: true       # ← FAIL if a declared source is missing (kills the silent-optional gap)
  eval:
    gate: eval-gates-rust.yaml  # per-spoke gate; blocks promotion
  router:
    triggers: [lane:vox_rust_expert_cross, "*.rs", "review rust"]
    priority: 10
```

Properties this buys:
- **Config-only extensibility** — a new spoke = a new validated record; no crate edits.
- **Drift-proofing** — `base.model_id`, `method`, `preset`, and `gate` are validated against registries (arch-check-style exhaustiveness); a typo or orphaned spoke fails CI, not silently.
- **Closes gap #4** — `require_sources: true` turns the silent-optional corpus hole into a hard error.
- **Generalization hook** — `method` per spoke is the seam that lets spokes diverge from QLoRA (see §5.3).

---

## 2. Spoke: VoxScript authoring (mature — harden)

**Current state.** The richest spoke: synthetic generation, AST-mutation (`ast_mutator`), doc-mining (` ```vox ` blocks), negative/error-correction pairs, multi-turn refinement, curriculum ordering, and a **diversity gate** (`eval_semantic_entropy`, mode-collapse alarm). Lanes: `vox_codegen`, `vox_lang_tier_b`, `vox_logic_composition`.

**Gaps.** (a) Eval measures parse-rate/pass@k but the diversity gate is the only guard against monoculture — fine. (b) It is coupled to a single base via the shared `run_train` call. (c) No explicit "VoxScript spoke contract" — it's the default everything-else.

**Recommended model/method.** A **small, fast code model** is the right call here (VoxScript is a constrained DSL; you want latency and a tight grammar, not frontier reasoning). QLoRA stays correct. **Specific base TBD — see §6 open research** (the 2025-2026 small-code-model landscape was the unverified axis).

**Build sketch.** Promote it to an explicit `spokes.yaml` record (no behavior change); make it the reference implementation for the schema.

---

## 3. Spoke: Rust authoring/review (half-built — fill)

**Current state.** `mix-rust.yaml` is dominated by **Rust→Vox cross-translation** (`vox_rust_expert_cross`, `rust_to_vox.rs`) plus a 464MB raw `rust_source.jsonl` sampled at 2%. `run_rust_mine` extracts translation pairs; `ExtractRs` mines raw workspace Rust.

**Gaps (code).** There is **no "write idiomatic Rust from scratch" or "review our Rust" lane.** The existing data teaches *translation*, not *authorship*. There is **no Rust-authoring eval** (no compile-check, no clippy-pass, no Rust pass@k). The 464MB raw dump is breadth without instruction-pairing.

**Best-practice data sourcing.** Your own codebase + CI is the asset: (a) mine `(diff, review-comment)` pairs from PR review history (`review_findings.jsonl` already exists) into review-style SFT/DPO; (b) generate `(instruction → idiomatic Rust)` pairs verified by `cargo check`/`clippy` as the reward signal — analogous to the VoxScript `run_frontend` round-trip verification already in `run_mutate`. This is a deterministic quality gate you already have the machinery for.

**Recommended model/method.** A **stronger code-specialized base** than the VoxScript spoke (Rust is harder; benefits from a larger code model). Candidate families surfaced but **unverified**: Qwen2.5-Coder, DeepSeek-Coder-V2 (MoE), Codestral, StarCoder2, Granite-Code. **Re-research live (§6).** Method: QLoRA for the authoring lane; consider DPO/ORPO for the review lane (preference over good-vs-bad fixes — `run_heal_to_dpo` shows you can already build preference pairs).

**Build sketch.** New lanes `vox_rust_authoring` + `vox_rust_review`; a `cargo check`/`clippy`-backed verifier; `eval-gates-rust.yaml` (compile-rate, clippy-clean-rate, review-recurrence — the latter already exists).

---

## 4. Spoke: Harness / agentic tooling (vaporware — build)

**Current state.** `mix-agents.yaml` exists; lanes `vox_tooling` / `vox_dogfood_agent` exist; **the corpus does not.** Every declared source is a non-existent file marked `optional: true`. This spoke is config without data.

**Goal.** Teach the model to operate the codebase: tool calls, skill invocation, discovery, running `vox.exe` for repo management — *not* writing VoxScript, but driving the harness around it.

**Best-practice data sourcing (the user-chosen "both" path).**
- **Mine real traces.** Capture agent execution (tool calls, multi-turn, A2A) into a trace store, then convert to SFT/DPO. This requires a **trace-capture mechanism that does not yet exist** — the `a2a_traces.jsonl` / `workflow_traces.jsonl` paths are aspirational. Building the *capture* is prerequisite to the spoke. (Industry pattern: streaming large agentic-trace corpora into clean ShareGPT-style SFT — surfaced via AgentTrove-style pipelines, **unverified this run**.)
- **Synthesize from surfaces.** Generate tool-use trajectories deterministically from **SKILL.md manifests, the `vox.exe` command surface (`--help`), and discovery registries** — you have all three as structured inputs. Candidate academic methods (Magnet: graph-translation from function signatures to call sequences with positive/negative trajectories for SFT+preference; GEM: four-stage synthesis from text corpora) were **sourced but unverified**; treat as leads to validate, not recipes.

**Eval.** Function-calling / agentic benchmarks (BFCL-style: single-turn, multi-turn, agentic/memory) — **unverified this run**; needs a live re-check before adopting as the gate.

**Recommended model/method.** A **tool-use / agentic-tuned base** (different lineage from the code spokes). **Unverified — re-research (§6).** Method: SFT on traces + DPO on positive/negative trajectories.

**Build sketch (ordered by dependency).** (1) trace-capture in the harness → (2) trace→SFT/DPO converter + schema → (3) surface-synthesis generator from skills/CLI/discovery → (4) dedup/diversity gate (reuse `eval_semantic_entropy`) → (5) `eval-gates-agents.yaml`.

---

## 5. Cross-cutting research

### 5.1 Router design (UNVERIFIED — open)

Inference-time routing options span keyword/lane-tag (cheapest, you already stamp `lane`), embedding-similarity, classifier, and LLM-router (most accurate, highest cost/latency). Semantic-router and multi-LLM-routing patterns were surfaced (Red Hat, AWS) but **not verified this run**. **Recommendation: start with the cheapest viable router — lane-tag/keyword — since you already emit `lane` and `router.triggers` fits the SSOT** (§1.4); treat learned routing as a later upgrade gated on measured misroute rate.

### 5.2 Multi-adapter serving vs separate servers (UNVERIFIED — open, and decisive)

The **critical unresolved constraint:** S-LoRA / LoRAX / Punica hot-swap many adapters cheaply on **one shared base** — but (per the LoRAX claim, *unverified*) **all adapters must share that single base.** If your spokes use **heterogeneous bases** (small-code for VoxScript, strong-code for Rust, agentic for harness), **single-base multi-adapter serving does not apply** — you'd need **separate model servers** (higher VRAM/ops). This is the **central tension between "best model per spoke" (§2-4) and "cheap MoE-style serving."** It must be resolved with verified evidence before the topology is locked. Two coherent end-states:
- **Homogeneous base + per-spoke adapters** → one base, hot-swap adapters (S-LoRA/LoRAX), cheap serving, *but* every spoke inherits one base's ceiling.
- **Heterogeneous bases** → best-fit model per spoke, *but* separate servers + a router that dispatches across them, more VRAM/ops, no adapter stacking across bases.

### 5.3 Generalizing beyond QLoRA — heterogeneous methods (PARTIALLY VERIFIED)

**Verified:** a single config schema *can* select among full FT / LoRA / QLoRA / DPO / IPO / KTO / ORPO / SimPO / GRPO per run (Axolotl). So a `method:` field per spoke (§1.4) is a proven abstraction. RAG-vs-fine-tune complementarity (cumulative gains) was **sourced but unverified.**

**Gains/losses of heterogeneity (synthesis — treat serving claims as provisional):**

| Axis | Homogeneous (one base, QLoRA, adapters) | Heterogeneous (per-spoke base + method) |
|---|---|---|
| Model fit per task | Capped by shared base | Best-fit per spoke |
| Serving cost / VRAM | Low (adapter hot-swap) | High (separate servers) |
| Adapter stacking | Possible (same base) | Impossible across bases |
| Eval comparability | Easy (one base) | Harder (per-spoke gates required) |
| Ops complexity | Low | High |
| Cold-start | Fast | Slower (load distinct models) |
| When right | Tight budget, similar tasks | Tasks with divergent skill ceilings |

**When a non-fine-tuned spoke is correct:** if a capability is *knowledge-retrieval-shaped* (e.g. "what does this CLI flag do") rather than *behavior-shaped*, a **RAG-only or prompt-only spoke** (no training) may dominate — the SSOT should permit `method: rag_only` / `prompt_only` so not every spoke pays the training/serving cost.

### 5.4 Cross-model SSOT (PARTIALLY VERIFIED — open)

Axolotl/torchtune confirm a single schema spanning multiple **methods** and **data sources**. A verified example of one SSOT spanning multiple **distinct base models** (a capability/contract decoupled from base implementation) was **not established this run.** The model-agnostic-pipeline and model-registry-governance sources are leads, unverified. **Recommendation: design the SSOT around a per-spoke *capability contract* (id, eval gate, router triggers, data contract) that is decoupled from `base.model_id`/`method`** — so the SSOT spans bases *by construction*, even if no single external tool proves the pattern. This is the abstraction that future-proofs heterogeneity (§5.2/§5.3) without committing to it now.

---

## 6. Open research (must re-run live before committing)

These axes had **primary sources identified but verification rate-limited to 0-0**. They are the highest-value, most-perishable unknowns:

1. **Per-spoke base-model selection (Q2)** — best 2025-2026 open-weight bases at a 16GB QLoRA budget for VoxScript (small/fast) vs Rust (strong code) vs agentic (tool-use), with spoke-relevant benchmarks (HumanEval/MBPP/Rust-specific; BFCL/agentic). *Most time-sensitive; re-research immediately before any commitment.*
2. **Cross-base serving constraint (Q5)** — confirm whether S-LoRA/LoRAX truly require a single shared base (decides §5.2 → decides whether "best model per spoke" is even affordable).
3. **Agentic data synthesis (Q6)** — validate Magnet / GEM / AgentTrove-style pipelines and a trace-schema standard before building the converter (§4).
4. **Verified multi-base SSOT precedent (Q4)** — does any real stack run one SSOT over distinct bases, or is the capability-contract abstraction (§5.4) net-new?

**Re-research guidance:** dispatch in **small batched groups (≤8 claims/round)** with pacing — the rate-limit failure this run was the documented large-fan-out pattern. Verify model-selection and the serving constraint *first* (they gate the topology).

---

## 7. Toward a future implementation & enhancement plan (gesture, not a plan)

Buildable pieces and their dependency order (each becomes a `writing-plans` input):

1. **`spokes.yaml` SSOT + validator** (§1.4) — schema, registry-backed validation, `require_sources: true`. *No model work; immediate drift/silent-gap fix. Unblocks everything.*
2. **Per-spoke eval gates** (`eval-gates-{rust,agents}.yaml`) — you cannot promote spokes you cannot measure (§1.2 gap #3).
3. **Rust-authoring spoke** (§3) — new lanes + `cargo check`/`clippy` verifier + eval gate. Reuses existing round-trip-verify machinery.
4. **Harness trace-capture + converter** (§4) — *prerequisite* to the agentic spoke; the corpus literally does not exist yet.
5. **Surface-synthesis generator** (§4) — skills/CLI/discovery → tool-use pairs.
6. **Router (lane-tag first)** (§5.1) — cheap, uses existing `lane` + SSOT `router.triggers`.
7. **Serving topology decision** (§5.2) — *gated on §6 item 2*; chooses homogeneous-adapters vs heterogeneous-servers.
8. **Per-spoke method generalization** (§5.3) — wire `method:` through `pipeline.rs::Train` (today hard-coded QLoRA).

**Sequencing logic:** SSOT + eval first (cheap, unblock, fix silent gaps) → fill the two underbuilt spokes in parallel (Rust is lower-risk; harness needs capture infra first) → router → *then* the model-selection + serving-topology decisions that the open research must inform. **Do not lock base models or serving topology until §6 items 1–2 are re-verified live.**

---

## 8. Sources

**Verified (primary):**
- Axolotl — [repo](https://github.com/axolotl-ai-cloud/axolotl), [config-reference](https://docs.axolotl.ai/docs/config-reference.html), [dataset-formats](https://docs.axolotl.ai/docs/dataset-formats/)
- torchtune — [ConcatDataset](https://meta-pytorch.org/torchtune/0.5/generated/torchtune.datasets.ConcatDataset.html), [datasets tutorial](https://meta-pytorch.org/torchtune/0.3/tutorials/datasets.html)

**Sourced but UNVERIFIED this run (re-check before relying):**
- Model selection: [Qwen2.5 tech report](https://arxiv.org/pdf/2412.15115), [DeepSeek-Coder-V2](https://arxiv.org/pdf/2406.11931), [BFCL](https://openreview.net/pdf?id=2GmDdhBdDk)
- Serving/routing: [S-LoRA](https://arxiv.org/pdf/2311.03285), [LoRAX](https://github.com/predibase/lorax), [HF multi-LoRA serving](https://huggingface.co/blog/multi-lora-serving), [Red Hat semantic router](https://developers.redhat.com/articles/2025/05/20/llm-semantic-router-intelligent-request-routing), [AWS multi-LLM routing](https://aws.amazon.com/blogs/machine-learning/multi-llm-routing-strategies-for-generative-ai-applications-on-aws/)
- Heterogeneous methods / RAG: [RAG vs fine-tuning](https://arxiv.org/abs/2401.08406)
- Agentic data: [Magnet](https://arxiv.org/pdf/2503.07826), [GEM](https://arxiv.org/pdf/2601.10355), [AgentTrove writeup](https://www.marktechpost.com/2026/05/29/how-to-use-agenttrove-streaming-1-7m-agentic-traces-and-building-a-clean-sharegpt-sft-dataset-in-python/)
- Cross-model SSOT: [model-agnostic pipelines](https://medium.com/@fahey_james/model-agnostic-fine-tuning-pipelines-one-workflow-any-foundation-model-810097ce14b3), [model-registry governance](https://introl.com/blog/model-registry-governance-mlops-production-ai-2025)
