---
title: "Vox Language Rules & Enforcement — Top-Level Plan (2026-05-09)"
description: "Five-phase plan to close the gap between Vox's stated language-design philosophy (LANGUAGE_DESIGN_PRIORITIES.md P0–P5, C1–C5) and machine-checkable enforcement. Absorbs a 73-item cross-language audit (Rust↔Vox AI-rules interplay) into Vox-only sequenced work. Optimizes for Vox as a large-language-model destination target: stable diagnostic IDs, generated-hash codegen provenance, single-source-of-truth across the Rust↔Vox seam, runtime fuel/panic-trap monitors, and an effect system that makes wrong programs structurally unrepresentable."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Canonical sequencing for Vox-language enforcement work; downstream phase plans (phase1..phase5) reference this document as parent."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-code-audit: gains LLM-call, secret-shape, ?-operator, doc-citation, decorator-position, and 8+ other detectors"
  - "vox-compiler: gains Id[T]-required-at-boundaries, named-error-type, closed-keyword-table, and effect-row foundations"
  - "vox-grammar-export, vox-codegen, vox-actor-runtime/builtins: collapse into single SSOT with xtask-driven generation"
  - "vox-eval: per-call fuel, alloc observer, stack-depth cap, panic-trap, capability-violation runtime trap"
  - "vox-capability-registry: promoted from advisory crate to language requirement on public fns"
  - "vox-bounded-fs: gains source-side @uses(fs(read:...)) glob declarations checked at compile time"
  - "AGENTS.md: every MUST gets a CI gate; new clauses for closed keyword table, generated-hash provenance, ADR citation in pub fn docs"
---

# Vox Language Rules & Enforcement — Top-Level Plan

> **Companion phase plans:**
> - [Phase 1 — SSOT collapse (xtask-gen the Rust↔Vox seam)](vox-language-rules-phase1-ssot-collapse-2026.md)
> - [Phase 2 — `vox-code-audit` extension with stable diagnostic IDs and serializable autofixes](vox-language-rules-phase2-lint-extension-2026.md)
> - [Phase 3 — Cheap typechecker rules ("wrong programs unrepresentable")](vox-language-rules-phase3-typecheck-rules-2026.md)
> - [Phase 4 — Runtime monitors (Rust-only domain)](vox-language-rules-phase4-runtime-monitors-2026.md)
> - [Phase 5 — Effect system + workflow determinism](vox-language-rules-phase5-effects-determinism-2026.md)
>
> **Language platform orientation (2026):** [Compiler pipeline map](vox-compiler-architecture-research-2026.md), [LSP capability matrix](vox-lsp-capabilities-ssot-2026.md), [Diagnostic UX taxonomy](vox-diagnostic-ux-ssot-2026.md).
>
> **Source audit:** A 73-item Rust↔Vox AI-rules interplay audit, originally generated against a sister project (FableForge) and re-scoped to Vox-only. The audit's item numbers are referenced inline as `[A.NN]` so a reader can trace any plan task back to the original observation.

---

## Why this plan exists

Vox's [`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md) P0–P5 + C1–C5 already encode the *philosophy* of strict, LLM-friendly language design — stronger than any project-level convention because the priorities operate at language-design time. But three gaps separate philosophy from enforcement today:

1. **Many policies are advisory prose, not gates.** The bare-keyword vs decorator rule ([AGENTS.md:131–164](../../../AGENTS.md)), the VoxScript-first glue rule ([AGENTS.md:100–129](../../../AGENTS.md)), `vox_secrets::resolve_secret` SSOT ([AGENTS.md:73–93](../../../AGENTS.md)), and the cryptography ban-list ([AGENTS.md:95–98](../../../AGENTS.md)) are all enforced only by reviewer attention. There is no compiler-side rejection for `env.get("OPENAI_KEY")` in Vox source, nor for direct `std.http.post_json` to LLM provider hostnames.
2. **The Rust↔Vox seam is hand-mirrored in 3+ places.** [`builtins/mod.rs`](../../../crates/vox-actor-runtime/src/builtins/mod.rs) is the SSOT for host functions on the Rust side, but the typechecker, the LSP completions, the [`mens/config/system_prompt.txt`](../../../mens/config/system_prompt.txt), and the docs page each maintain a parallel mirror. Drift is constant.
3. **No per-call resource caps, no panic-trap boundary, no machine-checked effect surface.** `vox-capability-registry` exists as a crate; `vox-bounded-fs` exists; but neither participates in source-level type checking, and `vox eval` has no fuel mechanism — a runaway script can hang CI.

