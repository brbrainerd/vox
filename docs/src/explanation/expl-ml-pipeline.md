---
title: "Native ML Training Pipeline"
description: "End-to-end Mens ML pipeline: corpus → native Candle+qlora-rs QLoRA via vox mens train; Burn scratch path legacy."
category: "Concepts"
last_updated: "2026-04-12"
training_eligible: true

schema_type: "TechArticle"
---

# Native ML Training Pipeline

Vox "dogfoods" itself: the language, compiler, and documentation all feed a native machine learning loop that trains the **Mens** code assistant model.

End-to-end map from `.vox` sources through goldens and corpus extraction to model inputs: [Vox source → Mens pipeline SSOT](../archive/research-2026-q1/vox-source-to-mens-pipeline-ssot.md). Training pair contract: [Mens training data contract](../reference/mens-training-data-contract.md).

**Canonical operator fine-tuning:** **`vox mens train`** with **Candle + qlora-rs** on **Hugging Face** weights. **`--backend qlora`** and **`--tokenizer hf`** are the **defaults**; no Python training loop. SSOT: [Mens native training](../reference/mens-training.md). **`PopuliTrainBackend::BurnLora` is rejected at runtime** in this dispatch — the supported trainer is **`CandleQlora`**.

**Legacy / side paths:** A **Burn + wgpu** scratch **LoRA** stack still lives in **`vox-tensor`** (`vox training native`, small `VoxTokenizer` model) — **no Python**, optional **CUDA** only if you build GPU features for other subsystems. Use it for experimentation, **not** as a substitute for Mens HF QLoRA. **Burn** also matters for **`vox mens merge-weights`** and **`vox mens serve`** on merged `.bin` checkpoints. Objectives and artifacts differ from Candle QLoRA — see [Burn vs QLoRA](../reference/mens-training.md#burn-lora-vs-candle-qlora--which-path-when-4080-super-and-beyond).

**GPUs:** For **QLoRA** on an NVIDIA workstation, build **`mens-candle-cuda`** and use **`vox mens train --device cuda`**. For **Burn scratch** training, **wgpu** (Vulkan / DX12 / Metal) is the default GPU path. Use CPU when drivers or CI forbid GPU.

---

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│  DATA SOURCES                                               │
│  golden/**/*.vox + examples.ssot.v1.yaml ──┐                │
│  docs … golden .vox ───┤──► vox mens corpus extract         │
│    (+ prose per mix policy)│         │                      │
│  vox-cli generate-data ───┘         │                       │
└─────────────────────────────────────│───────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  CORPUS PIPELINE                                            │
│  mens/data/validated.jsonl   (raw Vox → instruction pairs)│
│        │                                                    │
│        ▼                                                    │
│  vox mens corpus validate    (filter malformed pairs)     │
│        │                                                    │
│        ▼                                                    │
│  mens/data/train.jsonl       (rated + filtered pairs)     │
└─────────────────────────────────────│───────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  TRAINING (Mens — canonical)                                │
│                                                             │
│  **`vox mens train`** — Candle + **qlora-rs** QLoRA (default) │
│  `--backend qlora` + `--tokenizer hf` + HF safetensors      │
│  Optional **CUDA** (`mens-candle-cuda`) / **Metal**          │
│  SSOT: `reference/mens-training.md`                         │
│                                                             │
│  Legacy / other: `vox training native` — Burn scratch LoRA  │
│  (`VoxTokenizer` JSONL, wgpu/CPU). Not `vox mens` dispatch.   │
│  `vox train` (mens-dei): local bails → `vox mens train …`   │
└─────────────────────────────────────────────────────────────┘
                                      ▼
┌─────────────────────────────────────────────────────────────┐
│  EVAL + BENCHMARK GATES                                     │
│  vox mens corpus eval … → eval_results.json               │
│  VOX_BENCHMARK=1 → spawns vox mens eval-local (held-out)  │
│  Targets: vox_parse_rate ≥70%, coverage ≥50% (CI); VOX_EVAL_STRICT=1 fails promotion │
│  Held-out: VOX_BENCHMARK=1, VOX_BENCHMARK_MIN_PASS_RATE (default 0) │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Schema

All training pairs follow this JSONL schema (must match across all tools):

```json
{
  "prompt": "Write a minimal Vox program that prints hello",
  "response": "fn main() {\n    print(\"hello\")\n}\n",
  "category": "function",
  "rating": 5,
  "schema_version": "vox_dogfood_v1"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `prompt` | string | ✅ | The instruction/question (serde also accepts **`instruction`**) |
| `response` | string | ✅ | Valid Vox code (serde also accepts **`output`**) |
| `category` | string | recommended | Construct type (function, actor, etc.) |
| `rating` | u8 1-5 | recommended | Quality rating; 5=ground truth docs |
| `schema_version` | string | optional | Version for migration tracking |

---

## Tokenizer (training vs compile)

**Compile path:** source text is lexed by **`vox-compiler`** (`logos` [`Token`](../../../crates/vox-compiler/src/lexer/token.rs) enum)—this is unrelated to Mens model vocabulary. See [Vox source → Mens pipeline SSOT](../archive/research-2026-q1/vox-source-to-mens-pipeline-ssot.md).

**Mens QLoRA path (default):** supervised strings are tokenized with the **Hugging Face tokenizer** for the chosen `--model` (tens of thousands of BPE tokens). See [Mens native training](../reference/mens-training.md) § Tokenization SSOT.

**Lab / Burn scratch:** `vox-tensor` exposes a **deterministic small `VoxTokenizer`** (not a mirror of the Vox lexer keyword set):

- **95 printable ASCII characters** (IDs 3-97)
- **35 Vox compound tokens** (workflow, actor, fn, component, etc.)
- **3 control tokens**: `[PAD]=0`, `[UNK]=1`, `[EOS]=2`
- **Total vocab**: 133 tokens

```vox
// vox:skip
// Vox example — tokenized natively using VoxTokenizer
fn greet(name: str) to str {
    return "Hello, " + name
}
```

Encoding uses greedy longest-match on compound tokens before falling back to single chars.

---

## VoxTransformer Architecture (Burn scratch path)

The **Burn**-backed scratch transformer (`crates/vox-tensor/src/vox_nn.rs`, `gpu` feature) used with **`VoxTokenizer`** JSONL — distinct from **HF QLoRA** weights:

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
vox generate-data --limit 500 --output mens/data/train.jsonl
```

### 2. Extract corpus from real Vox files (canonical flow, PowerShell)

```powershell
.\target\release\vox.exe mens corpus extract examples/golden/ -o mens/data/validated.jsonl
.\target\release\vox.exe mens corpus extract docs/ -o mens/data/validated.jsonl 2>$null
.\target\release\vox.exe mens corpus validate mens/data/validated.jsonl --no-recheck -o mens/data/validated.jsonl
.\target\release\vox.exe mens corpus pairs mens/data/validated.jsonl -o target/dogfood/train.jsonl --docs docs/src/ --docs docs/src/research/ --docs docs/src/adr/
# Rustdoc merge skipped: response is Rust prose, not Vox code
```

### 3. Start Mens fine-tuning (canonical — Candle QLoRA, native Rust)

```powershell
# Build with CUDA for RTX-class GPUs (see mens-training SSOT / AGENTS.md)
# Then minimal path:
.\target\release\vox.exe mens train --device cuda --data-dir target/dogfood --output-dir target/dogfood/run
```

**Legacy Burn scratch** (small `VoxTokenizer` model, wgpu — not HF QLoRA):

```powershell
$env:VOX_BACKEND="cpu"; .\target\release\vox.exe train --data-dir target/dogfood --output-dir mens/runs/v1
# GPU: omit VOX_BACKEND=cpu when wgpu is available
```

### 4. Check eval gate

```powershell
.\target\release\vox.exe mens corpus eval target/dogfood/validated_mixed.jsonl -o mens/runs/latest/eval_results.json
```

---

## Documentation → Training Pair Loop

Every documentation page with `training_eligible: true` in its frontmatter and a ` ```vox ` code block automatically contributes training pairs via `vox mens corpus pairs --docs docs/src/`.

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

- **Native path (default)**: Prefer **`vox mens train`** with `VOX_BACKEND=cpu` for CI compatibility. Older workflows may still invoke **`vox train`**; **`--provider local`** now **bails** with the canonical Candle QLoRA command (no Python `train_qlora` script).
- **Workflow_dispatch `native_train: false`**: If still wired to **`vox train --provider local`**, expect the **bail** message directing operators to **`vox mens train --backend qlora`**. Use **`vox mens train`** directly in updated automation.
- **Eval strict mode**: `VOX_EVAL_STRICT=1` — training fails when eval gate thresholds are not met.
- **Benchmark gate**: `VOX_BENCHMARK=1` — runs held-out benchmark from `mens/data/heldout_bench/`; `VOX_BENCHMARK_MIN_PASS_RATE` (e.g. 0.80) fails promotion when pass rate is below threshold.
- **Artifact retention**: LoRA adapter `target/dogfood/run/` uploaded as `lora-adapter-$VCS_SHA`, retained 90 days. Eval results `eval_results.json` / `eval_gate_failed.json` retained 30 days.
- **Logging**: Training pair count and eval gate result (parse rate, coverage) are printed; eval gate failure writes `eval_gate_failed.json` and emits a warning.

### Runbook: Native training in CI

```bash
# CI uses VOX_BACKEND=cpu by default (no GPU drivers required)
VOX_BACKEND=cpu vox mens train --data-dir target/dogfood --output-dir target/dogfood/run
```

### Runbook: Evol-Instruct (optional, gated)

**Not wired** on the current slim `vox` binary. Use external tooling or scripts until a `corpus evol` subcommand lands.

```bash
# Intended future shape (not implemented):
# EVOL_GATE=1 vox mens corpus evol …
```

### Runbook: Optional extra corpus merge

Use **`vox mens corpus mix`** with `mens/config/mix.yaml`, or merge JSONL with your own tooling. There is no `vox corpus merge` subcommand today.

### Train matrix (canonical)

| Mode | Command | When to use |
|------|---------|-------------|
| **Mens Candle QLoRA (primary)** | `vox mens train --device cuda` (defaults: `--backend qlora`, `--tokenizer hf`; optional `--model <hf_repo>`) | Native **qlora-rs** + HF weights; CUDA/Metal feature builds; see [mens-training.md](../reference/mens-training.md) |
| Qwen3.5-4B (4080 16GB) | `cargo build -p vox-cli --release --features gpu,mens-candle-cuda` then `vox mens train --preset qwen_4080_16g --device cuda …` | Preset path; full proxy stack defaults on CUDA unless `--qlora-allow-partial-proxy-stack` |
| Burn scratch LoRA | `vox train --data-dir …` / `VOX_BACKEND=cpu` … | **Not** `vox mens` QLoRA — small **VoxTokenizer** model + wgpu/CPU in `vox-tensor` |
| **`vox mens train --backend lora`** | Rejected at runtime | Use **`--backend qlora`** for Mens dispatch (SSOT) |
| Legacy `vox train` (mens-dei) | `vox train …` | **`--provider local`** → bail message → **`vox mens train --backend qlora`**; Together remote; **`--native`** Burn-only scratch |
| CI strict | `VOX_EVAL_STRICT=1` | Fail promotion on eval gate failure |
| CI benchmark | `VOX_BENCHMARK=1` | Run held-out benchmark before promotion |

Artifact layout: `target/dogfood/train.jsonl` (canonical input), `target/dogfood/run/` (output). Version naming: `lora-adapter-$VCS_SHA`, `eval-gate-$VCS_SHA`.

---

## Next Steps

- [ADR 003 — Native training over Python](../adr/003-native-training-over-python.md) — History vs current Candle QLoRA
- [ADR 006 — Mens full-graph Candle QLoRA](../adr/006-mens-full-graph-qlora-qlora-rs.md)
- [Mens native training SSOT](../reference/mens-training.md)
- [Actors & Workflows](expl-actors-workflows.md) — Build durable constructs for the training pipeline
- [CLI Reference](../reference/cli.md) — `vox mens`, `vox train`
- [Architecture Overview](expl-architecture.md) — How the compiler pipeline works

