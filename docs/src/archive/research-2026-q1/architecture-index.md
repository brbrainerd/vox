---
title: "Architecture index"
description: "Guide to the current architecture, SSOT, research, and roadmap documentation under docs/src/architecture."
category: "Architecture SSOTs"
status: "current"
sort_order: 0
last_updated: "2026-04-12"
training_eligible: false
machine_readable_companion: "../../agents/ai-ide-feature-matrix-2026.json"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Architecture index

The `docs/src/architecture/` section contains several different kinds of documents. This page is the map.

## Documentation

- [Documentation governance](../contributors/documentation-governance.md)
- [Contributor hub](../contributors/contributor-hub.md)
- [Doc-to-code acceptance checklist](doc-to-code-acceptance-checklist.md)
- [Orphan surface inventory](orphan-surface-inventory.md)
- [Documentation hygiene and AI-agent guidelines 2026](doc-hygiene-agent-guidelines-2026.md)

## Contributor-relevant architecture (highest practical value)

If you are contributing code — not doing architecture research — these are the pages with highest day-to-day utility.

| Page | When to read it |
|---|---|
| [Testing standard](testing-standard.md) | Before writing any test |
| [Contributor hub](../contributors/contributor-hub.md) | The main entry point for all contributors |
| [Contribution loop](../contributors/contribution-loop.md) | Understanding the WRITE → VERIFY → TRAIN flywheel |
| [TOESTUB contributor guide](../contributors/toestub-contributor-guide.md) | Fixing `arch/*`, `stub/*`, and `skeleton/*` failures |
| [Coding agent instructions](../contributors/coding-agents.md) | Quick-reference for AI agents (loaded as context) |
| [God object defactor checklist](god-object-defactor-checklist.md) | When fixing `arch/god_object` CI failures |
| [TOESTUB scaling rules SSOT](scaling-toestub-rules.md) | When `scaling/*` findings appear |
| [TOESTUB self-healing architecture](toestub-self-healing-architecture-2026.md) | Understanding why rules exist |
| [AI agent panic and shortcut pathology](research-ai-panic-shortcuts-2026.md) | Why shortcuts devolve the codebase |
| [Doc-to-code acceptance checklist](doc-to-code-acceptance-checklist.md) | Before merging any docs change |
| [Vox source → MENS pipeline SSOT](vox-source-to-mens-pipeline-ssot.md) | How code becomes training data |
| [Compiler diagnostics ergonomics](compiler-diagnostics-ergonomics.md) | Understanding the error surface |

## Current architecture and authority docs

Use these when you need current policy and behavior. The canonical cross-domain map is [`contracts/documentation/canonical-map.v1.yaml`](../../../contracts/documentation/canonical-map.v1.yaml); this page is navigation, not the source of behavioral truth.

