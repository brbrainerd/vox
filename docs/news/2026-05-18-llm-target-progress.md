---
title: "LLM-target progress 2026-05-17/18"
date: 2026-05-18
category: news
training_eligible: false
training_rationale: "Engineering-process release notes; will be superseded by v0.6 release notes."
---

# LLM-target progress — 2026-05-17/18

A two-day push closed the largest LLM-target stub set and brought 5 of 9 CR-L audit runners to real measurement. This is the v0.6 progress note; the full v1.0 completion plan lives at [`docs/superpowers/specs/2026-05-18-v1-completion-plan.md`](../superpowers/specs/2026-05-18-v1-completion-plan.md).

## What shipped

**Real CR-L audit runners (replaced their stubs):**
- CR-L1 `vox audit humaneval` — runs against 18 seed fixtures, executes `@test` blocks in-process via `vox_compiler::eval::Interpreter`, reports per-fixture pass + corpus-validity rate. 56/56 tests passing.
- CR-L7 `vox audit deploy` — doctor leg against the 3 status:real Marquee fixtures (marquee-app, marquee-todo-auth, marquee-chat). 3/3 doctor-green.

**Real tooling:**
- `vox doctor --project [PATH]` — walks `.vox` files, compile-checks each, emits structured JSON report + `vox.doctor.project_check` telemetry.
- `vox repair --project [PATH]` — extends single-file repair to project scope; aggregates per-file outcomes.
- `vox-audit` panel infrastructure — `PanelClient` trait, live `OpenRouterPanelClient`, content-addressed disk cache, retry+backoff layer per `llm-panel.v1.yaml §operational_policy`.

**Real language-surface additions:**
- `@example` decorator — corpus-eligible reference fn, distinct from `@test`, lowers to `HirModule::examples`.
- `@public` post-`@endpoint` — opts the endpoint out of the @auth requirement.
- `@endpoint(kind: stream)` — streaming response surface (codegen + runtime deferred to v1.1).
- Two-parameter `Result[T, E]` — type-level + runtime + pattern-match all real. ADT-shaped errors round-trip through the in-process interpreter.
- `minimal_repro` field on diagnostic envelope — 3-line context window around the offending span for LLM consumption.

**Compiler typecheck depth:**
- Built-in `Some`/`None`/`Ok`/`Err`/`Error`/`Unit`/`panic`/`to_string` constructors and helpers.
- Polymorphic `print(value: any) → Unit`.
- `Id[T]` aliases to `Ty::Int` across all 3 lowering paths.
- `@table` row methods return Named row type (not anonymous Record) + Named↔Table unify rule.
- `@table` methods `filter`/`update`/`first` added.
- Permissive Record↔Record unify (partial-record subtyping).
- Object-literal-as-Map unify for `Map[K, V]` annotations.

**Marquee corpus:**
- Slot 2 (todo-auth) and Slot 3 (chat) bumped from `archetype-approved` to `real`. CR-P1 "3 marquee apps live" structurally satisfied.
- Slot 2 ships `Result[str, TodoError]` ADT-error returns with @test patterns matching `Error(TitleEmpty(_))` end-to-end.

**Held-out contamination guard (audit omission #4):**
- `build_held_out_manifest` + `verify_held_out_manifest` pair emits `contracts/eval/humaneval-vox/held-out.v1.json` and CI-verifies drift between spec.toml `training_eligible:false` flags and the manifest MENS training consumes.

## Corpus health

| Surface | Before | After |
|---|---|---|
| examples/golden doctor-green | 36/61 | 54/61 |
| humaneval-vox seed corpus | 0 fixtures | 18 fixtures (10 held-out) |
| in-process @test pass rate | n/a | 56/56 |
| Marquee slots real | 1/3 | 3/3 |
| CR-L runners real | 3/9 | 5/9 |
| vox-compiler lib tests | 285 (pre-existing) | 285 |
| vox-audit lib tests | 62 | 90 |

## What's still open

Per the [v1.0 completion plan](../superpowers/specs/2026-05-18-v1-completion-plan.md):

- 4 stub CR-L runners (SpecToApp, MensOnDistribution, RepairCorpus, PlanFidelity).
- Corpus growth: humaneval-vox 18 → 50/164; repair-corpus, plan-fidelity, spec-to-app 0 → minimum-viable.
- 7 examples/golden gaps (chain `.limit()`, Tensor builtin, deeper inference cases).
- Streaming response codegen + runtime (language surface real; emit deferred).
- `vox new` + `vox deploy` real platform integration (OCI publish + Fly.io wrapper or equivalent).
- Cross-file *coordinated* repair (per-file dispatch is real).

Total remaining estimated at ~108 hours focused work, or ~84 with `vox deploy` platform integration deferred to v1.1.

## Provenance

26+ commits over 2 days. Full commit history under `cc_bdesktop2/jovial-buck-e93ac0`; design docs at `docs/superpowers/specs/2026-05-{17,18}-*.md`.
