---
title: "ADR-029: Formal Intent and Tool Receipt Auditing"
description: "Architecture Decision Record for formal intent and cryptographic tool receipts in the Vox ecosystem."
category: "Architecture Decisions (ADRs)"
status: "research"
---
# ADR 029: Formal Intent and Tool Receipt Auditing

> Renumbered from ADR-024 on 2026-05-02 to resolve a duplicate-number collision with [`024-dashboard-axum-spa`](024-dashboard-axum-spa.md).

## Status
Proposed (2026-04-23)

## Context
AI agents in the Vox ecosystem perform high-stakes operations (file edits, VCS commits, database writes). Hallucinations and autonomous loops can lead to corrupted codebases and budget exhaustion. We need a way to verify that a tool call was explicitly intended by the orchestrator and actually executed as reported.

## Decision
We implement a two-tier verification system:
1. **Formal Intent**: Agents must claim a "receipt" for every tool call they wish to report as successful.
2. **Cryptographic Tool Receipts**: The orchestrator issues HMAC-signed receipts for every tool execution it brokers. Agents include these receipt IDs in their task completion claims.

## Consequences
- Agents cannot hallucinate tool outputs that were never executed.
- Socrates (hallucination defense) can explicitly check for "fabricated" claims.
- Auditing logs gain a cryptographic trail for every side-effect in the repository.
