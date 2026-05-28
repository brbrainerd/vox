# Release checklist — vox-mental-tracker vX.Y.Z

Tag: `vox-mental-tracker-vX.Y.Z`
Commit: `<sha>`
Date: `<YYYY-MM-DD>`
Released by: `<name>`

## Programmatic gates

Run `vox run apps/vox-mental-tracker/scripts/release_gates.vox` from a clean working tree (repository root).

- [x] G1 — Vitest passes
- [x] G2 — Playwright passes (web)
- [x] G3 — `vox check` clean
- [x] G4 — Contracts: JSON+YAML export specs valid
- [x] G5 — `tsc --noEmit` over emitted code
- [ ] G6 — Android E2E lane on emulator
- [ ] G7 — iOS E2E lane on simulator

CI run link: `<url>`

## Manual gates

- [ ] G8 — Android signed release APK installs and runs on a real device
- [ ] G9 — iOS archive uploaded to TestFlight
- [ ] G10 — Privacy manifest reviewed against current data flows
- [ ] G11 — Icons + splash render correctly on iOS notch + Android edge devices
- [ ] G12 — Deep link `voxmental://mood/3` opens correct route
- [ ] G13 — Push registration persists token; remote push opens correct route
- [ ] G14 — Offline-first SW: queue replays after reconnect
- [ ] G15 — `docs/user/privacy.md` audited

## Rollout

- [ ] Tag pushed
- [ ] GitHub release created with this checklist attached
- [ ] Users of prior version notified of any data-format / privacy changes (via release notes)

## Notes

`<freeform notes for this release: notable changes, known issues, follow-up work>`
