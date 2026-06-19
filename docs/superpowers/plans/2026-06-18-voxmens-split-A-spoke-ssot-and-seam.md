---
title: "VoxMens Split — Plan A: Spoke SSOT + Pipeline Seam (foundation)"
description: "First of three independently-shippable plans split out of the VoxMens hub-and-spoke build-out. Lands the spoke SSOT (domain-profiles base/eval_gate/router on the existing vox-lang/rust-expert/agents profiles), the typed corpus record, the strict mix, the `profile: Option<String>` pipeline seam, and the `vox ci spoke-check` validator. No GPU. Fully green and testable on its own; Plans B and C depend on this."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# VoxMens Split — Plan A: Spoke SSOT + Pipeline Seam

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** Atomic-green-commit, verify-before-use, two-strike circuit breaker. See `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md` §5.

**This is plan 1 of 3.** The master plan [`2026-06-18-voxmens-hub-and-spoke-buildout.md`](2026-06-18-voxmens-hub-and-spoke-buildout.md) was split (audit 2026-06-18: too large for one Antigravity run). Plans: **A (this) → B (corpora) → C (selection/routing/serving)**.

**Goal:** Land the hub-and-spoke *foundation* — the validated spoke SSOT, the typed tool-trace record, the strict mix runner, the `profile` seam every later wiring task consumes, and the `vox ci spoke-check` gate — all green, no GPU.

**Scope = master Phases 0–1 + two NEW tasks (A-SEAM, A-SPOKECHECK) spelled out below.** Execute the master's Phase 0 and Phase 1 task bodies (Tasks 1.1–1.6), with these audit corrections, THEN A-SEAM and A-SPOKECHECK.

## Audit corrections that apply to Plan A (from the master's correction block)
- **Task 1.4 (the spoke validator) moves OUT of layer-0 `vox-arch-check` into `vox ci spoke-check`** — implemented as task **A-SPOKECHECK** below (the master left it prose-only).
- **Task 1.5 edits the EXISTING `vox-lang`, `rust-expert`, and `agents` profiles in place** — it does NOT create a new `rust` profile (would fork Rust) or replace the `agents` block (would clobber its curriculum). Rust spoke id = **`rust-expert`** everywhere. (Verified: `rust-expert` and `agents` already exist in `mens/config/domain-profiles.yaml`; `mix-rust.yaml`/`mix-agents.yaml` already exist.)
- **The `profile: Option<String>` seam is task A-SEAM** — land it (own atomic, green commit) BEFORE master Task 1.6, because 1.6/2.3/3.4/6.1 all read it.
- No-stub check via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed>` (there is no `vox stub-check`; the gate is the heavy `vox ci toestub-budget`).

---

## Task A-SEAM `[SEQUENTIAL]`: Thread `profile: Option<String>` end-to-end through the pipeline

The master's Tasks 1.6 / 2.3 / 3.4 / 6.1 all read `profile.as_deref()` inside the `PipelineStage` loop, but `pipeline::run` has no `profile` parameter and no task lands it. This task adds the seam and commits it green so every downstream consumer compiles.

**Files:** Modify `crates/vox-ml-cli/src/.../pipeline.rs` (the `run` fn + `PipelineStage` loop) and the CLI caller of `pipeline::run`.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub fn run\(|PipelineStage::|fn run_pipeline|pipeline::run\(" crates/vox-ml-cli/src` — confirm `pipeline::run`'s real signature (the master cites `pipeline.rs:7`, 11 params, none a profile) and find every call site. Note the exact module path.

- [ ] **Step 2: Failing test.** Add a test asserting `run` accepts a `profile: Option<String>` and that when `Some("rust-expert")` is passed, the value is observable at the stage loop (e.g. via a returned/threaded field, or a `tracing` line you can assert through a captured log, or simplest: a thin pure helper `selected_profile(profile: &Option<String>) -> &str` returning `"default"` when `None` — test that). Keep it pure/observable so a weak model can verify without running a real pipeline.