This plan closes those gaps in five sequenced phases, each independently shippable.

---

## Design principles for this plan

These are the cross-cutting decisions that the per-phase plans inherit.

### 1. Diagnostic IDs are stable and machine-readable

Every new lint, type rule, and runtime trap ships with a stable ID in the namespace `vox/<category>/<kebab-name>`. Examples: `vox/effect/unjustified-net`, `vox/llm/direct-provider-call`, `vox/secret/env-get-shape`, `vox/runtime/fuel-exhausted`.

- IDs are *append-only*. Renaming a diagnostic requires a deprecation alias kept for two minor versions.
- Every diagnostic has an `explain` page reachable via `vox check --explain <id>` *and* a stable URL `voxlang.org/diag/<id>`.
- Every diagnostic carries a `since:` version field and (where applicable) an `adr:` field pointing to the deciding ADR.
- Catalog source: `crates/vox-code-audit/src/diagnostics/catalog.rs` — a single Rust enum with `#[diagnostic(...)]` attrs (Phase 2 Task 1). Generates the docs page, the `--explain` data, the LSP code-action map, and the Mens training-data schema.

**Why stability matters for LLM-target work:** A model trained on Vox 0.5 must still recognize Vox 0.7 errors. Renames break learned associations. Append-only with deprecation aliases preserves recall.

### 2. Single SSOT, generated outputs

Where two artifacts describe the same fact, generate one from the other. Every generated output carries:

```
// @generated from <source>:<line> at commit <hash>
// @generated-hash <blake3 of body>
```

CI rule: if the body's blake3 doesn't match the header, the file was hand-edited → reject. (Phase 1 Task 9.) This protects every codegen output Vox ships — `cli-command-surface.generated.md`, TypeScript output from `vox-codegen`, the system-prompt sections, the typechecker builtin manifest.

### 3. LLM-friendly diagnostic shape

Every diagnostic emitted by `vox check` (under `--json` or `--for-llm`) carries:

- `id`: stable ID
- `severity`: `error` | `warning` | `note`
- `span`: file + byte range + line/col (so an LLM can quote the line back)
- `excerpt`: 3 lines of context above + the offending line + 3 lines below
- `message`: human-readable
- `rationale`: 1-paragraph "why this rule exists" (constant per ID, not per occurrence)
- `suggested_fix`: serializable `LintFix` descriptor (Phase 2 Task 3) — `Replace { range, new_text }`, `InsertBefore { offset, text }`, `RemoveDecorator { name }`, etc.
- `confidence`: `certain` | `likely` | `speculative` (Phase 2 Task 4)
- `alternatives`: list of other plausible fixes if `confidence < certain`
- `adr`: optional ADR reference

The `--for-llm` mode additionally includes a *minimal repro* (smallest excerpt that reproduces the diagnostic alone). This is the single biggest delta between Vox-as-LLM-target and a typical compiler.

### 4. Warning → error transition is the default

New rules ship as warnings for ≥ one minor version, then escalate to errors. Each phase's tasks specify the target severity at land time and the escalation milestone. This avoids breaking corpus and downstream tools while still moving the line.

Exception: rules that fix *security* surfaces (`vox/secret/env-get-shape`, `vox/llm/direct-provider-call`, `vox/crypto/banned-crate`) ship as errors immediately, with a structured suppression list (`contracts/toestub/suppressions.v1.json`) for known-good exceptions.

