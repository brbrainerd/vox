# CI lanes for vox-mental-tracker

The app's CI workflow at `.github/workflows/vox-mental-tracker.yml` runs four parallel lanes plus an aggregator. All lanes scope to changes under `apps/vox-mental-tracker/**` or to the workflow itself.

| Lane | What it proves | Wall-clock budget |
|---|---|---|
| `vox-check` | `vox check apps/vox-mental-tracker/src/main.vox` passes against the parent workspace's compiler. | ~3 min (cold cargo build dominates) |
| `vitest` | All `tests/*.test.ts` pass under `pnpm exec vitest run`. Includes the Sherpa plugin TS declaration build as a precondition. | ~30 s |
| `playwright` | `tests/e2e/**` runs under Chromium. The lane resolves `@playwright/test`'s pinned version, restores `~/.cache/ms-playwright` (key: OS × version × `chromium`), and only runs `playwright install chromium` on cache miss — hot cache shaves ~30 s and ~110 MB per push. Tests self-skip when `BASE_URL` is unset, so the lane proves browser availability on every push and runs the full e2e suite once a preview server is provided. The preview server itself depends on the Vite scaffold plan ([`docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md`](../../../../docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md) → unblocks app-side Vite work). | ~30 s warm, ~2 min cold |
| `contracts` | Validates each file under `contracts/event-payloads/` parses as JSON and each `contracts/export/*.yaml` parses as YAML. | ~10 s |
| `app-summary` | `needs: [vox-check, vitest, playwright, contracts]`; fails if any required lane failed. The required check for branch protection. | ~5 s |

## Adding a new contract file

Drop the file in the matching directory (`event-payloads/` for `*.json`, `export/` for `*.yaml`) and the `contracts` lane picks it up automatically. If the file should validate against a JSON Schema, add the schema under `contracts/schema/` and extend the `contracts` job to invoke a JSON-Schema validator (Phase 5 follow-up).

## Running all lanes locally

```sh
# vox-check
cd <repo-root>
cargo run -q -p vox-cli -- check apps/vox-mental-tracker/src/main.vox

# vitest
cd apps/vox-mental-tracker
pnpm install
pnpm exec tsc -p plugins/vox-sherpa-transcribe/tsconfig.json
pnpm exec vitest run

# playwright (with a running preview server)
pnpm exec playwright install chromium
pnpm dev &  # or your preview server of choice
BASE_URL=http://127.0.0.1:5173 pnpm exec playwright test

# contracts
python3 -c "import json,glob; [json.load(open(p)) for p in glob.glob('contracts/event-payloads/*.json')]"
python3 -c "import yaml,glob; [yaml.safe_load(open(p)) for p in glob.glob('contracts/export/*.yaml')]"
```

## Lane shape rationale

The split mirrors the platform-side pattern in `docs/superpowers/plans/ci/2026-05-03-local-ci-pre-push-and-job-split.md`. Each lane has a single responsibility so failures give precise signal: a flaky e2e doesn't fail the contract validators, a missing schema doesn't block the type-check.
