---
title: "Phase Numbering Index"
description: "Disambiguates the three independent phase sequences used in vox plans. When a plan or commit says 'Phase N', look here first."
category: "Architecture SSOTs"
status: current
last_updated: "2026-05-11"
training_eligible: true
audience: contributors
---

# Phase Numbering Index

The vox project uses **three independent phase sequences**. They are not aligned with each other. When a code comment, plan doc, or commit message says "Phase N", check which sequence it belongs to.

| Sequence | Range | Topic | Canonical plan | As of 2026-05-08 |
|---|---|---|---|---|
| **Frontend interop** | Phases 1–5 | Build target split, TS-emit, HTTP ergonomics, schema codegen, bidirectional React interop | [external-frontend-interop-plan-2026](external-frontend-interop-plan-2026.md) | Phases 1–4 complete; Phase 5 in plan — see also **`vox emit client`**, **`vox dev --target=server`**, OpenAPI **`ErrorEnvelope`**, [`wire-format-v1-ssot.md`](wire-format-v1-ssot.md) |
| **GUI-native language** | Phases 0–8 | Vox compiler primitives for native UI (VUV, reactive modules, typed fragments) | [vox-gui-native-roadmap-2026](vox-gui-native-roadmap-2026.md) | Phases 0–7 complete; 8 in plan/partial |
| **Workspace reorg** | Phases 0–9 | Crate extraction, layer enforcement, dead-crate burn, build-time optimization | [2026-05-08-workspace-reorg-design](2026-05-08-workspace-reorg-design.md), [outcome](2026-05-08-workspace-reorg-outcome.md) | Phases 0, 1, 2, 4, 5, 9 complete; 3, 6, 7, 8 deferred |

## How to disambiguate at a glance

- "Phase 5" without qualifier → **frontend interop** (most common usage in recent history).
- Context mentions VUV, reactive modules, typed fragments, or native UI → **GUI-native**.
- Context mentions crate extraction, `vox-arch-check`, dead crates, or layer enforcement → **workspace reorg**.

## When writing new code or comments

Prefer feature names over phase numbers in code comments. Phase numbers are calendar-relative; feature names age better. If you must reference a phase, qualify it:

```
// Frontend interop Phase 5: React-component import bridge
// GUI-native Phase 8: corpus migration + MENS training
// Workspace reorg Phase 3 (deferred): further crate extraction
```

## Cross-references

This document is linked from:
- `AGENTS.md` §Architecture
- Each of the three canonical plans above (see their top-of-doc banners)