### 5. Test-first for every new rule

Per [AGENTS.md:182–202 (Test-First Policy)](../../../AGENTS.md), every new detector lands with:

1. A failing test in `crates/vox-code-audit/tests/golden/` — input `.vox` file plus `.expected.json` diagnostic snapshot.
2. The detector implementation.
3. The autofix descriptor + a round-trip test (apply the fix, re-run the detector, expect zero diagnostics).
4. A docs entry under `docs/src/reference/diagnostics/<id>.md` (generated section + hand-written rationale prose).
5. A negative example added to `examples/golden/anti/` with `error_kind:` and `expected_diagnostic:` frontmatter (Phase 2 Task 11).

### 6. Closed keyword table

The lexer's bare-keyword set is *closed* as of Phase 1 Task 12. New bare keywords require:

- An ADR with `keyword:` frontmatter listing the new keyword and the *single* concept it expresses.
- An `xtask add-keyword` invocation that updates the lexer table, the grammar export, the typechecker, and the system prompt — all from the ADR.
- A reviewer sign-off from a maintainer listed in `MAINTAINERS.md` `language-design` group.

This is the structural enforcement of [AGENTS.md:154–156](../../../AGENTS.md): "Do NOT introduce a new bare keyword for behavior that can be expressed as a decorator."

### 7. LLM-corpus feedback loop

Every detector emission, every autofix application, and every `vox check --explain` invocation is observable via `vox.lint.*` telemetry. Phase 4 Task 7 ships a periodic export to the Mens corpus pipeline so:

- Frequently-fired rules surface in the next training cycle.
- Rules with high autofix-rejection rates get reviewed for false-positive shape.
- The "Vox-distinctive idiom adoption rate" (item [A.62]) becomes a tracked metric.

This is what makes the language *learn from* its LLM users, not just emit code for them.

---

## Phase summary

| # | Phase | Scope | Audit items absorbed | Crates touched | Effort | Gate |
|---|---|---|---|---|---|---|
| 1 | SSOT collapse | xtask-gen builtin manifest, decorator catalog, grammar tables, LSP completions, system prompt, docs from one source. Generated-hash provenance on every emission. | 31, 32, 33, 34, 35, 36, 38, 39, 40, 41 | `vox-actor-runtime`, `vox-grammar-export`, `vox-codegen`, `vox-lsp`, `mens` (config), `xtask` (new gen subcmds) | ~3 weeks | Drift check in CI |
| 2 | Lint extension | `vox-code-audit` gains 14 new detectors with stable IDs, serializable autofixes, `--for-llm` JSON mode, `--explain` page generation, alternatives + confidence. | 5, 6, 14, 15, 16, 17, 19, 20, 22, 27, 28, 29, 30 | `vox-code-audit`, docs/src/reference/diagnostics/, `examples/golden/anti/` | ~3 weeks | New detectors → warning by default; security ones → error; CI drift check |
| 3 | Cheap typechecker rules | `Id[T]` required at API boundaries, named error types, `@deprecated` machine-checked, single workspace `syntax_version` enforced, `training_eligible` propagation, decorator-position parser policy. | 8, 9, 10, 25 (warning), 56, 61 | `vox-compiler` (typechecker module), `vox-corpus` (frontmatter validator) | ~4 weeks | Warning for one release, then error |
| 4 | Runtime monitors | Per-call fuel, alloc observer, stack-depth cap, panic-trap, runtime redactor, capability-violation runtime trap, telemetry export, provenance ledger, per-call eval sandbox. | 42, 43, 44, 45, 46, 48, 50, 51 | `vox-eval`, `vox-capability-registry`, `vox-codegen` (provenance), `vox-bounded-fs` | ~3 weeks | New CLI flags default-on in CI, default-off for end-user `vox run` (with `--strict` opt-in) |
| 5 | Effect system + workflow determinism | `@uses(...)` declarations on public fns proved by closure; `@pure` proved transitively; workflow body forbids non-deterministic builtins; `@uses(fs(read:...))` glob declarations checked against literal paths; closed keyword table enforcement; `actor`/`workflow`/`activity` effect-set rules. | 1, 2, 3, 7 (closed keyword table), 12, 60 | `vox-compiler` (effect inference + checker), `vox-actor-runtime` (effect annotations on builtins), `vox-capability-registry` (consumed by checker) | Multi-quarter; ship as warning per-effect, escalate one at a time | First effect (`net`) ships warning → error transition over two minor versions |

