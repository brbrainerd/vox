---
title: "ADR 005: Socrates anti-hallucination SSOT"
description: "Official documentation for ADR 005: Socrates anti-hallucination SSOT for the Vox language. Detailed technical reference, architecture gui"
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 005: Socrates anti-hallucination SSOT

## Status

Accepted — baseline implementation in progress.

## Context

LLM surfaces (MCP chat, planning, TOESTUB review, research-style flows) each used ad hoc confidence thresholds and prompts. That caused drift (e.g. prompt “≥80%” vs client filter `≥40`) and made abstention and escalation non-deterministic for agents.

## Decision

1. **Single policy crate** — `vox-socrates-policy` holds `ConfidencePolicy`, `RiskDecision`, and `RiskBand`; all crates import it for thresholds and classification.
2. **Orchestrator types** — `vox-orchestrator::socrates` defines `EvidenceItem`, `ClaimRecord`, `ConfidenceSignal`, `SocratesOutcome`, and optional `SocratesTaskContext` on `AgentTask`.
3. **Gating** — Task completion may run a Socrates gate when `socrates_gate_enforce` is true and the task has `socrates` context; shadow mode logs without blocking.
4. **Persistence** — Reliability and claim outcomes use Codex tables from schema V10 (`agent_reliability`, `claim_outcomes`).
5. **MCP** — Chat/plan responses may include optional `socrates` telemetry JSON.

## Consequences

- New workspace member `vox-socrates-policy` (minimal dependency surface).
- Schema migration V10 for reputation-style metrics.
- Documentation cross-links: `AGENTS.md`, `docs/agents/orchestrator.md`, handoff protocol, MCP reference.

## Rollout

1. Deploy policy crate + docs (no behavior change if gates off).
2. Enable `socrates_gate_shadow` in staging; inspect logs.
3. Enable `socrates_gate_enforce` for pilot agents/tasks with explicit `SocratesTaskContext`.

## References

- [Socrates protocol SSOT](../reference/socrates-protocol.md)
- `crates/vox-socrates-policy`
- `crates/vox-orchestrator/src/socrates.rs`


