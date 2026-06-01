# SP-3 — Quantize Merged QLoRA Output Design

**Date:** 2026-05-31
**Status:** Approved (decomposition); spec under review
**Depends on:** SP-1 (`vox-quantize` engine)
**Scope:** After a QLoRA fine-tune is merged into the base, produce a quantized deployable artifact — so adapters ship small *and* run quantized.

---

## 1. Motivation

QLoRA training already produces tiny adapters. `vox schola merge-qlora` merges LoRA deltas into the f32 base. The gap: the merged result is full-precision f32 and large. SP-3 pipes the merged model through the SP-1 engine to emit a quantized deployable model.

## 2. The verified complication

`merge-qlora` writes a **subset** of tensors — only the keys present in the adapter manifest (`base_key_map`), not a complete model (`vox-plugin-mens-candle-cuda/src/merge.rs:101-189`). Output is f32 SafeTensors + an `external_serving_handoff_v1.json` sidecar.

**Therefore SP-3 cannot just quantize the merge output.** It must reconstruct a complete model first:

```
complete_model = base_model_shards  (unmodified, non-adapted keys)
               ⊕ merged_subset       (adapted keys, override base)
```

then feed `complete_model` to `vox-quantize`.

## 3. Goals / non-goals

**In scope:**
- A `--quantize <mixture>` option on the merge path (or a follow-on `vox schola merge-qlora-quantized`) that: merges (existing) → recombines subset over base → quantizes (SP-1) → writes quantized SafeTensors-canonical artifact + report.
- Reuse SP-1 for all quantization; SP-3 is orchestration glue, no new quant logic.

**Out of scope:**
- Changing the merge math (exists, verified: `W' = W + (B @ A) * (alpha/rank)`).
- Inference on the result (SP-2).

## 4. Architecture

New thin module in `vox-ml-cli` (or `vox-populi` where merge dispatch lives) that:
1. Calls existing merge → merged subset f32.
2. Builds a combined model view: enumerate base shards, substitute adapted keys from the subset.
3. Invokes `vox_quantize::quantize` on the combined model.
4. Emits artifact + `QuantReport`.

Decision to settle in plan: do we materialize the combined f32 model to disk (simpler, more disk) or stream-substitute in memory into the quantizer (less disk, more coupling)? **Default: materialize to a temp dir**, quantize, clean up — simplest and reuses SP-1 unchanged.

## 5. Error handling

- Adapter/base key mismatch (a merged key absent from base) → typed error before quantizing.
- Propagate `QuantizeError`.

## 6. Testing

- Integration: tiny base model + tiny adapter fixture → merge → recombine → quantize; assert adapted keys reflect the delta, non-adapted keys match base, output quantized + metadata present.
- Negative: adapter key not in base → clear error.

## 7. Verified facts

- Merge owner: `vox-ml-cli/src/commands/schola/merge_qlora.rs`; core in `vox-plugin-mens-candle-cuda/src/merge.rs:102-191`.
- Output: f32 SafeTensors **subset** + `external_serving_handoff_v1.json`.
- Merge formula confirmed at `merge.rs:38-49,166`.