**Total committed work:** Phases 1–4, ~13 engineering weeks. Phase 5 is a multi-quarter effort whose Task 1 (the warning-mode `net`-effect inference) is sized into a child plan; later effects (`fs`, `time`, `random`, `secret`) follow the same shape.

---

## Out of scope (explicitly deferred or rejected)

The following audit items are *not* in the plan. Each is listed with rationale.

### Deferred to "Phase 6+ research" appendix

- **`@no_panic` decorator with reachability proof [A.4].** Research-tier in any language; needs flow-sensitive analysis Vox doesn't have. Descope to a Phase 2 lint that bans known-panicking builtins in actor message handler bodies (`vox/handler/panicking-builtin`).
- **`@secret` taint type system [A.11, full version].** A proper effect-row taint type is a multi-quarter compiler project. Ship the runtime redactor in Phase 4 (cheap, defense-in-depth) and a Phase 2 lint that flags struct fields named `*secret*|*token*|*key*` appearing as `tracing` span attributes (`vox/secret/leaked-to-span`). Defer the static taint type to a future phase.
- **Workflow journal verifier as a CLI [A.47].** Belongs in `vox-orchestrator` work, not language-rules work. ADR-019 already commits to it; cross-reference in Phase 4 Task 6 but do not build here.
- **State-machine totality proof [A.54].** Compiler proof of "all states have transitions, no unreachable states" is research-tier. Ship as a Phase 2 lint (`vox/state-machine/unreachable-state`) — best-effort, not proof.
- **`@auth(role=...)` proven on every endpoint [A.58].** Requires the full effect system. After Phase 5 Task 4 lands the `auth` effect, this becomes a corollary; until then, ship as a Phase 2 lint.

### Rejected outright

- **One-primitive-per-concept as a hard error [A.13].** Too subjective for compiler rejection. Keep as a Phase 2 lint with autofix.
- **Python-shape detector via classifier [A.18].** Corpus-side concern, not language-rule. Routes to `vox-corpus` curation review, not the compiler. Mention in Phase 2 §"Future work" only.
- **CodeRabbit tier wiring, break-budget counter, canary corpus [A.65, A.66, A.67].** Process and infra items, not language rules. Routes to a separate `vox-engineering-process-2026.md` doc (out of scope for this plan series).

### Anti-recommendations honored verbatim from the audit

These are *kept out* of Vox by deliberate decision. The phase plans cite them as guardrails when relevant:

- **No T-number citation scheme [A.69].** Vox uses `ADR-NNN`, `Phase N`, and `TASK-N.M`. Phase 2 Task 7 (`vox/doc/missing-adr-citation`) enforces only the ADR/TASK forms; the lint actively *rejects* `T\d+` references in `///` doc as a corpus drift signal.
- **No separate lint server [A.70].** `vox check` lives in `vox-code-audit`, in-process with the compiler. No "lint server" microservice. Phase 2 architecture explicitly forbids this.
- **No `safeJsonParseStrict`-style boundary helpers [A.71].** The Vox answer is `from_json[T]` returning `Result[T, ParseError]` with the type name as evidence. Phase 3 Task 4 (named error types) enforces this shape.
- **No T0–T4 type tier system [A.72].** Vox doesn't allow opting out of types. Confused inference → fix the generic.
- **No render-path-specific rules [A.73].** When reactive components land (ADR-032), the right answer is the Phase 5 effect system catching `time`/`random` in render closures categorically.

