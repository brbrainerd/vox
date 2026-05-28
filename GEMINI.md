---
title: "Antigravity Overlay"
description: "Antigravity-specific behavior and shell discipline for Windows + PowerShell."
category: "contributor"
status: "current"
training_eligible: true
training_rationale: "Defines Antigravity-specific rules and shell environment expectations."
---
# Antigravity Overlay (Windows + PowerShell)

This file is Antigravity-specific. It narrows behavior for this repo without replacing `AGENTS.md`.

## Scope

- Treat `AGENTS.md` as the cross-tool base policy.
- Use this file only for Antigravity-specific behavior and shell discipline.
- Keep rules concise, concrete, and executable.

## VoxScript-First Glue

Automation is **`.vox` only** (tiers: `--interp`, native, `--isolation wasm`); never new `.ps1` / `.sh` / `.py` glue. Bootstrap launchers stay thin. **Normative detail:** [`AGENTS.md §VoxScript-First Glue Code`](AGENTS.md).

## Shell Environment

- Workspace environment is Windows; **PowerShell is canonical** for the two retained launcher files and for interactive terminal work.
- Repo-wide doctrine (see [`AGENTS.md`](AGENTS.md)): on **any** OS, prefer **`pwsh`** for terminal/agent shell work when installed, so behavior aligns with `vox shell check` and `contracts/terminal/exec-policy.v1.yaml`.
- Prefer PowerShell-native commands for filesystem and process tasks **only when** not calling into project automation (which should be `.vox`).
- Use project tools directly (`vox`, `cargo`, `pnpm`, `rg`, `git`) instead of shell wrappers.

## Command Shape Rules (Important)

- Emit one terminal command per step by default.
- Do not chain commands with `|`, `&&`, `;`, or `||` unless explicitly required.
- Do not wrap routine commands in `bash -lc` or nested shell invocations.
- If a task is multi-step, execute it as separate terminal calls.

Reason: command approval/allowlist matching in current IDE ecosystems is often brittle on compound commands, especially in PowerShell contexts.

Research synthesis (Cursor, Gemini, Codex, PowerShell, bypass classes, future Vox contract): [`docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md`](docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md) (archived).

## Tooling Preferences

- Search text: `rg`
- Filesystem listing and checks: `Get-ChildItem`, `Test-Path`, `Resolve-Path` (interactive terminal only; use `vox run` for scripted file ops)
- File reads/writes from the IDE: use structured read/edit tools when available
- Package managers: `pnpm` for JS/TS
- **Python (`uv`) is NOT a preferred automation tool** — use `vox run` instead

## Safety Posture

- Treat allowlists as convenience, not as a hard security boundary.
- Keep destructive operations explicitly denied in IDE policy where supported.
- When unsure, choose decomposition and explicitness over shell cleverness.

## Cursor IDE overlay

For Cursor-specific rules, see [`.cursor/rules/`](.cursor/rules/).
The `build-environment.mdc` and `retired-surfaces.mdc` rules supplement the PowerShell discipline above.

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
