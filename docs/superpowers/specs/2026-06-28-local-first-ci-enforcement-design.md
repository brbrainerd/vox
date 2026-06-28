---
category: "CI & Quality"
title: "Local-First CI: Resilient Enforcement + Estate Tuning — Design"
date: 2026-06-28
status: design
---

# Local-First CI: Resilient Enforcement + Estate Tuning — Design

**Goal:** Move more CI onto the fast local self-hosted fleet **for speed and feedback
latency** (never cost), make the merge gate genuinely fleet-independent so doing so does
not break resilience, cut/re-tier redundant per-PR work, and only then turn the local-first
runner policy into an enforced gate.

> **Economic truth (anchors every decision):** vox is a **PUBLIC** repo — GitHub-hosted
> Actions minutes are **unlimited and free**. Cost is **never** a reason to prefer local.
> We optimize **per-PR wall-clock, merge-queue latency, and local feedback fidelity**, and
> keep jobs hosted **only** for neutral-infra resilience (the merge gate and the deploy
> critical path must survive the operator's workstation being off). This corrects the
> earlier draft's "prefer local throughout" framing, which contradicted the cited
> `compute-placement.md`.

## What changed from the first draft (and why)

An 11-agent adversarial audit (7 dimension auditors + 3 plan/spec skeptics + synthesis)
found the first draft was **broken**, not just incomplete. Three findings were verified by
hand against the live repo:

1. **Invariant 1 is FALSE in the YAML.** Branch protection requires exactly one context,
   `"Check, Build, and Test (Rust)"` (verified: `gh api .../branches/main/protection` →
   `["Check, Build, and Test (Rust)"]`). That context is the `ci-summary` job at
   `.github/workflows/ci.yml:1287`, `runs-on: [self-hosted, linux, x64]` (2-min aggregator).
   The `main-merge-queue` ruleset is **active**, so every merge serializes through a
   `merge_group` ci.yml run on the fleet. **If the workstation is off, the merge queue
   never drains and `ci-fallback-hosted.yml` cannot help — it is `workflow_dispatch`-only
   and not a required context.** The first draft cited Invariant 1 as the reason it was
   *safe* to register-not-migrate; in reality registration only silences the linter while
   cementing a broken invariant. **Tightening enforcement on top of this is the central
   defect.**

2. **The strict-flip footgun.** The first draft flipped `runner-policy-check` to `--strict`
   inside the **fast pre-push tier** — the exact path developers already bypass with
   `--no-verify` for the known stale-binary `graphify` SSOT false-positive. A hard gate
   there is bypassed precisely when a real hosted-runner regression appears. Worse,
   `ssot-drift` is *called by* the fast pre-push tier, so flipping both call sites runs the
   check twice and turns every `vox ci ssot-drift` caller into a hard gate as a side effect.

3. **Scope was far narrower than the ask.** The user wants test **cuts**, **speedups**,
   **per-PR→nightly tiering**, and **coverage-gap closure**. The first draft delivered only
   register + matrix + strict + docs — and the "matrix" was a bespoke `fromJson` job
   re-inventing the `full-ci` label idiom ci.yml already uses estate-wide (YAGNI), while
   silently dropping the *cheap* per-PR Win/macOS `cargo check` (the only per-PR
   `#[cfg(windows)]` compile proof — a real coverage regression).

**Audit claims that did NOT survive hand-verification** (folded in as *verify-then-act*,
never blind action): "distribution-parity is a pure duplicate — CUT it" (ci.yml's
full-workspace nextest runs **only on merge_group**, so distribution-parity gives unique
per-PR `voxup` signal → CUT *with a path-filtered replacement step*, not delete); "the GUI
vitest suite is in no workflow" (ci.yml runs Playwright + gui-honesty; the *unit* vitest
suite specifically is unconfirmed → verify before adding a lane).

## Decisions (ratified)

1. **Resilience precondition before enforcement.** The required gate must become
   fleet-independent and the fallback reachable **before** `--strict` lands.
2. **Strict only on the CI side.** Flip `runner-policy-check --strict` in the `ssot-drift`
   gate (which CI runs); keep the fast pre-push step **advisory-but-loud**. No `--no-verify`
   path silently bypasses the real gate.
3. **Speed/feedback, not cost.** Every migration and demotion is justified by wall-clock and
   feedback latency. No commit message or doc may cite "minutes" or "cost."
4. **Reuse the `full-ci` label idiom**, not a new dynamic-matrix mechanism, for per-PR
   tiering. Keep cheap per-PR `cargo check` for Win/macOS; defer only the expensive legs.
5. **Verify-then-act for every destructive cut.** No check is deleted or demoted without a
   step proving its signal is preserved elsewhere (or consciously, documented, dropped).

## Workstreams

Sequenced for max feedback-speed gain first; **B and F are the safety-critical core**, the
rest are independent improvements that can land incrementally.

### A — Register hosted exceptions (cosmetic; unblocks the linter)
Add `github-hosted-exceptions.md` rows for the deliberately-hosted workflows still missing
them (`version-tag-guard.yml`, `workflow-lint.yml`, and the 6 from the first pass —
`codeql.yml` already landed this session). Update the registry's own **Enforcement** note
(line ~40) from "default advisory" to "ENFORCED (`--strict`) in ssot-drift + CI." Re-run
`runner-policy-check --strict` against the **full** estate and record the exit code — do not
assume only 6 trip it.

### B — Resiliency precondition (NEW; hard-blocks F)
1. Make the required context fleet-independent: move the trivial `ci-summary` aggregator
   (`ci.yml:1287`, 2-min) to `runs-on: ubuntu-latest`; its 5 heavy `needs` stay self-hosted.
