---
title: "Deprecation policy — Mens native fine-tuning"
description: "Official documentation for Deprecation policy — Mens native fine-tuning for the Vox language. Detailed technical reference, architectur"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# Deprecation policy — Mens native fine-tuning

## Stable

- **`vox mens train`** with `--backend lora` and `--backend qlora`.
- **`vox schola merge-qlora`** (alias **`merge-adapter`**).
- **`vox mens merge-weights`** for Burn `*.bin` LoRA checkpoints.

## Deprecated / transitional

- **`vox train --native-lora`**: use **`vox mens train --backend lora`** (stderr deprecation already emitted from dispatch).
- **Backend-only mental model**: prefer the **contract** fields (tokenizer mode, quant mode, adapter method) when scripting; CLI flags remain the user-facing surface until a preset/JSON contract ships.

## Timeline

- No CLI flags removed in this iteration; aliases added (`merge-adapter`).
- Future removal of legacy paths will be announced in this doc + `mens-training.md` with one release notice.


