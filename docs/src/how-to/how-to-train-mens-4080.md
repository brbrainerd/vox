---
title: "How To: Train Mens on RTX 4080 Super"
description: "Official documentation for How To: Train Mens on RTX 4080 Super for the Vox language. Detailed technical reference, architecture guides"
category: "How-To Guides"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "HowTo"
---

# How To: Train Mens on RTX 4080 Super

**Canonical contracts, backends, and regression commands:** [Mens native training SSOT](../reference/mens-training.md). This page is a **step-by-step runbook** for RTX 4080 Super; do not duplicate SSOT tables here.

This runbook covers **two** native paths:

1. **Production Qwen 3.5 (recommended for Qwen3.5-4B-Instruct)** — **Candle QLoRA** (`--backend qlora`, NF4 frozen bases via qlora-rs). Build with **`mens-candle-cuda`** on Windows/Linux when you have an NVIDIA GPU and CUDA toolkit available for `candle-core`.
2. **Burn LoRA (GPT-2-shaped HF or Vox tokenizer)** — default `vox mens train` without `--backend qlora`; uses **wgpu** (Vulkan/DX12) on Windows.

## Recommended Path (Qwen3.5-4B, RTX 4080-class 16GB)

- **Build** (CUDA): from repo root, **`cargo vox-cuda-release`** (alias in `.cargo/config.toml` — same as `cargo build -p vox-cli --release --features gpu,mens-candle-cuda`). 
  > [!WARNING]
  > On Windows, you **MUST** use an interactive VS Developer Command Prompt or PowerShell shell explicitly bootstrapped with `vcvars64.bat`. Passing `vcvars64.bat` via nested subshells (e.g. `cmd.exe /c "vcvars64.bat && cargo..."`) aggressively drops the PATH configurations preventing `nvcc` from correctly executing `cl.exe`. 
