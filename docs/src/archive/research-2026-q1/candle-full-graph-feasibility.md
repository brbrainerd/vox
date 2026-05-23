---
title: "Feasibility: full-graph Candle training (qlora-rs)"
description: "Official documentation for Feasibility: full-graph Candle training (qlora-rs) for the Vox language."
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# Feasibility: full-graph Candle training (qlora-rs)

**Decision (2026-03):** keep **Candle** on the **proxy stack** (`o_proj` / GPT-2 `c_proj` + LM head) using public **qlora-rs** `QLoraTrainer::training_step_lm` over `&[&QuantizedLinear]` (ADR 007).

**Rationale:** full MHA + FFN in NF4 inside Candle would require either (a) a much larger in-tree graph aligned to every HF layout, or (b) upstream qlora-rs APIs beyond current sequential LM helper. **Burn** owns **full-graph f32 LoRA** today; **Candle** owns **practical NF4 QLoRA** on the bounded proxy.

**Suffix training:** CLI **`--qlora-ce-last-k K`** (default 1) applies the same embed→proxy→LM head to **multiple final token positions** per JSONL row, improving alignment with next-token LM on a sequence suffix without implementing full causal depth in Candle.

**Revisit when:** Burn ships production NF4 bases + unified adapter merge parity, or qlora-rs exposes a richer block training API without forking.


