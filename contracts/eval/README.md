# `contracts/eval/` — Vox v1.0 LLM-Target Evaluation Corpora

This directory holds the canonical fixture sets that gate the §5 LLM-Target Fidelity criteria of [`docs/src/architecture/v1-release-criteria.md`](../../docs/src/architecture/v1-release-criteria.md).

Created 2026-05-15 per [`v1-llm-target-implementation-plan-2026.md`](../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md) §1.1 P0.2.

## Sub-corpora

| Subdir | Gates | Count target | Owner role |
|---|---|---|---|
| [`humaneval-vox/`](humaneval-vox/) | CR-L1, CR-L2 | 164 problems (30 held-out) | Corpus eng + Lang platform lead |
| [`repair-corpus/`](repair-corpus/) | CR-L3 | 50 multi-file broken projects | Corpus eng + Runtime/repair lead |
| [`plan-fidelity/`](plan-fidelity/) | CR-L4 | 50 multi-step plans | Corpus eng + Agent infra lead |
| [`spec-to-app/`](spec-to-app/) | CR-L0 | 10 English specs | Corpus eng + Agent infra lead |

Marquee app fixtures (CR-L7) live separately under `apps/marquee/` per [`contracts/marquee/manifest.v1.yaml`](../marquee/manifest.v1.yaml).

## Conventions

### Manifest

Every sub-corpus has a `manifest.v1.yaml` declaring:
- `corpus_hash: blake3:<hex>` — derived from sorted content hashes of all fixtures. Rate measurements pin to this hash.
- `corpus_size: N` — fixture count.
- `held_out_for_training: [<id>, ...]` — subset that MUST be excluded from MENS training corpus (CR-L1 contamination guard, [implementation plan R1](../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md#5-risk-register-cr-l-specific)).
- `fixtures: [{id, path, training_eligible: bool}]` — full inventory.

### Fixture format

Each fixture is a `.spec.toml` (or analogous shape per sub-corpus) with:
- `id` — stable identifier, never recycled.
- `prompt` — the input given to the LLM / agent.
- `success_criteria` — machine-checkable predicate(s).
- `training_eligible: bool` — `false` for held-out items; CI gate in [`crates/vox-corpus/`](../../crates/vox-corpus/) verifies exclusion.

### Reproducibility

All measurement runs pin:
- `temperature: 0.0`
- `seed: 42`
- `attempts_per_fixture: 5` (majority-success counted; see implementation plan §1.3 P2.8)

The reference LLM panel is defined in [`llm-panel.v1.yaml`](llm-panel.v1.yaml).

### Reports

Each measurement run emits `contracts/reports/<thing>/<date>.json` per the [`vox audit <thing>` CI contract](../ci/vox-audit-contract.v1.yaml).

## Status

All four sub-corpora are **empty stubs as of 2026-05-15** awaiting P3 (Corpus engineering, weeks 8–20 of the implementation plan). Each subdir has its `manifest.v1.yaml` and `README.md` in place but no fixtures.

CI policy: until each manifest declares `corpus_size >= minimum_viable`, `vox audit <thing>` returns exit code 2 (infrastructure-error) and the corresponding CR-L gate is marked "infrastructure not ready" in release notes — not "failed."

## Existing sibling eval artifacts (pre-2026-05-15, not LLM-target gated)

This directory already contained pre-existing eval infrastructure when the CR-L sub-corpora were added. The pre-existing files are **not** part of the CR-L0..CR-L8 gating story and may have their own owners and consumers. Reviewers introducing CR-L work should not edit these without confirming with their existing owners:

- `benchmark-matrix.json` / `benchmark-matrix.schema.json` — pre-existing benchmark matrix.
- `complexity-budget.v1.json` — complexity-budget snapshot.
- `external-serving-handoff.schema.json` — external serving handoff shape.
- `gui_visual_rubric.v1.schema.json` / `vision-rubric-output.schema.json` / `vision_rubric.v1.schema.json` — GUI / vision rubrics.
- `mens-scorecard*` (5 files) — MENS scorecard infrastructure. **Likely relevant to [CR-L2] on-distribution rate measurement and [CR-L8] feedback loop** — the implementation plan P2.5 (CR-L2 harness) should evaluate reusing this scorecard pipeline before building a new one.
- `runtime-generation-kpi.schema.json` — runtime generation KPI.
- `syntax-k-event.schema.json` — syntax K-event tracking.

Cross-reference these from the per-sub-corpus harness implementations (P2.4–P2.7) so we don't duplicate effort.

## See also

- [`v1-release-criteria.md` §5](../../docs/src/architecture/v1-release-criteria.md) — CR-L0..CR-L8 criteria.
- [`vox-as-llm-target-audit-and-plan-2026.md`](../../docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md) — audit + gaps.
- [`v1-llm-target-implementation-plan-2026.md`](../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md) — phasing, fixture budget, CI contract.
- [`../ci/vox-audit-contract.v1.yaml`](../ci/vox-audit-contract.v1.yaml) — CI subcommand contract for `vox audit <thing>`.
- [`llm-panel.v1.yaml`](llm-panel.v1.yaml) — reference-LLM panel pin.
- [`../marquee/manifest.v1.yaml`](../marquee/manifest.v1.yaml) — Marquee app manifest (CR-L7 fixture set).
