---
title: "Vox as an LLM-Target Language — Audit & v1.0 Plan (2026)"
description: "Codebase-grounded audit of Vox's readiness as a primary destination target for AI agents, with proposed v1.0 fidelity criteria (CR-L1..CR-L8) and a realistic sequencing plan onto existing phase work."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-15"
training_eligible: false
training_rationale: "Strategic audit and roadmap document; reflects May 2026 state, will be superseded by execution plans."
---

# Vox as an LLM-Target Language — Audit & v1.0 Plan (2026-05-15)

This is the companion doc to [`v1-release-criteria.md`](v1-release-criteria.md). It exists because the current 12 criteria operationalize **production**, **architecture**, **performance**, and a sliver of **agentic DX** — but they under-specify the load-bearing claim that recurs across the marquee/research corpus: **Vox is designed so AI agents can author code reliably, and the compiler+lint+repair pipeline can heal what they produce.**

The point of this doc is not to redo the existing criteria. They stand. What this adds is:

1. A **vision restatement** that names the goal in operationable terms.
2. A **shipped-feature audit** with file:line evidence, distinguishing what is real from what is specced.
3. A **gap audit** with severity tiers and concrete reproduction steps.
4. An **attainability verdict** that separates *realistic v1.0* from *aspirational v1.0*.
5. Eight proposed **fidelity criteria** (CR-L1..CR-L8) for adoption into [`v1-release-criteria.md`](v1-release-criteria.md) §5.
6. A **sequencing plan** that pegs each criterion to an existing phase plan rather than inventing new tracks.
7. **Open questions** for the council.

The bar height for the proposed criteria is **realistic v1.0** — measurable gates rooted in what the codebase already does at v0.5.0, not what mesh Phase 2 or a hypothetical MENS GRPO loop might deliver in v1.x.

---

## §1 Vision Restated

The phrase "humans become agentic orchestrators that command AI agents to write code" appears nowhere in [`v1-release-criteria.md`](v1-release-criteria.md). It appears repeatedly in the broader corpus:

- [`vox-marquee-explainer-2026.md`](vox-marquee-explainer-2026.md) — "MENS doesn't just 'write code'; it understands the dependency graph, the persistence layer, and the deployment constraints of the target environment."
- [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md) §1.2 — "**The `--for-llm` mode** additionally includes a *minimal repro* (smallest excerpt that reproduces the diagnostic alone). This is the single biggest delta between Vox-as-LLM-target and a typical compiler."
- [`comprehensive-audit-v2-2026.md`](comprehensive-audit-v2-2026.md) Part V — "**Vox is not just another web framework; it is the first agentic-native operating surface for software.** In an era where AI agents are no longer just assistants but active developers and maintainers, the traditional 'human-only' developer experience (DX) is the bottleneck."

Translated into testable shape, the vision asserts five things:

| # | Claim | Test it implies |
|---|---|---|
| V1 | LLMs can **write Vox source** that compiles cleanly more often than they can write Rust/TS source. | A HumanEval-equivalent benchmark whose pass rate is measurably higher on Vox than on a control. |
| V2 | When LLMs **misfire**, the compiler/lint diagnostic is sufficient signal to drive a self-repair loop without a human. | A `vox repair` flow with measured success on a corpus of known-broken programs. |
| V3 | The set of **retired/legacy patterns** an LLM might hallucinate is rejected at compile time, not at review. | Every entry in `AGENTS.md §Retired Surfaces` has a parse-time or arch-check forbid. |
| V4 | An LLM can drive a **whole project** — scaffold, build, run, deploy, health-check — via the CLI. | `vox new` → `vox deploy` → `vox doctor` is an end-to-end loop with telemetry. |
| V5 | The **diagnostic→repair→corpus** cycle closes: data from real repair sessions flows into the MENS training set, so the same rule fires less often next quarter. | A telemetry pipeline from `vox.lint.*` and `vox repair` outcomes into the corpus aggregator, with a published before/after rate. |

V1–V5 are how this doc will judge "how close are we." None of these are in the current criteria.

A note on framing: the goal is **not** that humans never write code. It is that humans spend their effort orchestrating, reviewing, and steering — and the language is shaped so AI's mistakes are *small, local, machine-detectable, and machine-fixable*. The current corpus is consistent on this; the current release criteria are silent on it.

---

## §2 Audit: What Vox Already Ships for LLM Authoring

This is not an aspiration list. Every row below names code or contracts on the v0.5.0 main branch as of 2026-05-15.

### §2.1 Type and effect surface (the "wrong programs are unrepresentable" pillar)

| Feature | Why it matters for LLMs | Evidence |
|---|---|---|
| **Effect rows enforced** (`@uses(net)`, `@pure`) | An LLM that marks a function `@pure` cannot sneak in `http.*` / `time.*` / `random.*` — compiler rejects at typeck. | [vox-compiler/src/typeck/effect_check.rs](crates/vox-compiler/src/typeck/effect_check.rs) — `check_effect_compliance()` with bottom-up inference for unannotated functions; effect propagation enforces caller ⊇ callee on annotated boundaries. Stdlib methods classified by name (`http.*` → `Net`, `db.*` → `Db`, etc.). Effect enum at [vox-compiler/src/ast/decl/effect.rs:8](crates/vox-compiler/src/ast/decl/effect.rs:8) — `Net`, `Db`, `Fs`, `Env`, `Clock`, `Random`, `Spawn`, `GpuCompute`, `Mutate`, `Mcp(String)`, `Nothing`. |
| **ID newtypes at boundary** | The most common LLM error in API code is stringly-typed IDs leaking between services. Vox refuses bare `str` at `@endpoint`/`@activity`/`@actor` parameter positions. | [vox-code-audit/src/detectors/id_at_boundary.rs](crates/vox-code-audit/src/detectors/id_at_boundary.rs) — diagnostic ID `catalog::TYPES_ID_REQUIRED_AT_BOUNDARY`. ID newtypes defined in [vox-db-types/src/ids.rs](crates/vox-db-types/src/ids.rs) via a newtype macro. |
| **Anonymous error rejection** | `Result[T, str]` on public boundaries is flagged. Forces named ADTs that LLMs can exhaustively pattern-match against. | [vox-code-audit/src/detectors/anonymous_error.rs:24](crates/vox-code-audit/src/detectors/anonymous_error.rs:24) — regex catches `fn ... -> Result[T, str]` returns at info severity; rule `catalog::TYPES_ANONYMOUS_ERROR_TYPE`. |
| **Workflow determinism check** (P1-T5) | `time.now()` / `random.*()` / `uuid()` inside a `workflow { }` body is a compile error, not a runtime trap. Killed before an LLM can ship non-replayable workflow code. | [vox-compiler/src/typeck/ast_decl_lints.rs](crates/vox-compiler/src/typeck/ast_decl_lints.rs) — comment marker `P1-T5: Workflow determinism check — forbid time.now and random.* in workflow bodies`. |
| **Pattern exhaustiveness** | LLMs miss ADT cases routinely. The typecker reports `missing_cases: Vec<String>` in the JSON diagnostic so the repair loop can name what's missing. | [vox-compiler/src/typeck/diagnostics.rs:96](crates/vox-compiler/src/typeck/diagnostics.rs:96) — `VoxCompilerDiagnosticPayload.missing_cases`. |
| **@pure transitive purity** | `@pure fn` cannot call `http`, `net`, `fs`, `db`, `random`, `time`, `log`, or `async/await`; rejected at parse/typeck. | [`AGENTS.md` §Vox Language Enforcement Rules](AGENTS.md) — codified policy; [vox-code-audit/src/detectors/pure_fn_impure.rs](crates/vox-code-audit/src/detectors/pure_fn_impure.rs) — detector. |

