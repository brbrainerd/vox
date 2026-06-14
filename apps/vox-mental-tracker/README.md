# vox-mental-tracker

Local-first mental health tracker (Vox language; dual-target: web/PWA via Vite +
mobile via React Native/Expo). **No cloud sync in v1** — data stays on device;
share exports via the system sheet.

## Requirements

- **Vox** CLI (install per [external-app-bootstrap](../../docs/src/how-to/external-app-bootstrap.md) in the main Vox repo when this tree is vendored).
- **pnpm** for the web/Vite build; **npm/npx** inside the generated `mobile/` Expo project.

## Commands

From this directory (with `vox` on `PATH`):

```bash
vox check src/main.vox
pnpm install
pnpm build:web      # web/PWA → web-dist/
pnpm mobile:gen     # regenerate mobile/ Expo project from src/main.vox
pnpm mobile:start   # expo dev server (run `npm install` inside mobile/ first)
```

Android/iOS builds: see [`docs/how-to/build-android.md`](docs/how-to/build-android.md)
(EAS for installables; Tauri is desktop-only per the scope-tauri-desktop-only ADR).
The `mobile/` directory is generated output — never hand-edit it.

Automation scripts live under **`scripts/*.vox`** (run with `vox run`).

## Docs

- [`docs/README.md`](docs/README.md) — index (architecture, exports, Android build, privacy).
- `docs/how-to/clinical-export.md` — clinician-facing CSV/JSON contract notes + TS helpers.
- `docs/architecture/` — SSOT, failure-mode research, data model.
- `docs/user/privacy.md` — plain-language privacy stance.

## Repository layout

See plan: append-only **`HealthEventLog`** + derived views; exports under **`contracts/export/`**.

## Releasing

Per-release checklist in [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md). Detailed gate definitions in [`docs/how-to/release.md`](docs/how-to/release.md). To run the programmatic gates locally:

```bash
vox run apps/vox-mental-tracker/scripts/release_gates.vox
```