---

## Sequencing rationale

The five phases are ordered by *enabling power* — earlier phases make later phases cheaper, not the reverse.

- **Phase 1 first** because every later phase consumes the SSOT-generated outputs (the typechecker manifest from Phase 1 Task 1 is a Phase 3 dependency; the diagnostic catalog scaffolding from Phase 1 Task 8 is a Phase 2 dependency).
- **Phase 2 second** because lint rules with stable IDs and serializable autofixes establish the diagnostic shape that Phase 3 (typechecker) and Phase 4 (runtime traps) reuse.
- **Phase 3 third** because the typechecker rules depend on the diagnostic catalog being ready and on `Id[T]` being a stable surface across the compiler.
- **Phase 4 fourth** because runtime monitors must classify trap reasons using Phase 2 + 3 diagnostic IDs.
- **Phase 5 last** because the effect system needs everything below: catalog, autofix shape, typechecker integration points, runtime traps as fallback.

Phases 1 and 2 can be parallelized across two engineers; Phase 4 can begin as soon as Phase 1 Task 8 (diagnostic catalog scaffolding) merges, in parallel with Phase 2.

---

## LLM-target strengthening highlights

Beyond the audit's items, these are creative additions specifically aimed at making Vox a better LLM-completion target. Each is sized into a phase as noted.

| Addition | Why it helps LLMs | Phase |
|---|---|---|
| `vox check --for-llm` JSON mode with minimal repro per diagnostic | LLM agents working on a fix can quote the smallest reproducer back into a follow-up prompt without scrolling through unrelated code. | 2 (Task 5) |
| Diagnostic IDs are append-only with deprecation aliases | Models trained on Vox 0.5 still recognize Vox 0.7 errors. | 2 (Task 1, design rule) |
| `confidence` + `alternatives` on every diagnostic suggestion | LLMs know when to trust auto-fix vs. ask the user. Reduces over-confident bad fixes. | 2 (Task 4) |
| `// @generated-hash` on every codegen output | Models learn to never edit generated files; CI rejection makes the rule self-teaching. | 1 (Task 9) |
| `@example` decorator that becomes both a doctest and a Mens corpus entry | Kills doc/test/corpus drift; LLMs see one example, not three slightly-different copies. | 2 (Task 13) |
| Single-token decorator forms preferred (`@pure` not `@modifier(pure)`) | Fewer token-boundary errors during LLM completion; smaller surface to memorize. | 2 (Task 12 — decorator-position lint suggests collapse) |
| Idiom fingerprints emitted as `vox.idiom.*` telemetry | Lets the team measure "% of accepted code uses Vox-distinctive forms vs Python-shaped fallbacks" — feedback loop for corpus quality. | 4 (Task 7) |
| `vox check --rationale-required` mode for CI overrides | Every `// vox:skip` or suppression must carry a structured `reason:` field; emitted as JSON for review tools. | 2 (Task 14) |
| Symmetric error/fix pairs in diagnostic IDs | `vox/effect/missing-net-decl` ↔ `vox/effect/unjustified-net` — LLMs learn the inverse rule at training time. | 5 (Task 2 design) |
| `vox playground` deterministic seed mode | Fixed time/random seeds for examples; LLM-generated tests don't have intermittent failures. | 4 (Task 8) |
| Negative-example corpus auto-built from compiler test fixtures | Every parser/typeck test that asserts a diagnostic becomes a Mens negative example automatically. | 2 (Task 11) |
| Provenance tokens in error messages: `[since 0.6.0, ADR-024]` | LLMs can pattern-match version cohorts and pick the right fix for the user's Vox version. | 2 (Task 1) |

---

## Cross-cutting acceptance criteria

A phase is "done" when:

