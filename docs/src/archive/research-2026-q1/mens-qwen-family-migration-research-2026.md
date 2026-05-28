---
title: "Mens Qwen family migration and native stack (research 2026)"
description: "Qwen2 vs Qwen3.5 in Vox Candle paths; operator runbook vs code removal; external HF and QwenLM sources; deprecation tiers; TrainingPair and tokenizer implications."
category: "Architecture SSOTs"
status: "research"
sort_order: 17
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Prevents accidental removal of Qwen2 compatibility while standardizing new work on Qwen3.5 defaults."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Mens Qwen family migration and native stack (research 2026)

## Executive summary

- **Product default** in this repository is already **Qwen3.5-class** text bases (`DEFAULT_MODEL_ID` in `vox-populi` `mens/mod.rs`, nightly workflow `qwen35-native-nightly.yml`, [Mens training reference](../reference/mens-training.md)).
- **Qwen2** remains in-tree as **`HfArchitecture::Qwen2`**, **`InferenceModel::Qwen2`**, HF keymap tables, and **unit test fixtures** using `"model_type":"qwen2"` JSON snippets. That is intentional **compatibility and regression surface**, not legacy neglect.
- **Public ecosystem** still ships many Qwen2-named weights and LoRA adapters; “delete Qwen2 from Candle” is a **semver-scale** decision, not a documentation tweak.

This document defines **deprecation tiers**, a **migration story split** (runbook vs weight surgery vs code removal), and **external references** to re-check before any removal milestone.

## External references (April 2026 snapshot)

Re-verify URLs and claims before release-blocking decisions.

| Source | Use |
| --- | --- |
| [QwenLM: Qwen3 — Think Deeper, Act Faster](https://qwenlm.github.io/blog/qwen3/) | Product positioning: thinking vs non-thinking modes, multi-size lineup. |
| [QwenLM: Qwen2.5-Coder family](https://qwenlm.github.io/blog/qwen2.5-coder-family/) | Code-specialized line; still a credible baseline for comparisons. |
| [airank.dev: Qwen2.5-Coder-32B vs Qwen3 Coder Next](https://airank.dev/models/compare/qwen2-5-coder-32b-vs-qwen3-coder-next) | Third-party benchmark/cost framing (non-authoritative). |
| [Hugging Face Transformers: Qwen3_5 model doc](https://huggingface.co/docs/transformers/main/model_doc/qwen3_5) | `text_config` / `vision_config`, multimodal token ids; upstream pages may still contain scaffolding — treat as evolving. |

## Migration story: three layers of difficulty

| Layer | Meaning | Effort band |
| --- | --- | --- |
| **A — Operator runbook** | New work uses `Qwen/Qwen3.5-*`; refresh `tokenizer.json`; train or merge QLoRA; serve via Schola path in [Mens serving SSOT](../reference/mens-serving-ssot.md); re-run eval on fixed JSONL. | Small (documentation + checklist + one dry run). |
| **B — Adapter continuity** | Same LoRA directory must run on a new base without retrain — may require out-of-tree conversion or may be **unsupported**; document honestly. | Medium to large if promised automatically. |
| **C — Code removal** | Delete `Qwen2` branches in Candle and tests. | Large; requires audit, CI matrix, release notes. |

**Narrative for contributors:** default new recipes to Qwen3.5; keep Qwen2 paths until an explicit audit shows zero product dependency; prefer “retrain recommended” over silent weight conversion.

## Deprecation tiers (proposal)

| Tier | Qwen2 native path | Qwen3.5 |
| --- | --- | --- |
| **Supported** | Load + inference + tests maintained | Default for new training and docs. |
| **Frozen** | Bugfixes only; no new Qwen2-only features | Active development. |
| **Removed** | Delete after migration guide + major boundary | Single text architecture path (names TBD). |

## Repository audit checklist (for tier movement)

Execute before **Frozen** or **Removed**:

1. `rg` / search: `Qwen2`, `qwen2`, `HfArchitecture::Qwen2`, `InferenceModel::Qwen2` across `crates/vox-populi`, `crates/vox-cli`, workflows, `contracts/mens/`.
2. Confirm no operator-facing doc promises Qwen2 as **default**.
3. Confirm `training-presets` and `DEFAULT_MODEL_ID` stay aligned (`vox-populi` test `training_presets_yaml_contract.rs` in the workspace crate).
4. Update [Mens training reference](../reference/mens-training.md) cross-links if serve or merge matrix changes.

## Qwen3.5-specific technical notes (native stack)

- **Linear / hybrid attention blocks** — `hf_keymap.rs` branches on `HfArchitecture::Qwen35` and layer type (`linear_attention` vs full attention). Changes to upstream `config.json` naming must be reflected here.
- **RoPE and preflight** — `qlora_preflight.rs` includes Qwen3.5-specific rope key warnings; keep tests when touching layout discovery.
- **Thinking-mode tokens** — If training data includes chain-of-thought, define whether Mens supervised spans strip them for `vox_codegen` lanes ([Mens training data contract](../reference/mens-training-data-contract.md) lane policy).

## Multimodal (HF) vs native Candle

Hugging Face `Qwen3_5Config` documents `vision_config` and image placeholder token ids. **Native Candle QLoRA in this repo remains text-only** until a separate ADR and execution planner workstream adds a vision encoder and training contract. Until then, **multimodal serving** belongs in external runtimes (vLLM, Ollama, HF) as already described in [Mens training reference](../reference/mens-training.md) external serving section.

## See also

- [Mens vision and multimodal inputs (research 2026)](mens-vision-multimodal-research-2026.md)
- [Vox corpus lab (research 2026)](vox-corpus-lab-research-2026.md)
- [Candle full graph feasibility](candle-full-graph-feasibility.md) and ADR 006 / 007 linked from Mens docs
- [Mens training reference](../reference/mens-training.md)
- [Vox source → Mens pipeline SSOT](vox-source-to-mens-pipeline-ssot.md)

## Open questions

1. Minimum Qwen2 fixture set to keep permanently in `vox-populi` tests after tier **Frozen**.
2. Whether to publish a **single** `external_serving_handoff` extension field for `base_family` when VL is used only for eval, not training.
3. Official policy on community weight migration scripts (license, no vendoring without review).


