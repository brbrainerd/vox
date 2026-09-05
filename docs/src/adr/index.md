---
title: "Architecture Decision Records (ADR)"
description: "Official documentation for Architecture Decision Records (ADR) for the Vox language. Detailed technical reference, architecture guides, a"
category: "Architecture Decisions (ADRs)"
training_eligible: true

schema_type: "TechArticle"
---
# Architecture Decision Records (ADR)

This directory contains ADRs for the Vox project.

| ADR | Title |
|-----|--------|
| [001](001-burn-backend-selection.md) | Burn backend selection |
| [002](002-diataxis-doc-architecture.md) | Diátaxis doc architecture |
| [003](003-native-training-over-python.md) | Native training over Python |
| [004](004-codex-arca-turso-ssot.md) | **Codex over Arca over Turso (storage SSOT)** |
| [005](005-socrates-anti-hallucination-ssot.md) | **Socrates anti-hallucination (confidence SSOT)** |
| [006](006-mens-full-graph-qlora-qlora-rs.md) | **Mens full-graph Candle QLoRA (qlora-rs)** |
| [007](007-qlora-rs-multi-layer-training-api.md) | **qlora-rs 1.0.5 multi-layer training API gate** |
| [008](008-populi-transport.md) | **Mens control plane (HTTP; TLS at edge)** |
| [009](009-populi-hosted-baas.md) | **Hosted mens / BaaS (future trust model)** |
| [010](010-tanstack-web-spine.md) | **TanStack web spine (Router → Start, SSR topology)** |
| [011](011-scientia-publication-ssot.md) | **Scientia publication manifest SSOT** |
| [012](012-internal-web-ir-strategy.md) | **Internal web IR strategy for Vox frontend emission** |
| [013](013-openclaw-ws-native-strategy.md) | **OpenClaw WS-first native interop** |
| [014](014-async-openai-selective-adoption-spike.md) | **`async-openai` selective adoption (spike / no-go)** |
| [015](015-vox-docker-oci-portability-ssot.md) | **Vox Docker/OCI portability SSOT** |
| [016](016-oratio-streaming-whisper-and-constrained-decode.md) | **Oratio streaming Whisper + constrained decode** |
| [017](017-populi-lease-remote-execution.md) | **Populi lease-based authoritative remote execution (design intent)** |
| [018](018-populi-gpu-truth-layering.md) | **Populi GPU truth layering (verified vs policy labels)** |
| [019](019-durable-workflow-journal-contract-v1.md) | **Durable workflow journal contract v1 (interpreted runtime)** |
| [020](020-populi-mesh-scaling-transport-default.md) | **Populi mesh scaling transport default** |
| [021](021-generated-workflow-durability-parity.md) | **Generated workflow durability parity** |
| [022](022-orchestrator-bootstrap-and-daemon-boundaries.md) | **Orchestrator bootstrap factory + daemon boundaries** |
| [023](023-optional-telemetry-remote-upload.md) | **Optional telemetry remote upload (explicit CLI, Clavis, local spool)** |
| [024](024-dashboard-axum-spa.md) | **Dashboard as local Axum-served SPA** |
| [025](025-multi-agent-lock-coherence.md) | **Multi-Agent Lock Coherence and Lease Propagation** |
| [026](026-third-party-code-provenance.md) | **Third-Party Code Provenance Policy** |
| [027](027-dual-track-ui-surfaces.md) | **Dual-Track UI Surfaces (Vox-Native vs React/TanStack Interop)** |
| [028](028-deprecate-stub-durability-grammar.md) | **Remove stub durability/scheduling grammar (`@scheduled`, `@durable`, `workflow`, `activity`)** — superseded by ADR-041 (2026-05-23) |
| [029](029-formal-intent.md) | **Formal Intent and Tool Receipt Auditing** (renumbered from 024 on 2026-05-02) |
| [030](030-state-machine-ssot.md) | **State machine SSOT** |
| [031](031-deprecate-vox-vscode.md) | **Deprecate vox-vscode** |
| [032](032-vox-ui-reactive-modules.md) | **`.vox.ui` reactive modules** (gates Phase D of the Svelte-mineable features plan) |
| [033](033-typed-fragment-primitive.md) | **Typed parametric fragment primitive** (gates Phase F; deferred until Phase 6 primitives stabilize) |
| [034](034-candle-qlora-stack-upgrades.md) | **Candle / QLoRA stack upgrades** (deferred batch; GPU CI) |
| [035](035-swc-parser-alternatives-eval.md) | **SWC parser vs alternatives** (evaluation only; no silent migration) |
| [036](036-webir-hir-unification-compare-both.md) | **WebIR vs HIR unification (compare-both)** — core+projection decision, rubric, capability wiring |
| [037](037-tauri-convergence.md) | **Tauri convergence** — generated desktop/mobile app shell, Capacitor retirement, Sherpa plugin port |
| [038](038-ai-fixture-prompt-decorator.md) | **AI fixture `@prompt` decorator** |
| [039](039-ai-fixture-hole-decorator.md) | **AI fixture `@hole` decorator** |
| [040](040-ai-fixture-search-decorator.md) | **AI fixture `@search` decorator** |
| [041](041-durable-functions-completion-2026.md) | **Durable functions completion** — closes ADR-028; `workflow`/`activity`/`@scheduled`/`actor` now backed by real runtime (Stable for ADR-019 subset) |
| [042](../architecture/adr-042-vox-populi-types.md) | **Extract `NodeRecord` into `vox-populi-types` (L2)** (filed under `architecture/`) |
| [043](../architecture/adr-043-quantized-safetensors-ondisk-format.md) | **Quantized SafeTensors on-disk format** (filed under `architecture/`) |
| [044](044-ai-fixture-subagent-decorator.md) | **AI fixture `@subagent` decorator** (renumbered from a three-way 037 collision) |
| [045](045-tauri-gui-replaces-axum-dashboard.md) | **Tauri GUI replaces the Axum dashboard** (renumbered from a three-way 037 collision) |
| [046](046-pareto-frontier-reporting.md) | **Pareto-frontier reporting for model surfaces** — scoreboard/explain mark non-dominated rows over reliability, cost, and latency; explicitly reporting-only, with the four reasons a routing change was rejected |
| [047](047-iroh-transport.md) | **iroh QUIC replaces the bespoke populi mesh transport** (supersedes 008 and 020; partially supersedes 017; upholds 018) |

See also: [Internal Web IR implementation blueprint](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [WebIR operations catalog](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [WebIR supplemental execution map](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [Acceptance gates G1–G6](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [Internal Web IR side-by-side schema](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [WebIR appendix — tooling registry](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [WebIR K-complexity quantification](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [WebIR K-metric appendix](../archive/research-2026-q1/internal-web-ir-implementation-blueprint.md), [Codex vNext schema](../archive/research-2026-q1/codex-vnext-schema.md), [Codex BaaS](../archive/research-2026-q1/codex-baas.md).


