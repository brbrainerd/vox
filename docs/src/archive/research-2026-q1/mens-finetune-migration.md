---
title: "Migration: backend-centric flags → fine-tune contract"
description: "Official documentation for Migration: backend-centric flags → fine-tune contract for the Vox language."
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# Migration: backend-centric flags → fine-tune contract

## What changed

- **`vox mens train`** still uses `--backend lora|qlora`, but validation is **contract-first** inside **`vox-populi`** (`FineTuneContract`, `ExecutionPlanner`, `preflight_train`).
- **`--tokenizer hf`** is valid with **`--backend lora`** when the HF `config.json` is **GPT-2-shaped** (see planner gate). Llama/Mistral/Qwen layouts → **`--backend qlora`** until Burn HF parity lands.
- **Telemetry** adds stable keys under `telemetry_schema` (`execution_kernel`, `telemetry_schema` version, `candle_compat_mode` for Candle).
- **Training manifest** may include `manifest_schema_version`, `execution_kernel`, `finetune_contract_digest` (older runs default via serde).
- **Candle runs** emit **`populi_adapter_manifest_v3.json`** next to v2 meta; **`vox schola merge-qlora`** accepts **v2 or v3** meta JSON.
- **Alias:** `vox mens merge-adapter` → same as `merge-qlora`.

## Actions for operators

- Prefer **`vox mens train`** over legacy `vox train --native-lora` (already deprecated in CLI messaging).
- For QLoRA/NF4, keep **`--backend qlora --tokenizer hf --model …`**.


