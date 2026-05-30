# vox-effort-audit

AI-judged audit of git commit history. Walks commits in a range, calls the
model-agnostic judge facade per commit, optionally substitutes measured token
cost from local Claude Code transcripts, and emits a ranked report.

## CLI

```bash
vox audit effort --since "30 days ago"
vox audit effort --since v0.5.0 --until HEAD --model mens-r6.2
vox audit effort --limit 10 --no-transcripts
```

Outputs land in `target/audit/effort/<run-id>/`:
- `findings.jsonl` — one finding per commit, `schema_version = "1.0"`
- `report.md` — human-readable summary (Top-N + category breakdowns)
- `manifest.json` — run metadata (range, model, cost, coverage)

## Architecture

See `docs/superpowers/specs/2026-05-28-effort-audit-core-design.md`.

This is Slice 1 of 4. Cluster-and-route (S2), measured-cost completion (S3),
and auto-emit (S4) all consume the JSONL schema defined here.

## Live-network testing

Live judge calls cost real tokens. The default test suite uses `MockJudge`.
To run a live-network smoke against the configured judge model:

```bash
cargo test -p vox-effort-audit --features live-judge -- --ignored
```