- [Feature growth boundaries](feature-growth-boundaries.md)
- [Configuration SSOT](config-ssot.md) — precedence rules, toxonomy, layered config, and sync
- [Interop tier policy](interop-tier-policy.md)
- [MCP exposure from the Vox language](mcp-vox-language-exposure.md)
- [Capability registry authority](capability-registry-ssot.md) — `contracts/capability`, `vox ci capability-sync`, model manifest
- [Capability visualization views](capability-visualization-views.md)
- [Vox bell-curve strategy](vox-bell-curve-strategy.md)
- [Vox Lang Training SSOT (2026)](vox-lang-training-ssot-2026.md)
- [Doc-to-code acceptance checklist](doc-to-code-acceptance-checklist.md)
- [Orphan surface inventory](orphan-surface-inventory.md)
- [Legacy retirement roadmap 2026](legacy-retirement-roadmap.md) — **LLM guard**: deprecated surfaces, frozen files, safe-to-extend surfaces
- [Language surface authority](language-surface-ssot.md) — keywords / decorators / manifests
- [OpenAPI contract authority](openapi-contract-ssot.md) — committed YAML, validation, optional codegen
- [AI CLI generation standard](ai-cli-generation-standard.md) — AST/JSON schema constraints for MENS command generation
- [Outbound HTTP policy](outbound-http-policy.md) — `vox-reqwest-defaults` and migration order
- [Compiler diagnostics ergonomics](compiler-diagnostics-ergonomics.md) — `miette` vs custom errors, `quote` pilot
- [Vox shell operations boundaries](vox-shell-operations-boundaries.md) — host `pwsh` vs `vox shell` vs `.vox` `std.*` (no shell emulator product)
- [Plan adequacy (thin plans & telemetry)](plan-adequacy.md) — external limits, shared heuristics, expansion policy
- [CodeRabbit review coverage SSOT](coderabbit-review-coverage-ssot.md) — full-repo review scope, persistence, and lane hardening
- [Telemetry trust boundary map](telemetry-trust-ssot.md) — telemetry surfaces, trust planes, and canonical links
- [Telemetry taxonomy and contracts](telemetry-taxonomy-contracts-ssot.md) — roadmap event taxonomy and contracts
- [Telemetry retention and sensitivity](telemetry-retention-sensitivity-ssot.md) — roadmap retention and S0–S3 classes
- [Telemetry client disclosure](telemetry-client-disclosure-ssot.md) — VS Code / MCP host disclosure
- [Telemetry implementation blueprint 2026](telemetry-implementation-blueprint-2026.md) — phased rollout plan
- [Telemetry implementation backlog 2026](telemetry-implementation-backlog-2026.md) — executable checklist
- [Telemetry remote sink specification](telemetry-remote-sink-spec.md) — optional `vox telemetry upload` wire contract
- [Cryptography Policy SSOT](cryptography-ssot-2026.md) — cryptographic algorithms and `vox-crypto` architecture
- [Operations catalog authority](operations-catalog-ssot.md)
- [Completion policy authority](completion-policy-ssot.md)
- [HITL doubt loop](hitl-doubt-loop-ssot.md)
- [Cross-repo query observability](cross-repo-query-observability.md)
- [Vox organization](vox-organization.md)
- [Session management](session_management.md)
- [Security model](security_model.md)
- [News syndication security](news_syndication_security.md)
- [News syndication incident patterns](news_syndication_incident_patterns.md)
- [Memory system](memory_system.md)
- [Vox web stack SSOT](vox-web-stack-ssot.md)
- [Compiler IR pipeline](compiler-ir-pipeline.md)
- [IR emission SSOT (check vs build, VoxIrModule vs WebIR)](ir-emission-ssot.md)
- [Vox source → Mens pipeline SSOT](vox-source-to-mens-pipeline-ssot.md) — lexer/compiler → goldens → corpus → HF tokenizer
- [Populi data pipeline](populi-data-pipeline.md) — mesh control plane vs Mens training sources
- [RAG and research architecture 2026](rag-and-research-architecture-2026.md)
- [Hardware Discovery SSOT (Native Registry)](gpu-discovery-ssot-findings-2026.md) — canonical hardware truth, DXGI/DRM probes, and PTX shimming policy
- [Agentic Planning and MENS Multimodal Boundaries (SSOT)](agent-planning-multimodal-ssot.md)
- [Vox Library Mode — Framework-Agnostic Codegen SSOT](vox-library-mode-ssot.md)
- [**V0.5 Crate Reorganization and Stability SSOT (2026)**](v05-crate-reorganization-ssot-2026.md) — Tier classification for all 64 crates, React-as-primary declaration, `[package.metadata.vox]` annotation schema, `vox doctor` maturity lane, and V0.5 exit criteria.

## MENS System

For MENS architecture and training details, refer to:
- [Populi data pipeline](populi-data-pipeline.md)
- [GUI, v0/islands, vision, and Mens Qwen — virtuous-cycle implementation plan (2026)](vox-gui-vision-virtuous-cycle-implementation-plan-2026.md) — GUI verification loop, vision rubrics, fine-tuned Qwen3.5 vs optional VL lane
- [Mens native training SSOT](../reference/mens-training.md)
- [Mens training data contract](../reference/mens-training-data-contract.md)
- [Mens architecture 2026 synthesis](mens-architecture-2026-synthesis.md)
- [Mens lane segmentation research](mens-lane-segmentation-research.md)

## Research and synthesis

Use these when the question is exploratory, comparative, or evidence-gathering:

