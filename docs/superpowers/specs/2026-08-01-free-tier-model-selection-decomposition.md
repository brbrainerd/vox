---
title: "Free-Tier Model Selection & Onboarding — Decomposition (2026-08-01)"
description: "Sequencing note splitting the free-tier/onboarding research into four independent sub-projects, each with its own spec→plan→implementation cycle."
category: "architecture"
status: "current"
training_eligible: false
---

# Free-Tier Model Selection & Onboarding — Decomposition (2026-08-01)

Source: [`free-tier-model-selection-and-onboarding-research-2026-08-01.md`](../../src/architecture/free-tier-model-selection-and-onboarding-research-2026-08-01.md)
(§5 recommendations A–D, §4 risk register).

The research doc's four recommendations are genuinely independent subsystems — different crates,
different skillsets, one real dependency edge — so each gets its own brainstorm → spec → plan
cycle rather than one combined spec.

| # | Sub-project | Crates touched | Depends on | Size | Order |
|---|---|---|---|---|---|
| **A** | Zero-key OAuth flow — RFC 8252 loopback PKCE against OpenRouter, Clavis-backed storage (`SecretKind::OAuthRefreshToken`), quota-exceeded vs. no-credential error states | `vox-secrets`, `vox-orchestrator-mcp` (`provider_auth.rs`/`resolve.rs`), `vox-gui/src/commands` | none | Medium | **1st** |
| **B** | External rating registry — Epoch AI (Aider polyglot/METR/SWE-bench Verified) + OpenRouter `/benchmarks` + Artificial Analysis, cached/centralized fetch, decay-into-telemetry blend, ID reconciliation | `vox-orchestrator/src/models/{scoring,catalog}.rs`, `vox-config` | none (independent of A/C) | Medium–large | **2nd**, or parallel-eligible with A |
| **C** | Onboarding wizard GUI — new Vox Axis surface, wizard-visibility gate, reuses `BackendBanner`/`VersionMismatchBanner` dismiss pattern + the unbuilt §9.5 first-run-tour design, free-tier filter + `quality_score` rendering in `ModelsView.tsx` | `vox-gui/ui/src` (new surface + `Sidebar.tsx` registration), `vox-gui/src/commands` | **A** (its core screen drives the OAuth flow) | Medium–large | **3rd** |
| **D** | Hardware-fit badge — map `vox-plugin-nvml-probe`'s error/success into a Fit / Won't-fit / **Unknown** three-state badge in `ModelsView.tsx`, scoped NVIDIA-only in v1 | `vox-plugin-nvml-probe` (mapping only, probe itself untouched), `vox-gui/ui/src/components/surfaces/Models` | none | Small | Opportunistic — can run anytime, including in parallel with A |

**Recommended path**: brainstorm+spec+plan **A** now (this session) — it's the sharpest,
self-contained bug and the research doc's own top recommendation. **B** and **D** have no
dependency on A and can be sequenced in parallel by a separate session/agent if desired. **C**
should not be brainstormed until A's design (specifically its key-provisioning UI contract) is
settled, since C's wizard is largely a GUI wrapper around A's flow plus B's badges.

This doc is a sequencing note, not a design — each row gets its own
`docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` when brainstormed.