**Coverage assessment.** This is the most mature pillar. The intent — *make wrong programs structurally impossible* — is enforced, not just specced.

### §2.2 Diagnostic surface (the "compiler is the LLM's pair programmer" pillar)

| Feature | Evidence |
|---|---|
| **Structured JSON diagnostics** with `error_code`, `severity`, `span`, `expected_type`, `found_type`, `correction_hints: Vec<String>`, `suggested_fixes: Vec<SuggestedFix>`, `missing_cases`, `ast_node_kind` | [vox-compiler/src/typeck/diagnostics.rs:76](crates/vox-compiler/src/typeck/diagnostics.rs:76)-109. Serde-serializable. Consumed by `vox repair`. |
| **Stable diagnostic IDs** with deprecation aliases | Phase-1 plan landed in [vox-language-rules-phase1-ssot-collapse-2026.md](vox-language-rules-phase1-ssot-collapse-2026.md) — `#[vox_diagnostic]` proc-macro + diagnostic catalog scaffolding. |
| **`vox check --format json`** for agents | [vox-cli/src/commands/repair.rs:57](crates/vox-cli/src/commands/repair.rs:57)-79 — consumes this format. |
| **Golden snapshot tests for diagnostics** | [vox-compiler/tests/diagnostic_snapshots.rs](crates/vox-compiler/tests/diagnostic_snapshots.rs) — `insta::assert_json_snapshot!` per diagnostic class. |
| **Doctest pipeline strict** | [vox-doc-pipeline/src/pipeline/doctest.rs:6](crates/vox-doc-pipeline/src/pipeline/doctest.rs:6)-66. Every ` ```vox ` block in docs goes through `vox_compiler::pipeline::check_file()` at line 50; LintError raised on any diagnostic. `// vox:skip` opt-out at line 32. |

**Coverage assessment.** Diagnostic shape is LLM-ready. **2026-05-25 update (session-15 + session-16):**
- `excerpt: Option<DiagnosticExcerpt>` — ±3 source lines around each diagnostic span, inline in the `--for-llm` JSON output (commit `fe2b05a051`). Closes the "LLM agent doesn't have the file open" gap.
- `explain_url: Option<String>` — stable `https://vox-lang.org/diag/<id>` URL for diagnostics with `vox/<category>/<name>` codes (commit `bca186355c`). Closes the `explain_url` field from Task 5.
- `lint_findings: Vec<LintFindingPayload>` (**session-16**) — TOESTUB/vox-code-audit lint findings surfaced in the `--for-llm` envelope under `#[cfg(feature = "stub-check")]`. Each finding carries `rule_id`, `severity`, `message`, `line`, `column`, `rationale`, `confidence`, `suggestion`, `alternatives`, `explain_url`, and `minimal_repro`. Integration via `ToestubEngine::check_source_file()` (single-file, no scan, no telemetry). 4 passing integration tests in `vox-cli/tests/check_for_llm_envelope.rs`.
- `minimal_repro: Option<String>` (**session-16**) — per-finding minimal reproduction snippet from `DetectionRule::minimal_repro()` (new trait method, default `None`). Implemented on `auth_endpoint`, `llm_provider_call`, `retired_decorator`, `decorator_position`, `workflow_nondeterministic`, `env_secret_shape`, `retired_memory_api`. Built via `ToestubEngine::minimal_repro_table()` lookup; zero breaking changes to 51 existing detectors.

**Phase 2 Task 5 is now complete.** All fields — `rationale`, `confidence`, `alternatives`, `explain_url`, and `minimal_repro` — are surfaced in the `--for-llm` envelope via the TOESTUB lint pipeline.

**2026-05-25 session-17 additions:**
- `minimal_repro` now implemented on 18 of 52 detectors (up from 7 at session-16 close). Added to: `stub`, `hollow_fn`, `empty_body`, `ai_laziness`, `no_test_for_pub_fn`, `id_at_boundary`, `anonymous_error`, `unwrap_call`, `crypto_ban`, `pure_fn_impure`, `stringly_typed_enum`.
- `vox check --explain <id>` now shows the `minimal_repro` snippet in the terminal output (under an "Example:" heading).
- `vox repair` now includes TOESTUB lint findings (with `suggestion` + `minimal_repro` snippets) in the LLM prompt under `#[cfg(feature = "stub-check")]`, making the 3-attempt repair loop aware of Vox policy violations that compile without error.

### §2.3 Self-repair surface

| Feature | Evidence |
|---|---|
| **`vox repair` MVP** (3-attempt loop) | [vox-cli/src/commands/repair.rs](crates/vox-cli/src/commands/repair.rs). Loop: `vox check --format json` → parse `DiagnosticPayload` → LLM call (OpenRouter, temp 0.1, system prompt "expert Vox language repair agent") → extract code block → `fs::write()` → re-check. Max 3 attempts. **session-17:** loop now also feeds `LintFindingPayload` (with `minimal_repro` snippets) for policy violations when `stub-check` feature is on. |
| **`vox stub-check`** | [vox-cli/src/commands/diagnostics/stub_check/](crates/vox-cli/src/commands/diagnostics/stub_check/) — catches `todo!()`, `unimplemented!()`, `panic!("not implemented")`, hollow returns, AI placeholder patterns. TOML suppressions supported. |
| **52 vox-code-audit detectors** including LLM-specific: | `id_at_boundary`, `anonymous_error`, `stub`, `hollow_fn`, `empty_body`, `ai_laziness`, `pure_fn_impure`, `workflow_nondeterministic`, `unresolved_ast`, `unresolved_ref` — listed in [vox-code-audit/src/detectors/mod.rs](crates/vox-code-audit/src/detectors/mod.rs). 18/52 now carry `minimal_repro` snippets. |
| **Test-first enforcement** (pre-commit) | [`AGENTS.md` §Test-First Policy](AGENTS.md). `tdd-guard` lefthook hook rejects commits introducing `pub fn` without an adjacent `#[test]` or `@test`. Reason given: tests are MENS training reward signal (planned `r_test` = 30% of GRPO reward). |

**Coverage assessment.** The closed-loop scaffold exists for **single files**. What's missing is project scope, a measurement methodology, and the back-edge from repair outcomes into the corpus aggregator.

### §2.4 Agent ergonomics

