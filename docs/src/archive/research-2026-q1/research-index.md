---
title: "Research index"
description: "Guide to the research, findings, and roadmap-heavy documentation in the architecture section."
category: "Architecture SSOTs"
status: "research"
sort_order: 5
last_updated: "2026-04-17"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Research index

This page groups the research-oriented documentation in `docs/src/architecture/` so it is easier to discover without mistaking it for the current shipped architecture.

## Research classes

| Pattern | Typical status | Meaning |
| --- | --- | --- |
| `*-research-2026.md` | `research` | investigation, evidence gathering, constraints, and trade-offs |
| `*-findings-2026.md` | `research` | synthesized results or conclusions from a research wave |
| `*-implementation-plan-2026.md` | `roadmap` | ordered implementation proposal |
| `*-implementation-blueprint.md` | `roadmap` or `experimental` | intended technical design for a future or in-progress path |
| `planning-meta/*` | `current` process docs or `roadmap` planning docs | contributor planning governance, not public product narrative |

## Scope discipline and V0.5 planning

- **[V0.5 Crate Reorganization and Stability SSOT (2026)](v05-crate-reorganization-ssot-2026.md)** — Authoritative tier classification for all 64 workspace crates (Core/Active/Incubating/Frozen/Decision-pending), React-as-primary-target declaration, `[package.metadata.vox]` annotation schema, `vox doctor` maturity lane spec, V0.5 exit criteria, and a sequenced 20-task priority list. **Start here for scope decisions.**

## Pipeline and corpus SSOT (implementation)

- [Vox source → Mens pipeline SSOT](vox-source-to-mens-pipeline-ssot.md) — single map from `.vox` on disk to Mens training inputs (lexer vs HF tokenizer).
- [Populi data pipeline](populi-data-pipeline.md) — disambiguates mesh runtime data from training JSONL.
### Hardware and Telemetry SSOT (Current Implementation)

- [Hardware Discovery SSOT (Native Registry)](gpu-discovery-ssot-findings-2026.md) — canonical hardware truth, DXGI/DRM probes, and PTX shimming policy.
- **Orchestrator Telemetry Loop**: Periodic hardware health monitoring (utilization, temperature, power) integrated into the `vox-orchestrator` observer loop (since April 2026).
- **Vox Mens Probe**: CLI tool for real-time hardware verification and training profile recommendation.

### Corpus lab, vision, and Qwen family (research, April 2026)

