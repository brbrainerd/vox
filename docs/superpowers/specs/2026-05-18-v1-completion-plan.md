# v1.0 Completion Plan — High-Value Objectives After the 2026-05-17/18 Push

**Date:** 2026-05-18
**Status:** Proposed. Section-by-section approval expected; each P-block is independent and can be greenlit or deferred.
**Context:** 24 commits over 2026-05-17/18 closed the largest LLM-target stub set and brought 5 of 9 CR-L audit runners to real measurement. This plan inventories what's still between the current state and v1.0 GA, scopes each remaining item, and prioritizes by leverage.

## Where we actually are (2026-05-18)

Reading directly off the tree:

| Track | State | Evidence |
|---|---|---|
| CR-L0 spec-to-app runner | STUB | `crates/vox-audit/src/subcommands/stubs.rs::SpecToAppStub`; corpus 0/10 |
| CR-L1 humaneval runner | REAL + 18 seed fixtures (56/56 @test pass) | `crates/vox-audit/src/subcommands/humaneval.rs`; corpus 18/164 |
| CR-L2 mens-on-distribution | STUB | `MensOnDistributionStub`; depends on CR-L1 corpus + MENS sampling |
| CR-L3 repair-corpus | STUB (project-scope `vox repair` real, harness missing) | `RepairCorpusStub`; corpus 0/50 |
| CR-L4 plan-fidelity | STUB | `PlanFidelityStub`; corpus 0/50 |
| CR-L5 ACI default | REAL | `aci_default.rs` |
| CR-L6 retirement parity | REAL | `retirement.rs` |
| CR-L7 deploy doctor leg | REAL (3/3 marquee green) | `deploy.rs`; vox-new + vox-deploy legs partial |
| CR-L8 corpus-feedback | REAL | `corpus_feedback.rs` |
| Marquee slot count | 3/3 real | `contracts/marquee/manifest.v1.yaml` |
| Examples/golden corpus | 39/61 doctor-green | live `vox doctor --project` |
| Two-param Result[T, E] | REAL across type + runtime | commit 92e4c59a7 |
| Held-out contamination guard | REAL (build + verify pair) | commit 5d27ca6f3 |
| LLM-panel infra | REAL (trait + OpenRouter + cache + retry) | commits c5742f1cc / a7bf52785 / feaa0d7c8 |

**4 of 9 CR-L gates are still stubs. All depend on corpus engineering, not language work.**

## Failure-class taxonomy of the 22 remaining golden examples

Each failure class is small but discrete. Sampled `vox check` errors group into:

1. **`Undefined variable: Unit` in expression position** (≈ 6 examples) — `return Ok(Unit)` from auth_patterns, error_propagation, saga_compensation, etc. The interpreter side resolves `Unit` (eval/expr.rs added in c497b73b8) but the typecheck-side `lookup_var` doesn't. ~1-hour fix.
2. **`Result(List(Named(X))) vs List(Table(X, ...))`** (pagination, others) — the Named↔Table unify rule in [314cbc360](../../../crates/vox-compiler/src/typeck/unify.rs) doesn't lift through `List<>`. Need to recurse one level on container types. ~1-hour fix.
3. **`Record vs Map`** (nested_types, ref_types) — `{ a: 1, b: 2 }` literal vs `Map[Str, Int]` annotation. Object literal with all-same-value-types could unify with Map; needs a check_expr arm. ~2-hour fix.
4. **`@scheduled` reserved-keyword tombstone** (scheduled_tick) — ADR-028 retired. Either update the example to use plain `fn` or delete the example. ~5-min decision.
5. **`panic` builtin missing** (decimal_math) — `panic("...")` is canonical Vox source style but the typecheck builtins don't carry it. Add to `BuiltinTypes::register_all`. ~30-min fix.
6. **`Str` vs `Char`** (ref_types) — char literals (`'x'`) being typed as Str. Lexer/typecheck mismatch. ~1-hour investigation.

## Plan structure

Five P-blocks, prioritized by ROI for v1.0. P1 and P2 are the v1.0 musts; P3-P5 are strong-should items that round out the corpus and runtime story.

---

### P1 — Close the 4 remaining CR-L audit runners (v1.0 must)

**Goal:** Replace `SpecToAppStub`, `MensOnDistributionStub`, `RepairCorpusStub`, `PlanFidelityStub` with real runners. Each gate produces a real number against its corpus, even if the corpus itself is below minimum-viable.

