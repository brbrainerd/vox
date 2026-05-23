---
title: "Architecture Classification SSOT (2026)"
description: "Authoritative taxonomy for classifying architectural artifacts, research findings, and implementation blueprints."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Establises the categorization logic for repository-wide documentation hygiene."
archived_date: 2026-04-18
---

# Architecture Classification SSOT (2026)

## Overview
To manage the scaling complexity of the Vox architectural documentation (270+ artifacts), all documents must be classified into one of the following nine domains. This taxonomy ensures consistent organization in the [Architecture Index](architecture-index.md) and enables automated hygiene checks.

## Domain Taxonomy

| Code | Domain | Scope | Examples |
|---|---|---|---|
| **F** | **Foundations** | Compiler, Language, Types, IR | `compiler-ir-pipeline.md`, `language-surface-ssot.md` |
| **I** | **Infrastructure** | Database, Telemetry, Secrets, Persistence | `clavis-ssot.md`, `telemetry-trust-ssot.md`, `voxdb-connect-policy.md` |
| **M** | **Intelligence (Mens)** | ML Training, Tensor, Vision, Synthetic Data | `mens-training-ssot.md`, `ast-token-alignment-2026.md` |
| **O** | **Orchestration** | Agents, DEI, A2A, Workflow, Planning | `orchestrator-symphony-research-2026.md`, `agent-planning-multimodal-ssot.md` |
| **U** | **Interface** | CLI, Web, GUI, VS Code Extension | `cli-design-rules-ssot.md`, `vox-web-stack-ssot.md`, `islands-standard.md` |
| **S** | **Trust & Safety** | Socrates, Reliability, Security, Privacy | `socrates-ssot.md`, `trust-reliability-layer.md`, `news_syndication_security.md` |
| **P** | **Operations** | CI/CD, Packaging, Build Glue, Scripts | `vox-as-glue-research-2026.md`, `crate-topology-buckets.md` |
| **R** | **Research** | Findings, Scouting, Comparisons, Benchmarks | `gpu-discovery-ssot-findings-2026.md`, `mens-lane-segmentation-research.md` |
| **G** | **Governance** | Policies, Inventories, Hygiene, Deprecation | `doc-hygiene-agent-guidelines-2026.md`, `legacy-retirement-roadmap.md` |

## Classification Rules
1. **Primary Domain**: Every document must have a `domain` field in its YAML front matter matching one of the codes above.
2. **Naming Convention**: Documents should ideally be prefixed with their domain code if they are in a sub-registry, but the `domain` field is the source of truth.
3. **Cross-Domain Docs**: If a document covers multiple domains, choose the one with the highest functional impact. Use cross-references for the others.

## Implementation Status
- [ ] Update `architecture-index.md` to use this taxonomy.
- [ ] Backfill `domain` field to all 270+ artifacts.
- [ ] Consolidate `research-index.md` and `INDEX.md` into the new structure.


