---
title: "How To: Train Populi on RTX 4080 Super"
category: how-to
constructs: [function, workflow]
last_updated: 2026-03-04
training_eligible: true
difficulty: intermediate
---

# How To: Train Populi on RTX 4080 Super

This runbook is the canonical first-time path to train Populi locally on Windows with PowerShell, using Vox native training.

## Recommended Path

- Default: `vox train --native --data-dir target/dogfood --output-dir populi/runs/v1`
- Input contract: `target/dogfood/train.jsonl`
- Backend: `wgpu` on Windows (Vulkan or DX12); no CUDA/Python required
- Fallback only: QLoRA for large-model workflows that need Python quantization libraries

## Prerequisites

1. Build Vox CLI (release binary):
   ```powershell
   & "$env:USERPROFILE\.cargo\bin\cargo.exe" build -p vox-cli --release
   ```
2. Generate canonical corpus input:
   ```powershell
   New-Item -ItemType Directory -Force -Path populi/data,target/dogfood | Out-Null
   .\target\release\vox.exe corpus extract examples/ -o populi/data/validated.jsonl
   .\target\release\vox.exe corpus extract docs/ -o populi/data/validated.jsonl 2>$null
   .\target\release\vox.exe corpus validate populi/data/validated.jsonl --no-recheck -o populi/data/validated.jsonl
   .\target\release\vox.exe corpus pairs populi/data/validated.jsonl -o target/dogfood/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/
   # Rustdoc merge skipped: response is Rust prose, not Vox code
   ```
3. Optional GPU backend selection:
   ```powershell
   $env:VOX_BACKEND = "vulkan"   # or "dx12" or "cpu"
   ```

## First Training Run (Native)

```powershell
.\target\release\vox.exe train --native --data-dir target/dogfood --output-dir populi/runs/v1
```

Or run the end-to-end automation script:

```powershell
.\scripts\run_populi_pipeline.ps1 -DataDir target/dogfood -OutputDir populi/runs/v1 -Backend vulkan
```

Expected outputs:

- `populi/runs/v1/model_final.bin`
- `populi/runs/v1/checkpoint_epoch_*.bin`
- `populi/runs/v1/eval_results.json`
- `populi/runs/v1/benchmark_results.json` (if benchmark gate enabled)

## Quality Gates

- Eval thresholds:
  - `VOX_EVAL_MIN_PARSE_RATE` (default `0.80`)
  - `VOX_EVAL_MIN_COVERAGE` (default `0.60`)
- Strict enforcement:
  - `VOX_EVAL_STRICT=1` to fail run on threshold miss

```powershell
.\target\release\vox.exe corpus eval target/dogfood/train.jsonl -o populi/runs/v1/eval_results.json
```

## Runtime Profiles

- Fast dogfood:
  - 1 epoch, smaller dataset while iterating on pipeline code/docs
- Full run:
  - Full corpus + rustdoc merge and benchmark gate enabled

## See Also

- [Native ML Training Pipeline](expl-ml-pipeline.md)
- [scripts/README.md](../../scripts/README.md) - QLoRA fallback details
