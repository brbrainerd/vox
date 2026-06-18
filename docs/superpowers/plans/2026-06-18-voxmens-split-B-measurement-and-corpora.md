---
title: "VoxMens Split — Plan B: Measurement + Corpora"
description: "Second of three plans split from the VoxMens hub-and-spoke build-out. Builds the per-spoke eval-metric producers + check_run handlers + gate YAMLs (master Phase 2), the Rust authoring corpus (Phase 4), and the agentic corpus (Phase 5). Depends on Plan A's spoke SSOT + profile seam. No model selection or serving."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# VoxMens Split — Plan B: Measurement + Corpora

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** Atomic-green-commit, verify-before-use, two-strike circuit breaker.

**This is plan 2 of 3.** Prereq: **Plan A is fully landed** (`2026-06-18-voxmens-split-A-spoke-ssot-and-seam.md`) — the spoke SSOT exists and `pipeline::run` has the `profile` seam. Master: [`2026-06-18-voxmens-hub-and-spoke-buildout.md`](2026-06-18-voxmens-hub-and-spoke-buildout.md).

**Goal:** Make the spokes *measurable and fed* — land the eval producers→handlers→gate YAMLs and build the Rust and agentic corpora.

**Scope = master Phases 2, 4, 5.** Execute those phase bodies from the master plan, with the audit corrections below.

## Prereq gate (run first)
- [ ] Confirm Plan A's boundary holds: `cargo run -p vox-cli -- ci spoke-check` exits 0; `rg -n "profile: Option<String>|profile," crates/vox-ml-cli/src` shows the seam exists; `rg -n "^  rust-expert:" -A6 mens/config/domain-profiles.yaml` shows `base`/`eval_gate`/`router`. If not, STOP — Plan A is not done.

## Audit corrections that apply to Plan B
- **Producer→consumer artifact is fine as written:** `check_run` ALREADY reads `eval_results.json` (`check_run.rs:197`, parsed once and shared). Keep the convention "producers write metrics into `eval_results.json`; handlers read from `eval_results.json`." The real requirement is that each NEW gate name (`rust_compile_rate`, `clippy_clean_rate`, `tool_call_valid_json_rate`, …) gets a **handler arm** in `check_run` — a gate name with no handler is silently ignored. Mirror the `eval_json` access at `check_run.rs:197-213`.
- **`supervised_ratio.min_pct` is unit-sensitive** — it is a FRACTION (`0.10`), NOT `10.0` (which demands 1000% and always fails). Before writing any gate YAML, `rg -n "supervised_ratio" -A2 mens/config/eval-gates.yaml` and copy the reference gate's exact numeric/unit. Dry-run `check_run` against a known-good run to confirm the gate does not block spuriously.
- **Task 4.2 — never commit an `unimplemented!()` body across a task boundary.** The real `compile_batch_in_workspace` body and its commit are ONE atomic unit; keep the `#[ignore]` integration test separate, but the function must be real before any commit. (Verify `compile_batch_in_workspace`'s real signature with `rg` before calling it.)
- **Rust spoke id = `rust-expert`** in every `load_domain_profile(...)` call and gate-file name mapping.
- No-stub check via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed>`; no `vox stub-check` (use `vox ci toestub-budget` for final only).
- **Phase 2 wiring tasks (2.3 etc.) consume the `profile` seam landed in Plan A** — pass `Some("<spoke>")` now; do not re-add the seam.

## Plan B green boundary (before Plan C)
- `mens/config/eval-gates-rust.yaml` + `eval-gates-agents.yaml` exist; `vox ci spoke-check` now treats them as present (no pending warning).
- New gate names have `check_run` handler arms (no silently-ignored gates); a dry `check_run` against a fixture run produces the expected GateResults.
- Rust + agentic corpora generate deterministically with their verifiers; `cargo test -p vox-corpus -p vox-ml-cli` green.
- `cargo run -p vox-arch-check` exits 0.

## Execution Handoff
On completion, proceed to Plan C (`2026-06-18-voxmens-split-C-selection-routing-serving.md`).
</content>