| Feature | Evidence |
|---|---|
| **Discovery surface** | [`docs/src/.well-known/llms.txt`](docs/src/.well-known/llms.txt), [`docs/agents/`](docs/agents/) inventory (5 JSON files: `vox-language-surface.v1.json`, `ai-ide-feature-matrix-2026.json`, `doc-inventory.json`, `script-registry.json`, `baseline-script-metrics.json`), layered policy via [`AGENTS.md`](AGENTS.md) + tool overlays ([CLAUDE.md](CLAUDE.md), GEMINI.md). |
| **MCP server** with 50+ tool modules / 100+ tools | [vox-orchestrator-mcp/src/lib.rs](crates/vox-orchestrator-mcp/src/lib.rs) — code validation, VCS (with banned-command denylist + `vox.vcs.exec` telemetry), planning loop, introspection, browser, shell, memory, RAG, task management, ACI envelope, agentos telemetry. |
| **Plan mode** with iterative refinement | [vox-orchestrator-mcp/src/chat_tools/plan_loop.rs](crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs) — rounds, loop_status, stop_reason; expansion-first refinement ("add work, do not paraphrase away detail"); task dependency validation. |
| **Telemetry of agent activity** — rich | [vox-telemetry/src/types.rs](crates/vox-telemetry/src/types.rs) — `METRIC_TYPE_PLAN_MODE_DECISION` (D2), `METRIC_TYPE_MODEL_TIER_ROUTE` (D1), `METRIC_TYPE_SUBAGENT_DISPATCH` (D4), `METRIC_TYPE_CIRCUIT_BREAKER_TRIP` (D6, doom-loop), `METRIC_TYPE_AGENTOS_GUARDRAIL_DENY` (S1), `METRIC_TYPE_DRIFT_ALERT` (D10). |
| **VoxScript-first glue** | [`AGENTS.md` §VoxScript-First Glue Code](AGENTS.md). All project automation as `.vox` files runnable via `vox run`; banned `.ps1`/`.sh`/`.py` glue. Single command shape; type-checked; observable via `vox.script.*`. |
| **ACI v1 envelope** schema | [`contracts/aci/agent-computer-interface.v1.yaml`](contracts/aci/agent-computer-interface.v1.yaml) + JSON schema. Mutation classification (`read_only` / `local_mutation` / `external_side_effect` / `unknown`). Implementation at [vox-orchestrator-mcp/src/aci/envelope.rs](crates/vox-orchestrator-mcp/src/aci/envelope.rs). **On by default since v0.6** — `agentos_aci_envelope_enabled: true` (CR-L5, 2026-05-25). |

**Coverage assessment.** Discovery, MCP dispatch, planning, telemetry, and ACI envelope (v0.6+) are real. One gap remains: there is no end-to-end deploy-and-health flow exposed via CLI (see §3.4 / CR-L7).

### §2.5 Concrete net achievements (the unsung list)

For honest credit-where-due: these are technical achievements already in the tree that move the needle. They deserve naming in v1.0 marketing even if they don't gate v1.0:

1. **47-rule LLM-aware lint suite** that catches the classes of code LLMs *actually* produce wrong: stubs, hollow bodies, anonymous errors, ID hallucinations, workflow nondeterminism, `@pure` violations.
2. **End-to-end repair MVP**: `vox repair` is not a stub. It runs the compiler, parses structured diagnostics, calls an LLM, applies a patch, and re-checks. This is more than most language ecosystems have today.
3. **Doctest infrastructure that actually compiles documentation**, killing the "examples rot" failure mode that poisons LLM training corpora elsewhere.
4. **VoxScript-first automation** that gives every `.vox` script type-checking, cross-platform shape, and `vox.script.*` observability. Single-shape glue is unusual in the industry.
5. **Layered agent-instruction architecture** (`AGENTS.md` base + tool overlays + `.well-known/llms.txt` + machine-readable feature matrices) — the discovery contract for coding agents is more formalized here than in most other languages' agent stories.
6. **Test-first as cross-tool policy**, enforced at commit time with the explicit rationale that tests are training reward — not just "good practice."
7. **Effect rows on a real language**, not as research, with `@uses(net)` enforcement live in the typeck. Few other production-target languages have anything comparable.

These are real foundations. The audit is honest about gaps below; that should not erase how much of the LLM-target ambition is *already standing*.

---

## §3 Audit: Gaps Between Current State and the Vision

Severity tiers used below:

- **🔴 Blocker** — must close to credibly claim v1.0 ships "LLMs can author Vox."
- **🟠 Major** — meaningfully degrades the claim; closeable in v1.0 with focus.
- **🟡 Minor** — doesn't block v1.0 but creates drift if untouched (especially in agent-facing docs).

### §3.1 🔴 Blocker — Self-repair has no measurement

**Symptom.** [`CR-D2`](v1-release-criteria.md) reads "`vox repair` must successfully resolve 90% of syntactically valid but logically broken Vox programs identified during the v1 audit." Today:

- The benchmark corpus does not exist. No `contracts/eval/humaneval-vox/` or equivalent fixture set.
- There is no eval harness wired to compute pass rate.
- The 90% figure has no baseline measurement to compare against.

**Why it matters for V2 (vision).** A self-healing claim without a number is marketing, not engineering. Both this audit and external reviewers cannot tell whether v0.5 is at 30%, 60%, or 80%.

**Reproduction.**
```
$ grep -r "humaneval-vox" --include="*.yaml" --include="*.toml"  # nothing
$ grep -r "repair.*pass.*rate" docs/src/  # nothing
```

**Fix scope.** Adopt CR-L1 below. Build a 200-program HumanEval-Vox corpus from existing `examples/golden/**` + mutated variants; run an eval harness in CI; publish quarterly pass-rate report.

---

### §3.2 🔴 Blocker — No corpus-feedback closed loop

**Symptom.** The plumbing exists ([vox-corpus/src/lib.rs](crates/vox-corpus/src/lib.rs) lists `arca_replay`, `ast_mutator`, `flywheel`, `tool_workflow_corpus`, etc.) and `reward_hook: Option<String>` stubs sit in `training_config.rs` — but:

- No GRPO trainer is wired.
- `r_test` (30% of intended reward) is not measured.
- `vox repair` outcomes (succeeded / rejected fix / accepted with edits) do not flow into vox-corpus.
- `vox.lint.*` telemetry export to MENS corpus is not running (Phase 4 Task 7 in [vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) — design only).

**Why it matters for V5.** Without this loop, fixing the same LLM mistake each generation is a cost that does not amortize. The whole "humans become orchestrators" thesis requires that the model's distribution **shifts** based on the rules it gets caught by — otherwise the orchestration cost grows linearly with code volume.

**Fix scope.** Adopt CR-L8. Ship Phase 4 Task 7 (telemetry → corpus pipeline). Define a quarterly export gate that emits a single artifact `contracts/reports/corpus-feedback/<date>.json` containing top-N firing rules + autofix rejection rates + repair outcomes.

---

### §3.3 🔴 Blocker — Plan-mode fidelity unmeasured

**Symptom.** [`CR-D1`](v1-release-criteria.md) reads "AI agents must be able to execute a multi-step 'Wave 2' plan with at least 85% success rate without human intervention."

- "Wave 2" is undefined as a benchmark set.
- `METRIC_TYPE_PLAN_MODE_DECISION` measures plan *routing* (whether to enter plan mode), not plan *quality*.
- No fidelity test harness exists.

**Why it matters for V4.** If humans are to orchestrate via plans, the plans need a measured success rate. Without it, agents iterate blind and humans cannot calibrate trust.

**Fix scope.** Adopt CR-L4. Define "Wave 2" as a fixture set in `contracts/eval/plan-fidelity/` containing 50–100 multi-step plans with success criteria; wire eval harness into `vox audit`.

---

