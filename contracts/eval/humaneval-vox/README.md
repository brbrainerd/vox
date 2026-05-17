# `contracts/eval/humaneval-vox/` — HumanEval-Vox

Canonical benchmark for [CR-L1](../../../docs/src/architecture/v1-release-criteria.md). Anchored at 164 problems for direct comparability with HumanEval-Python (Chen et al., 2021).

## Gate

- **Bar:** ≥ 80% compile + test-pass rate across the reference LLM panel (median scoring).
- **Sub-bar (demote):** < 60%.

## Fixture format

Each fixture is a directory `problems/<NNN-name>/` holding three files:

```
problems/<NNN-name>/
├── spec.toml      # id, training_eligible, provenance, prompt
├── reference.vox  # reference solution
└── tests.vox      # @test blocks exercising reference's API
```

`spec.toml` shape:

```toml
id = "humaneval-vox-001-greet"
training_eligible = false                      # false = held-out (excluded from MENS training)
provenance = "examples-golden-lift"            # examples-golden-lift | hand-authored | ast-mutation
derived_from = "examples/golden/hello.vox"

prompt = """
Write a Vox function `greet(name: str) to str` that returns
`"Hello "` followed by the given name and a trailing `"!"`.
"""
```

`reference.vox` contains the canonical solution. `tests.vox` re-declares the function (so both files are independently compile-checkable) and adds `@test fn ...` blocks exercising it.

## Held-out subset

30 of the 164 problems carry `training_eligible: false`. The CI gate in `crates/vox-corpus/` verifies these are never ingested by MENS training pipelines. This is the corpus-contamination guard ([implementation plan R1](../../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md#5-risk-register-cr-l-specific)) — without it, CR-L1 numbers are leaked-evaluation marketing.

## Status

**Seed corpus landed 2026-05-17.** 18 fixtures (10 lifts from `examples/golden/`, 8 hand-authored) covering pure fns, control flow, Option, Result, string ops, and arithmetic. The runner at [`crates/vox-audit/src/subcommands/humaneval.rs`](../../../crates/vox-audit/src/subcommands/humaneval.rs) ships two measurement layers:

- **Corpus-validity (always on, deterministic).** Every fixture's `reference.vox` and `tests.vox` must compile clean via `vox_compiler::pipeline::check_file`. As of the seed batch, all 18 pass at 100% validity.
- **LLM-panel pass-rate (opt-in via `--llm-panel`).** Returns `InvalidInput` with a `note` until the OpenRouter-style client lands (deferred follow-on reusing [`vox-cli/src/commands/repair.rs`](../../../crates/vox-cli/src/commands/repair.rs)).

P3.1 grows the corpus toward the 164-problem target (minimum-viable at 50 with 10 held-out).

## Provenance

Each fixture must declare its provenance to avoid contamination:
- `derived_from: "examples/golden/<file>"` — if mechanically lifted from existing examples (these become `training_eligible: false` by default since MENS likely trained on them).
- `derived_from: "hand-authored-2026-MM"` — net-new problems.
- `derived_from: "ast-mutation-of/<source>"` — programmatic mutations.
