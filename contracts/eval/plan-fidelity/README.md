# `contracts/eval/plan-fidelity/` — Plan-Mode Fidelity Corpus

Canonical multi-step plan benchmark for [CR-L4](../../../docs/src/architecture/v1-release-criteria.md) (which operationalizes [CR-D1]'s 85% Wave-2 success number).

## Gate

- **Bar:** ≥ 85% of plans complete successfully without human intervention.
- **Sub-bar (demote):** < 65%.

## Fixture format

Each fixture is `plans/<id>.plan.toml`:

```toml
# vox:skip
id = "plan-fidelity-001-add-endpoint-with-auth"
training_eligible = true
wave = 2                                  # 1 | 2 | 3 — complexity tier

[input]
goal = "Add a @endpoint(kind: query) function `get_user(id: Id[User])` to src/users.vox with @auth(scheme: bearer) and a test."
preconditions = "Project compiles clean at HEAD."

[plan_expected_shape]
min_steps = 3
max_steps = 8
must_include_tasks = ["add-endpoint", "add-test", "verify-compile"]
must_include_decorator_calls = ["@endpoint", "@auth"]

[success_criteria]
command = "vox check --strict && vox test --filter=get_user"
expected_exit = 0
file_must_contain = [
  { path = "src/users.vox", regex = "@endpoint\\(kind: query\\)\\s*\\n\\s*@auth" },
  { path = "tests/users_test.vox", regex = "get_user" },
]
```

## Wave taxonomy

Per [CR-D1] terminology:

| Wave | Complexity | Example |
|---|---|---|
| 1 | Single-file edit, no cross-cutting concerns | "Rename function `foo` to `bar` and update callers in same file" |
| 2 | Multi-file, requires decorator coverage / type discipline | "Add @endpoint with @auth + matching test" |
| 3 | Cross-cutting (effect rows / workflow purity / retirement migration) | "Migrate all uses of `@component fn` to `component Name() {}`" |

Target distribution: 10 Wave-1 + 30 Wave-2 + 10 Wave-3 = 50 total.

## Mining strategy

Source from real orchestrator session transcripts where possible (extracts from [`crates/vox-orchestrator-mcp/`](../../../crates/vox-orchestrator-mcp/) telemetry once the corpus-feedback pipeline lands in P2.1-P2.2). Hand-curate for quality.

## Status

**Empty as of 2026-05-15.** Lands during P3.4 (target close 2026-10).
