---
title: "Agent Shell Fluency Eval Design (2026)"
description: "Optional A/B eval design for testing whether agents produce more correct shell commands in PowerShell vs. Bash. NOT a shipped eval — design only."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-30"
training_eligible: false
training_rationale: "Eval design notes; not project policy or model output."
schema_type: "TechArticle"
---
# Agent Shell Fluency Eval Design (2026)

## Status

**Design only — not run, not shipped.** This document exists to answer the question "if we ever wanted to prove agents are better at PowerShell than Bash (or vice versa), what would the cheapest defensible eval look like?" so the question stops circulating without a concrete answer.

The current Vox terminal exec policy ([terminal-exec-policy-ssot.md](terminal-exec-policy-ssot.md)) does **not** depend on this claim and does not need this eval to be run. Decline to run unless someone is making a stronger claim than the SSOT.

## Hypothesis under test

> Holding task and model fixed, an agent emits a working shell command on the first try more often when prompted to use PowerShell 7 than when prompted to use Bash.

"Working" = exit code 0 AND stdout passes a task-specific structural assertion.

This hypothesis is **independent** of the policy claims in the SSOT. The SSOT is about host-side allowlisting and parsing; this eval is about model-side codegen.

## Why this is NOT a MENS spoke

A spoke in the MENS hub-and-spoke architecture is a corpus axis that the model trains against. This eval is an **inference-time A/B**, not a training corpus. Building it as a spoke would:

1. Require generating thousands of (task, bash, pwsh) triples — expensive, and the question can be answered with ~20 tasks × 2 shells × 5 trials = 200 runs.
2. Pre-commit to a training intervention before knowing whether the gap exists or matters.
3. Conflict with [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md) by elevating shell codegen to a corpus citizen when Vox is the glue language.

If the eval shows a real gap and the gap matters for a downstream Vox feature, then a corpus tweak inside the existing Vox spoke is the next step — still not a new spoke.

## Eval shape

**Tasks (n=20).** Pulled from real agent transcripts in this repo's session logs. Each task is a one-shot operational request: "list crates with stale lockfiles," "find files modified in the last week under `crates/vox-ml-cli` larger than 10 KB," "extract the `version` field from every `Cargo.toml` under `crates/`," etc. Mix of file inspection, text processing, process introspection, and network fetch.

**Conditions (n=2).** Identical task prompt, only the shell instruction differs:
- `bash` — "Solve this with a single bash command (or bash one-liner pipeline). Output only the command."
- `pwsh` — "Solve this with a single PowerShell 7 command (or pipeline). Output only the command."

**Trials (n=5 per cell).** Temperature 0.7 to capture variance. 20 × 2 × 5 = 200 runs.

**Models.** Whichever models are in use for agent work in this repo at eval time (currently Opus 4.7, Sonnet 4.6). Run independently per model — do not pool.

**Scoring.** For each run, execute the emitted command in a clean sandbox and apply a per-task structural check on stdout (e.g. "is valid JSON," "every line matches `^[a-z0-9-]+ \d+\.\d+\.\d+$`"). Binary pass/fail. No partial credit — the agent either produced a correct one-shot or it did not.

**Primary metric.** Pass rate per (model, shell). Report `pass@1` and `pass@5` (any of 5 trials passes).

**Secondary metric.** Wall-clock to first pass, only for runs that pass.

## What the result would mean

| Outcome | Interpretation |
|---|---|
| pwsh > bash by ≥10pp on pass@1 | Mild evidence to prefer pwsh prompts in this repo's agent surfaces. Still not a MENS spoke. |
| Gap < 5pp either way | Hypothesis falsified; treat as noise. Do not change anything. |
| bash > pwsh by ≥10pp | Existing PS-first stance is purely a policy/parsing argument; do not extend it to codegen. |

## What this eval does NOT measure

- Whether agents follow up correctly on shell errors (this is one-shot only).
- Whether the emitted command is **safe** (the policy SSOT covers this; eval bypasses sandboxing).
- Whether agents correctly use Vox (`vox run scripts/foo.vox`) instead of either shell. That is a separate eval and is the claim that actually matters for this repo's policy.

## Cost estimate

- Implementation: ~1 engineer-day (task list, harness, scoring rubrics).
- Run: ~200 model calls × small tokens ≈ <$5 at current Opus pricing.
- Analysis + writeup: ~½ day.

## Decision gate

Run this eval **only if** someone proposes a policy change that depends on the codegen-fluency claim. As of 2026-04-30 no such proposal exists, so this document is on the shelf.
