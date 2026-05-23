---
title: "Journey: Native Rust LLM Training"
description: "How to use Vox's native ML toolchain to fine-tune open weights directly from your application data without diving into Python environments."
category: "Getting Started"
sort_order: 4

schema_type: "HowTo"
---

# Journey: Native Rust LLM Training

## The Curse of Python ML Environments

When you have domain-specific application data housed in a Rust or typical structured backend and want to use it to fine-tune a model, you hit a massive tooling disconnect.

You have to pull the data directly from production, dump it into JSONL files, transfer them, spin up complex Virtual Environments (venv/Conda), manage nested CUDA PyTorch dependencies, and fight Python multi-threading environments in Jupyter notebooks. Your application logic effectively divorces the ML operations layer.

## The Vox Paradigm: Zero-Python Native Fine-tuning

The Vox toolchain resolves this tension by providing native hardware-accelerated **QLoRA** fine-tuning via **MENS**: **`vox mens train`** dispatches **Candle + qlora-rs** in **`vox-populi`** (HF weights through **Rust `hf-hub`**). **`vox-tensor`** supplies **`VoxTokenizer`**, JSONL loading, and the **Burn** scratch path — a different lane from HF QLoRA.

You can extract corpus pairs, assemble **`train.jsonl`**, and run training **without a Python training loop**. The operator surface is the **CLI** and corpus commands today; in-language orchestration remains a product direction.

Authoritative pipeline map (sources → compiler → goldens → corpus → Mens): [Vox source → Mens pipeline SSOT](../archive/research-2026-q1/vox-source-to-mens-pipeline-ssot.md). Dataset contract: [Mens training data contract](../reference/mens-training-data-contract.md).

## Illustrative snippet (not the shipped CLI)

The following **Vox-shaped pseudocode** sketches how training might be expressed in source; the **supported path today** is **`vox mens train`** (see [mens-training.md](../reference/mens-training.md)).

```vox
// vox:skip
// Illustrative imports — operator workflow uses: vox mens train …
import vox.mens.training
import vox.mens.qlora

// We assume we have a table of high-quality agent queries and outputs.
@table type AgentTelemetry {
    query: str
    optimal_response: str
}

@action
fn finetune_from_telemetry() to Result[str] {
    // 1. Fetch training subset directly from your database
    let records = db.query(AgentTelemetry).take(5000);
    
    // 2. Map structural DB logic into instruction dataset layout
    let dataset = records.map(fn(r) {
        { prompt: r.query, completion: r.optimal_response }
    });
    
    // 3. Initiate a hardware-accelerated QLoRA training session (Candle backend)
    let session = training.qlora_finetune(
        dataset,
        "base_models/Meta-Llama-3-8B-Instruct",
        {
            r: 16,
            lora_alpha: 32,
            target_modules: ["q_proj", "v_proj"],
            batch_size: 4,
            epochs: 3
        }
    )?
    
    return Ok("Trained adapter saved to: " + session.adapter_path)
}
```

## Running the process (operator)

On NVIDIA hardware, build **`vox-cli`** with **`mens-candle-cuda`** (see [mens-training.md](../reference/mens-training.md) and workspace build notes in `AGENTS.md`). Then:

```bash
vox mens corpus pairs …   # produce target/dogfood/train.jsonl (see expl-ml-pipeline)
vox mens train --device cuda --data-dir target/dogfood --output-dir mens/runs/latest
```

`--backend qlora` and `--tokenizer hf` are **defaults**: weights are fetched natively; **no PyTorch** training stack.

## Maturity and limitations

- **Maturity:** `stable` for the **`vox mens train`** CLI path on supported presets; GPU kernels require the documented CUDA build alias (see `AGENTS.md`).
- **Limitation ids:** [L-005](../../../contracts/journeys/limitations.v1.yaml) (default `vox-cli` build may omit GPU train/serve features until rebuilt with the Mens CUDA feature set).

## Deep Dives

- **[ADR 003 — Native Rust Training Over Python](../adr/003-native-training-over-python.md)**: Why the project left Python/Unsloth for the pipeline, and how **native Candle QLoRA** superseded the “Python for QLoRA” assumption.
- **[ADR 006 — Mens full-graph Candle QLoRA with qlora-rs](../adr/006-mens-full-graph-qlora-qlora-rs.md)**: qlora-rs integration and scope.
- **[Native ML Training Pipeline](../explanation/expl-ml-pipeline.md)**: Corpus → **`vox mens train`** → eval gates.
- **[Mens native training SSOT (Candle QLoRA)](../reference/mens-training.md)**: Contract, preflight, merge/serve matrix, and CLI truth table.
