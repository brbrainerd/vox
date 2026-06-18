---
name: brainstorming
description: Use before any creative or design work (new feature, component, behavior change) and whenever a plan task leaves a design choice underspecified - explores intent and options before implementation.
---

# Brainstorming (Vox Adaptation)

## Overview

Turn an idea or an underspecified task into a decided design before writing code. The output is a written decision, not a conversation.

**Announce at start:** "I'm using the brainstorming skill."

## Hard Gate

Do NOT write code, scaffold, or invoke an implementation skill until a design choice is written down and (when a human is in the loop) approved. This applies regardless of how simple the task looks. "Simple" tasks are where unexamined assumptions waste the most work.

## Process

1. **State the decision** in one sentence ("How should X relate to Y?").
2. **Explore context** — read the relevant files/docs/commits first; do not design against assumptions.
3. **Offer 2-3 concrete options**, each with a one-line trade-off and your recommendation first.
4. **Pick one** and record *why* — in the spec, a commit message, or a comment.
5. **Scope check** — if the request spans multiple independent subsystems, decompose into sub-projects first; brainstorm only the first.

## For fast/low-reasoning executors (e.g. Gemini 3.5 Flash)

- Never invent a "clever" fourth option that depends on APIs you have not verified exist (`rg` first).
- Prefer the option that reuses existing, verified code over a novel mechanism.
- If you cannot choose confidently, STOP and surface the options rather than guessing.

## Key Principles

- One question / one decision at a time. YAGNI ruthlessly. Always explore alternatives before settling.
- Persistence-vs-presentation, capture-vs-meaning, and other "does declaring X imply Y?" questions are design decisions — make them explicit, do not assume.

## Terminal State

The next skill after brainstorming is `writing-plans`. Do not jump to other implementation skills first.
