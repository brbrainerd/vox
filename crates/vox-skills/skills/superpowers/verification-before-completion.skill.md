---
name: verification-before-completion
description: Use when about to claim work is complete, fixed, or passing, before committing or opening a PR - requires running verification commands and pasting the output before any success claim.
---

# Verification Before Completion (Vox Adaptation)

## Overview

Evidence before assertions, always. Never say "done", "fixed", or "passing" without command output proving it.

**Announce at start:** "I'm using the verification-before-completion skill."

## The Ritual (run in order, paste real output)

1. `cargo test -p <crate>` — must show PASS counts (and the specific new test names).
2. `cargo clippy -p <crate> -- -D warnings` — must be clean.
3. `vox stub-check` (or `cargo run -p vox-arch-check` where relevant) — TOESTUB / architecture compliance.
4. `cargo fmt -p <crate>` — **never** `cargo fmt --all` (overflows the Windows arg limit).
5. `cargo check -p <crate>` — confirm the tree compiles.

If any step fails, the task is **not** done. Fix at the source; do not weaken the test to make it pass.

## Rules

- The claim of success and the evidence must appear together. No "should pass" — run it.
- A skipped step is a failed verification. State explicitly what you did not run.
- For a fast/low-reasoning executor: do not infer success from "the edit applied". Run the commands.

## Definition of Done

A task is done only when: tests green (with new tests proving the behavior), lints clean, stub/arch checks pass, formatted, committed. Anything less is in-progress.
