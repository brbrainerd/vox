---
title: "Agent instruction architecture"
description: "How Vox layers AGENTS, tool-specific overlays, continuation prompts, and CI gates for durable agent behavior."
category: "Contributors"
status: "current"
last_updated: "2026-04-02"
training_eligible: true

schema_type: "TechArticle"
---

# Agent instruction architecture

This page defines how to keep agent instructions short, durable, and enforceable across long-running sessions.

## Why this exists

Instruction files are loaded into context and lose influence as sessions grow. The fix is not "more text"; it is strict layering.

- Keep always-loaded policy small and stable.
- Move volatile guidance to tool-specific overlays.
- Put verification in CI gates whenever possible.

## Layer model

| Layer | Surface | Purpose | What belongs here |
| --- | --- | --- | --- |
| Base policy | `AGENTS.md` | Cross-tool, always-loaded constraints | Repo non-negotiables, secret policy, short navigation pointers |
| Tool overlay | `GEMINI.md` (Antigravity), other tool-specific files | Environment/tool-specific behavior | PowerShell discipline, command-shape constraints, IDE quirks |
| Recency reinforcement | continuation prompt | Mid/late-session behavior shaping | Anti-decay behavioral directives, execution posture |
| Machine enforcement | `vox ci` and policy contracts | Verifiable guarantees | Stub gates, schema checks, completion quality controls |

Decision rule:

- If it is machine-verifiable, prefer CI.
- If it is a cross-tool invariant, put it in `AGENTS.md`.
- If it is IDE or shell specific, put it in a tool overlay.
- If it is about attention drift in long sessions, use continuation prompts. For handling attention decay in long sessions, see [Continuation Prompt Engineering](continuation-prompt-engineering.md).

## Command policy strategy (PowerShell-first)

Permission matchers in multiple IDEs can fail on compound shell commands. Do not depend on brittle parser behavior for safety.

Long-form evidence, vendor links, and SSOT terminal policy: [Terminal execution policy research findings 2026](../archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md), [Terminal AST validation research 2026](../archive/research-2026-q1/terminal-ast-validation-research-2026.md). Enforced allowlist: [`contracts/terminal/exec-policy.v1.yaml`](../../../contracts/terminal/exec-policy.v1.yaml) (validated by `vox shell check` and `vox ci exec-policy-contract`).

Prefer:

- One command per terminal step (unless the user or policy explicitly allows pipelines; narrow pipeline patterns may be allowlisted under exec-policy).
- **`pwsh` on Linux and macOS when installed** — same cmdlet surface and the same `vox shell check` semantics as on Windows.
- PowerShell-native filesystem cmdlets instead of POSIX habits copied into a PowerShell session.
- Stable project tools: `rg`, `git`, `cargo`, `pnpm`, `uv`, `vox`.

Avoid by default:

- Pipelines and chain operators (`|`, `&&`, `;`, `||`) in policy-critical commands.
- Wrapper shells (`bash -lc`, nested shell calls) for routine tasks.
- Linux-only command habits in Windows sessions when PowerShell equivalents exist.

## Copy-paste block for Antigravity customizations

Use this block in Antigravity customizations when you want a strict PowerShell-first command policy.

```markdown
# Windows PowerShell command policy

- Environment is Windows. Use PowerShell-compatible commands.
- Use one terminal command per step.
- Do not emit compound commands with `|`, `&&`, `;`, or `||` unless explicitly required by the user.
- Do not use wrapper shells like `bash -lc` for routine tasks.
- Prefer `rg` for search.
- Prefer `Get-ChildItem`, `Test-Path`, `Resolve-Path` for filesystem tasks.
- Use project tools directly: `vox`, `cargo`, `pnpm`, `uv`, `git`.
- If a task needs multiple actions, execute separate commands in sequence instead of chaining.
- Treat allowlists as convenience only; keep risky/destructive commands denied explicitly in IDE policy where available.
```

## Copy-paste block (PowerShell 7 on Linux / macOS)

Use when the agent host has **`pwsh`** installed and you want parity with Windows cmdlet semantics and `vox shell check`.

```markdown
# PowerShell 7 command policy (Unix-like host)

- Use `pwsh` as the interactive shell when available.
- Use one terminal command per step by default; avoid pipelines unless required and consistent with exec-policy.
- Prefer `Get-ChildItem`, `Test-Path`, `Resolve-Path`, `Join-Path` over `ls` / string-built paths.
- Prefer `rg` for search; use `vox`, `cargo`, `pnpm`, `uv`, `git` directly.
- Validate risky lines locally with `vox shell check --payload "..."` when unsure.
```

## Provenance and confidence

When documenting IDE behavior:

- Mark vendor-documented behavior as **documented**.
- Mark forum reports as **community-reported**.
- Mark reverse-engineered patch analyses as **community-reverse-engineered**.

Do not present undocumented internals as canonical facts.

## Maintenance

Update this page when changing instruction architecture or shell discipline policy. Also review:

- [`AGENTS.md`](../../../AGENTS.md)
- [`docs/src/architecture/terminal-exec-policy-research-findings-2026.md`](../archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md)
- [`docs/src/contributors/continuation-prompt-engineering.md`](continuation-prompt-engineering.md)
- [`docs/src/contributors/documentation-governance.md`](documentation-governance.md)
