# `contracts/eval/spec-to-app/` — End-to-End Agent Authorship Corpus

Canonical English-spec-to-passing-app benchmark for [CR-L0](../../../docs/src/architecture/v1-release-criteria.md), the integration test for the v1.0 LLM-target claim.

## Gate

- **Bar:** ≥ 60% of specs produce a passing application at ≤ $5/spec median LLM cost across the reference panel.
- **Sub-bar (BLOCK GA):** < 40% pass rate **or** > $10/spec median cost.

CR-L0 is the *only* criterion whose sub-bar blocks GA outright. It is the integration test for the entire LLM-target story; every other CR-L is a sub-loop measurement.

## Fixture format

Each fixture is `specs/<id>.spec.toml`:

```toml
# vox:skip
id = "spec-to-app-001-todo-crud"
training_eligible = true
difficulty_tier = "T1"                    # T1 (todo-CRUD-class) ... T3 (dashboard-with-auth-class)

[input]
english_spec = """
Build a single-tenant todo list. Users should be able to add a todo,
mark it done, delete it, and see the list. Use a SQLite database.
Endpoints should be JSON. Include tests.
"""

[constraints]
max_cost_usd = 5.00                       # CR-L0 ceiling
max_iterations = 25                       # bounded outer loop
allowed_capabilities = ["working_tree_write", "branch_create", "push_allowed"]
forbidden_patterns = ["@component fn", "@server fn", "@py.import"]

[success_criteria]
all_must_pass = true
checks = [
  { name = "vox check --strict", expected_exit = 0 },
  { name = "vox test", expected_exit = 0 },
  { name = "vox deploy --dry-run", expected_exit = 0 },
  { name = "vox doctor", expected_exit = 0 },
  { name = "@endpoint-coverage", regex_per_file = "@auth|@public" },  # CR-L9 forward-compat
]

[expected_artifacts]
# Files that should exist after a successful run.
must_exist = ["src/main.vox", "src/db/schema.vox", "tests/todo_test.vox", "Vox.toml"]
must_not_exist = []
```

## Difficulty tiers

| Tier | Description | Example |
|---|---|---|
| **T1** | Single-archetype, single-file scope | "Todo CRUD with SQLite" |
| **T2** | Multi-feature, auth required | "Multi-user notes with @auth(scheme: bearer)" |
| **T3** | Marquee-class | "Dashboard with realtime updates, multi-tenant data isolation, deploy-ready" |

Target distribution: 4 T1 + 4 T2 + 2 T3 = 10 specs.

## Why these numbers (60% / $5)

Per [implementation plan §7.1](../../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md#71-cr-l0-adopted-2026-05-15):

- Naive product of other CR-L bars: 0.85 (planning) × 0.95 (on-distribution) × 0.70 (project repair) × 0.95 (deploy) ≈ 0.54. So 60% is mildly ambitious but reachable.
- $5/spec allows ~50K input + 10K output tokens at 2026-Q4 panel rates — enough headroom for 5–10 LLM round trips with project context.

## Status

**Empty as of 2026-05-15.** Lands during P3.6, depends on Marquee apps existing first (P3.5).
