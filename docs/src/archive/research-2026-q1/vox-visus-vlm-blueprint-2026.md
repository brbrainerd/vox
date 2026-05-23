---
title: "vox-visus-vlm-blueprint-2026.md"
description: "Documentation for vox-visus-vlm-blueprint-2026.md."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# Vox Visus: Visual Intelligence Blueprint (Wave 2-3)

This document defines the technical architecture for Wave 2 (CI/CD) and Wave 3 (Training Flywheel) of the Vox Visus subsystem.

## Objective
Enable automated visual regression testing and continuous model improvement for GUI-heavy applications by integrating VLMs (Qwen 3.5-VL) into the Vox orchestration loop.

## Architecture: The Hub-and-Spoke Vision Lane
Visual intelligence is routed through a specialized lane to optimize for high-VRAM requirements (VLMs) vs. low-latency text requirements.

1.  **Evidence Collection (Hub)**: `vox visus audit` captures screenshots and AXTrees, storing them in VoxDb CAS.
2.  **Routing (Orchestrator)**: The orchestrator detects `Visus` requests and pins them to agents with `visus_eligible` and `multi_modal` capabilities.
3.  **Inference (Spoke)**: The designated agent executes the grounding prompt against the screenshot + AXTree metadata.
4.  **Flywheel (Closure)**: `vox visus train` ingests approved audit logs back into the MENS `gui-vision` corpus.

## Data Structures

### AttachmentManifest (Handoff)
```rust
pub struct AttachmentManifest {
    pub attachments: Vec<AttachmentEntry>,
}
pub struct AttachmentEntry {
    pub sha256: String, // CAS Key
    pub mime_type: String,
    pub label: String,
}
```

### Vision-Augmented Inference Request
The inference client must convert CAS hashes into base64 `image_url` parts for OpenAI-compatible providers:
```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "Audit this GUI for overlaps." },
    { "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }
  ]
}
```

## Implementation Phases (Remaining)
- **Wave 1b (Client Hardening)**: Update `vox-openai-wire` and `vox-orchestrator` infer adapters for multi-modal payloads.
- **Wave 2 (CI/CD)**: Finalize GitHub Actions workflow for PR visual auditing.
- **Wave 3 (Flywheel)**: Verification of `vox visus train` output quality.