- **Data**: `target/dogfood/train.jsonl` (from corpus pairs/mix); optional `record_format: tool_trace` in mix for command/tool supervision rows (`category` `tool_trace`). See **`mens/schemas/tool_trace_record.schema.json`** and **`mens/data/tool_traces.example.jsonl`**.
- **Train**:
  ```powershell
  .\target\release\vox.exe mens train `
    --backend qlora --tokenizer hf `
    --preset qwen_4080_16g `
    --model Qwen/Qwen3.5-4B `
    --data-dir target/dogfood `
    --output-dir mens/runs/qwen35_qlora `
    --device cuda `
    --qlora-require-full-proxy-stack
  ```
  `--qlora-require-full-proxy-stack` is recommended for strict shard completeness on native qwen3_5 runs. LM-head-only mode is currently deferred/not implemented in the native trainer.
- **Artifacts**: `candle_qlora_adapter.safetensors`, `candle_qlora_adapter_meta.json`, `populi_adapter_manifest_v3.json`, `training_manifest.json`, `telemetry.jsonl`.
### Go-live checklist (local CUDA dogfood)

1. **Shell**: VS Developer / MSVC environment so **`cargo vox-cuda-release`** (or `cargo check -p vox-cli --features gpu,mens-candle-cuda`) succeeds.
2. **CLI**: `vox mens train --help` lists **`--qlora-*`** flags including **`--qlora-ce-last-k`**.
3. **Corpus**: refresh `train.jsonl` or set **`VOX_TRAIN_SKIP_CORPUS_MIX=1`** when the mix step is unnecessary.
4. **Run**: canonical QLoRA command from above with **`--log-dir mens/runs/logs`** (or your path); tail the log.
5. **Acceptance**: first log lines show **finite** loss; optional **`--qlora-ce-last-k 4`** for a stronger suffix LM signal (see SSOT).
6. Thin wrapper (optional): [`scripts/mens/train_dogfood.vox`](../../../scripts/mens/train_dogfood.vox).

- **Merge (Candle)**: in-tree **`vox mens merge-qlora`** (alias **`merge-adapter`**) or **`vox schola merge-qlora`** — same merge surface; produces **f32 safetensors** subsets — not Burn `*.bin`. See the SSOT train → merge → serve table in [`mens-training.md`](../reference/mens-training.md). **`vox mens serve` (Burn)** loads LoRA or merged **Burn** checkpoints; it does **not** load Candle merge-qlora safetensors. For querying merged QLoRA weights, use an external stack (e.g. export to HF/Ollama) or keep the **adapter** path your inference tool supports.

## Burn LoRA path (non-Qwen or GPT-2-shaped HF)

- Default: `vox mens train --data-dir target/dogfood --output-dir mens/runs/v1`
- Input contract: `target/dogfood/train.jsonl`
- Backend: `wgpu` on Windows (Vulkan or DX12); no CUDA required for Burn

## Prerequisites

1. Build Vox CLI (release binary):
   ```powershell
   & "$env:USERPROFILE\.cargo\bin\cargo.exe" build -p vox-cli --release
   ```
2. Generate canonical corpus input:
   ```powershell
   New-Item -ItemType Directory -Force -Path mens/data,target/dogfood | Out-Null
   .\target\release\vox.exe mens corpus extract examples/ -o mens/data/validated.jsonl
   .\target\release\vox.exe mens corpus extract docs/ -o mens/data/validated.jsonl 2>$null
   .\target\release\vox.exe mens corpus validate mens/data/validated.jsonl --no-recheck -o mens/data/validated.jsonl
   .\target\release\vox.exe mens corpus pairs mens/data/validated.jsonl -o target/dogfood/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/
   # Rustdoc merge skipped: response is Rust prose, not Vox code
   ```
3. Optional **Burn** GPU backend selection (passed to **`vox mens train --device`**; **`best`** is default):
   ```powershell
   # Prefer flags on the train command, not legacy env, for `vox mens train`:
   # --device best | vulkan | dx12 | cpu
   ```
4. Optional training profile (RTX 4080 Super 16GB VRAM):
   ```powershell
   $env:VOX_TRAIN_PROFILE = "safe"   # Conservative: batch 2, seq 256 (shared GPU, avoids OOM)
   # $env:VOX_TRAIN_PROFILE = "balanced"  # Default for 16GB: batch 4, seq 512, rank 16
   # $env:VOX_TRAIN_PROFILE = "throughput" # Aggressive: batch 6 (may OOM if OS uses GPU)
   ```
   Device probe auto-detects 16GB and recommends batch 4, seq 512, rank 16. Use `vox mens probe` to verify.

## Full mixed corpus → entire LoRA run (4080 preset)

Use this when you want **all sources** from `mens/config/mix.yaml` (not a tiny dogfood slice).

1. **Build** release CLI with **`--features gpu`** (default is `mens-base` only; native train / QLoRA need the GPU feature stack). Add **`--features mens-dei`** only if you need legacy **`vox train`** (Together / **`--native`** Burn scratch; **`--provider local`** bails to **`vox mens train`**) or Mens DeI surfaces (`generate`, `review`, …):
   ```powershell
   & "$env:USERPROFILE\.cargo\bin\cargo.exe" build -p vox-cli --release --features gpu
   ```
   If this fails, fix `vox-cli` compile errors before training.

2. **Mix** into the default mix output path (strict: all non-optional sources must exist and contribute rows):
   ```powershell
   .\target\release\vox.exe mens corpus mix --config mens/config/mix.yaml
   ```
   Writes `target/dogfood/train_mixed.jsonl` per mix config plus **`target/dogfood/train_mixed.mix_report.json`**. If your tree is missing generated files, use **`--allow-missing-sources`** once (same as legacy warn-only mix) or run the corpus pipeline stages first.

3. **Point training** at that file as `train.jsonl` (preflight requires this exact name inside `--data-dir`):
   ```powershell
   New-Item -ItemType Directory -Force -Path target/dogfood | Out-Null
   Copy-Item -Force target/dogfood/train_mixed.jsonl target/dogfood/train.jsonl
   ```

4. **Train (Qwen + Candle QLoRA)** with the **`qwen_4080_16g`** preset (16GB-oriented; see SSOT [mens-training.md](../reference/mens-training.md)):
   ```powershell
   .\target\release\vox.exe mens train `
     --backend qlora --tokenizer hf `
     --preset qwen_4080_16g `
    --model Qwen/Qwen3.5-4B `
     --data-dir target/dogfood `
     --output-dir mens/runs/rtx4080_full `
     --device cuda `
     --background
   ```
   **`--background`** alone attaches logs under **`mens/runs/logs`** (repo root when detected) and returns immediately; equivalent to **`--log-dir mens/runs/logs`**. On Windows the child process is spawned with **breakaway-from-job** flags to reduce IDE teardown killing the trainer. Tail: **`Get-Content mens/runs/logs/train_*.log -Wait -Tail 25`**. Alternatives: **`vox mens train … --background`**, or **`pwsh scripts/populi/release_training_gate.ps1`** only for CI gates (not full training).

   On OOM, use `--preset safe` / `4080_safe`, lower `--seq-len`, raise `--grad-accum`, lower `--rank`, or set `VOX_CANDLE_DEVICE=cpu` (slow).

