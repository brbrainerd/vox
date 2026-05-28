# `contracts/eval/repair-corpus/` — `vox repair` Project-Scope Corpus

Canonical multi-file broken-project benchmark for [CR-L3](../../../docs/src/architecture/v1-release-criteria.md).

## Gate

- **Bar (project-scope):** ≥ 70% of projects reach `vox check` clean + tests passing after `vox repair .`.
- **Bar (single-file aim):** ≥ 90% of single-file fixtures reach `vox check` clean after `vox repair <file>.vox`.
- **Sub-bar (block project-scope):** < 50% — demote project-scope to v1.1, ship single-file only.

## Fixture format

Each fixture is a directory `projects/<id>/` containing a full Vox project tree:

```
projects/
  001-type-mismatch-cascade/
    project.spec.toml      # metadata
    Vox.toml               # real project manifest
    src/
      main.vox             # source with deliberate bugs
      lib/
        users.vox
    tests/
      user_test.vox        # passing tests (after repair)
    BROKEN.md              # human-readable description of what's wrong
```

`project.spec.toml` shape:

```toml
# vox:skip
id = "001-type-mismatch-cascade"
training_eligible = true
bug_class = "type-error"                # type-error | effect-violation | logic | exhaustiveness | api-misuse
bug_count = 4                           # number of distinct bugs introduced
files_touched = ["src/main.vox", "src/lib/users.vox"]
expected_repair_attempts = 2            # advisory; actual attempt budget per implementation plan

[difficulty]
tier = "medium"                         # easy | medium | hard
reasoning = "Three of four bugs are local; one requires cross-file type inference."

[success_criteria]
command = "vox check --strict && vox test"
expected_exit = 0
```

## Bug taxonomy (canonical 5 classes)

| Class | Example |
|---|---|
| `type-error` | Bare `str` where `Id[User]` required, mismatched return type |
| `effect-violation` | Calling `http.*` from a `@pure fn` |
| `logic` | Off-by-one, wrong operator, swapped arguments |
| `exhaustiveness` | Missing ADT case in a match |
| `api-misuse` | Wrong decorator (`@server` instead of `@endpoint(kind: server)`), retired patterns |

Target distribution: 10 fixtures per class × 5 classes = 50 total.

## Mining strategy

Per [implementation plan §3](../../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md#3-fixture-corpus-budget):
- Use [`crates/vox-corpus/`](../../../crates/vox-corpus/) `ast_mutator` to programmatically introduce bugs into existing examples.
- Hand-curate the resulting set for difficulty distribution.
- ~8 hours per fixture for quality bar (project tree + bug introduction + test verification + spec doc).

## Status

**Empty as of 2026-05-15.** Lands during P3.3 (target close 2026-10).
