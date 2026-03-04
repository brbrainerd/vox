---
title: "Native ML Training Pipeline"
category: explanation
constructs: [function, actor, workflow]
last_updated: 2026-03-02
training_eligible: true
difficulty: advanced
---

# Native ML Training Pipeline

Vox "dogfoods" itself: the language, compiler, and documentation all feed a native machine learning loop that trains the **Populi** code assistant model. The entire pipeline runs in Rust using [Burn](https://burn.dev) — **no Python and no CUDA required**.

**Native training is GPU-compatible without CUDA or Python.** It uses **wgpu** (Vulkan, DirectX 12, or Metal) for GPU acceleration. Your GPU is used automatically; CUDA is not used. Set `VOX_BACKEND=cpu` only when running in CI or on machines without GPU drivers.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  DATA SOURCES                                               │
│  examples/*.vox ──────────┐                                 │
│  docs/src/*.md (code  ────┤──► vox corpus extract          │
│    blocks with frontmatter)│         │                       │
│  vox-cli generate-data ───┘         │                       │
└─────────────────────────────────────│───────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  CORPUS PIPELINE                                            │
│  populi/data/validated.jsonl   (raw Vox → instruction pairs)│
│        │                                                    │
│        ▼                                                    │
│  vox corpus validate           (filter malformed pairs)     │
│        │                                                    │
│        ▼                                                    │
│  populi/data/train.jsonl       (rated + filtered pairs)     │
└─────────────────────────────────────│───────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  TRAINING                                                  │
│                                                             │
│  Default: Native Burn (Rust) — GPU, no CUDA, no Python       │
│  vox train --data-dir target/dogfood --output-dir populi/runs │
│  → Uses VoxTransformer (12-layer, 8-head, 512-dim)          │
│  → wgpu (Vulkan/DX12/Metal); VOX_BACKEND=cpu for CI only   │
│  → Checkpoints per-epoch to output-dir                       │
│                                                             │
│  Legacy: Python QLoRA (>7B models only)                     │
│  vox train --provider local --data-dir populi/data          │
│  → 4-bit quantized fine-tune via train_qlora.vox            │
│  → Requires Python (uv), CUDA GPU (≥8GB VRAM)               │
└─────────────────────────────────────────────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  EVAL + BENCHMARK GATES                                     │
│  vox corpus eval train.jsonl → eval_results.json           │
│  vox corpus benchmark populi/data/heldout_bench → benchmark_results.json │
│  Targets: vox_parse_rate ≥70%, coverage ≥50% (CI); VOX_EVAL_STRICT=1 fails promotion │
│  Held-out: VOX_BENCHMARK=1, VOX_BENCHMARK_MIN_PASS_RATE (default 0) │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Schema

All training pairs follow this JSONL schema (must match across all tools):

```json
{
  "prompt": "Write a Vox actor that tracks a counter",
  "response": "actor Counter:\n    state count: int = 0\n    on increment() to int:\n        count = count + 1\n        count",
  "category": "actor",
  "rating": 5,
  "schema_version": "vox_dogfood_v1"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `prompt` | string | ✅ | The instruction/question |
| `response` | string | ✅ | Valid Vox code |
| `category` | string | recommended | Construct type (function, actor, etc.) |
| `rating` | u8 1-5 | recommended | Quality rating; 5=ground truth docs |
| `schema_version` | string | optional | Version for migration tracking |

---

## Tokenizer

`vox-tensor` includes a **deterministic, dependency-free character-level tokenizer** (`VoxTokenizer`):

- **95 printable ASCII characters** (IDs 3-97)
- **35 Vox compound tokens** (workflow, actor, fn , @component, etc.)
- **3 control tokens**: `[PAD]=0`, `[UNK]=1`, `[EOS]=2`
- **Total vocab**: 133 tokens

```vox
// Vox example — tokenized natively using VoxTokenizer
fn greet(name: str) to str:
    "Hello, " + name
```

Encoding uses greedy longest-match on compound tokens before falling back to single chars.

---

## VoxTransformer Architecture

The native Burn-backed model (`crates/vox-tensor/src/nn.rs`):

| Parameter | Value | Notes |
|-----------|-------|-------|
| Layers | 12 | Transformer encoder blocks |
| Attention heads | 8 | Multi-head self-attention |
| Model dimension | 512 | Embedding size |
| FFN dimension | 2048 | Feed-forward inner size |
| Dropout | 0.1 | Applied in attention + FFN |
| Max sequence length | 512 | Tokens per training example |
| Vocab size | 133 | VoxTokenizer vocabulary |

---

## Running the Pipeline

### 1. Generate synthetic training data

```bash
vox generate-data --limit 500 --output populi/data/train.jsonl
```

### 2. Extract corpus from real Vox files (canonical flow, PowerShell)

```powershell
.\target\release\vox.exe corpus extract examples/ -o populi/data/validated.jsonl
.\target\release\vox.exe corpus extract docs/ -o populi/data/validated.jsonl 2>$null
.\target\release\vox.exe corpus validate populi/data/validated.jsonl --no-recheck -o populi/data/validated.jsonl
.\target\release\vox.exe corpus pairs populi/data/validated.jsonl -o target/dogfood/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/
# Rustdoc merge skipped: response is Rust prose, not Vox code
```

### 3. Start local training (native Rust — GPU by default, no CUDA/Python)

```powershell
# Uses wgpu (Vulkan/DX12/Metal); no CUDA or Python required
.\target\release\vox.exe train --data-dir target/dogfood --output-dir populi/runs/v1
# For CI or CPU-only:
$env:VOX_BACKEND="cpu"; .\target\release\vox.exe train --data-dir target/dogfood --output-dir populi/runs/v1
```

### 4. Check eval gate

```powershell
.\target\release\vox.exe corpus eval target/dogfood/train.jsonl -o populi/runs/v1/eval_results.json
```

---

## Documentation → Training Pair Loop

Every documentation page with `training_eligible: true` in its frontmatter and a ` ```vox ` code block automatically contributes training pairs via `vox corpus pairs --docs docs/src/`.

This creates a **closed feedback loop**: better docs → more training data → better model → better completions → easier to write docs.

**Frontmatter format for training-eligible docs**:

```yaml
---
title: "My Guide"
category: how-to
constructs: [function, workflow]
training_eligible: true
difficulty: intermediate
---
```

---

## CI Integration

The ML pipeline runs automatically via `.github/workflows/ml_data_extraction.yml`:

- **Nightly**: Full corpus re-extraction at 4 AM UTC
- **On push**: Triggered when `*.vox`, compiler crates, or `docs/src/**` change
- **Manual**: `workflow_dispatch` with `force_train` or `native_train` option
- **Grammar drift**: Fingerprint check forces full re-extraction when syntax changes

### CI training job (GPU runner)

The **train** job runs on a self-hosted GPU runner when corpus changes or when manually triggered:

- **Native path (default)**: Runs `vox train` with `VOX_BACKEND=cpu` for CI compatibility. No Python required.
- **Legacy QLoRA path**: Set `native_train: false` in workflow_dispatch; runs `vox train --native false --provider local`. Requires Python (uv) and CUDA for >7B models.
- **Eval strict mode**: `VOX_EVAL_STRICT=1` — training fails when eval gate thresholds are not met.
- **Benchmark gate**: `VOX_BENCHMARK=1` — runs held-out benchmark from `populi/data/heldout_bench/`; `VOX_BENCHMARK_MIN_PASS_RATE` (e.g. 0.80) fails promotion when pass rate is below threshold.
- **Artifact retention**: LoRA adapter `target/dogfood/run/` uploaded as `lora-adapter-$VCS_SHA`, retained 90 days. Eval results `eval_results.json` / `eval_gate_failed.json` retained 30 days.
- **Logging**: Training pair count and eval gate result (parse rate, coverage) are printed; eval gate failure writes `eval_gate_failed.json` and emits a warning.

### Runbook: Native training in CI

```bash
# CI uses VOX_BACKEND=cpu by default (no GPU drivers required)
VOX_BACKEND=cpu vox train --data-dir target/dogfood --output-dir target/dogfood/run
```

### Runbook: Evol-Instruct (optional, gated)

```bash
# Set EVOL_GATE=1 to enable; requires local LLM at EVOL_ENDPOINT (default localhost:11434)
EVOL_GATE=1 vox corpus evol target/dogfood/train.jsonl -o populi/data/evolved_pairs.jsonl --limit 50
# Merge evolved pairs into train.jsonl if desired
```

### Runbook: Optional extra corpus merge

If you have additional curated JSONL data, merge it into the canonical train set before training:

```bash
vox corpus merge target/dogfood/train.jsonl path/to/extra/train.jsonl -o target/dogfood/train.jsonl --dedup
```

### Train matrix (canonical)

| Mode | Command | When to use |
|------|---------|-------------|
| Native (default) | `vox train --native` | ≤7B models, no Python/CUDA |
| Legacy QLoRA | `vox train --native false --provider local` | >7B, CUDA GPU, Python/uv |
| CI strict | `VOX_EVAL_STRICT=1` | Fail promotion on eval gate failure |
| CI benchmark | `VOX_BENCHMARK=1` | Run held-out benchmark before promotion |

Artifact layout: `target/dogfood/train.jsonl` (canonical input), `target/dogfood/run/` (output). Version naming: `lora-adapter-$VCS_SHA`, `eval-gate-$VCS_SHA`.

---

## Next Steps

- [Actors & Workflows](expl-actors-workflows.md) — Build durable constructs for the training pipeline
- [CLI Reference](ref-cli.md) — Full `vox corpus` and `vox training` command reference
- [Architecture Overview](expl-architecture.md) — How the compiler pipeline works