## First Training Run (Native)

```powershell
.\target\release\vox.exe mens train --data-dir target/dogfood --output-dir mens/runs/v1
```

Or run the end-to-end automation script:

```powershell
.\scripts\run_mens_pipeline.ps1 -DataDir target/dogfood -OutputDir mens/runs/v1 -Backend vulkan
```

Expected outputs:

- `mens/runs/v1/model_final.bin`
- `mens/runs/v1/checkpoint_epoch_*.bin`
- `mens/runs/v1/eval_results.json`
- `mens/runs/v1/benchmark_results.json` (if benchmark gate enabled)

## Quality Gates

- Eval thresholds:
  - `VOX_EVAL_MIN_PARSE_RATE` (default `0.80`)
  - `VOX_EVAL_MIN_COVERAGE` (default `0.60`)
- Strict enforcement:
  - `VOX_EVAL_STRICT=1` to fail run on threshold miss
- Optional held-out benchmark (build with `--features mens-dei`; paths via env):
  - `VOX_BENCHMARK=1` — after training, spawns `vox mens eval-local`
  - `VOX_BENCHMARK_MODEL` — checkpoint path (else auto-detect under output dir)
  - `VOX_BENCHMARK_DIR` — held-out bench directory (default `mens/data/heldout_bench`)

```powershell
.\target\release\vox.exe mens corpus eval target/dogfood/validated_mixed.jsonl -o mens/runs/latest/eval_results.json
```

## Runtime Profiles

- Fast dogfood:
  - 1 epoch, smaller dataset while iterating on pipeline code/docs
- Full run:
  - Full corpus + rustdoc merge and benchmark gate enabled

## Model Card

After training, the model card is rendered from `mens/model_card/`:

```powershell
uv run --project scripts render-model-card --run-dir mens/runs/v1
```

## Dogfood operator checklist (real corpus, 4080 QLoRA)

Use this before claiming a full dogfood run is complete (CI cannot substitute for your GPU box).

**Cursor / agents:** full **`vox ci mens-gate`** can exceed tool timeouts — use **`pwsh scripts/populi/release_training_gate.ps1 -Detach`** and tail **`target/mens-gate-logs/`** (see [mens-training.md](../reference/mens-training.md)).

1. **Corpus**: `mens corpus mix --config mens/config/mix.yaml` → copy/rename to **`target/dogfood/train.jsonl`** (preflight requires that filename in `--data-dir`).
2. **Build**: **`cargo vox-cuda-release`** natively from a `vcvars64.bat` loaded interactive terminal (`nvcc` relies on absolute discovery and crashes in subshells).
3. **Train**: `vox mens train --backend qlora --tokenizer hf --preset qwen_4080_16g` (or **`--preset 4080`**, same profile) + `--model`, `--data-dir`, `--output-dir`, `--device cuda`; keep `--qlora-require-full-proxy-stack` on for strict native shard completeness.
4. **Artifacts**: Confirm **`candle_qlora_adapter.safetensors`**, **`candle_qlora_adapter_meta.json`**, **`populi_adapter_manifest_v3.json`**, **`training_manifest.json`**, **`telemetry.jsonl`** under the output dir.
5. **Merge / serve**: Candle merge is **`vox schola merge-qlora`** (f32 shard subsets); **`vox mens serve`** stays Burn-only — see SSOT [Merge / export](../reference/mens-training.md#merge--export--inference).
6. **Optional automation**: [`scripts/mens/train_dogfood.vox`](../../../scripts/mens/train_dogfood.vox) documents the native-tier dogfood entrypoint; see [scripts/README.md](../../../scripts/README.md).

## See Also

- [Native ML Training Pipeline](../explanation/expl-ml-pipeline.md)
- [How To: Publish Mens to Hugging Face](../reference/mens-cloud-gpu.md)
- [scripts/README.md](../../../scripts/README.md) — thin delegates + optional RTX 4080 QLoRA helper script


