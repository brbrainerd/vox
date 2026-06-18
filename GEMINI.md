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

## Agent Skills & Plan Execution

Antigravity mounts repo skills from `.agents/skills/`, which is wired to the in-repo SSOT **`crates/vox-skills/skills/superpowers/`** (brainstorming, subagent-driven-development, dispatching-parallel-agents, verification-before-completion, test-driven-development, systematic-debugging, requesting-code-review, using-git-worktrees, writing-plans, deep-research, research). Antigravity cannot see Claude's external `~/.claude/` skill cache, so any skill a plan references must load from here.

`.agents/` is gitignored (a per-machine mount). The wiring is a live relative symlink `.agents/skills → ../crates/vox-skills/skills/superpowers`. On a fresh clone where the symlink didn't materialize (Windows `core.symlinks=false`, or a headless session), populate the mount from the SSOT instead:

```
vox run --mode interp scripts/sync-superpowers-skills.vox -- --write
```

Never edit `.agents/skills/` directly — it mirrors the SSOT; edit the source under `crates/vox-skills/skills/superpowers/` and re-sync.

When executing a written implementation plan from `docs/superpowers/plans/`, first read [`docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md) (execution model, parallel-vs-sequential dispatch rule, skill map) and [`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) (why plans are shaped atomic-green-commit + verify-before-use + two-strike). Honor each task's `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tag; never run two subagents that write the same file.

## Cursor IDE overlay

For Cursor-specific rules, see [`.cursor/rules/`](.cursor/rules/).
The `build-environment.mdc` and `retired-surfaces.mdc` rules supplement the PowerShell discipline above.

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.
