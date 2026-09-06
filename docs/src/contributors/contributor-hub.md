---
title: "Contributor hub"
description: "Start here for contributor-facing Vox documentation, governance, inventories, and review checklists."
category: "Contributors"
status: "current"
sort_order: 0
training_eligible: true

schema_type: "TechArticle"
---

# Contributor hub

This page is the reader-facing entry point for contributor documentation.

If you are evaluating Vox as a language or product, start with the [project README](../../../README.md), the [FAQ](../explanation/faq.md), and the [tutorials](../tutorials/tut-getting-started.md). If you are changing this repository, start here.

## Start here

- [AGENTS.md](../../../AGENTS.md) - required contributor and agent policy entry point, with vox-secrets as the secret-management SSOT.
- [Agent instruction architecture](agent-instruction-architecture.md) - instruction layering model (`AGENTS.md`, tool overlays, continuation prompts, CI gates).
- [Coding Agent Instructions](coding-agents.md) - heuristics and rules for agents, including god object constraints and stale docs guidelines.
- [Documentation governance](documentation-governance.md) - where docs live, which surface owns what, status vocabulary, and review cadence.
- [Documentation Reality Audit Program](docs-reality-audit-program.md) - doc/code/contract triage backlog, scoring, and `vox ci docs-reality-audit`.
- [CI runner contract](../ci/runner-contract.md) - canonical `vox ci` guidance, runner labels, and line-ending policy.
- [Doc inventory verifier](../reference/doc-inventory.md) - machine-readable doc inventory workflow and drift expectations.
- [Architectural governance (TOESTUB)](../../agents/governance.md) - repository governance, organization rules, and quality policy.
- [`docs/agents/`](../../agents/) — full agent-facing support docs (orchestrator behavior, handoff protocol, editor contracts, time awareness).

## I want to…

Quick routing by goal. All links resolve from this directory.

| Goal | Start here |
| --- | --- |
| Fix a TOESTUB / stub-check CI failure | [TOESTUB contributor guide](toestub-contributor-guide.md) |
| Fix a `god_object` or `sprawl` violation | [TOESTUB contributor guide](toestub-contributor-guide.md) (god-object rules) |
| Understand why my code affects model quality | [Contribution loop](contribution-loop.md) |
| Add a golden `.vox` example | [Examples corpus how-to](../how-to/examples-corpus.md) |
| Write or update documentation | [Documentation governance](documentation-governance.md) |
| Contribute to the compiler / parser | [How-To: parser and HIR](../how-to/how-to-contribute-parser-hir.md) |
| Contribute to MENS training pipeline | [How-To: Mens native training](../how-to/how-to-contribute-mens.md) |
| Add a CLI command | [CLI reference](../reference/cli.md) + [Command compliance](../reference/command-compliance.md) |
| Work with secrets or credentials | [Secrets SSOT](../reference/secrets-ssot.md) |
| Understand the agentic quality model | [Coding agent instructions](coding-agents.md) + [Governance (TOESTUB)](../../agents/governance.md) |
| Read architecture or research context | [Where things live](../architecture/where-things-live.md) · Starlight sidebar section **Architecture SSOTs** |

## Contributor map

Use these surfaces intentionally:

| Need | Start with |
| --- | --- |
| Cursor IDE rules and per-glob patterns | [`.cursor/rules/`](../../../.cursor/rules/) |
| Secrets, credentials, env parity | [AGENTS.md](../../../AGENTS.md), [Secrets SSOT](../reference/secrets-ssot.md) |
| Agent behavior consistency across long sessions and IDEs | [Agent instruction architecture](agent-instruction-architecture.md), [Continuation prompt engineering](continuation-prompt-engineering.md) |
| Antigravity-specific overrides | [GEMINI.md](../../../GEMINI.md), [Agent instruction architecture](agent-instruction-architecture.md) |
| Terminal shell discipline, exec-policy, `vox shell check` | [AGENTS.md](../../../AGENTS.md), [CLI reference](../reference/cli.md) (`vox shell`), [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml) |
| CLI or command-surface changes | [CLI reference](../reference/cli.md), [Command compliance](../reference/command-compliance.md) |
| Documentation updates or new docs | [Documentation governance](documentation-governance.md) |
| Telemetry, metrics, privacy boundaries | [Telemetry trust SSOT](../architecture/telemetry-trust-ssot.md), [Telemetry metric contract](../reference/telemetry-metric-contract.md) |
| Architecture or roadmap context | [Where things live](../architecture/where-things-live.md) · Starlight sidebar **Architecture SSOTs** |
| Contracts and schema-backed behavior | [contracts/README.md](../../../contracts/README.md), related reference pages under `docs/src/reference/` |
| MCP, HTTP, Populi mesh, SSE, WebSockets | [Communication protocols](../reference/communication-protocols.md), [protocol catalog](../../../contracts/communication/protocol-catalog.yaml); research [Protocol convergence research 2026](../archive/research-2026-q1/protocol-convergence-research-2026.md) |
| CI, workflow, or policy guardrails | [CI runner contract](../ci/runner-contract.md), [Dependency policy](dependency-policy.md), [Pre-push local CI parity](#pre-push-local-ci-parity) (below), [Architectural governance (TOESTUB)](../../agents/governance.md) |
| VS Code / Cursor extension, MCP tool calls from the editor, Oratio speech UX | [`apps/editor/vox-vscode/README.md`](../../../apps/editor/vox-vscode/README.md), [VS Code ↔ MCP compatibility](../reference/vscode-mcp-compat.md), [Speech capture architecture](../reference/speech-capture-architecture.md) |

Fast local policy rerun for this lane:

- `vox ci policy-smoke` runs `cargo check -p vox-orchestrator`, then command-compliance and the same rust ecosystem parity test used by `vox ci rust-ecosystem-policy` in one command.

## Pre-push: local CI parity

**Default `git push` hook:** **`vox ci pre-push`** runs the **fast** profile (scoped docs + critical guards). Use **`vox ci pre-push --complete`** for the full static gate before risky merges; CI still runs the heavy docs-quality sweep.
Install once via **`vox ci install-hooks`**. See [local CI parity](local-ci-pre-push.md) for **`--full`**, heartbeats, **`--no-verify`** policy, and tuning **`VOX_PREPUSH_BASE`**.

## Contributor expectations

- Prefer updating the canonical surface instead of copying prose into a second location.
- When code changes alter public behavior, update the corresponding docs in the same PR.
- Treat `contracts/` as machine SSOT, `docs/src/reference/` as human lookup, `docs/src/architecture/` as design and rationale, and `docs/agents/` as contributor and automation support.
- Use `vox ci` guards where they exist instead of replacing them with one-off shell checks.
