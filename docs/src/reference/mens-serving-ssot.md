---
title: "Mens local serving SSOT (Schola + orchestrator)"
description: "Single operator story for serving Candle QLoRA training outputs: vox-schola HTTP, POPULI_URL, orchestrator mesh config, and external handoff."
category: "Language Reference"
last_updated: "2026-04-07"
training_eligible: true

schema_type: "TechArticle"
---

# Mens local serving SSOT (Schola + orchestrator)

## What this page is for

After **`vox mens train`** / **`vox-schola train`** (Candle QLoRA, default), the **supported local inference server** is **`vox-schola serve`** (also reached via **`vox mens serve --model <run_dir>`**, which spawns `vox-schola`). It loads the run directory (`candle_qlora_adapter.safetensors`, `tokenizer.json`, shards) and exposes:

- **`POST /v1/chat/completions`** — OpenAI Chat Completions
- **`POST /api/chat`** — Ollama-shaped chat (used by MCP `vox-mcp` when the provider is Ollama)
- **`POST /api/generate`** — Ollama-shaped generate (**required** for **`vox-gamify`** streaming and **`vox-actor-runtime` `PopuliClient::generate`**)
- **`GET /api/tags`** — model list for probes
- **`GET /api/version`** — JSON including a **`cuda`** hint when `--device` is CUDA (for capability probes)
- **`POST /api/embeddings`** — **501** (not implemented; use Ollama.app or another stack for embeddings)

This is **not** the same process as **Ollama.app** on `http://localhost:11434`, but it speaks a **compatible subset** of Ollama HTTP so you can point **`POPULI_URL`** (or **`OLLAMA_URL`**) at Schola’s listen address.

## Quick start

1. Train (example): `vox mens train --device cuda --output-dir mens/runs/latest`
2. Serve: `vox-schola serve --model mens/runs/latest --port 11435 --model-name my-mens`  
   (or `vox mens serve --model mens/runs/latest` with the same effective flags where forwarded)
3. Point clients at Schola:
   - **`POPULI_URL=http://127.0.0.1:11435`** (precedence over **`OLLAMA_URL`**; see [`vox_config::inference::local_ollama_populi_base_url`](../../../crates/vox-config/src/inference.rs))
   - **`POPULI_MODEL=my-mens`** must match the name returned by **`GET /api/tags`** (Schola’s `--model-name`, else the run directory’s final path component)

## Orchestrator and agent-to-agent

The in-tree orchestrator’s **`AiTaskProcessor`** uses **`vox_gamify::FreeAiClient`**, which calls **`POST …/api/generate`** for the local Ollama lane. **Schola implements `/api/generate`**, so orchestrator streaming works when **`POPULI_URL`** targets Schola.

**`Vox.toml` `[mesh]`** (or legacy **`[mens]`**) can record a stable inference base for operators and tooling:

```toml
[mesh]
control_url = "http://127.0.0.1:9847"   # Populi mesh control plane (optional)
inference_base_url = "http://127.0.0.1:11435"  # Schola or Ollama-shaped server
```

This maps to **`OrchestratorConfig::populi_inference_base_url`**. **Processes still read `POPULI_URL` from the environment** today: when starting workers or daemons, set **`POPULI_URL`** to that value (or export **`VOX_ORCHESTRATOR_POPULI_INFERENCE_BASE_URL`** and copy into **`POPULI_URL`** in your launcher). The config field is the **SSOT for the intended URL** in workspace TOML.

The default model registry uses **`POPULI_MODEL`** for the local Ollama provider entry ([`ModelConfig::default`](../../../crates/vox-orchestrator/src/models/spec.rs)); keep it aligned with Schola’s advertised model id.

## MCP

MCP’s Ollama bridge uses **`POST /api/chat`**, which Schola already supported. With **`OLLAMA_HOST`** or equivalent base URL pointing at Schola, MCP and Schola interoperate without code changes.

## Machine-readable handoff

Training completion writes **`external_serving_handoff_v1.json`** in the run directory (schema: [`contracts/eval/external-serving-handoff.schema.json`](../../../contracts/eval/external-serving-handoff.schema.json)). **`vox mens merge-qlora`** / **`vox-schola merge`** write the same filename next to the merged shard’s parent directory for **external** (vLLM / HF / Ollama import) workflows.

## Burn `vox mens serve` (`execution-api`)

A separate, **Burn checkpoint** HTTP server exists behind **`execution-api`** for **`*.bin` / `merge-weights`** artifacts. That path is **not** the default QLoRA story; prefer Schola for trained QLoRA runs. See [Mens native training SSOT](mens-training.md) for the train → merge → serve matrix.

## Related

- [Mens native training SSOT (Candle QLoRA–first)](mens-training.md)
- [Model routing and provider cascade](../how-to/how-to-model-routing.md)

