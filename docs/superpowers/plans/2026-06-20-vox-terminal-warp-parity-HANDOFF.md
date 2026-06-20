---
title: "Vox Terminal (Warp-parity) — Sonnet 4.6 handoff prompt"
description: "Copy-paste prompt to start a fresh Claude 4.6 Sonnet session executing the Vox Terminal implementation plan."
category: "Plans"
status: "draft"
training_eligible: false
training_rationale: "Throwaway execution prompt."
---

# Handoff prompt — paste into a fresh Claude 4.6 Sonnet (Claude Code) session

> Copy everything in the fenced block below.

```text
You are implementing the Vox Terminal (Warp-parity standalone) feature in the `vox` repo.

PLAN (your single source of truth, read it fully first):
  docs/superpowers/plans/2026-06-20-vox-terminal-warp-parity.md

REQUIRED SUB-SKILL: invoke `superpowers:subagent-driven-development` (preferred) or
`superpowers:executing-plans` and follow it to drive the plan task-by-task.

BEFORE WRITING ANY CODE, read in this order:
  1. AGENTS.md  →  docs/src/architecture/where-things-live.md
  2. docs/src/contributors/claude-code-sonnet-handoff-limitations.md  (your operating envelope)
  3. The plan above, including §14 "Phase-4 audit log".

START AT: Track 1, Task 1.1. Tracks 2–7 MUST NOT begin until Track 1's public API is
merged and frozen (Task 1.10). Within Track 1, tasks are strictly ordered.

NON-NEGOTIABLE OPERATING RULES (see the limitations doc for the why):
  - vox-terminal-core has ZERO UI deps and does NOT reimplement the agent loop — it
    ADAPTS vox-orchestrator. Do not copy the loop/feedback/hopper/budget logic.
  - Clean-room only re: Warp: its core is AGPLv3. Read it as reference, NEVER vendor its
    code. Take the Alacritty grid model from `alacritty_terminal` upstream, not from Warp.
  - Per-task verification gate (show the output, no "done" without evidence):
        cargo test -p <crate>
        cargo clippy -p <crate> -- -D warnings
        cargo run -p vox-arch-check
  - New crate ⇒ same-commit rows in crates/vox-arch-check/layers.toml AND
    docs/src/architecture/where-things-live.md.
  - cargo fmt --all is BANNED → cargo fmt -p <crate>. No new .ps1/.sh/.py (VoxScript only).
  - NEVER pipe cargo to head/grep on Windows (orphans processes) → redirect to a file.
  - Subagents are READ-ONLY in this sandbox: do not fan out writing subagents; the main
    session writes/verifies/commits. Parallelize Tracks 2–5 (after the freeze) only via
    worktrees the main session owns.

STOP-AND-ASK GATES (do not resolve these yourself):
  - G-LEGAL: is vox-term AGPL? (Default: no → clean-room.) 
  - G-SHELL: which underlying shell is "ideal for LLM generation"? (2–3 targeted fetches only.)
  - G-PRIV: the training-corpus consent/opt-in model.
  Pause and ask the owner when you reach each.

UNVERIFIED CAVEATS (read the named files before the affected task — see §14):
  - Exact feedback/HITL call signatures for Task 1.9 (read crates/vox-orchestrator/src/feedback/*).
  - GUI per-tab Session lifecycle mapping for Track 3 (read the Console surfaces).
  - MENS corpus schema for Track 5 (gated by G-PRIV).

Work in a dedicated git worktree off the latest main. Commit per task as the plan specifies.
After each task, report the verification output, then proceed to the next task.
```