- **[Vox corpus lab: mass examples, metrics, and eval harness (research 2026)](vox-corpus-lab-research-2026.md)** — Tier A/B/C layout, compiler lanes vs golden parity, Syntax-K and WebIR aggregates, optional UI and vision rubrics, Mens `validate-batch` integration sketch.
- **[Mens vision and multimodal inputs (research 2026)](mens-vision-multimodal-research-2026.md)** — `TrainingPair` limits, orchestrator hints vs attachments, screenshot-to-JSON pipeline, Candle text-only vs remote VLMs.
- **[Mens Qwen family migration and native stack (research 2026)](mens-qwen-family-migration-research-2026.md)** — Qwen2 vs Qwen3.5 retention tiers, operator runbook vs code removal, external QwenLM and Hugging Face references.
- **[Custom Model Pretraining vs Qwen Fine-Tuning Research (2026)](custom-model-pretraining-research-2026.md)** — Analysis of hardware costs (cloud vs local clusters), advantages of full pretraining, and hard limitations of the current Qwen 3.5 QLoRA pipeline.
- **[GUI, v0/islands, vision, and Mens Qwen — virtuous-cycle implementation plan (2026)](vox-gui-vision-virtuous-cycle-implementation-plan-2026.md)** — 50+ tracked ideas with repo anchors: WebIR, `vox island`, Playwright/MCP screenshots, orchestrator vision, Mens Qwen3.5 text vs optional VL rubric lane, execution waves W0–W5.
- **[Orchestrator `attachment_manifest` RFC (2026)](orchestrator-attachment-manifest-rfc-2026.md)** — MIME+hash task attachments and vision routing without substring-only hints (spec ahead of types).
- **[Agentic Planning and MENS Multimodal Boundaries (SSOT)](agent-planning-multimodal-ssot.md)** — Consolidates the Phase 4/5 integration boundaries: explicit "no-pixels-in-prompt" vision validation, explicit deterministic Agentic Planning (V2) gating schemas, and SSE telemetry bridges via vox-codex-api.
- **[GUI Visual Intelligence: Image Analysis Lane Architecture (research 2026)](gui-visual-intelligence-research-2026.md)** — Comprehensive taxonomy of GUI visual bugs AI and LLMs miss (stacking context traps, hydration flash, cross-browser font divergence, clip/overflow, invisible blockers), hub-and-spoke image analysis lane design, Qwen2.5-VL vs Gemini Vision routing, AXTree + screenshot hybrid payload construction, Vox TypeScript codegen-specific risks, continuous feedback flywheel design, and 4-wave implementation roadmap.
- **[CSS and AI Inference: The Computed Styles Cascade Problem (research 2026)](css-ai-inference-research-2026.md)** — Documents the gap between static CSS text analysis and runtime browser computed styles, detailing why CSS cascading makes predictive visual styling exceptionally difficult for AI.
- **[CSS Determinism: Implementation Plan for Vox Style Emission (roadmap 2026)](css-determinism-implementation-plan-2026.md)** — Six-wave implementation plan grounded in live code audit of `codegen_ts/emitter.rs`, `web_ir/`, and the `StyleBlock`/`StyleNode` pipeline. Covers property allowlist validation, selector parsing, specificity tracking, typed value buckets, `@layer` scoping, and a design token system.
- **[Visus Audit Grounding Prompt (SSOT)](prompts/visus-audit-grounding.v1.md)** — Specialized instructions for Qwen 3.5-VL to perform hybrid pixel-level grounding and AXTree verification for GUI audits.
- **[Vox Visus: Image Intelligence and VLM Flywheel Implementation Plan (2026)](vox-visus-vlm-implementation-plan-2026.md)** — Wave-gated roadmap for integrating visual intelligence into the Vox orchestrator. Covers Wave 0 (Foundations), Wave 1 (VLM Spoke Integration), Wave 2 (Golden Baseline Arca Integration), and Wave 3 (Training Flywheel Closure).
- **[AST-Aware Tokenization & Syntax Weighting (finding 2026)](ast-token-alignment-2026.md)** — Strategy and implementation of mapping AST spans to token streams for syntax-critical loss weighting (identifiers, types, control flow weighting k=2.5-5.0). Connects `vox-compiler` spans to `vox-populi` CE loss.

### Contributor and quality research (April 2026)

- **[AI agent panic and shortcut pathology](research-ai-panic-shortcuts-2026.md)** — the shortcut pathology model, VCS guardrails, and PANIC- tracking.
- **[Terminal execution policy research findings](terminal-exec-policy-research-findings-2026.md)** — PowerShell-first terminal discipline for agentic workflows.
- **[Documentation hygiene and AI-agent guidelines 2026](doc-hygiene-agent-guidelines-2026.md)** — LLM instructions for maintaining SSOT integrity.

## Suggested reading paths

## LLM Target Language Pipeline (April 2026)

- **[LLM Target Language Implementation Plan 2026](llm-target-language-implementation-plan-2026.md)** — **Authoritative, file-level implementation plan** produced after a second full audit pass. Corrects false positives from the gap analysis, documents 6 confirmed bugs (with exact file/line references), specifies all deprecated surfaces with machine-readable `> [!DEPRECATED]` callouts for AI tool consumption, and organizes work into 5 waves. **Start here for implementation guidance.**
- **[LLM Target Language Gap Analysis 2026](llm-target-language-gap-analysis-2026.md)** — Original 77-gap cross-cutting audit. False positives in B-1, B-2, B-4, B-6 corrected with `> [!NOTE]` callouts. Superseded by the implementation plan above for implementation guidance; retained as audit record.
- **[Vox Language Testing Pipeline](vox-language-testing-pipeline.md)** — five-stage LLM delivery gate, contract-first generation, `@llm` annotation design, `vox test` CLI spec, ARS skill `vox.testing.validate`.
- **[Fuzzy & Partial Parsing](research-fuzzy-parsing-2026.md)** — skeleton AST on parse failure, phonetic keyword matching, incremental verification, constrained decoding integration.
- **[Grammar-Constrained Decoding](research-grammar-constrained-decoding-2026.md)** — XGrammar-2 vs llguidance vs GBNF comparative analysis, CVE-2026-2069, Stream of Revision, hybrid orchestrated inference architecture.
- **[AST-Aware Tokenization & Syntax Weighting](ast-token-alignment-2026.md)** — mapping AST spans to token streams for syntax-critical loss weighting.

---

### Deep Research Clusters (April 2026)

