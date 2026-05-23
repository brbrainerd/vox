---
title: "Telemetry Trust (SSoT)"
description: "Canonical boundaries and trust policies for Vox telemetry."
category: "Architecture SSOTs"
sort_order: 10
status: "current"
---

# Telemetry Trust Policy (SSoT)

> **Note**: Comprehensive research and historical blueprinting for the Vox telemetry architecture were finalized in Q1 2026. The full corpus of design documents is preserved in `docs/src/archive/research-2026-q1/`.

## Core Policy

1. **Zero-Surprise Telemetry**: All telemetry in Vox is opt-in or strictly locally-bounded by default. Remote sinks must be explicitly authorized.
2. **Local-First Analysis**: The primary consumer of `vox` telemetry is the local **`vox-orchestrator`** stack (CLI + optional `vox-orchestrator-d`).
3. **Data Residency**: No source code, secrets, or PII may be included in default telemetry payloads. 

For implementation details and configuration of remote sinks, see:
- [`docs/src/adr/023-optional-telemetry-remote-upload.md`](../adr/023-optional-telemetry-remote-upload.md)
- `vox telemetry --help`
