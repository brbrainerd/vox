---
title: "Populi data pipeline (control plane vs Mens corpus)"
description: "Clarifies Populi/mesh runtime data paths versus Mens training corpus sources—two different pipelines that share orchestration branding."
category: "Architecture SSOTs"
status: "current"
sort_order: 4
last_updated: "2026-04-12"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Populi data pipeline (control plane vs Mens corpus)

**Populi** in this repo names the **HTTP mesh / control plane** (`VOX_MESH_*`, node registry, A2A, optional GPU hints). That is **runtime coordination data**, not the same artifact stream as **Mens training JSONL**.

## Mesh / control plane (operational)

- **SSOT:** [mens / Populi reference](../reference/populi.md) (env contract, HTTP API shapes).
- **Telemetry:** optional Codex rows for control events—see [orchestration unified](../reference/orchestration-unified.md).
- **Examples:** mesh worker script lives at `examples/golden/mesh/noop.vox` (Docker `/opt/vox/mesh-noop.vox`).

## Mens training corpus (offline ML)

- **SSOT:** [Vox source → Mens pipeline](vox-source-to-mens-pipeline-ssot.md), [Native ML pipeline](../explanation/expl-ml-pipeline.md), [Mens native training](../reference/mens-training.md).
- **Sources:** primarily `examples/golden/**/*.vox` plus configured mix paths (`vox mens pipeline`, `vox_corpus`).

## Rule of thumb

| Question | Answer |
| --- | --- |
| Where do I add a **verified** `.vox` snippet for docs? | `examples/golden/` + `{{#include}}`; see [`examples.ssot.v1.yaml`](../../../examples/examples.ssot.v1.yaml). |
| Where do mesh nodes register? | Populi HTTP client + registry—see Populi reference. |
| What tokenizes Mens supervised strings? | **HF tokenizer** for the base model on the QLoRA path—not the Vox lexer. |