- [ ] **Step 3: Implement.** Add `profile: Option<String>` as the LAST parameter of `pipeline::run` (last position avoids reshuffling existing positional args). Thread it into the `PipelineStage` loop closure capture. Update EVERY call site found in Step 1 to pass `None` for now (later plans pass `Some(spoke)`). Do not change behavior when `None`.

- [ ] **Step 4: Run → green.** `cargo test -p vox-ml-cli` (the seam test passes; existing tests unaffected since callers pass `None`). `cargo build -p vox-ml-cli`.

- [ ] **Step 5: Commit.**

```bash
git add crates/vox-ml-cli/src
git commit -m "feat(mens): thread profile: Option<String> seam through pipeline::run (no behavior change)"
```

---

## Task A-SPOKECHECK `[SEQUENTIAL]`: `vox ci spoke-check` validator command

The master's Task 1.4 validator must NOT live in layer-0 `vox-arch-check` (it would pull `mens/config` parsing into the arch gate). Implement it as a `vox ci spoke-check` subcommand instead.

**Files:** Modify `crates/vox-ml-cli/src/.../ci` (or wherever `vox ci` subcommands are defined — confirm in Step 1); add the validator module.

- [ ] **Step 1 (verify-before-use):** `rg -n "enum CiCommand|enum.*Ci.*Command|=> run_|match.*ci|affected-crates|bom-check" crates/vox-ml-cli/src crates/vox-cli/src/commands/ci | head -30` — find the `vox ci` subcommand enum + its dispatch `match`, and mirror an existing simple gate (e.g. `bom-check`, which was recently added) for the clap variant + dispatch arm + pre-push registration shape.

- [ ] **Step 2: Failing test.** Add a test that loads `mens/config/domain-profiles.yaml`, and for every profile asserts: (a) `base.model`, `base.method`, `base.preset` present; (b) `eval_gate` path exists on disk OR is in a known-pending allowlist; (c) `router.triggers` non-empty. Seed a fixture with one bad profile and assert the validator returns a non-empty `Vec<Violation>`.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-ml-cli spoke_check`.

- [ ] **Step 4: Implement.** Add `SpokeCheck` to the `vox ci` subcommand enum + a dispatch arm calling `run_spoke_check()`, which loads `domain-profiles.yaml` (reuse `load_domain_profile`/`DomainProfilesFile::load` from master Task 1.3), validates each profile, prints violations, and exits non-zero if any. Register it in the pre-push/`guards-fast` gate list (mirror `bom-check`). **Until Plan B creates `eval-gates-rust.yaml`/`eval-gates-agents.yaml`, those two paths are "known-pending"** — accept them with a warning, not a hard fail (so Plan A stays green before Plan B lands the gate files).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-ml-cli spoke_check`; then `cargo run -p vox-cli -- ci spoke-check` (or the real binary) → exits 0 against the current config.

- [ ] **Step 6: Verify + commit.** clippy/fmt; no-stub `rg`; then:

```bash
git add crates/vox-ml-cli/src
git commit -m "feat(ci): vox ci spoke-check validates the domain-profiles spoke SSOT"
```

---

## Plan A green boundary (must hold before starting Plan B)
- `mens/config/domain-profiles.yaml`: `vox-lang`, `rust-expert`, `agents` each carry `base`/`eval_gate`/`router` (edited in place; no `rust` fork; `agents` curriculum preserved).
- The typed tool-trace record + strict mix (master Phase 1) compile and are tested.
- `pipeline::run` accepts `profile: Option<String>`; all callers pass `None`; tree green.
- `vox ci spoke-check` exists, is registered, and exits 0 (pending eval-gate files warn, not fail).
- `cargo run -p vox-arch-check` exits 0; full `cargo test -p vox-ml-cli -p vox-populi -p vox-corpus` green.

## Execution Handoff
On completion, proceed to Plan B (`2026-06-18-voxmens-split-B-measurement-and-corpora.md`). Run order overall: **A → B → C**.