2. Make `ci-fallback-hosted.yml` a real safety valve: add a nightly `schedule:` and a
   `pull_request` trigger gated by an `if:` `fleet-down` label, and give its gate job the
   **same `name:`** as the required context so it can satisfy branch protection during an
   outage.
3. Add a `runner-policy-check` rule (+ unit test): a `merge_group`-only or
   conditionally-skipped job must **never** be a branch-protection required context (prevents
   the permanently-"expected" queue-deadlock class).
4. Document a **merge-queue break-glass runbook** (how to relax the `main-merge-queue`
   ruleset during a fleet outage; the admin bypass does NOT apply inside a required queue).

### C — Per-PR → merge_group/nightly tiering (reuse `full-ci` label idiom)
Precondition: `gh api .../branches/main/protection` confirms no Win/macOS context is required.
- `cross-platform-check.yml`: **keep** per-PR `cargo check --workspace --exclude vox-gui
  --target <win/mac>`; move only full clippy + full nextest legs to merge_group + weekly
  schedule. Drop the redundant `push:main` trigger. Remove the per-merge `os_compat.py`
  step (keep weekly `os-compat-report.yml`).
- `gui-cross-build.yml`: keep one fast Linux GUI compile per-PR; Win/macOS Tauri legs →
  merge_group. (Step 0: read the file; it may be N discrete jobs — branch the impl.)
- `compile-matrix.yml`: CUT `compile-help-windows`/`-macos` (their `vox compile --help`
  smoke is subsumed by cross-platform-check's per-PR `cargo check --workspace`); keep the
  Linux native-binary/Tauri smoke. (No `merge_group` trigger needed → avoids the deadlock
  class entirely.)
- `codeql.yml`: drop `pull_request`; run on `merge_group` + the existing weekly cron.
- `mobile-e2e-android.yml`: drop `pull_request` → merge_group + nightly (mirror the iOS
  sibling).

### D — Test cuts / merges (verify-then-act)
- `distribution-parity.yml`: **verify** ci.yml's merge_group `nextest --workspace` covers
  `voxup`'s `distribution_parity` test, then CUT the standalone workflow **and** add a
  path-filtered `-p voxup --test distribution_parity` step to ci.yml's per-PR `tests` job so
  PR-time signal survives on the fleet.
- Relocate `continue-on-error` advisory scans (crate-build-audit, plugin-candidacy,
  build-bench, graphify-freshness, cargo-outdated) out of ci.yml's merge-gate `tests` job
  into `bench-nightly.yml`.

### E — Speedups
- Add `sccache --show-stats` (`if: always()`) to setup/lints/compiler-gates/tests/audits —
  there is **zero** sccache hit-rate telemetry today and a documented history of silent
  sccache failure. (Cheapest change; proves the whole local-first speed thesis.)
- Split `guards-fast`: keep deterministic fast guards blocking; move cargo-deny/audit/shear
  + `.vox` audits + plugin-abi-parity `--build` to a parallel non-required `guards-slow`.
- Fix or delete `all-features-matrix` (24 per-crate target caches vs one shared target +
  sccache; workspace `--all-features` already runs in `audits`).

### F — Strict flip (was the whole point; now LAST and CI-only)
Flip `runner-policy-check --strict` in the `ssot-drift` gate (`docs.rs:613`,
`run(root, false)` → `run(root, true)?`). **Do NOT** flip `pre_push.rs:1084` — keep it
advisory-but-loud. Hard preconditions: Workstream B landed + branch-protection verified
fleet-independent. Ship a Rust unit test for both strict directions. Verify via the real
path: `cargo run -p vox-cli -- ci ssot-drift` exits 0.

### G — Feedback-loop / coverage backfill
- `vox ci env-doctor`: probe the same system deps `ci.yml:105` installs
  (libdbus/glib/gtk/webkit/soup/javascriptcoregtk) from one SSOT list; run first in
  pre-push. Closes the libdbus/GTK env-only-failure class we hit repeatedly this session.
- Per-PR GUI lane (verify-then-act): if the unit `vitest` suite is in no workflow, add
  `vitest run` + `tsc --noEmit` on PRs touching `crates/vox-gui/ui/**`.
- Derive `ACT_WORKFLOWS` from registered ubuntu-eligible exception rows (not the hardcoded
  3); gate Workstream-F verification on it or honestly downgrade that check to advisory.

## Resiliency guardrails (every migration MUST satisfy)

1. The required status context is fleet-independent (hosted aggregator OR a second
   always-hosted required check).
2. `ci-fallback-hosted` is required-EQUIVALENT and auto-reachable (nightly schedule +
   fleet-down-label trigger + matching job name). An exception row is NOT sufficient.
3. merge_group-only / conditionally-skipped jobs are NEVER required contexts (enforced).
4. Every per-PR→merge_group demotion pairs with a weekly hosted `schedule:`
   belt-and-suspenders.
5. A merge-queue break-glass runbook exists before `--strict`.
6. Nightly hosted mirror of the core Rust build/test gate so a multi-day outage still
   yields a recent portable green signal on main.
7. Nothing GPU/bench/mutation moves onto merge_group (keep the queue light).

## Sequencing

`B (resiliency) → C+D (latency cuts) → E (speedups) → G (feedback) → A+F (register + strict
flip, last)`. B unblocks F; everything else is independent and incremental.

## Out of scope

- Self-hosted Windows/macOS runners (no spare-hardware decision).
- Installing the docker-compose plugin on the self-hosted docker runner (registered
  exception instead).
- Rewriting the autoscaler or `--act` engine (G only widens `--act` coverage).
