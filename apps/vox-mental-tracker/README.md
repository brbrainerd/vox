# vox-mental-tracker

Local-first mental health tracker scaffold (Vox language + Capacitor). **No cloud sync in v1** — data stays on device; share exports via the system sheet.

## Requirements

- **Vox** CLI (install per [external-app-bootstrap](../../docs/src/how-to/external-app-bootstrap.md) in the main Vox repo when this tree is vendored).
- **pnpm** for Capacitor/Vite assets.

## Commands

From this directory (with `vox` on `PATH`):

```bash
vox check src/main.vox
vox build src/main.vox -o dist
pnpm install
npx cap add android   # once
vox build src/main.vox -o dist --target android
```

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