- [Research index](research-index.md)
- [AI IDE feature research findings 2026](ai-ide-feature-research-findings-2026.md)
- [Terminal execution policy research findings 2026](terminal-exec-policy-research-findings-2026.md)
- [Telemetry unification research findings 2026](telemetry-unification-research-findings-2026.md)
- [Context management research findings 2026](context-management-research-findings-2026.md)
- [Semantic Proximity, Split-Brain Detection, and Safe Symbol Surfacing (research 2026)](research-semantic-proximity-split-brain-2026.md) — detecting near-duplicate / divergently-named symbols, KCH hallucination prevention, `ProximityCandidate` model, `vox ci proximity-drift` gate
- [Protocol convergence research 2026](protocol-convergence-research-2026.md)
- [ASR speech-to-code scouting 2026](asr-speech-to-code-findings-2026.md) — model WER comparison, Canary/Qwen/Whisper/Moonshine/Parakeet overview
- [ASR speech-to-code full architecture 2026](asr-speech-to-code-architecture-2026.md) — preprocessing stack, Rust crate design, WER estimates by adaptation tier, MENS integration, training pathway
- [Vox syntax highlighting SSOT 2026](vox-syntax-highlighting-ssot-2026.md) — universal coloring strategy across VS Code/Cursor, Neovim/Helix/Zed, GitHub, and mdBook using `tree-sitter-vox` injection queries + TextMate grammar
- `*-research-2026.md`
- `*-findings-2026.md`
- synthesis pages that are explicitly labeled as research

## Planning and roadmap

Use these when a page describes intended implementation rather than current behavior:

- [Qwen 3.6 integration research (groundwork)](qwen36-integration-research.md) — pre-implementation checklist vs Qwen 3.5 SSOT; native vs API paths
- [Qwen3.5 multimodal Phase 2 backlog](qwen35-multimodal-phase2-backlog.md) — vision/video tokens after text-only 3.5 is green
- [Context management implementation blueprint](context-management-implementation-blueprint.md)
- [Context management phase 1 backlog](context-management-phase1-backlog.md)
- `*-implementation-plan-2026.md`
- [React / v0 interop migration charter 2026](react-interop-migration-charter-2026.md) — governance, KPIs, cutover checkpoints
- [React / v0 interop backlog 2026](react-interop-backlog-2026.md) — granular WS01–WS26 checklist index
- [React / v0 interop research findings 2026](react-interop-research-findings-2026.md)
- [React / v0 interop implementation plan 2026](react-interop-implementation-plan-2026.md)
- [React / v0 hybrid adapter cookbook (SPA + SSR)](react-interop-hybrid-adapter-cookbook.md)
- [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md)
- [Populi GPU truth probe specification (Native Layer A)](populi-gpu-truth-probe-spec.md) — updated for `HardwareRegistry` integration
- [Populi node lifecycle, drain, and GPU hotplug](populi-node-lifecycle-hotplug.md) — lifecycle model and backlog vs shipped gates
- Normative **decision** docs for Populi GPU / mesh placement: [ADR 017: Populi lease-based remote execution](../adr/017-populi-lease-remote-execution.md), [ADR 018: Populi GPU truth layering](../adr/018-populi-gpu-truth-layering.md), [ADR 020: Populi mesh scaling — default transport posture](../adr/020-populi-mesh-scaling-transport-default.md), [work-type placement matrix](../reference/populi-work-type-placement-matrix.md) — aspirational batch/K8s notes remain in [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md) until dedicated ADRs are filed
- [ADR 022: Orchestrator bootstrap factory and daemon boundaries](../adr/022-orchestrator-bootstrap-and-daemon-boundaries.md) — shared `build_repo_scoped_orchestrator`, MCP/CLI identity parity, `vox-dei-d` boundary
- `*-implementation-blueprint.md`
- `*-roadmap.md`
- planning-meta documents under `planning-meta/`

## How to read this section

- If you need shipped behavior, prefer pages labeled `status: current` or pages that mirror code and contract surfaces.
- If you need rationale, open the matching ADR or architecture authority page.
- If you need future direction, read roadmap and planning documents as plans, not as claims of current capability.