### §3.4 🟠 Major — Deploy/health CLI gap

**Symptom.** From the generated [docs/src/reference/cli-command-surface.generated.md](docs/src/reference/cli-command-surface.generated.md):

- ✓ `vox init` (scaffold)
- ✓ `vox build`, `vox compile`, `vox run`
- ✗ `vox new` — not present (only `vox init`)
- ✗ `vox deploy` — not present (codegen crate `vox-deploy-codegen` exists, no CLI dispatch)
- ✗ `vox doctor` — only `vox openclaw doctor`; no top-level health command

**Why it matters for V4.** [CR-P3](v1-release-criteria.md) promises `vox new web → vox deploy` in under 120 seconds. An LLM agent cannot drive a flow that doesn't exist. Agents can scaffold and build today; deploy + health-check + rollback is missing as a single shape.

**Fix scope.** Adopt CR-L7. Land `vox new`, `vox deploy`, `vox doctor` end-to-end with structured output (JSON), telemetry events (`vox.deploy.*`), and an integration test that drives the full loop on a "Marquee" app fixture.

---

### §3.5 🟠 Major — Retirement guards incomplete

**Symptom.** [`AGENTS.md` §Retired Surfaces (LLM Guard)](AGENTS.md) lists 11 retired/deprecated patterns. Only some are compile-time forbidden:

