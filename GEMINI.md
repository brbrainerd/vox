---
title: "Antigravity System Prompt — Gemini 3.5 Flash on this repo"
description: "Antigravity-specific behavior rules for this repo. Distilled from 3 months of Gemini 3.5 Flash failures inside Antigravity. Concrete, positive, executable."
category: "contributor"
status: "current"
training_eligible: true
training_rationale: "Defines Antigravity-specific rules and shell environment expectations — execution constraints unique to Gemini 3.5 Flash running agentically in Antigravity IDE."
---
# Antigravity System Prompt — Gemini 3.5 Flash

This file is Antigravity-specific. `AGENTS.md` is the cross-tool base policy; this file narrows and extends it for Gemini 3.5 Flash executing implementation plans inside Antigravity IDE.

Read [`AGENTS.md`](AGENTS.md) first. When they conflict, this file wins.

See [agent-instruction-architecture.md](docs/src/contributors/agent-instruction-architecture.md) for the instruction layering model.

---

## The Three Laws (non-negotiable, read before anything else)

**1. Every task ends committed and green.**
Run the gate specified in the plan before committing. A plan killed between tasks must leave a tree that compiles and all existing tests pass. If a change would break the build, split it into a preparatory commit (types/stubs, compile-clean) followed by the wiring commit.

**2. Verify before use.**
Before writing any code that calls a function, imports a module, or references a path — run `rg '<SymbolName>' crates/` and confirm the exact signature and import path exist in THIS repo. The plan may name the right concept but the wrong identifier. `rg` is the source of truth.

**3. Prove the effect, not the shape.**
A test that asserts a substring (`contains("fn parse_config")`) or a candidate ordering is hollow green. The acceptance test must exercise the boundary: compile the output, call the function, dispatch the model. If the plan's gate is `cargo test -p vox-config`, run exactly that command with no flags added or removed.

---

## Atomic Task Execution

Each task in the plan is designed to be atomic:

- Complete all edits for that task, then run the specified gate, then commit.
- Commit message: one imperative line matching the task title.
- If the gate is red, fix it within that task before moving to the next — never commit a non-green tree.
- If you cannot make the gate green after one correction, STOP, report what failed, and hand back. Do not loop.

### Self-contained context

Each task repeats the context you need. Do not rely on remembering what you did in earlier tasks. If a task says "the struct defined in Task 3", re-read Task 3. Gemini 3.5 Flash has weaker long-context recall than Pro; the plans are shaped to work around this.

---

## Parallel vs. Sequential Tasks

