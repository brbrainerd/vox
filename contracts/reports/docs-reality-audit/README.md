# Documentation Reality Audit — report artifacts

Machine-readable backlog for [Documentation Reality Audit Program](../../documentation/docs-reality-audit.program.v1.yaml).

| File | Role |
|------|------|
| `inventory.v1.json` | Seed claims (high-authority docs/contracts) for cycle audits |
| `findings.v1.json` | Triaged mismatches (`CodeDeficit`, `DocDeficit`, …) |
| `metrics.v1.json` | Snapshot emitted by `vox ci docs-reality-audit metrics --write` |
| `*.schema.json` | JSON Schema for CI verification |

Refresh metrics after editing findings:

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
```
