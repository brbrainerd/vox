# App Phase 6 — Release-readiness gate — Implementation Plan

**Goal:** Ship-ready checklist with evidence per gate, plus the release how-to.

**Why:** Final phase. Translates the work of phases 1/2/4/5 (and 3 once unblocked) into a documented release artifact a non-author can verify and ship.

**Architecture:** Pure documentation + `apps/vox-mental-tracker/scripts/release_gates.vox` (`vox run …`) for programmatic gates G1-G4.

**Tech Stack:** markdown, optionally a vox-ci subcommand.

---

## Tasks

- [ ] **A1.** `apps/vox-mental-tracker/docs/how-to/release.md`: per-gate checklist with linked evidence:
  - Gate G1 — All vitest tests green (link to last green CI run).
  - Gate G2 — All Playwright E2E green.
  - Gate G3 — `vox check` clean.
  - Gate G4 — Contracts schema-valid.
  - Gate G5 — Capacitor Android build succeeds (`pnpm build:android` or per `docs/how-to/build-android.md`).
  - Gate G6 — Capacitor iOS build succeeds (skip if no Apple toolchain available; document gap).
  - Gate G7 — Privacy doc reflects current data flows (`docs/user/privacy.md`).
  - Gate G8 — Failure-modes research is current (`docs/architecture/failure-modes-research-2026.md`).
- [ ] **A2.** `apps/vox-mental-tracker/scripts/release_gates.vox` runs each programmatic gate (G1-G4) and exits non-zero if any fail. Manual gates (G5-G8) are checklist-only.
- [ ] **A3.** Update `apps/vox-mental-tracker/README.md` with a "Releasing" section pointing at the how-to.
- [ ] **A4.** Add a `RELEASE_CHECKLIST.md` template under `apps/vox-mental-tracker/` for per-release tracking.

## Verification

- [ ] **B1.** Run `vox run apps/vox-mental-tracker/scripts/release_gates.vox` on the current tip; record output.
- [ ] **B2.** Walk the manual gates and tick boxes in a sample `RELEASE_CHECKLIST.md`.
