---
title: "vox-visus-vlm-implementation-plan-2026.md"
description: "Documentation for vox-visus-vlm-implementation-plan-2026.md."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# Vox Visus: Image Intelligence and VLM Flywheel Implementation Plan (2026)

This document outlines the wave-gated implementation for integrating **Vox Visus** (Voice of Vision) visual intelligence into the Vox orchestrator and browser engine.

## Wave 0: Foundations (Completed)
- [x] **CLI Surface:** `vox visus audit` and `vox visus baseline`.
- [x] **Browser Glue:** Deterministic overlap detection in `vox-browser`.
- [x] **Storage Layer:** SQL schema for visual baselines and audit logs in Arca.
- [x] **Grounding Prompt:** Versioned SSoT for Qwen 3.5-VL pixel-grounding.

## Wave 1: VLM Spoke Integration (Completed)
- [x] **Capability Advertising:** Add `visus_eligible` and `multi_modal` to `AgentCapabilities`.
- [x] **Orchestrator Routing:** Update `RoutingService` to respect vision capability penalties.
- [x] **Task Ingestion:** Parse `[[category:visus]]` and `[[visus]]` hints in `AgentTask`.
- [x] **Handoff Support:** Inject vision requirements when accepting `AttachmentManifest` payloads.
- [x] **Model Registry:** Register `qwen/qwen-3.5-vl` and the `visus` premium alias.

## Wave 2: CI/CD & Automated Auditing (Completed)
- [x] **Workflow:** GitHub Actions workflow `vox-visus-audit.yml`.
- [x] **Evidence Collection:** Automated screenshot and AXTree capture during PRs.

## Wave 3: Training Flywheel Closure (Completed)
- [x] **Data Ingestion:** `vox visus train` command for MENS corpus expansion.
- [x] **Flywheel:** Feeding approved audit findings back into the `gui-vision` corpus.

## Future Waves (Roadmap)
- [ ] **Cross-Browser Parity:** Integrate multi-engine audits (Safari, Firefox) via Playwright spokes.
- [ ] **Temporal Analysis:** Visual auditing of animations and hydration transitions.
- [ ] **Socrates Integration:** Real-time visual confidence scoring for agentic GUI interactions.

