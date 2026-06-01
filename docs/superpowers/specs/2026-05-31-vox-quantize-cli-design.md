# SP-4 — Standalone `vox quantize` CLI Design

**Date:** 2026-05-31
**Status:** Approved (decomposition); spec under review
**Depends on:** SP-1 (`vox-quantize` engine)
**Scope:** A first-class CLI surface to quantize any local SafeTensors model, driving the SP-1 engine.

---

## 1. Motivation

SP-1 delivers a library. SP-4 exposes it as an operator command so users can quantize a downloaded Qwen3.5/2.5 checkpoint without writing code, and immediately see the size/quality trade-off via the engine's `QuantReport`. Sequenced second (right after SP-1) because it is the cheapest end-to-end proof that the engine works on real models.

## 2. Goals / non-goals

**In scope:**
- A `vox quantize` subcommand in **`vox-ml-cli`** (L3 binary).
- Flags to select input model dir, output dir, named mixture, and verification toggle.
- Human-readable report: per-tensor target dtype, compression ratio, worst MSE, fallback notes.
- `--json` output mode for scripting (`vox run scripts/*.vox` consumers).

**Out of scope:**
- Running inference on the result (SP-2).
- Calibration data input (deferred PTQ phase).
- Pulling models from HF Hub (assume a local dir; HF download is a separate existing concern).

## 3. CLI surface

```
vox quantize --input <model_dir> --output <out_dir> --to <mixture> [--no-verify] [--json]
```

- `--to` accepts `q4_k_m | q5_k_m | q6_k | q8_0` (maps to `QuantMixture`).
- `--input` must contain `config.json` + `*.safetensors` (single or sharded). Validated up front with a clear error if missing.
- Exit non-zero on `QuantizeError`; print the typed error.

Placement follows existing convention: a new module under `crates/vox-ml-cli/src/commands/` (sibling to `schola/`, `mens/`), wired into the command enum the same way `MergeQlora` is (`commands/mens/populi/action_populi_enum.rs` pattern).

## 4. Output

Default (human): a table — `tensor | src→dst dtype | params | MSE | note`, followed by a summary line (`5.1 GB → 1.4 GB, 3.6× , worst MSE 4.2e-4`). `--json` emits the full `QuantReport` serialized.

## 5. Error handling

Pure pass-through of `QuantizeError` with CLI-friendly messages. Missing `config.json`, unreadable shards, and unknown `--to` values are caught before engine invocation.

## 6. Testing

- CLI arg parsing unit tests (mixture mapping, missing-flag errors).
- Integration: run `vox quantize` against the SP-1 tiny-model fixture in a temp dir; assert exit 0, artifact written, `--json` parses, compression ratio sane.
- Snapshot test of the human table format.

## 7. Verified facts

- `merge-qlora` lives in `vox-ml-cli/src/commands/schola/merge_qlora.rs` with action wiring in `commands/mens/populi/action_populi_enum.rs` — the template for adding `quantize`.
- `vox-ml-cli` is L3 binary, `max_loc = 20_000` (`layers.toml`).
- `vox-ml-cli` may depend on `vox-quantize` (L3→L2 is downward, allowed).

## 8. Open items for plan

- Whether `quantize` is a top-level `vox quantize` or nests under `vox mens quantize` / `vox schola quantize`. **Default: top-level `vox quantize`** (model-agnostic, not MENS-specific). Confirm during planning.