**Pattern:** Mirror the `HumanEvalRunner` shape (commit 68841b39f + 0c67eecb7):
- Load manifest from `contracts/eval/<gate>/manifest.v1.yaml`
- Walk fixtures from the on-disk corpus dir
- Compute corpus_hash, fixture count, per-fixture result
- Return `AuditReport` with `incomplete: true` when below minimum-viable; real number when above

**P1.1 — RepairCorpusRunner** (~6 hours)
- Walk `contracts/eval/repair-corpus/projects/*/`; each project is a multi-file Vox dir with a deliberately-introduced bug + an `expected.json` describing the convergence criterion.
- Invoke `vox repair --project <dir>` (already real per commit 08c086cc0) on each fixture.
- Outcome per fixture: clean | repaired | residual | infra_error. Aggregate to a pass rate.
- Status currently stub; corpus currently 0; harness lands as real even when corpus is empty (returns `InfrastructureError` honestly).

**P1.2 — PlanFidelityRunner** (~5 hours)
- Walk `contracts/eval/plan-fidelity/plans/*/plan.toml`; each declares a multi-step plan plus success criteria (e.g. "produces a PR with these tests passing").
- For each plan, drive the orchestrator's plan-mode via `vox-orchestrator-mcp::chat_tools::plan_loop` against a reference LLM panel.
- Outcome per plan: success | partial | abandoned. Aggregate to fidelity rate.
- Real LLM calls gated by `VOX_OPENROUTER_API_KEY`; without credentials returns honest `InfrastructureError`.

**P1.3 — SpecToAppRunner (CR-L0, block-GA gate)** (~8 hours)
- Walk `contracts/eval/spec-to-app/specs/*/spec.toml`; each declares an English spec + success criteria + max_cost_usd.
- For each spec, drive an autonomous agent loop (system prompt + the MCP orchestrator + the LLM panel) up to `iteration_budget_per_spec` rounds.
- Cost-meter every LLM call against the per-spec ceiling (`$5.00` per ratified D15).
- Outcome per spec: pass (vox check clean + tests pass + vox doctor green) | fail | over-budget. Aggregate.
- This is the CR-L0 integration test; the runner exists to drive it even when corpus is small.

**P1.4 — MensOnDistributionRunner** (~3 hours)
- Reuses the CR-L1 humaneval corpus (per `llm-panel.v1.yaml` D7 — "include MENS in the panel median for CR-L1/L2/L4").
- For each fixture, sample MENS through the OpenRouter `mens-current` panel member and count emissions that pass the full lint suite (vox check --strict + vox-code-audit + retirement-guard).
- Outcome: on-distribution rate.

**P1 acceptance:**
- All 9 vox-audit subcommands are real implementations (no `stubs.rs` left).
- Each runner returns a real number when corpus is non-empty; `InfrastructureError` with explanatory note when below minimum-viable.
- `cargo test -p vox-audit --lib` stays 90+ passing with new harness tests.

**Honest scope:** the *bars* (CR-L0 ≥60%, CR-L3 ≥70%, CR-L4 ≥85%) are not in scope here. The runners produce real measurements; meeting the bars is corpus + iteration work tracked separately under P3.

---

### P2 — Close the 6 remaining typecheck-class gaps in golden examples (v1.0 must)

**Goal:** examples/golden 39/61 → 61/61. This is the corpus the LLM training pipeline draws from and the surface every AI agent sees; failing examples mean training-data rot.

**P2.1 — `Unit` in expression position** (~1 hour)
- Register `Unit` as a value binding (Ty::Unit) in `BuiltinTypes::register_all`.
- Unblocks ~6 examples (auth_patterns, error_propagation, saga_compensation, …).

**P2.2 — Named↔Table unify lift through container types** (~1 hour)
- Extend the unify arm at typeck/unify.rs to allow Named(X) ↔ Table(X, _) inside List/Option/Result.
- The current arm only catches the bare-name case; recurse one level so `List[Named("Post")]` unifies with `List[Table("Post", _)]`.
- Unblocks pagination.vox + any other `Result[List[Row]]` annotation site.

**P2.3 — Object-literal-as-Map unify** (~2 hours)
- When ObjectLit has all-same-value-type fields and the expected is `Map[K, V]`, infer-as-Map instead of inferring-as-Record.
- Add a `check_expr` arm for ObjectLit-with-expected-Map. Unify K with Str (the JSON-like key shape) and V with the common value type.
- Unblocks nested_types, ref_types Map-expected sites.

**P2.4 — `@scheduled` example retire** (~5 minutes)
- scheduled_tick.vox uses `@scheduled` which is ADR-028 reserved. Update the example to use plain `fn` with a comment about durability runtime, OR delete the example outright and remove from the index.

