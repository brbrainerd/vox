---
name: requesting-code-review
description: Use when completing a task or feature, before committing or merging - a self-review (and optional peer/automated review) pass against requirements and house rules.
---

# Requesting Code Review (Vox Adaptation)

## Overview

A focused review pass before a change is committed. For an autonomous executor this is a disciplined self-review; with a reviewer available, it is the request you make.

**Announce at start:** "I'm using the requesting-code-review skill."

## Self-Review Checklist (run before every commit)

1. **Existence (anti-hallucination):** Does every symbol, type, path, and API I referenced actually exist? Re-`rg` anything you are not certain of. A fast model invents plausible APIs — verify them.
2. **No stubs:** Did I add any stub, placeholder, TODO, or hollow function? Forbidden (see `AGENTS.md` / no-stubs policy). Scope down to a smaller real artifact instead.
3. **Scope fidelity:** Does the change match the task's stated Files block exactly? No unrelated edits, no scope creep.
4. **DRY:** Did I duplicate logic that already exists? Reuse it instead.
5. **House rules:** VoxScript-first automation; no `cargo fmt --all`; `docs/src/` `.md` has frontmatter with required keys `title`, `description`, `category`, `status`, `training_eligible`; TOESTUB limits respected.
6. **Tests prove behavior:** Is there a test that actually exercises the new behavior (not just compiles)?

Fix issues inline; do not expand scope. If a finding is large, surface it rather than silently fixing.

## Receiving feedback

Treat review feedback (human or automated) with technical rigor: verify each point before acting; push back with evidence where a suggestion is wrong; do not perform agreement. Implement only what survives scrutiny.
