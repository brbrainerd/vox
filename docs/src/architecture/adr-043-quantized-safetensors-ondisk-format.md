---
title: "ADR-043: Quantized SafeTensors On-Disk Format"
description: "Quantized models are stored as SafeTensors-canonical u8 block tensors plus a quant-metadata.json sidecar."
category: architecture
---

# ADR-043: Quantized SafeTensors On-Disk Format

## Status
Accepted (2026-05-31)

## Context
Charter §0.2.3 mandates SafeTensors as the only on-disk weight format. GGML k-quant
blocks are not a native SafeTensors dtype.

## Decision
`vox-quantize` writes quantized tensors as 1-D `u8` SafeTensors tensors carrying the raw
GGML block bytes (via `QTensor::data()`), accompanied by a `quant-metadata.json` sidecar
mapping each tensor to `{ ggml_dtype, orig_shape, orig_dtype, quantized }`, plus the
mixture name and writer version. KEEP-F32 tensors (norms, biases, `A_log`, `dt_bias`) are
stored unchanged. The source `config.json` is copied alongside. The sidecar is part of the
artifact contract — the `u8` blobs are uninterpretable without it.

## Consequences
- Stays SafeTensors-canonical; no GGUF on disk (charter-compliant).
- Downstream loaders (SP-2 inference) reconstruct `QTensor`s from the `u8` bytes + metadata
  via candle's `qtensor_from_ggml` / `QTensor::new`.
- Artifacts remain hash-addressable for the CAS/bundle machinery.

The `ggml_dtype` field is the Debug string of candle's `GgmlDType` (e.g. `Q4K`, `Q8_0`, `F32`). candle exposes no `FromStr` for `GgmlDType`, so the SP-2 reader must maintain an explicit `&str -> GgmlDType` mapping. This string is the round-trip contract between writer and reader.
