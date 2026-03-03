---
title: "Native ML Training Pipeline"
category: explanation
constructs: [function, actor, workflow]
last_updated: 2026-03-02
training_eligible: true
difficulty: advanced
---

# Native ML Training Pipeline

Vox "dogfoods" itself: the language, compiler, and documentation all feed a native machine learning loop that trains the **Populi** code assistant model. The entire pipeline runs in Rust using [Burn 0.19](https://burn.dev) — no Python required for the core loop.

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
│  TRAINING (choose one)                                      │
│                                                             │
│  Option A: Native Burn (Rust)                               │
│  vox training native --data-dir populi/data                 │
│  → Uses VoxTransformer (12-layer, 8-head, 512-dim)          │
│  → Wgpu backend (GPU) or NdArray (CPU fallback)             │
│  → Checkpoints per-epoch to populi/runs/v1/                 │
│                                                             │
│  Option B: Python QLoRA (HuggingFace / Unsloth)             │
│  uv run vox-train --model Qwen2.5-Coder-1.5B               │
│  → 4-bit quantized fine-tune on top of base model           │
│  → Requires CUDA GPU (≥8GB VRAM recommended)                │
└─────────────────────────────────────────────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  EVAL GATE                                                  │
│  vox corpus eval train.jsonl → eval_results.json           │
│  Targets: vox_parse_rate > 80%, construct_coverage > 60%   │
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

### 2. Extract corpus from real Vox files

```bash
vox corpus extract examples/ -o populi/data/validated.jsonl
vox corpus pairs populi/data/validated.jsonl \
  -o populi/data/train.jsonl \
  --docs docs/src/
```

### 3. Start local training (native Rust — safe for any hardware)

```bash
vox training native \
  --data-dir populi/data \
  --output-dir populi/runs/v1
```

### 4. Check eval gate

```bash
vox corpus eval populi/data/train.jsonl \
  -o populi/runs/v1/eval_results.json
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
- **Manual**: `workflow_dispatch` with `native_train` option
- **Grammar drift**: Fingerprint check forces full re-extraction when syntax changes

---

## Next Steps

- [Actors & Workflows](expl-actors-workflows.md) — Build durable constructs for the training pipeline
- [CLI Reference](ref-cli.md) — Full `vox corpus` and `vox training` command reference
- [Architecture Overview](expl-architecture.md) — How the compiler pipeline works
