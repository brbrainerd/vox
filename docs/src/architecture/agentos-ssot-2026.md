---
title: "AgentOS & Agent-Computer Interface SSOT (2026)"
description: "Single baseline for contract-first ACI envelopes, mutation classification, guardrails, checkpointing, and semantic retrieval bridges."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-10"
training_eligible: true
training_rationale: "Anchors AgentOS implementation to contracts and crate boundaries."
schema_type: "TechArticle"
audience: ["contributors", "agents"]
related:
  - docs/src/architecture/terminal-exec-policy-ssot.md
  - docs/src/architecture/search-retrieval-ssot-2026.md
  - contracts/aci/agent-computer-interface.v1.yaml
---

# AgentOS & Agent-Computer Interface SSOT (2026)

## 1. Scope

This document is the **architecture SSOT** for **AgentOS** work inside Vox: structured tool I/O (ACI), safety guardrails, semantics-aware checkpoint/replay, and semantic filesystem/intent operations layered on [`vox-search`](../../../crates/vox-search/).

It does **not** replace:

- **Terminal allowlists** — [`terminal-exec-policy-ssot.md`](terminal-exec-policy-ssot.md) + [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml).
- **Retrieval fusion rules** — [`search-retrieval-ssot-2026.md`](search-retrieval-ssot-2026.md).

## 2. Contracts

| Artifact | Role |
|----------|------|
| [`contracts/aci/agent-computer-interface.v1.yaml`](../../../contracts/aci/agent-computer-interface.v1.yaml) | Human-readable ACI v1 metadata and enum semantics. |
| [`contracts/aci/agent-computer-interface.v1.schema.json`](../../../contracts/aci/agent-computer-interface.v1.schema.json) | JSON Schema for MCP tool responses carrying a sibling `aci` block. |

## 3. Mutation classification

Every MCP tool dispatch SHOULD attach `aci.mutation_kind`:

| Kind | Meaning |
|------|---------|
| `read_only` | Observation-only; safe for fast-forward replay without reverting FS. |
| `local_mutation` | Workspace-local writes (files, VCS index, etc.). |
| `external_side_effect` | Network, spend, or host-global effects. |
| `unknown` | Conservative default until classified. |

## 4. Crate map

See [`where-things-live.md`](where-things-live.md) — AgentOS rows under orchestrator, MCP, CLI shell, and search.

## 5. Execution tiers

- **MCP tools**: normalized JSON validated against ACI schema when [`OrchestratorConfig::agentos_aci_envelope_enabled`](../../../crates/vox-orchestrator/src/config/orchestrator_fields.rs) is true (default **false** until clients opt in).
- **Host shell**: adapters live under `vox-cli` `commands/runtime/shell/`; policy remains PowerShell-first for AST allowlisting unless a structured backend is explicitly selected.
- **Structured shell data in Vox scripts:** `std.fs` / `std.process` / `std.csv|toml|yaml|io` are **native Rust** builtins — see [`vox-shell-stdlib-ssot-2026.md`](./vox-shell-stdlib-ssot-2026.md). They are unrelated to `vox_run_shell` except via separate AgentOS probes.
- **`aci.shell_backend` on `vox_run_shell`:** when envelopes attach, the field reflects the MCP argument `backend` (`powershell` default; `nu` / `nushell` → `nushell`). See [`vox-orchestrator-mcp/src/aci/envelope.rs`](../../../crates/vox-orchestrator-mcp/src/aci/envelope.rs).

## 6. Telemetry

When the guardrail kernel denies a tool preflight and `VoxDb` is connected, MCP dispatch appends a `research_metrics` row with [`METRIC_TYPE_AGENTOS_GUARDRAIL_DENY`](../../../crates/vox-telemetry/src/types.rs) (`orch.agentos.guardrail_deny`), session `mcp:<repository_id>`, and JSON metadata matching [`contracts/telemetry/agentos-guardrail-deny.v1.schema.json`](../../../contracts/telemetry/agentos-guardrail-deny.v1.schema.json).

## 7. Change checklist

- Bump `x-vox-version` / `version` when breaking the schema; rename files if breaking.
- Register new contract paths in [`contracts/index.yaml`](../../../contracts/index.yaml).
- Run `cargo test -p vox-orchestrator-mcp aci_` after envelope changes.