1. All tasks in the phase plan have a landed PR with passing CI.
2. The phase's new AGENTS.md clauses (if any) have been merged with backlinks to the phase plan.
3. `where-things-live.md` is updated for every new module/file added.
4. Every new detector has a `docs/src/reference/diagnostics/<id>.md` page.
5. The Mens corpus pipeline has consumed the new diagnostic catalog and produced a training-data delta report (`docs/src/reference/mens-corpus-deltas/<phase>-<date>.md`).
6. The phase's `vox.lint.*` / `vox.runtime.*` telemetry events are documented in [`docs/src/architecture/telemetry-trust-ssot.md`](telemetry-trust-ssot.md).
7. A retrospective note at the bottom of the phase plan records actual vs estimated effort, scope changes, and lessons.

---

## Risks and mitigations

| Risk | Probability | Mitigation |
|---|---|---|
| Generated-hash CI gate (Phase 1 Task 9) fires on legitimate hand-edits during initial migration | High in week 1 | Land the gate as warning-only for one minor version; switch to error in the next. Document the regen command in the failure message. |
| Phase 2 detector PRs cause noisy CI on existing corpus | Medium | Each detector lands with a "burn-down PR" that fixes existing violations *before* the detector escalates from `note` to `warning`. |
| Phase 3 `Id[T]` requirement breaks downstream `.vox` files in user projects | Medium | Ship `vox migrate id-strings` codemod alongside the typechecker rule. Soft-warn for one minor version. |
| Phase 4 fuel cap defaults are wrong, causing false-positive trap in CI | Low | Default fuel is `None` for end-user `vox run`; CI sets a generous default (10M steps) that can be raised per-test via `vox run --fuel <N>`. |
| Phase 5 effect inference is too permissive (allows impure calls in `@pure` fns) or too restrictive (rejects legitimate code) | High | Ship as warning-only for two minor versions; collect false-positive reports via `vox check --report-false-positive`; iterate. |
| Audit items absorbed into multiple phases drift in scope between phases | Medium | This top-level plan is the SSOT for "which audit item lives in which phase"; phase plans cross-reference back here. |

---

## Anti-goals

This plan does *not*:

- Add new Vox language constructs (no new keywords, no new decorator categories beyond what already exists).
- Change the existing `vox` CLI surface in incompatible ways (new flags ok; renames not).
- Touch the Mens training pipeline beyond consuming the new diagnostic catalog and corpus deltas.
- Introduce new runtime dependencies on the host (no new C libs, no `cmake`/`nasm` chains — see [AGENTS.md:95–98](../../../AGENTS.md)).
- Build any "lint server" or microservice ([A.70] explicitly forbids this).

---

## Tracking

- **Parent issue:** TBD (open after this plan lands).
- **Phase issues:** one per phase, opened in sequence; each phase's tasks become checkboxes in its issue.
- **Status field:** updated in this document's frontmatter (`status: roadmap` → `in-progress` → `shipped`) and at the top of each phase plan.
- **Retrospective cadence:** after each phase ships, append a §Retrospective to that phase's plan within 5 working days.

---

## See also

- [`LANGUAGE_DESIGN_PRIORITIES.md`](../../../LANGUAGE_DESIGN_PRIORITIES.md) — P0–P5, C1–C5; the philosophy this plan grounds in enforcement.
- [`AGENTS.md`](../../../AGENTS.md) — current advisory surface; many sections gain "enforced by" backlinks after Phase 2.
- [`docs/src/architecture/where-things-live.md`](where-things-live.md) — concept-to-crate lookup; updated in every phase.
- [`docs/src/architecture/layers.toml`](layers.toml) — layer assignments; `vox-arch-check` enforces.
- [`docs/src/adr/`](../adr/) — ADRs cited by Phase 2 Task 7's doc-citation lint.
- [`docs/src/architecture/agentic-vcs-automation-impl-plan-phase1-2026.md`](agentic-vcs-automation-impl-plan-phase1-2026.md) — format model for the per-phase plans.