Each task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`.

- `[PARALLEL-SAFE]` tasks write disjoint files and may run concurrently.
- `[SEQUENTIAL]` tasks share a file or depend on a prior commit. Run them one at a time in order.
- When in doubt, treat as `[SEQUENTIAL]`.

---

## Verification Gates — Run Exactly As Written

Gates are written to be precise. Run them exactly as specified.

**Correct:** `cargo test -p vox-config`
**Correct:** `cargo build -p vox-config`

The gate string in the plan is the contract. Passing a narrower or weaker gate — `--warn-only`, `|| true`, `--no-verify`, filtering to a single test function when the gate specifies `--lib` — does not satisfy it.

If the gate is red at baseline for reasons unrelated to your task, STOP and report. Do not relabel `layers.toml`, add `orphan_exempt`, flip `publishable`, or edit shared architecture config to clear someone else's pre-existing red.

---

## Symbol Verification — the Pre-flight Habit

For any symbol you plan to reference (function, struct, trait, module path), run:

```
rg 'SymbolName' crates/
```

Read the result. Use the exact identifier and the exact import path from the search result.

When the plan says "confirmed present" and gives a path, verify once at the start of that task. When the plan gives a function signature, use it verbatim.

The most common cause of gate failure in this repo is calling a function with the right concept name but the wrong identifier or the wrong crate path.

---

## Shared Architecture Files — Touch Only What the Plan Assigns

The plan lists every file you should touch. If fixing your task would require editing:

- `layers.toml`
- `contracts/mcp/tool-registry.canonical.yaml`
- `contracts/operations/catalog.v1.yaml`
- `AGENTS.md`, `GEMINI.md`
- Any `*-registry.yaml`, `*-policy.yaml`, or `*.schema.json` under `contracts/`

...and the plan does not explicitly assign that file to your task — STOP and report it. Do not make the edit yourself to clear a gate. These files are SSOTs; unplanned edits cause drift that outlasts your session.

---

## Branch and Delivery

- Work on a branch named `agy/<slug>` (the worktree is already set up for you).
- Include ONLY this plan's commits on the branch. Do not accumulate unrelated changes.
- Before reporting completion, list every file you changed — including any shared config. Do not omit files.

---

## Performance-Sensitive Code

When the plan names a hot path (inner loop, per-item scorer, minhash shingle), read the existing implementation before writing. Match the idiom. Avoid per-call allocations in the hot path; reuse buffers where the plan shows a pattern.

---

## Shell Environment

Windows workspace. PowerShell (`pwsh`) is canonical.

- One command per step. Do not chain with `&&`, `|`, `;`, `||` unless the plan explicitly requires it.
- Use project tools: `cargo`, `vox`, `pnpm`, `rg`, `git`.
- For text search: `rg`. Never run a recursive search from the workspace root; narrow to a crate subdirectory.
- For file ops in scripts: `vox run --mode interp <script.vox>`.
- For package management: `pnpm` for JS/TS.

---

## VoxScript-First Glue

New automation is `.vox` only. Do not create `.ps1`, `.sh`, or `.py` glue scripts. If the plan says "run X" and X is a VoxScript, run it with `vox run --mode interp X` or `vox run --mode native X` as appropriate.

---

## Tests That Prove Behavior

Write the smallest test that exercises the observable effect:

- For a function that returns a value: assert the return value equals the expected value.
- For a Tauri command: call it through the registered handler and assert the JSON response.
- For codegen: run the generated output through the compiler (`cargo build` or `tsc --noEmit`).
- For a registry: look up the registered name and assert it dispatches.

A test asserting that generated code contains a string (`assert!(output.contains("fn foo"))`) does not prove the code compiles or runs. Add a compilation step.

---

## Two-Strike Rule

If a gate fails after your first correction attempt:

1. Report exactly which gate failed and what the error was.
2. Report what you tried.
3. STOP. Do not attempt a third correction.

Hand back to the plan author with the above report. The plan author will correct the launch statement and re-delegate.

---

## Agent Skills

Antigravity mounts repo skills from `.agents/skills/`, which mirrors `crates/vox-skills/skills/superpowers/`. If `.agents/` is not populated (Windows symlink miss), run:

```
vox run --mode interp scripts/sync-superpowers-skills.vox -- --write
```

When executing an implementation plan from `docs/superpowers/plans/`, honor each task's `[PARALLEL-SAFE]` / `[SEQUENTIAL]` tag.

Reference docs (read before executing unfamiliar plan shapes):
- Execution model: [`docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md)
- Why plans are atomic+verify-before-use: [`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md)

---

## Quick Reference Card

| Situation | Action |
|---|---|
| About to call `foo()` | Run `rg 'fn foo' crates/` first; use the exact path from the result |
| Gate is red after my first fix | STOP — report the error; do not attempt a third correction |
| Gate is red at baseline (pre-existing) | STOP — report; do NOT edit `layers.toml` or shared config to clear it |
| Task says `[SEQUENTIAL]` | Run it after the previous task's commit is green |
| Plan says to edit `layers.toml` | Check the task assignment — if the plan doesn't assign it to you, stop |
| Need to write a test | Assert the return value or compile the output; avoid substring asserts |
| My commit message | One imperative line matching the task title; no "also" or "and" |

See [`AGENTS.md`](AGENTS.md) for the instruction-layer model and cross-tool policy.
