---
title: GUI Visual AI Adversarial Review
description: Non-gating, cache-driven AI review of Playwright screenshots of every vox-gui surface against the front-end design principles.
category: "Architecture SSOTs"
---

# GUI Visual AI Adversarial Review

## The rule
Every vox-gui surface is screenshotted on the post-merge / `full-ci` Playwright sweep. A surface is
AI-reviewed against the design principles ONLY when its screenshot content hash changes (or it is new).
This is **advisory** — it never gates CI; it warns and collects reports.

## How it works
- **Discover**: `SURFACE_REGISTRY` (SSOT) → every `viewKey` with `tier != 'none'`.
- **Capture**: `e2e/visual-review.spec.ts` shoots hi-DPI PNGs (2x), emits `manifest.json` with per-surface `sha256` + timing.
- **Cache**: `contracts/reports/gui-visual-review/cache.v1.json` maps `viewKey → screenshot_sha256 + verdict`. Same hash → reuse verdict, no AI call. Changed hash → warn + re-review.
- **Review**: `gui-visual-review` (binary in `vox-orchestrator-mcp`) sends the PNG to an OpenRouter vision model and parses a JSON verdict against the design-principles rubric.
- **Report**: versioned `contracts/reports/gui-visual-review/<date>.json` + `ledger.jsonl` (trend/spike). Spike = run review-time > `spike_factor` × trailing median → warning only.

## Model (pluggable, not pinned)
`contracts/orchestration/visual-review.config.v1.json` lists vision models in priority order (default `google/gemini-3-flash-preview`; `anthropic/claude-opus-4.8` for escalation). The resolver picks the first config entry the model registry marks `supports_vision`; per-surface model is recorded for future A/B learning.

## What it never does
- Never fails CI (binary exits 0; CI step `continue-on-error`).
- Never gates on appearance change — it warns and re-reviews; it does not pixel-diff-gate.
