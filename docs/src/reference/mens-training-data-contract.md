---
title: "Mens training data (JSONL) contract"
description: "Contract for Mens JSONL: preflight rules, loader policies (VOX_MENS_TRAIN_JSONL_STRICT), mix/filter semantics, optional lane metadata, and how documentation extraction relates to the default code-only production mix."
category: "Language Reference"

schema_type: "TechArticle"
---

# Mens training data (JSONL) contract

Status note: Mens currently defaults to **code-oriented** production mixes. Documentation extraction exists, but documentation Q&A is **not** the default production training lane.

## Preflight (`preflight_train_jsonl`)

Before loading, native Candle QLoRA training runs `preflight_train_jsonl`:

- **No blank lines** — empty lines are errors (fail fast).
- **Line length cap** — default large cap (bytes); oversize lines error.
- **Non-empty file** required.

## Loading (`vox_tensor::data::load_all_with_policy`)

| Policy | Env | Behavior |
|--------|-----|----------|
| **Skip** (default) | (default) | Non-empty lines that are not valid `TrainingPair` JSON are **silently skipped** (`vox_tensor::data`). |
| **Fail fast** | `VOX_MENS_TRAIN_JSONL_STRICT=1` | First malformed non-empty line aborts with `InvalidData` and line context. |

Use **strict** in CI or when preparing golden corpora so silent data loss is visible.

## Mix / filter semantics

- **`min_rating`**: pairs below rating threshold are excluded after parse.
- **`--context-filter`**: retains only rows whose category contains the needle; **empty result** errors (`No training pairs found`).
- **In-loop skips** (short sequences, curriculum, etc.) are counted in training logs/telemetry; see Candle QLoRA training loop.
- **Lane metadata contract** (backward compatible):
  - optional `lane` (`vox_codegen`, `vox_docs_qa`, `vox_tooling`, `vox_speech`, `vox_trajectory_repair`, `vox_retrieval_grounded`),
  - optional `response_mode` (`code_only`, `prose_only`),
  - optional `task_family` (freeform short tag).
  Missing fields are backfilled by corpus mix before write.
- **Default production lane policy**: code-only by default (`include_lanes: [vox_codegen]` in `mens/config/mix.yaml`).
  Docs QA/prose rows are excluded unless operators explicitly opt in.

## Trajectory and retrieval lanes (moonshot alignment)

To improve compact-plan generation and self-healing behavior without embedding repository internals into model weights, keep trajectory/retrieval rows explicit and opt-in:

- **`vox_trajectory_repair`**: failed-attempt -> corrected-attempt pairs with tool/action traces.
- **`vox_retrieval_grounded`**: rows where output cites retrieved docs/contracts/artifacts rather than hidden memory.
- Recommended `task_family` tags:
  - `planner_brief`,
  - `repair_loop`,
  - `contract_reconciliation`,
  - `artifact_summary`.

Promotion guidance:

- Keep `vox_codegen` as default production lane.
- Enable trajectory/retrieval lanes in staged evaluation profiles first.
- Track `cost_per_success_step` and repair-convergence metrics before broad rollout.

## Documentation extraction today

- `crates/vox-corpus/src/corpus/extract_docs.rs` can emit:
  - `lane: "vox_codegen"` rows from fenced ` ```vox ` blocks,
  - `lane: "vox_docs_qa"` rows from section-level prose extraction.
- `crates/vox-cli/src/commands/mens/pipeline.rs` writes documentation extraction output to `mens/data/mix_sources/docs.jsonl`.
- The default `mens/config/mix.yaml` currently includes only `vox_codegen`, so prose documentation Q&A is not part of the default mixed training corpus.
- `mens/config/training_contract.yaml` currently affects the resolved `train_path`; its `context_filter` comment is advisory unless another training path explicitly wires that value into runtime config.

## Documentation metadata

Documentation-derived JSONL rows may carry extra metadata fields beyond the core `TrainingPair` shape. Those fields are for provenance and future retrieval or docs-QA workflows; current training loaders ignore unknown fields unless a stricter downstream consumer opts in.

## `vox mens corpus validate-batch` (compiler gate)

- With **recheck** enabled (default; use `--no-recheck` to skip), rows whose `response` / `code` / fenced Vox markdown bodies look like codegen are run through the same **`vox` frontend** as `vox check` (lex → parse → typecheck → HIR validation). Rows with `response_mode: prose_only` or docs-only lanes without Vox bodies are skipped.
- **`--quarantine <path>`** — JSONL of rejected rows with reasons.
- **`--report <path>`** — JSON summary (`rejected_malformed_json`, `rejected_compiler`, samples).
- **`VOX_MENS_TRAIN_JSONL_STRICT=1`** — fail the command if any row is rejected (use in CI when promoting a golden mix).

## Related

- `docs/src/reference/mens-training.md` — tooling overview.
- `docs/src/operations/voxdb-cutover-runbook.md` — DB + telemetry sidecar rollout.
