---
title: "VoxMens Split — Plan C: Selection + Routing + Serving"
description: "Third of three plans split from the VoxMens hub-and-spoke build-out. Lands the VRAM-scaled model-registry + resolver (master Phase 3), per-spoke training method dispatch beyond QLoRA (Phase 6), the lane-tag inference router (Phase 7), and the serving-topology decision + end-to-end validation (Phase 8). Depends on Plans A and B."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# VoxMens Split — Plan C: Selection + Routing + Serving

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** Atomic-green-commit, verify-before-use, two-strike circuit breaker.

**This is plan 3 of 3.** Prereq: **Plans A and B fully landed.** Master: [`2026-06-18-voxmens-hub-and-spoke-buildout.md`](2026-06-18-voxmens-hub-and-spoke-buildout.md).

**Goal:** Select per-spoke base models scaled to host VRAM, dispatch per-spoke training methods, route inference by lane tag, decide serving topology, and validate end-to-end.

**Scope = master Phases 3, 6, 7, 8.** Execute those phase bodies from the master plan, with the audit corrections below.

## Prereq gate (run first)
- [ ] Confirm Plan B's boundary: `eval-gates-rust.yaml`/`eval-gates-agents.yaml` exist; `vox ci spoke-check` exits 0 with no pending warnings; Rust/agentic corpora present. If not, STOP.

## Audit corrections that apply to Plan C
- **`detect_available_vram_mb()` (Phase 3 / Task 3.4) must be confirmed to exist before being called.** `rg -n "vram|detect_available_vram|gpu.*mb|-> Option<u32>" crates/vox-ml-cli/src crates/vox-populi/src`. If no concrete `fn ... -> Option<u32>` is reachable from `pipeline.rs`, the task STOPs with a handoff note — do NOT write a call to a hallucinated helper. Inline the real fn name/signature once found.
- **Per-spoke method dispatch (Phase 6) consumes the `profile` seam from Plan A** and the per-spoke `base.method` from the SSOT (Plan A). Pass `Some("<spoke>")`; do not re-thread the seam.
- **Rust spoke id = `rust-expert`** in resolver, router triggers, and e2e dry-runs.
- **Router `signal.contains(needle)` with `*.rs`/`*.vox` needles is a substring match** — it will also match `foo.rsync` etc. Acceptable for v1, but note the false-positive risk in the task (or anchor on a suffix check).
- **Phase 8 e2e dry-run (`--profile <spoke>`) requires the CLI surface `--profile` flag**, not just the internal `pipeline::run` param. Add/verify the `--profile` clap arg on the train/run command (ties back to Plan A's seam being exposed at the CLI).
- No-stub check via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed>`; no `vox stub-check`.

## Plan C green boundary (initiative complete)
- Model-registry + resolver pick the largest variant within detected VRAM; resolver is unit-tested with synthetic VRAM values.
- Per-spoke `method` flows through training; non-QLoRA methods dispatch correctly (or fail closed with a clear error).
- Lane-tag router resolves a signal → spoke deterministically; tested.
- Serving-topology decision committed; `vox ci spoke-check` + full `cargo test` green; e2e `--profile <spoke>` dry-run succeeds for all three spokes.

## Execution Handoff
This completes the VoxMens hub-and-spoke initiative across Plans A→B→C.
