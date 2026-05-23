---
title: "SCIENTIA A2A evidence-gathering tasks"
description: "Bounded remote task envelopes for read-heavy scientia_evidence hydration (no autonomous claim generation)."
category: "Architecture SSOTs"
last_updated: "2026-03-28"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# SCIENTIA A2A evidence-gathering tasks

Orchestrator / mesh A2A can delegate **read-heavy, idempotent** jobs that return structured JSON for `metadata_json.scientia_evidence` or `publication_status_events`. This document names task *kinds* for operators and agent authors; routing uses existing `RemoteTaskEnvelope` types in `vox-orchestrator` (`a2a` / `envelope` modules).

## Allowed task families

| Task kind (logical) | Goal | Must not |
|---------------------|------|----------|
| `scientia.gather.benchmark_lineage` | Collect baseline/candidate run ids and report paths | Invent benchmark outcomes |
| `scientia.gather.repo_docs` | List ADR/research paths and linked corpus | Summarize novelty |
| `scientia.gather.repro_artifacts` | Find checksum / manifest paths | Claim reproducibility passed |
| `scientia.gather.venue_requirements` | Fetch venue checklist text (cached) | Assert submission eligibility |
| `scientia.gather.credential_presence` | Clavis/env **presence** bits only | Expose secret values |

## Envelope rules

1. **Payload** is JSON with `task_kind`, `publication_id`, `repository_id` (when known), and `idempotency_key`.
2. **Result** merges into `scientia_evidence` or appends a status event with `detail_json` pointing at file paths and digests.
3. **Refusal:** if grounding artifacts are missing, return `blocked_reasons` — never backfill with LLM prose.
4. **Human loop:** meaningful advance, novelty, and final abstract remain human-attested per [how-to: Scientia publication](../how-to/how-to-scientia-publication.md).

## Related

- Discovery ranking: `vox_scientia_publication_discovery_scan` / `vox scientia publication-discovery-scan`
- LLM assist (bounded): `vox_scientia_assist_suggestions` (`use_llm=false` for heuristic-only)


