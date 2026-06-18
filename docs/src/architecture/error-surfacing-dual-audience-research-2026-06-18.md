---
title: "Surfacing Errors to Humans and LLMs — Dual-Audience Research"
description: "How captured program behavior and diagnostics should be presented simultaneously to humans (CLI/GUI) and to LLMs for diagnosis. Covers LLM root-cause analysis evidence, the telemetry-quality finding, dual-audience serialization, and Vox's existing --for-llm diagnostic envelope as the reuse point."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-18"
---

# Surfacing Errors to Humans and LLMs — Dual-Audience Research

**Status:** Research / feasibility (no implementation).
**Companions:** [auto-debugging](auto-debugging-zero-annotation-research-2026-06-18.md) (capture) · [design-hygiene](auto-derivation-design-hygiene-2026-06-18.md).
**Confidence grading:** ★★★ confirmed · ★★ reputable source · ★ inferred.

---

## 1. Thesis

Captured behavior and diagnostics should be **born dual-audience**: a human-recognizable form (CLI + GUI) and an LLM-optimized form, from one stream. The format owner (the language/toolchain) is best placed to do this — and Vox already proves it at compile time.

## 2. LLM root-cause analysis: real but bounded ★★★

**OpenRCA** (ICLR 2025, 335 real failure cases): Claude 3.5 resolved **only 5.37%** with *oracle* telemetry (3.88% with sampled); an **RCA-agent** lifted it to **11.34%**; current leader Claude Opus 4.6 scores **0.349**. ([OpenRCA](https://netman.aiops.org/wp-content/uploads/2025/05/13411_OpenRCA_Can_Large_Langua.pdf), [leaderboard](https://llm-stats.com/benchmarks/openrca))

**Two lessons:**
1. LLM diagnosis solves a *minority* of hard cases → **pipe it context, don't trust it blindly** (advisory, with human/CI in the loop).
2. **Telemetry quality dominates** (oracle ≫ sampled). The leverage is feeding an LLM *good, severity-ranked, curated* context — not raw firehose logs. This is exactly what an inferred-severity layer (see [auto-debugging §4](auto-debugging-zero-annotation-research-2026-06-18.md)) produces.

## 3. Human-facing surfacing ★★

Established practice: structured diagnostics with severity, source excerpts, and fix suggestions; color/TTY-aware CLI; a GUI timeline/heatmap for runtime events. Interrogative debugging (Whyline) shows humans benefit from *queryable* "why" answers, not just flat logs.

## 4. Dual-audience serialization: thin prior art ★

No source cleanly demonstrates **one** instrumentation stream optimally serving both a human CLI/GUI and an LLM. OpenRCA's oracle-vs-sampled gap is the closest signal that LLMs benefit from *curated* (not necessarily *different*) context. **This is the genuine Vox research opportunity** — and the reason any "LLM-optimized serialization" feature should be validated with an A/B (does a bespoke LLM format beat the human timeline fed verbatim?) before investment.

## 5. Vox already half-owns this ★★★

Vox's compile-time path is a working dual-audience precedent:
- `VoxCompilerDiagnosticPayload` (`crates/vox-compiler/src/typeck/diagnostics.rs`) carries `minimal_repro`, `excerpt`, `explain_url`, `suggested_fixes` — an LLM-recognizable envelope.
- `vox check --for-llm` emits that envelope; human CLI diagnostics live in `crates/vox-cli-core/src/diagnostics.rs` (color/TTY/`NO_COLOR`).

The unbuilt step is extending this from **compile-time diagnostics** to **runtime behavior**: attach a severity-ranked execution-context window (from the [auto-debugging](auto-debugging-zero-annotation-research-2026-06-18.md) tracer) to the same envelope. Plan: `../../superpowers/plans/2026-06-18-track-b-zero-annotation-debugging.md` (Task 7, light-gated on the §4 A/B).

## 6. Design implications (carried to design-hygiene)

- **Advisory, not gating** — LLM diagnosis is bounded; surface, don't auto-act.
- **Curate before piping** — rank by inferred severity; oracle-quality context is the leverage.
- **Reuse the envelope** — do not invent a second diagnostic format; extend `VoxCompilerDiagnosticPayload`.

## Sources
OpenRCA (ICLR 2025) + leaderboard · Whyline · Vox `--for-llm` envelope (in-repo) — URLs/paths inline above.
