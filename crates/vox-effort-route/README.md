# vox-effort-route

Routes effort-audit findings to verified, drafted enforcement artifacts. Reads
S1's `findings.jsonl`, groups findings deterministically (enum-bucket plus a
conditional embedding sub-cluster for oversized buckets), re-judges each cluster
through the model-agnostic facade with an adversarial verification pass, and
emits ranked recommendations plus staged `.proposed` draft artifacts.

## CLI

```bash
vox audit effort-route --findings target/audit/effort/<run-id>/findings.jsonl
vox audit effort-route --findings <path> --out-dir target/audit/route/run-1
vox audit effort-route --findings <path> --model mens-r6.2
```

`--findings` points at the `findings.jsonl` an `vox audit effort` run produced
(`schema_version = "1.0"`). The judge model is resolved through
`vox-orchestrator::models::select`; its Vox-capability gates whether `VoxScript`
is an allowed artifact form.

## Output layout

Outputs land under the staging dir (`target/audit/effort-route/<run-id>/` by
default, or `--out-dir`):

- `recommendations.jsonl` — one `RecommendationRow` per cluster,
  `schema_version = "1.0"` (the contract S4 consumes)
- `recommendations.md` — human-readable summary (Top-N ranked by member tokens
  then confidence, per-form breakdown, methodology note; author-identity-free)
- `artifacts/*.proposed` — one staged draft artifact per verified recommendation

## ArtifactForm

Each verified recommendation drafts one artifact in the cheapest enforcement
form for its cluster:

| `ArtifactForm` | Target surface | Staging extension |
| --- | --- | --- |
| `AgentsMdRule` | one-paragraph rule in `AGENTS.md` | `.agents-rule.md.proposed` |
| `CodeAuditDetector` | a `vox-code-audit` lint detector spec | `.detector.md.proposed` |
| `ArchRule` | a `vox-arch-check` / `layers.toml` rule | `.arch-rule.toml.proposed` |
| `CiGate` | a CI contract entry or test/example fixture | `.ci.yaml.proposed` |
| `VoxScript` | a small `vox run` script (Vox-capable models only) | `.vox.proposed` |
| `CorpusNegativeExample` | a MENS fine-tuning negative example | `.corpus.jsonl.proposed` |
| `None` | legitimate work; no structural fix | (no file) |

`VoxScript` is gated behind model Vox-capability; on a non-Vox-capable run that
form falls back to `CiGate`.

## Staged, not applied

Every drafted artifact is written with a `.proposed` extension into the staging
dir and is **never** applied into the build tree. Applying proposals is S4's job.

## Architecture

See `docs/superpowers/specs/2026-05-30-effort-route-design.md`.

This is Slice 2 of 4. It consumes S1's (`vox-effort-audit`) JSONL contract;
measured-cost completion (S3) and auto-emit (S4) come later. S4 consumes the
`recommendations.jsonl` schema defined here.

## Live-network testing

Live decide/verify and embedding calls cost real tokens. The default test suite
uses `MockRouter` and a mock embedder. A manual end-to-end smoke runs a real S1
audit and routes its findings:

```bash
cargo run -p vox-cli -- audit effort --since "30 days ago" --limit 30
cargo run -p vox-cli -- audit effort-route --findings <that path>
```