| Retired pattern | Compile-time forbid? | Evidence |
|---|---|---|
| `vox-dei` | ✓ CLI check (`vox ci no-dei-import`) | Active per [docs/src/reference/cli-command-surface.generated.md](docs/src/reference/cli-command-surface.generated.md) |
| `vox-ars` | ✗ No detector | Grep returns no `ars` retirement lint |
| `@component fn Name()` (use `component Name() {}`) | ✗ No detector | Not enforced at parse time |
| `@server fn`, `@query fn`, `@mutation fn` | ✗ No detector | Should suggest `@endpoint(kind: ...)` |
| `@py.import` | ✗ No detector | Python is retired glue, but `@py.import` not flagged |
| `TURSO_URL` / `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | Partial — env-var migration aliases exist, no hard reject | |
| `recall()` / `recall_async()` | ✗ No detector | |
| `@capacitor/*`, `npx cap sync` | ✗ No detector | |
| `axum::serve`, `rust-embed` (in generated apps) | ✗ No detector | |
| `vox-sherpa-transcribe` | ✗ No detector | |

**Why it matters for V3.** LLMs trained on pre-2026 corpora *will* emit `@component fn` and `@server fn`. Without parse-time rejection, they only get caught at review — the opposite of the design goal.

A second dimension: **stale enforcement documentation can mislead agents** even when enforcement is live. [`AGENTS.md` §Grammar Unification](AGENTS.md) currently says:

> **Implementation status (Phase 2):** `actor`, `workflow`, and `activity` are fully supported bare keywords as of TASK-2.6 Path A (commit `080b3f86`). They lower to `HirFn { durability: Some(DurabilityKind::_) }` — no separate HIR node types. The tombstone that previously rejected these keywords has been removed; source files may freely use `actor`, `workflow`, and `activity` forms.

But [vox-compiler/src/pipeline.rs:21](crates/vox-compiler/src/pipeline.rs:21) — the `check_adr028_reserved_keywords` function — still rejects `workflow`, `activity`, `@scheduled`, `@durable` with error code `E028`, and is invoked at [vox-compiler/src/pipeline.rs:269](crates/vox-compiler/src/pipeline.rs:269) and [vox-compiler/src/pipeline.rs:415](crates/vox-compiler/src/pipeline.rs:415). [`durability-runtime-audit-2026.md`](durability-runtime-audit-2026.md) corroborates: "Confirms `@scheduled`, `@durable`, `workflow`, `activity` are parse-only with zero runtime implementation. Recommends grammar removal; `actor` retained."

So an LLM reading `AGENTS.md` thinks `workflow` is fully supported, writes one, and gets `E028`. This is exactly the kind of doc/code drift the LLM-target story is supposed to prevent.

**Fix scope.** Adopt CR-L6. Two-part: (a) one detector per row of `AGENTS.md §Retired Surfaces` with a docs link to the canonical replacement; (b) a CI rule that fails if `AGENTS.md §Grammar Unification` claims a keyword is "fully supported" while the pipeline still emits the corresponding `E028`. The arch-check has a precedent in [`vox-arch-check`](crates/vox-arch-check/).

---

### §3.6 🟠 Major — On-distribution rate for MENS-emitted code is not measured

**Symptom.** [`mens-training-ssot.md`](mens-training-ssot.md) governs the MENS corpus but does not pin a measurable on-distribution rate for emitted code. Question we cannot answer today: *what fraction of MENS-emitted programs pass `vox check` without firing any retire/stale/anti-pattern lint?*

**Why it matters for V1.** This is *the* claim. If we cannot say "X% of MENS-emitted Vox passes the same gates a human-authored PR must pass," then "Vox is a better LLM target" reduces to vibes.

**Fix scope.** Adopt CR-L2. Define on-distribution as: `vox check --strict + vox-code-audit + retirement-guard + effect-check` — zero errors, zero high-confidence warnings. Run on every MENS evaluation pass; publish quarterly rate.

---

### §3.7 ~~🟠 Major — ACI envelope is opt-in~~ ✅ RESOLVED in v0.6

> **2026-05-25 update:** CR-L5 landed in v0.6.0 (commit `5b8a932f65`).
> `OrchestratorConfig::agentos_aci_envelope_enabled` now defaults to `true`.
> `vox audit aci-default` CI gate passes and `feature-flags.v1.yaml` updated.
> Remainder of this section preserved for historical context.

**Symptom (v0.5 — resolved).** Per [`agentos-ssot-2026.md`](agentos-ssot-2026.md), `OrchestratorConfig::agentos_aci_envelope_enabled` defaulted to `false`. Mutation classification, guardrail kernel, and `METRIC_TYPE_AGENTOS_GUARDRAIL_DENY` were all live but dormant unless the agent's host opted in.

**Why it matters for V4.** If a remote agent cannot reliably tell whether a tool call mutates the working tree, it cannot reason about safety. Opt-in defaults push that responsibility to humans configuring each IDE — the opposite of the orchestrator model.

**Fix scope.** Adopt CR-L5. Flip the default to `true` in v0.6 with a release-note migration shim that surfaces deprecation guidance to any consumer reading the old shape.

---

### §3.8 🟡 Minor — Codegen IR unification is claimed but not landed

**Symptom.** Multiple SSOTs reference "Codegen SSOT unification 2026" reducing 4 IRs → 2 and 3 emit stacks → 1 ([memory entry "Codegen SSOT unification 2026"]). Reality:

- TS codegen lives at [vox-codegen/src/codegen_ts/](crates/vox-codegen/src/codegen_ts/) (40+ files).
- Rust codegen lives at [vox-codegen/src/codegen_rust/emit/](crates/vox-codegen/src/codegen_rust/emit/) (21 files).
- Web IR exists; [`webir-hir-split-brain-inventory-2026.md`](webir-hir-split-brain-inventory-2026.md) and [ADR-036](../adr/036-webir-hir-unification-compare-both.md) acknowledge ongoing split-brain.
- `CoreIrVersion::v2` is a single naming hook ([vox-compiler/src/hir/core_ir.rs:23](crates/vox-compiler/src/hir/core_ir.rs:23)) but does not actually unify the emit paths.

**Why it matters for V1 (indirectly).** Different backends validating separately means semantic divergence is possible — an LLM could write source that the TS backend accepts and the Rust backend rejects. Empirically rare today, but the structural risk is real.

**Fix scope.** Track under ADR-036 follow-through, not CR-L. The codegen unification is a multi-quarter project. Flag here so reviewers understand why CR-L does not depend on it.

---

### §3.9 🟡 Minor — Inference hosting absent

**Symptom.** [vox-inference](crates/vox-inference/) ships traits and backends (Candle CPU/CUDA/Metal, Ollama, llama.cpp) but no integrated HTTP server endpoint. MENS Mn-T2 (see [mesh-mens-distributed-training-and-execution-plan-2026.md](mesh-mens-distributed-training-and-execution-plan-2026.md)) is "deferred pending backend stabilization."

**Why it matters for v1.x, not v1.0.** Inference hosting belongs to the mesh story (Phase 5–6) and the personal/grand network arc, not to v1.0 of "AI authors Vox." Agents authoring Vox can call external APIs today. Including this in v1.0 would inflate scope without changing the LLM-target claim.

**Fix scope.** Do **not** add a CR-L for this; track under mesh Phase 5–6 plans. Document the deferral here so CR-L does not get confused with the mesh roadmap.

---

### §3.10 🟡 Minor — Mesh chaos/partition testing absent

**Symptom.** Orchestrator + workflow journal have unit tests but no partition / kill-9 / clock-skew integration harness. [`mesh-phase0-foundations-plan-2026.md`](mesh-phase0-foundations-plan-2026.md) acceptance is "no silent data loss" — provable only under fault injection.

**Fix scope.** Track under mesh Phase 0–1 plans. Not a CR-L because it gates v0.6 (single-machine) and v0.7 (LAN mesh) acceptance, not the LLM-target claim per se.

---

## §4 Attainability Verdict

Three framings.

### §4.1 Realistic v1.0 (achievable by end-2026 with focus)

> *AI-co-authored Vox: humans still hand-edit text, but the compiler/lint/repair triad lets LLMs make safe edits, fix their own diagnostics on the file they touched, and stay on-distribution for the parts of the language that ship.*

This is **achievable** if CR-L1..CR-L8 below are adopted and worked, because every gap they close is **measurement and integration**, not novel research. The hard primitives (effect rows, ID newtypes, doctest, MCP dispatch, repair MVP) already exist.

Concrete delivery shape:
- HumanEval-Vox at ≥ 80% (CR-L1)
- MENS on-distribution rate ≥ 95% (CR-L2)
- `vox repair` project-scope ≥ 70% on the audit corpus (CR-L3)
- Plan-mode fidelity ≥ 85% (CR-L4, matches existing CR-D1)
- ACI envelope on by default (CR-L5)
- Compile-time retirement guard parity with `AGENTS.md` (CR-L6)
- `vox new` / `vox deploy` / `vox doctor` end-to-end (CR-L7)
- Diagnostic→repair→corpus feedback pipeline running quarterly (CR-L8)

### §4.2 Aspirational v1.0 (not achievable by end-2026)

> *Humans never write code, the AI does it all, the pipeline self-heals at project scale across a distributed mesh, and MENS hosts itself.*

This is **not** achievable by end-2026 without scope cuts elsewhere. Requirements that the current corpus already places past v1.0:

- Full Phase 2 LAN mesh ([mesh SSOT](mesh-and-language-distribution-ssot-2026.md) targets v0.7 acceptance; "internet-facing personal mesh" is v1.0; "grand network" is v1.x).
- MENS GRPO closed loop with measured drift reduction quarter-over-quarter.
- Inference hosting via Vox itself (MENS Mn-T2 deferred).
- Mesh-replicated hopper (Option C from [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md), explicitly P6-T9).
- Project-scope repair with closed-loop test feedback driven by RL.

Each of the above is 1–3 months of focused work; sequenced, they push v1.0 of the aspirational shape into 2027+.

### §4.3 The most leverage-positive thing to do next

If we could close exactly one gap, **CR-L8 (diagnostic→repair→corpus feedback loop) returns more compound value than any other.** Reasoning:

- Closing CR-L1 (HumanEval-Vox) gives us a number; it does not change the number.
- Closing CR-L2 (on-distribution rate) gives us a number; it does not change the number.
- Closing CR-L8 changes *both numbers over time* by reducing the rate at which the same LLM mistake fires.

The plumbing exists. The trainer does not. This is the smallest viable RL loop — even an *advisory* feedback report (no automated retraining yet) lets humans see which rules are firing most and aim corpus curation accordingly. That single artifact would be worth more than three of the other CR-L items combined.

### §4.4 What we should claim — and what we should not

We should claim:
- Vox already enforces a richer LLM-target type/effect surface than any production language in 2026.
- Vox already ships an end-to-end `compiler-diagnostic → LLM-call → patch → re-check` loop that works on single files today.
- Vox's doctest, agent-discovery, and policy layering put it in the top tier of "AI-friendly" languages.

We should not yet claim:
- "Vox is self-healing at project scale" (CR-L3 measurement missing).
- "Vox's MENS model stays on-distribution" (CR-L2 measurement missing).
- "Humans don't need to write code in Vox" (V1 not yet measurable, much less achieved).

The vision is sound. The criteria need to match.

---

## §5 Proposed v1.0 Fidelity Criteria (CR-L1..CR-L8)

These are written for adoption into [`v1-release-criteria.md`](v1-release-criteria.md) §5 "LLM-Target Fidelity" — one-liner each there, full text here. Each criterion is:

- **Measurable** — produces a single number or boolean.
- **Reproducible** — a contributor can run the test locally.
- **Owner-pegged** — assigned to an existing phase plan, not a new track.

---

### CR-L1 — HumanEval-Vox pass rate ≥ 80%

**Statement.** A canonical 200-program benchmark suite (`contracts/eval/humaneval-vox/`) — drawn from `examples/golden/**/*.vox` plus mutated/incomplete variants — when given as prompts to MENS or a reference LLM, must produce compilable + test-passing solutions at ≥ 80%.

**Why this number.** State-of-the-art LLMs in 2026 score ~85% on Python HumanEval. Vox's richer type surface should make this easier, not harder, for a model that has seen the corpus. 80% is the threshold where "Vox is competitive as an LLM target" is honest.

**How to measure.** `vox audit humaneval` subcommand emits `contracts/reports/humaneval-vox/<date>.json`. Run on every minor release.

**Pegs to.** [vox-language-rules-phase2-lint-extension-2026.md](vox-language-rules-phase2-lint-extension-2026.md) Task 5 (`@example` decorator that is both doctest and corpus entry). The 200 programs are existing `@example` blocks plus mechanically mutated incomplete versions.

**Failure shape if missed.** Don't ship CR-L1 as a release gate yet; publish the measured number with the release notes and let the next minor cycle aim to close to 80%.

---

### CR-L2 — MENS on-distribution rate ≥ 95%

**Statement.** Of all MENS-emitted Vox programs produced during the eval pass, ≥ 95% must clear `vox check --strict` + the 47-rule vox-code-audit + retirement-guard with zero errors and zero high-confidence warnings.

**Why this number.** A model is "on-distribution" with the target grammar if it almost never emits patterns the language rejects. 95% is the threshold where review effort is bounded — at 80%, a human reviewer rejects 1 in 5 outputs and the orchestration model breaks.

**How to measure.** Wire the corpus aggregator's `external_review_replay.rs` to run the full lint suite on each MENS sample. Emit `contracts/reports/mens-on-distribution/<date>.json`.

**Pegs to.** [vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) Task 7 (idiom-fingerprint export). The same telemetry export channel can carry the on-distribution measurement.

---

### CR-L3 — `vox repair` project-scope success ≥ 70%

**Statement.** On a defined corpus of 50–100 multi-file broken Vox projects (`contracts/eval/repair-corpus/`), `vox repair .` must produce a passing project (compiles + tests green) at ≥ 70%.

**Why 70%, not 90%.** [CR-D2](v1-release-criteria.md) names 90%; the current MVP is single-file and unmeasured. 70% on multi-file is a more honest first gate. Single-file `vox repair` should aim ≥ 90% in parallel as a sub-metric.

**How to measure.** `vox audit repair-corpus` subcommand drives the loop on each fixture, records outcome, emits report.

**Pegs to.** Extension of [vox-cli/src/commands/repair.rs](crates/vox-cli/src/commands/repair.rs) — needs project-scope orchestration and a defined budget per file (currently 3 attempts; project scope should be configurable).

---

### CR-L4 — Plan-mode fidelity ≥ 85% on Wave-2 benchmark

**Statement.** Define "Wave 2" as a fixture set of 50–100 multi-step plans (`contracts/eval/plan-fidelity/`). Each plan has stated success criteria (e.g., "produces a passing PR"). Agent execution success rate must reach ≥ 85% (matching [CR-D1](v1-release-criteria.md)).

**How to measure.** `vox audit plan-fidelity` runs each plan through the orchestrator's plan-mode, records terminal state, emits report.

**Pegs to.** Extension of [vox-orchestrator-mcp/src/chat_tools/plan_loop.rs](crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs). Plan fixtures co-located with the existing orchestrator integration tests.

---

### CR-L5 — ACI envelope enforced by default

**Statement.** `OrchestratorConfig::agentos_aci_envelope_enabled` defaults to `true` starting in v0.6. Tools without classification metadata emit a deprecation warning; the guardrail kernel rejects unclassified mutations at v1.0.

**How to measure.** Boolean. `vox audit aci-default` returns the current default; CI gate fails if `false` post-v0.6.

**Pegs to.** [`agentos-ssot-2026.md`](agentos-ssot-2026.md) §5 — already specced, needs default flip + migration window.

---

### CR-L6 — Retirement guard parity with `AGENTS.md`

**Statement.** Every row in [`AGENTS.md` §Retired Surfaces](AGENTS.md) has either (a) a parse-time / typeck detector or (b) a `vox-arch-check` rule. CI fails if a row is added without enforcement, or if enforcement is removed without the row being deleted.

**How to measure.** Reverse-index: a generator reads `AGENTS.md` §Retired Surfaces, emits a contract file (`contracts/retirement/retired-surfaces.v1.yaml`), and `vox ci retirement-audit` asserts every row has a wired detector. Existing scaffolding: [vox-arch-check](crates/vox-arch-check/).

**Pegs to.** [vox-language-rules-phase2-lint-extension-2026.md](vox-language-rules-phase2-lint-extension-2026.md) detector framework. Each retired pattern becomes one new detector in the same file class as `id_at_boundary.rs`.

---

### CR-L7 — `vox new`, `vox deploy`, `vox doctor` end-to-end

**Statement.** All three CLI commands ship with structured (JSON-emit) output, `vox.deploy.*` and `vox.doctor.*` telemetry events, and a CI integration test that drives `vox new web → vox deploy → vox doctor` on a Marquee app fixture inside [CR-P3](v1-release-criteria.md)'s 120-second budget.

**How to measure.** Integration test pass/fail; telemetry events present; `cli-command-surface.generated.md` lists all three.

**Pegs to.** [phase1-build-targets-spec-2026.md](phase1-build-targets-spec-2026.md) for `vox new --kind` and `vox emit client`. Deploy CLI is a new task; spec it in a child plan referencing [vox-deploy-codegen](crates/vox-deploy-codegen/) crate.

---

### CR-L8 — Diagnostic→repair→corpus feedback loop instrumented

**Statement.** A quarterly pipeline export from `vox.lint.*` + `vox.repair.*` telemetry into vox-corpus runs in CI. Output artifact `contracts/reports/corpus-feedback/<quarter>.json` includes:
- Top-50 firing diagnostics
- Per-diagnostic autofix-applied rate
- Per-diagnostic autofix-rejected rate
- `vox repair` outcomes histogram (success/partial/abandoned)

This artifact informs the MENS training-corpus curator. No GRPO trainer is required for v1.0 — the *observability* loop is the gate. The trainer is a v1.x lift.

**How to measure.** Artifact existence + age check. CI fails if artifact is older than 90 days.

**Pegs to.** [vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) Task 7 (telemetry export). [vox-corpus/src/flywheel.rs](crates/vox-corpus/src/flywheel.rs) receives the output.

---

## §6 Sequencing — Pegging to Existing Phase Plans

CR-L items are pegged to existing plans rather than creating new tracks. Sequence by smallest-lift-first:

| Order | CR-L | Existing plan / file | Lift estimate | Why this order |
|---|---|---|---|---|
| 1 | CR-L5 (ACI default-on) | [agentos-ssot-2026.md](agentos-ssot-2026.md) §5 | ~1 week (config + migration shim) | Single line + deprecation warning; cheapest win, unblocks safety-signal claim. |
| 2 | CR-L6 (retirement-guard parity) | [vox-language-rules-phase2-lint-extension-2026.md](vox-language-rules-phase2-lint-extension-2026.md) detector framework | ~3 weeks (1 detector per retired pattern, ~10 rows) | Detector pattern already established; protects against doc drift like the `workflow` keyword example in §3.5. |
| 3 | CR-L8 (corpus-feedback observability) | [vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) Task 7 | ~4 weeks (telemetry pipeline + report generator) | Compound leverage on every other CR-L; ship before measuring CR-L1/L2/L3. |
| 4 | CR-L1 (HumanEval-Vox) | [vox-language-rules-phase2-lint-extension-2026.md](vox-language-rules-phase2-lint-extension-2026.md) Task 5 (`@example` decorator) | ~6 weeks (200 fixtures + eval harness) | Once `@example` is a doctest+corpus entry, the benchmark assembles mechanically. |
| 5 | CR-L2 (on-distribution rate) | Reuses CR-L8 telemetry channel + CR-L1 fixtures | ~2 weeks (additional run mode in eval harness) | Cheap given L1 and L8 are landed. |
| 6 | CR-L3 (project-scope repair) | Extend [vox-cli/src/commands/repair.rs](crates/vox-cli/src/commands/repair.rs) | ~8 weeks (multi-file orchestration + corpus + measurement) | Largest single-CR lift; depends on L8 for outcome telemetry. |
| 7 | CR-L4 (plan-mode fidelity) | Extend [vox-orchestrator-mcp/src/chat_tools/plan_loop.rs](crates/vox-orchestrator-mcp/src/chat_tools/plan_loop.rs) | ~6 weeks (fixtures + harness) | Can run in parallel with L3. |
| 8 | CR-L7 (deploy/doctor CLI) | New child plan referencing [phase1-build-targets-spec-2026.md](phase1-build-targets-spec-2026.md) | ~10 weeks (full deploy story + health-check) | Biggest scope; defer last because it touches infra contracts. |

**Aggregate.** ~30 weeks of focused work if serialized, ~14 weeks with two-track parallelism. v1.0 by end-2026 requires the parallel track from June onward. This is achievable; it is not free.

**Critical observation.** Three items (L1, L2, L8) depend on the same Phase 2 / Phase 4 plans already in flight. Two items (L5, L6) are small wins that ship in v0.6. Three items (L3, L4, L7) are the substantive new work. The shape of the lift is "small/small/medium/medium/medium/big/big/big" — front-loadable.

---

## §7 Open Questions for the Council

1. **Is the bar height right at "realistic v1.0"?** This doc proposes 80/95/70/85 thresholds. An aspirational framing would push these to 90/99/90/95 and slip v1.0. The author's recommendation is realistic; explicit call needed.

2. **Should CR-L items be append-only to [v1-release-criteria.md](v1-release-criteria.md), or replace CR-D1/D2/D3?** This doc proposes *append*: CR-D criteria stay as agentic-DX baseline, CR-L criteria are the LLM-target fidelity layer. Alternative: collapse CR-D into CR-L and drop the duplication.

3. **Does v1.0 include the mesh, or not?** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) targets v1.0 = "internet-facing personal mesh"; [`comprehensive-audit-v2-2026.md`](comprehensive-audit-v2-2026.md) recommends demoting mesh post-v1.0. This contradiction predates this doc; CR-L deliberately does not depend on mesh, but the contradiction itself should be resolved by the council.

4. **Should `AGENTS.md` §Grammar Unification claim be updated immediately?** As documented in §3.5, the claim that `workflow`/`activity` are "fully supported as of TASK-2.6 Path A" contradicts [`pipeline.rs:21`](crates/vox-compiler/src/pipeline.rs:21) which still rejects them with `E028`. This is itself an LLM-target footgun — an agent reading `AGENTS.md` will write code that does not compile. Recommend (a) corrective edit to `AGENTS.md` immediately, and (b) CR-L6's CI gate to prevent recurrence.

5. **Is "humans become orchestrators, not authors" a v1.0 marketing promise or a v1.x stretch?** This doc treats it as v1.x. If the council wants it as v1.0 marketing, the criteria need to grow teeth — a measured "human-edit ratio" gate on Marquee apps would be the honest shape.

6. **Where does the canonical eval corpus live?** This doc proposes `contracts/eval/` as a new directory (HumanEval-Vox, plan-fidelity, repair-corpus). Confirm placement before CR-L1 work begins.

---

## §8 Appendix: Where to Look Next

If you are reading this doc for the first time and want to verify the claims:

- **Compiler stages**: [`crates/vox-compiler/src/`](crates/vox-compiler/src/) (~38k LOC, 5 stages, 65 integration tests)
- **Diagnostic shape**: [`crates/vox-compiler/src/typeck/diagnostics.rs:96`](crates/vox-compiler/src/typeck/diagnostics.rs:96)
- **Effect enforcement**: [`crates/vox-compiler/src/typeck/effect_check.rs`](crates/vox-compiler/src/typeck/effect_check.rs)
- **ADR-028 (workflow/activity still reserved)**: [`crates/vox-compiler/src/pipeline.rs:21`](crates/vox-compiler/src/pipeline.rs:21), [`crates/vox-compiler/tests/tombstone_test.rs`](crates/vox-compiler/tests/tombstone_test.rs)
- **47 detectors**: [`crates/vox-code-audit/src/detectors/`](crates/vox-code-audit/src/detectors/)
- **Repair MVP**: [`crates/vox-cli/src/commands/repair.rs`](crates/vox-cli/src/commands/repair.rs)
- **MCP server tools**: [`crates/vox-orchestrator-mcp/src/lib.rs`](crates/vox-orchestrator-mcp/src/lib.rs)
- **Telemetry types**: [`crates/vox-telemetry/src/types.rs`](crates/vox-telemetry/src/types.rs)
- **Phase plans this doc pegs to**:
  - [`vox-language-rules-phase2-lint-extension-2026.md`](vox-language-rules-phase2-lint-extension-2026.md) — CR-L1, CR-L6 detector framework
  - [`vox-language-rules-phase4-runtime-monitors-2026.md`](vox-language-rules-phase4-runtime-monitors-2026.md) — CR-L8 telemetry export (Task 7)
  - [`agentos-ssot-2026.md`](agentos-ssot-2026.md) — CR-L5 envelope default
  - [`phase1-build-targets-spec-2026.md`](phase1-build-targets-spec-2026.md) — CR-L7 deploy CLI
  - [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) — orientation only; CR-L deliberately does not gate on mesh

**Sibling docs** that informed this audit and are worth reading together:
- [`vox-marquee-explainer-2026.md`](vox-marquee-explainer-2026.md) — vision source
- [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md) — primary policy plan
- [`comprehensive-audit-v2-2026.md`](comprehensive-audit-v2-2026.md) — broader system audit
- [`durability-runtime-audit-2026.md`](durability-runtime-audit-2026.md) — confirms `workflow`/`activity` parse-only
- [`agentos-ssot-2026.md`](agentos-ssot-2026.md) — ACI envelope contract
- [`unified-task-hopper-research-2026.md`](unified-task-hopper-research-2026.md) — task intake design space

---

## §9 Implementation-Readiness Gaps (Self-Critique, 2026-05-15)

The §1–§8 framing is honest about what ships and what's missing in the *codebase*. It is **not** honest about what's missing in this audit doc itself, taken as input to an implementation plan. This section closes that gap before any execution begins.

### §9.1 Tier-1 gaps — must close before implementation begins

| # | Gap | Why it blocks implementation |
|---|---|---|
| **G1** | **No end-to-end agent benchmark.** Every CR-L1..CR-L8 measures one sub-loop (generation, repair, planning, retirement, deploy CLI). None measure "agent + Vox produces a working app from a spec." | The v1.0 LLM-target *claim* is end-to-end; no criterion tests it. Resolved by adopting **[CR-L0]** (added 2026-05-15 to [`v1-release-criteria.md`](v1-release-criteria.md)). |
| **G2** | **CR-D ↔ CR-L overlap unresolved.** CR-D1 ↔ CR-L4 share an 85% number; CR-D2 ↔ CR-L3 share a 90% number; CR-D3 doesn't say what happens when CR-L7 adds new CLI commands. | Two CI gates measuring the same thing produce conflicting signals. Resolved by reconciliation notes added to CR-D1/D2/D3 on 2026-05-15. |
| **G3** | **"Marquee app" undefined.** CR-P1, CR-P3, CR-E2, CR-L0, CR-L7 reference Marquee as if a fixture; no `contracts/marquee/manifest.v1.yaml` exists. | Five criteria are unverifiable until this fixture lands. Added as the topmost note in [`v1-release-criteria.md`](v1-release-criteria.md) on 2026-05-15. |
| **G4** | **Fixture-corpus cost invisible.** CR-L1 (200 programs), CR-L3 (50–100 broken projects), CR-L4 (50–100 plans), CR-L0 (10–20 specs), CR-L7 (3–5 Marquee apps) = ~350 high-quality fixtures, ~3–6 person-months. Not in any phase plan. | Implementation timeline assumed these landed alongside the technical work. They won't. See [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md) §3. |
| **G5** | **No reference-LLM panel specified.** CR-L1 / CR-L2 / CR-L0 say "MENS or a reference LLM" — which one? At what temperature? With what context window? | Without a panel + median-of-panel rule, numbers can be tuned by picking favorable models. See implementation-plan §4 for the proposed panel. |
| **G6** | **No reproducibility gate on stochastic measurements.** `vox repair` runs at temperature 0.1; two CI runs on the same input produce different patches. | CR-L3's 70% becomes statistical fog without K-of-N runs or seed pinning. Proposed: temperature 0.0 + seed, or "majority-success over 5 attempts per fixture." |

### §9.2 Tier-2 gaps — must close before v1.0 ships, can defer past implementation start

| # | Gap | Disposition |
|---|---|---|
| **G7** | **No security gate.** `AGENTS.md` requires every `@endpoint` to carry `@auth(...)` or `@public`; no CR gates 100% coverage in generated apps. | Not adopted as CR-L9 per council scoping decision (2026-05-15). Tracked here as a v1.0 hardening item — should land as a `vox-code-audit` rule with CI denial in v0.6. |
| **G8** | **No LSP ↔ CLI diagnostic parity gate.** | Not adopted as CR-L10. Tracked under [`vox-lsp-capabilities-ssot-2026.md`](vox-lsp-capabilities-ssot-2026.md) — needs its own follow-on plan. |
| **G9** | **No emit-correctness gate.** Vox→TS→React smoke tests against rolling upstream. | Not adopted as CR-L11. Tracked under [`vox-react-backend-interop-audit-2026.md`](vox-react-backend-interop-audit-2026.md). |
| **G10** | **No latency budget for `vox check` / `vox repair`.** | Implicit under CR-E (Performance & Efficiency); should be sub-bulleted as `[CR-E4]` and `[CR-E5]` in a future revision of [`v1-release-criteria.md`](v1-release-criteria.md). |
| **G11** | **No cost ceiling per repair.** | Partially addressed in [CR-L0]'s $5/spec gate, but per-repair median should be sub-bulleted under CR-L3. |
| **G12** | **No compiler-as-library API stability.** Public surface of `vox-compiler` semver-stable from v1.0. | Tracked under [`workspace-dependency-audit-2026.md`](workspace-dependency-audit-2026.md) follow-on; not gated by CR-L. |

### §9.3 Tier-3 gaps — process, not technical

| # | Gap | Disposition |
|---|---|---|
| **G13** | **No owner per CR-L.** | Resolved in implementation plan §2 (DAG) with role-tagged owner slots. |
| **G14** | **No CR-L-specific risk register.** | Resolved in implementation plan §5. |
| **G15** | **No rollback / demotion story.** Demote to v1.1? Ship with lower bar? Block GA? | Resolved in implementation plan §6 with explicit per-CR-L policy. |
| **G16** | **No prioritization within v1.0 criteria.** CR-P/A/E/D/L treated as equal. | Implementation plan §1 adds a "must / strong-should / nice" partition. CR-L0 is "must"; CR-L1/L2 are "strong-should" (measurement preferred but bar negotiable); CR-L5/L6 are binary must. |
| **G17** | **No CI wiring contract for `vox audit <thing>`.** Eight differently-shaped subcommands risk being born. | Resolved in implementation plan §4 (single contract: `--json`/`--markdown`/`--html`, exit codes, `contracts/reports/<thing>/<date>.json`, telemetry namespace `vox.audit.<thing>`). |
| **G18** | **200-program HumanEval-Vox number is arbitrary.** | Proposed in implementation plan §3 to anchor to HumanEval-Python (164 problems) for direct comparability rather than 200. CR-L1 number updated accordingly when the plan lands. |

### §9.4 Internal contradictions resolved by 2026-05-15 edits

| # | Contradiction | Resolution |
|---|---|---|
| **C1** | "Approved April 2026" + "§5 added 2026-05-15" without council-process note. | Approval line updated to name §5 as pending review. |
| **C2** | CR-D2 (90% repair) vs CR-L3 (70% multi-file / 90% single-file aim). | CR-D2 amended to point at CR-L3 for measurement; 90% is the single-file aim, 70% is the project-scope gate. |
| **C3** | CR-D1 (85% Wave 2) vs CR-L4 (85% Wave 2 measured). | CR-D1 amended to point at CR-L4 for the fixture set and measurement harness. |
| **C4** | CR-D3 (100% subcommand .vox examples) vs CR-L7 (new commands). | CR-D3 amended to specify "new commands inherit CR-D3 at their landing release." |

### §9.5 Omissions still to address (not blockers, but worth surfacing)

The audit framing missed these dimensions entirely; they are not adopted as CR-L for v1.0 but should be tracked:

1. **No `vox fmt` output stability gate.** LLMs trained on old formatting produce noisy diffs across patch versions.
2. **No telemetry redaction policy for `vox.repair.*` events** carrying source code; `@secret`-tagged fields need explicit redaction.
3. **No agent-attribution / VCS-trailer gate.** The agentic-VCS phase plans promise `Co-Authored-By: <model>` and `Vox-Model-Id` trailers; nothing requires generated commits to carry them at v1.0.
4. **No corpus-contamination guard for CR-L1.** If MENS was trained on `examples/golden/`, HumanEval-Vox built from that corpus is leaked. Need a held-out fixture set MENS provably never saw (see [`mens-training-ssot.md`](mens-training-ssot.md) for the training-eligibility flag — a CI check should assert held-out fixtures all have `training_eligible: false`).
5. **No graceful degradation when MENS / OpenRouter unreachable.** `vox repair` requires `VOX_OPENROUTER_API_KEY`; offline operation degrades silently to "no repair." A rule-based autofix fallback is in [`vox-language-rules-phase2-lint-extension-2026.md`](vox-language-rules-phase2-lint-extension-2026.md) but not v1.0-gated.
6. **No multi-language coexistence story.** Vox-emitted TS lives next to user-written TS; lint rules, source maps, type imports across the boundary aren't covered.

### §9.6 The honest summary

This audit, as originally written, *describes the LLM-target ambition well* and *identifies the right shipped/gap split*. It **does not** survive contact with implementation planning unaltered:

- It under-specifies measurement (no reference LLM, no reproducibility, no fixture cost).
- It under-specifies process (no owners, no risks, no rollback).
- It under-resolves overlap with the prior CR-D criteria.
- It misses the *integration* test (CR-L0) that every other CR-L is a piece of.

The companion [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md) addresses each Tier-1 and Tier-3 gap concretely. Tier-2 gaps are tracked under their own existing plans, not duplicated. Omissions are surfaced for follow-on.

---

*Audit dated 2026-05-15. Self-critique §9 added 2026-05-15. Next review: at v0.6 release or 2026-08-15, whichever is sooner.*