- **[Research Synthesis: Grand Strategy Seed 2026](research-synthesis-grand-strategy-seed-2026.md)** — the master framework connecting these discoveries.

#### LLM Hallucination & Type System Impact (Wave 1)
- **[LLM-Native Language Design](research-llm-native-lang-design-2026.md)** — cluster overview with Vox implications
- **[Vox as the First AI-Native Language: Reducing K-Complexity](vox-llm-native-language-research-2026.md)**
- **[Language Alias Canonicalization for LLM Target Languages](research-language-alias-canonicalization-2026.md)**
- [Cognitive Science of LLM Hallucinations](research-ts-hallucination-cognitive-science-2026.md)
- [Empirical Evidence for Type Systems](research-ts-hallucination-empirical-evidence-2026.md)
- [Frontier Model Challenges](research-ts-hallucination-frontier-2026.md)
- [K-Complexity Reduction Strategies](research-ts-hallucination-k-complexity-2026.md)
- [Memory Management & Per-Actor GC for LLMs](memory-management-llm-research-2026.md) — Erlang-style GC vs Borrow Checker vs Global GC trade-offs.
- [Actor GC Implementation Blueprint](actor-gc-implementation-blueprint-2026.md) — Operational architecture and codebase mutation pathways for injecting the local runtime allocators.
- [Adopting Erlang's Benefits for LLM-Native Code Generation](erlang-benefits-for-llms-research-2026.md) — Preemption via loop-yielding and supervisor "let-it-crash" integration.
- [Zero-Shot Invariants Validation](research-ts-hallucination-zero-shot-invariants-2026.md)
- [Works Cited: Hallucination & Type Systems](research-ts-hallucination-works-cited-2026.md)

#### Continual Learning & Flywheel Risks (Wave 2)
- **[Continual Learning Flywheel Risks](research-continual-learning-flywheel-2026.md)** — cluster overview with risk taxonomy
- [MAD and Mode Collapse](research-cl-mad-mode-collapse-2026.md)
- [The Compile-Pass Oracle and Semantic Drift](research-cl-oracle-semantic-drift-2026.md)
- [Catastrophic Forgetting in QLoRA](research-cl-qlora-catastrophic-forgetting-2026.md)
- [Schola / Scientia Typicality Bias & Slop](research-cl-slop-typicality-bias-2026.md)
- [Minimum Viable Corpus for QLoRA](research-cl-qlora-minimum-corpus-2026.md)
- [Negative Examples via DPO/NAT](research-cl-nat-dpo-2026.md)
- [Risk Taxonomy and Telemetry Mitigations](research-cl-risk-taxonomy-telemetry-2026.md)
- [Works Cited: Continual Learning Flywheel](research-cl-works-cited-2026.md)
- **[MENS Synthetic Corpus: Limitations and Mitigation Strategies (research 2026)](mens-synthetic-corpus-limitations-research-2026.md)** — maps all active synthetic corpus strategies to their known failure modes and proposes 8 concrete mitigations (AST mutation, DPO wiring, anchor floor, curator LLM, CURLoRA, fictional knowledge graphs, automated flywheel, Rust cross-pollination).
- **[MENS Corpus: Full Implementation Plan (2026)](mens-corpus-implementation-plan-2026.md)** — 4-wave execution plan grounded in mix-report audit (97.3% synthetic monoculture confirmed). Specifies W0 emergency corpus bootstrap, W1 DPO lane wiring and missing mix-config creation, W2 AST mutation + Rust→Vox corpus expansion, W3 semantic quality gates, W4 automated flywheel. Includes exact CLI commands, file specs, dependency graph, and volume projections.
- **[TOESTUB Line Limit & MENS Corpus Size Research (2026)](toestub-line-limit-mens-research-2026.md)** — Investigation into Vox's actual TOESTUB God Object limits (1700 lines) vs documentation (500 lines) and an analysis on optimal LLM chunking/file sizes for SFT pipelines using modern models like Qwen3-4B.

#### GRPO Reward Shaping for Code LLMs (Wave 3)
- **[GRPO Reward Shaping for Code LLMs](research-grpo-reward-shaping-2026.md)** — cluster overview with architectural adjustments
- [Efficacy of Binary Parse-Rate Signalling](research-grpo-binary-parse-rate-2026.md)
- [GRPO VRAM Efficiency and Small-Batch Dynamics](research-grpo-vram-small-batch-2026.md)
- [AST Coverage Scoring and Reward Hacking](research-grpo-ast-reward-hacking-2026.md)
- [Empirical Justification for Reward Weights](research-grpo-reward-weights-2026.md)
- [Optimization Landscape of Positive-Only Loops](research-grpo-positive-only-optimization-2026.md)
- [Gap Analysis and Adjustments](research-grpo-gaps-and-adjustments-2026.md)
- [Works Cited: GRPO Reward Shaping](research-grpo-works-cited-2026.md)

#### AI Agent Context and Handoff Continuity (Wave 4)
- [Vox A2A Orchestration Hardening Findings (2026)](a2a-orchestration-hardening-findings-2026.md) — synthesizes the April wave: loop blocking, durable journals, and surgical context injection.
- [Empirical Evidence for Context Compaction](research-agent-handoff-empirical-compaction-2026.md)
- [Context Bleed and Identity Confusion](research-agent-handoff-context-bleed-2026.md)
- [SOTA Context-Aware Protocols](research-agent-handoff-sota-protocols-2026.md)
- [Context Retrieval Policies](research-agent-handoff-retrieval-policies-2026.md)
- [A2A Protocol Evidence Sharing](research-agent-handoff-a2a-evidence-sharing-2026.md)
- [Context Truncation Failure Modes](research-agent-handoff-truncation-failure-2026.md)
- [Production Failure Catalog](research-agent-handoff-failure-catalog-2026.md)
- [Design Pattern Recommendations](research-agent-handoff-design-patterns-2026.md)
- [Implementation Checklist](research-agent-handoff-checklist-2026.md)
- [Works Cited: Agent Handoff Continuity](research-agent-handoff-works-cited-2026.md)

#### Autonomous Research Localization & MENS Research Lane (Wave 6)
- **[Local autonomous research findings 2026](local-autonomous-research-findings-2026.md)** — SearXNG meta-search integration, native Rust scraping stack (`vox-scraper`), DuckDuckGo fallback, and performance tiering.
- **[MENS Research Track Blueprint 2026](mens-research-track-blueprint-2026.md)** — Lane G (`research-expert`) spec, GRPO+RLVR reward functions, synthetic fact-chain generator, and Socrates integration.
- **[GraphRAG Iterative Retrieval Research 2026](graphrag-iterative-retrieval-research-2026.md)** — Multi-hop retrieve-reason-retrieve loops, stopping heuristics, and C2RAG constraint checking.

#### Scientia distribution, discovery, and publication surfaces

- **[SCIENTIA multi-platform ranking, discovery, and anti-slop SSOT (research 2026)](scientia-multi-platform-ranking-discovery-research-2026.md)** — Tiered citations for social and scholarly ranking surfaces; ingest vs syndicate posture; manifest-centered projection profiles; operator KPI sketches for signal vs noise. Complements [external discovery](scientia-external-discovery-research-2026.md) and [impact / readership](scientia-impact-readership-research-2026.md).
- **[Syndication Ecosystem & Multi-Platform Publishing Research 2026](syndication-ecosystem-research-2026.md)** — Analysis and adoption strategy for third-party Rust SDKs (`atrium`, `megalodon`, `twapi-v2`) to reduce maintenance burden and eliminate manual `reqwest` manipulation for social publishing channels.
- **[Scientia Community Publishing Playbook 2026](scientia-community-publishing-playbook-2026.md)** — Operational playbook for multi-platform community management with minimal overhead. Covers Discord webhook setup, Reddit OAuth + anti-spam rules, GitHub Discussions GraphQL API, `vox-publisher` data model extension requirements, Clavis secret registration needs, and subreddit policy pack templates. Companion to the multi-platform ranking research above.
- **🔬 [Scientia Publication Endpoints — Ground-Truth Research & Implementation Policy (April 2026)](scientia-publication-endpoints-research-2026.md)** — v2. Comprehensive code audit + web research across all 18 publication targets. Adds: ResearchGate full policy (no API exists; passive via DOI; do not implement), ORCID member API (highest-leverage new scholarly target), Figshare REST API (datasets/supplementary). Corrects v1 errors: Reddit User-Agent WAS correct; `social_retry.rs` has **zero call sites** (dead code); `bluesky/mastodon/discord/linkedin` are **absent from `switching.rs`** allowlist and retry infrastructure. Defines formal implementation policy: channel classification taxonomy (ActivePush/ScholarlyDeposit/ManualAssist/PassiveDiscovery/Deferred), gate requirements per class, 13-column hallucination inventory, and 8-wave task backlog with ~50 EP-NNN gap IDs. Last verified: 2026-04-13.

#### Multi-Repository Context Isolation (Wave 5)
- **[Multi-repo context isolation: research findings 2026](multi-repo-context-isolation-research-2026.md)** — `.voxignore` SSOT policy, scope guard architecture, agent instruction file hierarchy, IDE workspace isolation, Git worktree patterns, security threats (IDPI, slopsquatting, scope escalation), context engineering guidelines, monorepo/polyrepo AI-readiness analysis, and `vox repo init` scaffold specification. Directly actionable: gaps table, implementation priorities, and cross-references to `cross-repo-query-observability.md` and `context-management-research-findings-2026.md`.

#### Independent Deep Research Tracks
- **[Semantic Proximity, Split-Brain Detection, and Safe Symbol Surfacing (research 2026)](research-semantic-proximity-split-brain-2026.md)** — Programmatic detection of conceptually related but divergently named code (Type-2 through Type-4 clones). Defines the Knowledge Conflating Hallucination (KCH) risk, the `[0.65, 0.95]` proximity zone, identifier-morpheme distance, doc-comment mining, the `ProximityCandidate` data model, Socrates disambiguation annotation design, the `SymbolProximity` corpus extension to `vox-search`, the anti-conflation MENS DPO lane, a `vox ci proximity-drift` ratchet gate, 6 identified Vox-specific gaps, and a 5-wave implementation roadmap. Cross-references: `rag-and-research-architecture-2026.md`, `nomenclature-migration-map.md`, `research-language-alias-canonicalization-2026.md`, `mens-synthetic-corpus-limitations-research-2026.md`.
- [Agent Trust Reliability Evaluation](research-trust-reliability-signals-2026.md)
- [AI Plan Adequacy Heuristics](research-plan-adequacy-heuristics-2026.md)
- [AI-Augmented Testing & Hourglass Architecture Research](ai-augmented-testing-hourglass-research-2026.md)
- [Compiler Testing Research](research-pbt-oracles-compiled-lang-2026.md)
- [Multi-Agent Mesh Economics](research-multi-agent-mesh-economics-2026.md)
- [Grammar-Constrained Decoding for Code LLMs](research-grammar-constrained-decoding-2026.md)
- [LLM Output Mediation and Programmatic Validator Generation](research-llm-output-mediation-validation-2026.md) — Proposes a unified `LlmMediator<T>` architecture connecting `vox-constrained-gen` (Tier 1), `vox-jsonschema-util` (Tier 2), Socrates confidence (Tier 3), and the trust layer into a single composable seam. Covers dynamic finite-response-set schema derivation, MCP reduction strategy, RLVR training alignment, and a four-wave implementation roadmap. Cross-references grammar-constrained decoding, trust reliability, HITL doubt loop, and capability registry.
- **[AI Agent Panic and Shortcut Pathology (research 2026)](research-ai-panic-shortcuts-2026.md)** — Why AI coding agents suppress errors, use `git restore`/`git reset` destructively, delete tests, and hardcode answers rather than fixing root causes. Synthesizes specification gaming theory, context anxiety, sycophancy, and goal misgeneralization research. Defines 7 platform-level countermeasures for Vox (VCS tiering, allow-drift CI gate, step-friction counter, MENS DPO negative lane, GRPO shortcut signal, annotation fingerprinting, test count differential gate) with a `PANIC-NNN` task tracking prefix.
- **[Clavis as a one-stop secrets manager: research findings 2026](clavis-one-stop-secrets-research-2026.md)** — Comprehensive gap analysis for evolving Vox Clavis into a full-lifecycle secrets management platform. Covers: complete env-var taxonomy across 9 secret classes, user-facing feature requirements, OWASP NHI Top 10 alignment, AI-agent credential isolation boundaries, MCP OAuth 2.1 target model, A2A credential delegation via RFC 8693 Token Exchange, runtime secret redaction pipeline, KEK/DEK envelope encryption model, competitive feature gap table vs. Doppler/Infisical/Pulumi ESC/Vault. Extends [clavis-secrets-env-research-2026.md](clavis-secrets-env-research-2026.md).
- **[Environment Variables vs. Clavis: Strategy and Cross-System Settings 2026](env-vars-clavis-strategy-2026.md)** — Answers when env vars are necessary vs. harmful, the three-tier taxonomy (bootstrap/secrets/config), concrete Docker/deployment guidance, and the architecture for letting logged-in Vox users share settings and secrets across machines, orchestrators, and mesh nodes. Synthesizes the existing Clavis research with new research on distributed config, cross-system sync, and the Hybrid (Keyring + VoxDB ciphertext) model for sovereign cross-device secret sync.
- **[Clavis V2: Full Implementation Plan (2026)](clavis-implementation-plan-2026.md)** — Codebase-verified, code-grounded implementation plan for the full Clavis V2 platform. Anchored in the live codebase (spec.rs, vox_vault.rs, resolver.rs, clavis.rs CLI). Defines: single canonical data structure for all ~580 secrets (TaxonomyClass + LifecycleMeta + scope_description on SecretSpec, 3 new ResolutionStatus variants, 4 new SecretMaterialKind variants); 4 new VoxDB tables (version history, audit log, profile overrides, A2A delegations); updated write path with atomic multi-table transactions; 12 new/updated CLI subcommands (set-secret, rotate, rollback, history, list, diff, run, audit-log, delegate, revoke-delegation); runtime secret scrubber (redact.rs + aho-corasick); consumer wiring for all 8 platform crates; 8-wave execution plan with verification steps per wave; 5 new security invariants extending the V1 threat model.
- **[Cryptography Research Findings 2026](cryptography-research-findings-2026.md)** — ZIG/AEGIS eradication and AES performance evaluation.
- **[Rosetta language comparison: C++, Rust, Python pedagogy research 2026](rosetta-language-comparison-research-2026.md)** — Pedagogy and mechanism findings supporting the Rosetta Inventory rewrite.
- [Vox and Erlang architectural comparison 2026](vox-erlang-comparison-research-2026.md) — Capability breakdown contrasting Vox's durable typed executions and agent orchestration with Erlang's distributed hot code swapping BEAM architecture.
- [FableForge to Vox conversion analysis 2026](fableforge-to-vox-conversion-research-2026.md) — Feasibility study contrasting FableForge's TypeScript/Convex/Next.js stack with Vox's AI-native unified architecture.


### Documentation

- [Orphan surface inventory](orphan-surface-inventory.md)
- [Architecture index](architecture-index.md)
- planning-meta documents when you need contributor process detail

### Packaging and portability

- [Vox Docker-backed portability research 2026](vox-docker-dotvox-portability-research-2026.md)
- [Vox Docker-backed portability implementation plan 2026](vox-docker-dotvox-portability-implementation-plan-2026.md)
- [Vox packaging research findings 2026](vox-packaging-research-findings-2026.md)
- [Vox packaging implementation blueprint](vox-packaging-implementation-blueprint.md)

### Language and architecture direction

- [AI IDE feature research findings 2026](ai-ide-feature-research-findings-2026.md)
- [Prompt engineering, system prompts, document-skills, and SCIENTIA (research 2026)](prompt-engineering-document-skills-scientia-research-2026.md)
- [Terminal execution policy research findings 2026](terminal-exec-policy-research-findings-2026.md) — PowerShell-first shells, IDE allow/deny limits, future unified contract
- [Telemetry unification research findings 2026](telemetry-unification-research-findings-2026.md)
- [Telemetry implementation blueprint 2026](telemetry-implementation-blueprint-2026.md) — roadmap implementation plan
- [Telemetry implementation backlog 2026](telemetry-implementation-backlog-2026.md) — executable checklist
- [Protocol convergence research 2026](protocol-convergence-research-2026.md)
- [Populi GPU network research 2026](populi-gpu-network-research-2026.md)
- [Populi GPU mesh implementation plan 2026](populi-gpu-mesh-implementation-plan-2026.md) — paired **decision** docs: [ADR 017](../adr/017-populi-lease-remote-execution.md), [ADR 018](../adr/018-populi-gpu-truth-layering.md), [ADR 020](../adr/020-populi-mesh-scaling-transport-default.md), [placement matrix](../reference/populi-work-type-placement-matrix.md); probe SSOT: [Hardware Discovery SSOT](gpu-discovery-ssot-findings-2026.md), [Native Layer A Spec](populi-gpu-truth-probe-spec.md), [node lifecycle / hotplug](populi-node-lifecycle-hotplug.md)
- [Mobile/Desktop Convergence & Language Extension Research 2026](mobile-desktop-convergence-research-2026.md) — unified browser view, std.mobile namespace, agent/environment parser gaps, Web API vs Capacitor strategy, maintainability quantification
- [Vox bell-curve strategy](vox-bell-curve-strategy.md)
- [Feature growth boundaries](feature-growth-boundaries.md)
- [Interop tier policy](interop-tier-policy.md)
- **[Web Framework Interop Research 2026](web-framework-interop-research-2026.md)** — Codebase-grounded analysis of React, TanStack, Svelte 5, SolidJS 2.0, Next.js 16, Qwik, Astro, and Vite 8/Rolldown. Includes full artifact inventory from `codegen_ts/emitter.rs`, framework coupling audit of every generated file, corrected gap analysis (e.g. `vox-client.ts` is already framework-agnostic), and the "library mode" vs "app mode" multi-backend strategy. Cross-references `web_ir/`, `react_bridge.rs`, `app_contract.rs`, ADR 010, ADR 012.
- **[Web Framework Interop Backlog 2026](web-framework-interop-backlog-2026.md)** — 160-item prioritized task backlog derived from the interop research audit. Covers 18 subsystems (library mode, Zod, route manifest, WebIR multi-backend, React bridge decoupling, app contract, vox-client, ADTs, CLI templates, reactive refactoring, shared route IR, WebIR validation, CSS emission, Axum alignment, testing, documentation, npm publishing). Each item traces to a specific source file and line range.
- **[Vox Library Mode — Framework-Agnostic Codegen SSOT](vox-library-mode-ssot.md)** — Architectural definition of the framework-agnostic Vox codegen output. Details the zero-dependency SDK generation (Zod schemas, typed fetch client, JSON route manifests) and the transition policy for existing React-coupled projects. Enables Vox consumption from Svelte, Vue, or vanilla TypeScript.

### Hygiene and maintenance

- **[Dependency Sprawl Audit and Resolution (2026)](dependency-sprawl-research-2026.md)** — Records the workspace-wide audit of sprawling Cargo dependencies, centralization into the root `[workspace.dependencies]`, and implementation of TOESTUB CI-CD enforcement rules.
- **[Version Tracking SSOT Research 2026](version-tracking-ssot-research-2026.md)** — Architectural guidance on centralizing Cargo crate versions (`workspace.package.version`), establishing payload versioning limits for Vox IR artifacts, and bonding documentation to strict Git tag releases to reduce technical debt.
- **[VoxScript as Universal Glue Code — Implementation Plan 2026](vox-as-glue-implementation-plan-2026.md)** — Phased roadmap for decommissioning legacy shell/PowerShell technical debt and standardizing on native .vox automation.
- **[VoxScript as Universal Glue Code (research 2026)](vox-as-glue-research-2026.md)** — Policy, security model, execution-tier map, cross-platform deployment analysis, and wave-gated migration plan for replacing all `.ps1`, `.sh`, and `.py` automation scripts with `.vox` files driven by `vox run`. Covers bootstrap exception, capability-permissions model, stdlib gap analysis (`fs`/`process`/`clavis` builtins), and `workflow`-vs-`fn main()` guidance for durable pipelines.

### Agentic planning and orchestration

- [Research Synthesis: Symphony Conduction vs. Agent Orchestration 2026](orchestrator-symphony-research-2026.md) — Extensive structural mapping of real-world conduction (Ictus, DAGs, HITL) to `vox-dei`
- [Claude Code Ultraplan research 2026](claude-code-ultraplan-research-2026.md) — architecture deep-dive, cost model, failure modes, and actionable Vox recommendations
- **[Unified Agentic Control Surface Research 2026](agentic-control-surface-research-2026.md)** — Tri-state pilot console, "Second Pass" validation, and Doubt metaphor unification.
- [Dynamic agentic planning 2026](res_dynamic_agentic_planning_2026.md) — earlier research seed for planning-mode architecture
- [Orchestrator multi-agent groundwork 2026](orchestrator-multi-agent-groundwork-2026.md)
- [Context management research findings 2026](context-management-research-findings-2026.md)
- [Context management implementation blueprint](context-management-implementation-blueprint.md)
- [Vox agentic loop and MENS plan](vox_agentic_loop_and_mens_plan.md)
- [VCS for agent state and artifact snapshotting research 2026](vcs-agent-state-research-2026.md) — Using Jujutsu to automate artifact persistence and reversibility over Vox DEI.
- **[Network Neuroscience Theory and AI Agent Orchestration (Research 2026)](network-neuroscience-theory-research-2026.md)** — Applying NNT principles (small-world topology, dynamic reconfiguration) to Vox's multi-agent orchestrator.
- **[Network Neuroscience Theory Orchestration Implementation Plan (2026)](nnt-orchestration-implementation-plan-2026.md)** — The 3-wave implementation roadmap for dynamic affinity routing and MENS GRPO `r_routing_efficiency` optimization.

### SCIENTIA novelty / publication ledger (contracts)

- Finding-candidate and novelty-evidence v1 JSON Schemas live under `contracts/scientia/` (`finding-candidate.v1.schema.json`, `novelty-evidence-bundle.v1.schema.json`); example fixtures under `contracts/reports/scientia-*.example.v1.json`. CI: `vox ci scientia-novelty-ledger-contracts` (also nested in `vox ci ssot-drift`). CLI spot-check: `vox scientia finding-candidate-validate`, `vox scientia novelty-evidence-bundle-validate`.
- **🔴 PRIMARY IMPLEMENTATION SSOT (use this for all implementation work):** [scientia-pipeline-ssot-2026.md](scientia-pipeline-ssot-2026.md) — unified inbound + outbound gap remediation specification. Code-verified against real sources. 28 implementation tasks (G1–G28) organized into 9 dependency-ordered execution groups. Includes canonical data model, DB schema changes, env var registry, Clavis secret registry, and LLM-executor verification ritual. Supersedes gap analysis and wave playbook for implementation decisions.
- **Impact / readership / citation-adjacent signals (research seed):** [scientia-impact-readership-research-2026.md](scientia-impact-readership-research-2026.md) and tunable weights in [`contracts/scientia/impact-readership-projection.seed.v1.yaml`](../../../contracts/scientia/impact-readership-projection.seed.v1.yaml) (orthogonal to novelty; no default publish gate).
- **Multi-platform ranking, discovery, and anti-slop SSOT (research 2026):** [scientia-multi-platform-ranking-discovery-research-2026.md](scientia-multi-platform-ranking-discovery-research-2026.md) — social and scholarly feed mechanics (tiered sources), ingest vs syndicate, projection profiles, anti-slop metrics; bridges outbound `vox-publisher` syndication and inbound external discovery.
- **Publication-worthiness + SSOT unification research plan:** [scientia-publication-worthiness-ssot-unification-research-2026.md](scientia-publication-worthiness-ssot-unification-research-2026.md) (standards-to-signals matrix, canonical metadata graph proposal, detection calibration protocol, Codex research snapshot persistence blueprint, automation boundary ledger).
- **Implementation wave playbook (historical context):** [scientia-implementation-wave-playbook-2026.md](scientia-implementation-wave-playbook-2026.md) (232-task execution map, wave outputs, first-30 lock order, and contract inventory).
- **Comprehensive gap analysis (historical context):** [scientia-gap-analysis-2026.md](scientia-gap-analysis-2026.md) — 45 identified problems with solutions, severity ratings, and a 7-wave execution order.
- **Scientia Worthiness × Socrates Unification (research 2026):** [scientia-socrates-unification-research-2026.md](scientia-socrates-unification-research-2026.md) — deep structural analysis of isomorphisms between the Worthiness publication gate and the Socrates real-time confidence protocol. 38+ integration ideas organized into 8 themes (shared numeric language, inbound pipeline, A2A communication, MENS training, etc.), explicit separation-of-concerns boundaries, risk map, and wave-gated implementation roadmap.
- **Scientia Publisher & Orchestrator Hardening Plan (roadmap 2026):** [scientia-publisher-hardening-implementation-plan-2026.md](scientia-publisher-hardening-implementation-plan-2026.md) — ordered execution plan for de-factoring God Objects across vox-publisher, vox-orchestrator, and vox-cli to adhere to the 500-line TOESTUB policy.
- **🔴 PRIMARY IMPLEMENTATION TASK LIST v2 (use this to execute work):** [scientia-publication-pipeline-implementation-plan-2026.md](scientia-publication-pipeline-implementation-plan-2026.md) — 31 explicit tasks (T-001 to T-031) across 8 waves. **v2 corrects 13 factual errors from v1** including: Bluesky XRPC URL had wrong method path AND wrong request field conflation; `SyndicationResult` already had bluesky/mastodon/linkedin/discord fields; `social_retry` was already wired (not dead code); Zenodo adapter is fully complete (564L, create+upload+publish+retry); Mastodon API accepts JSON body; Discord resolves its own Clavis webhook; LinkedIn REST endpoint is `/rest/posts` not `/v2/posts`; all four social Clavis SecretIds already exist. Includes exact Rust code patterns, per-task verification commands, wave-gated dependency ordering, and a permanent Do-Not-Implement registry.

## Labeling rule

If a page is primarily research or a roadmap, say so in the title, frontmatter, or first paragraph. Do not rely on filenames alone.


