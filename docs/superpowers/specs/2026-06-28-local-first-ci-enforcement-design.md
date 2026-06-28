---
category: "CI & Quality"
title: "Local-First CI Enforcement — Design"
date: 2026-06-28
status: design
---

# Local-First CI Enforcement — Design

**Goal:** Make the existing (advisory) local-first runner policy real: prefer the
self-hosted Linux Docker fleet throughout CI/CD, run CI locally first, and turn the
already-built `runner-policy-check` from a warning into a hard gate — without
pretending Windows/macOS can run in a Linux Docker fleet.

## Premise (what already exists)

The local-first apparatus is already built and only needs tightening:

- `vox ci runner-policy-check` (`crates/vox-cli-ci/src/runner_policy_check.rs`) —
  scans `.github/workflows/*.yml`, warns on GitHub-hosted `runs-on` without a row in
  `docs/src/ci/github-hosted-exceptions.md`. `run(root, strict)`; default `strict=false`.
  Wired into `vox ci ssot-drift` and the fast `vox ci pre-push` tier (advisory).
- `vox ci pre-push --act` — mirrors hosted-runner workflows locally in Docker (nektos/act).
- `docs/src/ci/github-hosted-exceptions.md` — the exception registry.
- `ci-fallback-hosted.yml` — manual degraded-mode mirror when the fleet is down.
- `AGENTS.md` §"Run CI locally first" + `runner-contract.md` §"Local-first CI".

**Current drift:** `runner-policy-check` flags 6 workflows with no exception row:
`ci.yml`, `deploy-telemetry.yml`, `distribution-parity.yml`, `docker-telemetry.yml`,
`version-tag-guard.yml`, `workflow-lint.yml`.

## Hard technical constraint (shapes scope)

The self-hosted fleet is **Linux Docker on WSL2**. It **cannot** host Windows or macOS:
- Windows containers require a Windows Docker host (Hyper-V/process isolation).
- macOS cannot be containerized at all (Apple license + no runtime).

Therefore Win/macOS CI stays GitHub-hosted, but is moved **off per-PR** to
`merge_group` + weekly `schedule` only (decision: "Scheduled + merge-queue only").

## Decisions (ratified)

1. **Enforcement:** Hard gate — flip `runner-policy-check` to `--strict` in pre-push +
   ssot-drift after the exception registry is complete.
2. **Win/macOS:** Keep hosted, run only on `merge_group` + nightly `schedule`, not per-PR.
3. **The 6 flagged workflows are deliberately hosted, not drift.** A second discovery
   during planning: all 6 carry documented rationale and are covered by
   `docs/src/ci/compute-placement.md` (deploy critical path on free public minutes;
   Invariant 1 = "the merge gate never hard-depends on the workstation"; Invariant 4 =
   "the self-hosted fleet is never on the path between a green main and a live deploy").
   **Decision: honor the policy — register all 6 as exceptions, do NOT migrate.** This
   keeps fleet-outage resilience intact while making local-first *enforced* for everything
   new.

## Workstreams

### A — Register the 6 deliberately-hosted workflows as exceptions
Add rows to `github-hosted-exceptions.md` (no `runs-on` edits). Each row cites its
`compute-placement.md` rationale. Result: `runner-policy-check` exits clean.

| Workflow | Runner | Documented reason |
|----------|--------|-------------------|
| `deploy-telemetry.yml` | `ubuntu-latest` | Coolify deploy critical path; free public minutes; Invariant 4 |
| `docker-telemetry.yml` | `ubuntu-latest` | GHCR image build on deploy path; free public minutes |
| `distribution-parity.yml` | `ubuntu-latest` | Fleet-independent required parity check (Invariant 1) |
| `version-tag-guard.yml` | `ubuntu-latest` | Lightweight tag-only release guard; fleet-independent |
| `workflow-lint.yml` | `ubuntu-latest` | actionlint/zizmor; install in seconds, no fleet resources |
| `ci.yml` | `ubuntu-latest` (1 job) | `docker compose config` parse; self-hosted docker runner lacks compose plugin |

### B — (folded into A)
Registration is now the whole of A; there is no separate migration step.

### C — Win/macOS off per-PR
Add a tiny `matrix-setup` job that emits the matrix `include` JSON, omitting Win/macOS
legs when `github.event_name == 'pull_request'`. Apply to `cross-platform-check.yml`,
`gui-cross-build.yml`, `compile-matrix.yml`. Linux self-hosted leg stays per-PR;
Win/macOS run on `merge_group` + `schedule` only. (`setup-e2e.yml` already nightly-only.)

### D — Flip enforcement to strict
`pre_push.rs:1084` `run(root, false)` → `run(root, true)`; mirror in the ssot-drift
inclusion. TDD: a unit test asserting strict mode returns `Err` on an unregistered
hosted workflow, and the migrated tree returns `Ok`.

### E — Update the rules
- `runner-contract.md` §Local-first: "advisory enforcement" → "enforced (strict)".
- `github-hosted-exceptions.md`: rows added in A; document the Win/macOS "not per-PR"
  policy and cross-link `compute-placement.md` as the placement SSOT.
- `AGENTS.md` §"Run CI locally first": strengthen to reflect the hard gate.

### F — Confirm "runs locally first" works
Verify `vox ci pre-push --act` mirrors the hosted lanes locally. This is the gate before
D/E land.

## Sequencing

`A + B + C` (parallel-safe, independent files) → **verify** (`runner-policy-check` clean +
`--act` smoke) → `D` (strict flip, depends on clean check) → `E` (docs, depends on D).

## Execution model (sub-agents + workflows)

- **A, B, C** are file-isolated and independent → run as a **Workflow** `parallel()`
  fan-out: one agent per workflow file (migrate or register), each returning a structured
  edit summary. Each agent uses `isolation: 'worktree'` only if they touch shared files
  (they don't — distinct workflow files), so plain parallel agents suffice.
- **Verification** is a single agent: runs `runner-policy-check` + an `--act` smoke,
  returns pass/fail. Barrier after A/B/C.
- **D** (strict flip + test) is one TDD agent, gated on verification passing.
- **E** (docs) is one agent, gated on D.
- A final **review** agent (superpowers:code-reviewer) audits the whole diff before push.

The Workflow uses `pipeline()` for the A/B/C→verify→D→E dependency chain where each
stage gates the next, with the A/B/C migration fanned out via `parallel()` inside stage 1.

## Testing

- D ships a Rust unit test (strict-mode Err on unregistered hosted, Ok on clean tree).
- Verification agent proves `runner-policy-check` exits 0 and `--act` runs a hosted lane.
- Final review agent confirms no workflow lost coverage (every migrated job still triggers
  on the same events, minus the deliberate Win/macOS per-PR removal).

## Out of scope

- Installing a Windows/macOS self-hosted runner fleet (no spare hardware decision).
- Installing the docker-compose plugin on the self-hosted docker runner (registered as
  an exception instead).
- Rewriting the runner autoscaler or the `--act` mechanism.