**P2.5 — `panic` builtin in typecheck** (~30 minutes)
- Add `panic` to BuiltinTypes::register_all with signature `Fn(Ty::Str) -> Ty::Never`.
- The eval-side already handles it (`AssertionFailed`); just the typecheck registration is missing.

**P2.6 — Char-literal typing** (~1 hour)
- `'x'` lexes to a CharLit token but type-checks as Str. Trace through HIR lowering for CharLit — likely a missing `HirExpr::CharLit` case in checker/expr.rs.
- Unblocks ref_types and any other char-using example.

**P2 acceptance:**
- `vox doctor --project examples/golden` → 61/61 (or honest decision to deprecate stragglers).
- No vox-compiler test regressions (285 passing stays 285).
- No vox-audit regressions (90 passing stays 90).

---

### P3 — Corpus growth (v1.0 strong-should, ratified D11=a accepts slip)

**Goal:** Move every eval corpus from `stub` to at least `minimum-viable` status so CR-L runners produce gating numbers, not advisory ones.

**P3.1 — humaneval-vox 18 → 50 (minimum-viable)** (~6 hours)
- Mine remaining fixtures from examples/golden/* using the `@example` decorator (commit 43a40410c). For each suitable golden file, extract the canonical fn + write a matching tests.vox.
- 32 new fixtures × ~10 min each = ~5 hours; +1 hour for hash + manifest update + held-out coverage.
- Council per-fixture cost target from the implementation plan §3 was 2 hours; we're under that because the seed runner pattern is established.

**P3.2 — repair-corpus 0 → 15 (minimum-viable)** (~6 hours)
- 15 multi-file broken-Vox projects, 3 per bug class (type-error, effect-violation, logic, exhaustiveness, api-misuse) per the manifest.
- Use the `ast_mutator` from `vox-corpus` to seed mutations from clean projects; hand-curate for quality.
- Each fixture is a directory `projects/<id>/{Vox.toml, src/main.vox, expected.json}`.

**P3.3 — plan-fidelity 0 → 15 (minimum-viable)** (~5 hours)
- Mine 15 plans from real orchestrator session transcripts (post-P2.1 telemetry per the implementation plan §1.4).
- Wave distribution: 3 wave-1 + 9 wave-2 + 3 wave-3 per the manifest D24 ratification.
- Each fixture is `plans/<id>/plan.toml` with a goal + numbered steps + success criteria.

**P3.4 — spec-to-app 0 → 3 (minimum-viable)** (~4 hours)
- 3 English-spec fixtures (1 per tier: T1 single-file, T2 multi-feature+auth, T3 marquee-class) per manifest D15.
- Each fixture is `specs/<id>/spec.toml` with `{name, prompt, success_criteria, max_cost_usd, max_iterations}`.

**P3 acceptance:**
- Every corpus manifest's `count_current` ≥ its `minimum_viable.count`.
- Every CR-L runner can compute a real pass rate (no infrastructure-error returns when invoked).
- `vox audit all --no-canonical-report` produces a full report with numbers in every gate.

---

### P4 — Final compiler/runtime polish (v1.0 strong-should)

**P4.1 — Streaming response runtime (CR-L7 stream surface completion)** (~12 hours)
- Language surface for `@endpoint(kind: stream)` landed in commit e42ff52b4. The codegen + runtime side is open.
- Decision: SSE (server-sent events) for v1.0; WebSocket for v1.1. Justification: SSE is HTTP-native, no protocol upgrade dance, fits the existing Axum codegen.
- Work: emit Axum SSE handler from `HirEndpointKind::Stream`; add a small `StreamEmitter` wrapper in the runtime; wire the marquee chat fixture's `watch_room()` to actually stream.

**P4.2 — Cross-file coordinated repair (CR-L3 depth)** (~16 hours)
- Per-file repair landed in commit 08c086cc0. Cross-file coordination — where fix A in file X requires fix B in file Y to converge — is the deeper P4.1 lift.
- Approach: build a diagnostic dependency graph from the workspace's `vox check --format json` output; group diagnostics that share spans across files; submit grouped diagnostic sets to the LLM in single prompts.
- Stretch goal; honest fallback is to deliver per-file with a published gap note.

**P4.3 — `vox new` + `vox deploy` real platform integration (CR-L7 completion)** (~24 hours)
- `vox new` exists at 90 LoC (commit 7feb7886c); `vox deploy` at 265 LoC. Neither produces a deployable artifact end-to-end.
- Decision needed: default deployment target (Fly.io vs Railway vs OCI-publish-only). Recommend OCI publish for v1.0 (platform-agnostic), with Fly.io as the "happy path" wrapper.
- Work: container Dockerfile emission from `vox-deploy-codegen`; `vox deploy` invokes `docker build` + `docker push`; `vox new web` produces a project that survives the full pipeline.

**P4 acceptance:**
- `vox new web my-app && vox deploy my-app && vox doctor --project my-app` succeeds end-to-end within the [CR-P3] 120-second budget.
- Marquee Slot 3 (chat) `watch_room()` actually streams to a connected client.

---

### P5 — Documentation + audit truth-up (v1.0 must, low effort)

**Goal:** Reconcile the audit doc + implementation plan with reality. Several gaps called out as "open" in `vox-as-llm-target-audit-and-plan-2026.md` were closed during this session and should be marked.

**P5.1 — Audit doc state sync** (~2 hours)
- Update `docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md` to mark closed items: ACI default-on (CR-L5), retirement parity (CR-L6), corpus-feedback (CR-L8), deploy doctor leg (CR-L7 partial), humaneval seed corpus (CR-L1 partial).
- Update the gap audit §3 to reflect closures.

**P5.2 — Implementation plan phase status** (~2 hours)
- Update `docs/src/architecture/v1-llm-target-implementation-plan-2026.md` phase tables.
- Mark Phase 1 (quick wins) as fully landed.
- Mark Phase 2 (measurement infra) as 5/9 runners real.
- Phase 3 corpus engineering: humaneval seed landed, others pending.

**P5.3 — Release notes draft** (~3 hours)
- `docs/news/2026-Q2-language-target-progress.md` summarizing the 2026-05-17/18 push for v0.6 release notes.
- One paragraph per CR-L item closed; honest note about what remains.

**P5 acceptance:**
- No doc claims an item is open that this session closed.
- No doc claims an item is real that's still a stub.
- A reader can trust the audit + implementation plan as state-of-the-world.

---

## Effort summary

| Block | Hours | Calendar | Priority |
|---|---|---|---|
| P1 — CR-L runners | 22 | 1 week | v1.0 must |
| P2 — Typecheck gaps | 6 | 1-2 days | v1.0 must |
| P3 — Corpus growth | 21 | 1 week | v1.0 strong-should |
| P4 — Compiler/runtime polish | 52 | 2-3 weeks | v1.0 strong-should |
| P5 — Doc truth-up | 7 | 1-2 days | v1.0 must |
| **Total** | **108 hours** | **~5-6 weeks focused** | |

This is the realistic remainder to v1.0 GA per the council-ratified Q1-2027 target. P4.3 (vox deploy platform integration) is the longest single item and the most platform-dependent; deferring it to v1.1 reduces total to ~84 hours (~3-4 weeks).

## Recommended sequencing

If you approve incrementally:
1. **First sub-block:** P2 (6 hrs) — single-session, fastest visible payoff (39→61 examples).
2. **Then:** P5 (7 hrs) — quick doc reconciliation while P2 fix list is fresh.
3. **Then:** P1.1 + P1.2 + P1.4 (14 hrs) — three CR-L runners that don't need spec-to-app's autonomous-agent complexity.
4. **Then:** P3.1 + P3.2 (12 hrs) — humaneval MV + repair MV. Unblocks gating numbers.
5. **Then:** P1.3 (8 hrs) — CR-L0 spec-to-app runner, the integration test.
6. **Then:** P3.3 + P3.4 (9 hrs) — finish the remaining minimum-viable corpora.
7. **Then or defer:** P4 — compiler/runtime polish; the streaming + deploy items are where v1.0 vs v1.1 lines get drawn.

## Decision asks

I want one of these per P-block before starting:

1. **P1 — CR-L runners:** approve / approve specific sub-items / defer.
2. **P2 — Typecheck gaps:** approve all 6 / approve subset / defer.
3. **P3 — Corpus growth:** approve all / approve only humaneval+repair / defer corpus engineering as a separate workstream.
4. **P4 — Compiler/runtime polish:**
   - P4.1 (streaming): SSE for v1.0 / WebSocket for v1.0 / defer to v1.1.
   - P4.2 (cross-file repair): approve / accept the per-file v1.0 baseline.
   - P4.3 (vox deploy): OCI publish only / OCI + Fly.io wrapper / defer to v1.1.
5. **P5 — Doc truth-up:** approve.

Once approved I execute in the recommended sequence with check-ins at each P-block boundary.
