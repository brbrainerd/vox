# Release how-to

Per-release ship checklist. Programmatic gates G1-G4 are validated by `vox run apps/vox-mental-tracker/scripts/release_gates.vox` (repository root); manual gates G5-G8 are walked by hand.

## Programmatic gates

### G1 — Vitest suites green

`pnpm exec vitest run` exits 0. Evidence: link the green CI run for the `vitest` lane on the release commit.

### G2 — Playwright E2E green (with preview server)

`BASE_URL=http://127.0.0.1:5173 pnpm exec playwright test` exits 0 against a running preview server. Evidence: the `playwright` CI lane on a CI run *and* a local run with `BASE_URL` set so the gated specs actually execute (CI defaults to self-skip without `BASE_URL`).

### G3 — `vox check` clean

`vox check apps/vox-mental-tracker/src/main.vox` exits 0 with 0 errors and 0 warnings. Evidence: `vox-check` CI lane on the release commit.

### G4 — Contracts schema-valid

Every file under `apps/vox-mental-tracker/contracts/event-payloads/*.json` parses as JSON; every `apps/vox-mental-tracker/contracts/export/*.yaml` parses as YAML. (JSON-Schema validation is a follow-up; until that lands the parse-only check is the gate.) Evidence: `contracts` CI lane.

## Manual gates

### G5 — Capacitor Android build succeeds

Follow `docs/how-to/build-android.md`; a debug `.apk` is produced. Smoke-test it on a connected device: home screen renders, a quick-add (Mood 3) increments the saved counter, the voice page round-trips a transcript.

### G6 — Capacitor iOS build succeeds

If an Apple toolchain is available: `npx cap add ios && npx cap sync ios && npx cap open ios`, build for a simulator. If unavailable, document the gap in the release notes; iOS parity returns when the toolchain is back online.

### G7 — Privacy doc current

`docs/user/privacy.md` reflects every data flow the release introduces or modifies. Compare the doc's "What we store / never leaves your device" sections to the current `HealthEventLog` + `RawTranscript` schema and any new endpoint that touches a network.

### G8 — Failure-modes research current

`docs/architecture/failure-modes-research-2026.md` covers any new failure surface introduced this release (new sensor, new export path, new platform plugin). At minimum, scan for stale references and renumber if anything was promoted out of "research" into "implemented".

## Process

1. From a clean working tree on the release commit, run `vox run apps/vox-mental-tracker/scripts/release_gates.vox` from the repository root. Capture the output.
2. Walk gates G5-G8 manually; tick each in a copy of `RELEASE_CHECKLIST.md` (template at the app root).
3. Tag the commit `vox-mental-tracker-vX.Y.Z` and push.
4. Attach the filled `RELEASE_CHECKLIST.md` to the GitHub release.

## Rollback

If a gate regresses post-release:

- Programmatic regressions: revert the offending commit and re-run gates.
- Privacy/data-flow regression (G7): file a tracking issue, hold rollouts until addressed.
