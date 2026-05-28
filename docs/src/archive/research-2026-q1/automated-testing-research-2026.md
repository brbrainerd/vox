---
title: "Automated Testing Research for the Vox Language"
description: "State of the art, implications, and roadmap for automated test generation including PBT, mutation testing, and LLM-driven synthesis."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-04"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Automated Testing Research for the Vox Language
## State of the Art, Implications, and Roadmap (2026)

> **Status:** Research Document — April 2026  
> **Author:** Bert Brainerd
> **Related:** `vox-test-harness`, `vox-eval`, `vox-integration-tests`, `vox-skills`, `vox-compiler`, `vox-lsp`  
> **Canonical path:** `docs/src/architecture/automated-testing-research-2026.md`

---

## 1. Executive Summary

This document answers two questions:

1. **Is automated test generation for the Vox language possible and desirable?** — Yes on both counts, with meaningful nuance.
2. **What does the state of the art tell us about how to do it well?** — The field has converged on a layered model: language-native test syntax → property/fuzz testing → LLM-guided generation → feedback-driven self-healing within sandboxed execution, all governed by strict budget and safety guardrails.

Vox is in a uniquely strong position to pursue this because it already has a compiler pipeline, a WASI/sandbox backend in its greenfield architecture, a skills system (`vox-skills`) for tool orchestration, an existing `vox-test-harness` crate, and a native AI stack (`vox-populi`). The question is not *whether* to build this, but *which layers to build in which order* to avoid overengineering.

archived_date: 2026-04-18
---

## 2. What the World Has Built: State of the Art Survey

### 2.1 Language-Native Test Frameworks (The Baseline)

Modern compiled languages treat testing as a **first-class citizen of the toolchain**, not an afterthought. The lessons:

| Language | Model | Key Insight |
|---|---|---|
| **Rust** | `#[test]`, `#[cfg(test)]`, `cargo test`, doctests from `///` comments | Tests live adjacent to code; documentation and tests unified via doctests |
| **Go** | `_test.go` files, `go test`, `Example` functions as live docs | Convention over configuration; table-driven tests are idiomatic |
| **Swift** | `@Test` and `@Suite` macros (2024), `#expect()` with rich diagnostics | Macros eliminate boilerplate; failure messages capture full expression context |
| **Zig** | `test` keyword inline, `comptime` assertions at compile time | `comptime` blurs the compile/run boundary; zero-overhead inline tests |
| **Python** | `doctest` (stdlib), `pytest`, Hypothesis for PBT | Doctests as living documentation; PBT via Hypothesis is the most mature implementation |

**Key takeaway:** All top-tier languages embed testing at the *language and toolchain level*, not as a library plugin. This creates the **zero-friction baseline** for subsequent AI-driven test generation to build on.

---

### 2.2 Property-Based Testing (PBT) and Fuzzing

Rather than specifying exact input/output pairs, PBT generates thousands of random inputs and verifies mathematical *properties* hold across all of them.

**Tools ecosystem:**
- **Haskell QuickCheck** — the original; simple type-driven generation
- **Python Hypothesis** — mature, with complex strategy composition and best-in-class shrinking
- **Rust `proptest`** — strategy-based, superior input shrinking (preferred recommendation, 2025)
- **Rust `quickcheck`** — simpler, type-based; lower barrier to entry
- **Coverage-guided fuzzing** — `libFuzzer`, `AFL`, `cargo-fuzz`; finds crash inputs via instrumented feedback loops

**The shrinking model:** When PBT finds a counterexample, it *shrinks* it to the minimal failing case. `proptest`'s integrated shrinking significantly outperforms type-based shrinking for complex data structures — critical for a compiler's AST types.

**Key insight for Vox:** PBT is particularly valuable for compiler and language runtime testing — precisely Vox's domain. Generating random Vox programs and asserting:
- "The compiler does not panic"
- "Lowering is idempotent (`lower(lower(ast)) == lower(ast)`)"
- "The type checker accepts all syntactically valid programs that match the grammar"

...are all natural property-based targets that would catch real bugs.

archived_date: 2026-04-18
---

### 2.3 Mutation Testing

Mutation testing asks { *"Do my tests actually catch bugs?"* It works by:
1. Introducing synthetic bugs ("mutants") — swapping `+` for `-`, changing `if` conditions, removing return values
2. Running the full test suite against each mutant
3. Reporting "surviving mutants" (mutants the tests didn't detect) as quality gaps

**Tools:** Stryker (JS/TS/.NET), PITest (JVM), Diffblue (AI-assisted, Java)

**Status (2025–2026):**
- Computationally expensive (O(n×m) test executions for n tests and m mutants)
- Not suitable as a per-commit CI gate for large codebases
- Recommended pattern: **run asynchronously/nightly on changed files only** (selective mutation)
- Emerging: **LLM-guided mutation** — Meta's ACH system (Automated Compliance Hardening, 2025) prompted LLMs to write tests *specifically targeting each mutant*, pushing mutation scores from ~80% to ~95%
- **LLM-as-a-judge** to filter equivalent mutants (syntactically different but semantically identical) — eliminating the "equivalent mutant" false alarm problem

**Key takeaway for Vox:** Code coverage is a vanity metric; mutation score is the quality metric. Apply mutation testing to the Vox compiler's most critical subsystems (HIR lowerer, type checker, codegen). This is a natural `vox ci` command: `vox ci mutation-score --path crates/vox-compiler`.

---

### 2.4 LLM-Based Automatic Test Generation

The most active research area in software engineering (2025). The converged best-practice pipeline:

```
[Source Code + Spec/Docs]
    → LLM generates initial test suite
    → Compilation check (static analysis)
    → Execution in isolated sandbox
    → Mutation analysis → identify surviving mutants
    → Feed: {failures + surviving mutants + coverage gaps} → LLM
    → LLM refines and extends test suite
    → Repeat until quality threshold met
    → Human review before merge
```

**Notable industrial systems:**
- **GitHub Copilot / Cursor / Claude Code** — IDE-integrated; generate tests on-demand from context menus and chat
- **Qodo (formerly Codium)** — analyzes code structure, generates edge cases across Python/JS/TS/Java
- **Cover-Agent** (open-source) — iteratively increases test coverage via LLM + execution feedback
- **Mutahunter** — extends LLM generation with a mutation testing validation loop
- **Diffblue Cover** — RL-based (no LLM prompts needed) autonomous JUnit test writing; maintains tests as code changes
- **Mabl / Testim / QA Wolf** — "agentic" end-to-end test platforms with self-healing locators

**The test oracle problem (the hardest unsolved issue):**
For any given input, the oracle must determine whether the output is *correct*. LLMs address this via:
- **Documentation-derived oracles** — infer assertions from Javadocs, docstrings, type signatures
- **Metamorphic testing** — relative correctness between related inputs (`sort(sort(x)) == sort(x)`) avoids needing an absolute oracle
- **LLM-as-judge** — a second LLM pass evaluates whether generated test assertions capture meaningful behavior
- **Formal spec oracles** — preconditions/postconditions (`@spec`) used as generation hints

**Known failure modes:**
- **Hallucinated tests** — syntactically valid, passing, but asserting nothing meaningful
- **False positives / flaky tests** — brittle assertions on non-deterministic outputs erode CI trust
- **Semantic weakness** — 100% line coverage with 0% mutation score
- **Context blindness** — LLMs miss domain-specific business invariants; providing full CUT (Class Under Test) consistently outperforms providing only the MUT (Method Under Test)
- **Hallucination rates fluctuate by task** — are not a fixed property of a model; depend on prompt quality and task complexity

**Research findings (AIware 2025):** Providing the **Class Under Test** (full context) -> the LLM when generating oracles improves accuracy significantly over providing only the method signature. Context engineering matters more than raw model scale.

archived_date: 2026-04-18
---

### 2.5 Formal Verification and Design by Contract

**Design by Contract (DbC):**
- Preconditions, postconditions, class invariants embedded in function/type signatures
- Eiffel is the canonical language; `debug_assert!` in Rust is the lightweight industrial approximation
- Runtime enforced (detection, not prevention); violations terminate the program
- Maintenance burden is the primary objection in practice

**Formal Verification (2025 state):**
- Dafny, F\*, Lean, Verus (Rust), Isabelle, Coq
- SMT solvers (Z3) automate much of the proof work
- **"Vericoding" trend (2025–2026):** LLMs generate formally verified code — they write the most difficult part (loop invariants, proof annotations) — making formal verification accessible beyond specialists
- FM 2026 (Formal Methods conference) TAP track formally unifies the dynamic testing and static proof communities
- Consensus: **formal verification handles the 80% of requirements that are mathematically definable; testing handles the rest**

**Refinement types:**
- LiquidHaskell, F\* allow constraints like `v : Vec<i32> where v.len() > 0` at the type level
- Eliminates entire classes of unit tests by making violations compile-time errors
- Relevant precedent for Vox's non-null safety philosophy (already implemented)

**Key takeaway for Vox:** The Vox type system's `Result[T, E]` bivariance and strict non-null policy are early steps toward refinement types. A long-horizon goal is adding lightweight postconditions (`@spec(ensures: ...)`) that `vox-compiler` enforces in debug mode. This is the correct foundation for AI oracle generation.

---

### 2.6 Sandbox Execution for AI-Generated Code

Running AI-generated code safely is a mandatory architectural constraint, not an optional optimization.

**WASM/WASI sandboxing (2025–2026 consensus):**
- **Security by construction** — no host access unless explicitly granted; opposite of Docker's shared kernel
- Sub-millisecond cold starts vs. Docker's multi-second startup
- **Microsoft Wassette** — bridges WASM components with the Model Context Protocol (MCP) for AI agent tool discovery in sandboxed contexts
- **Cloudflare Dynamic Workers (April 2026)** — ephemeral isolated V8 contexts created at runtime for AI-generated code execution
- **MCP + WASM is the emerging standard** for safe distribution of AI agent tools

**MicroVM alternatives:**
- Firecracker (AWS Lambda), gVisor (Google Cloud Run) — stronger hardware-level isolation, higher overhead
- E2B, Blaxel, Runloop — production sandbox-as-a-service with sub-100ms resume times and persistent filesystems

**The standard autonomous repair loop (RepairAgent, ICSE 2025):**
```
1. Monitor: CI failure detected (compilation error or test failure)
2. Diagnose: LLM analyzes error output, stack trace, affected source range
3. Plan + Generate: patch candidate (code change)
4. Execute in Sandbox: compile + run tests against patch
5. Evaluate:
    - Success: commit patch or open PR for human review
    - Failure: observe new error, incorporate into context, iterate
6. Budget check: hard stop at N=5 iterations; escalate to human
```

**Critical risk: runaway recursion.** Agents that fail to converge iterate indefinitely, consuming compute budget. The hard iteration cap and a LLM-budget-per-session constraint (managed by `vox-scaling-policy`) are mandatory safety mechanisms.

**Key takeaway for Vox:** The WASI/Sandbox backend already exists in the Greenfield architecture diagram. The repair loop maps directly onto the ARS execution runtime. The infrastructure is present; the orchestration layer connecting them is the implementation gap.

archived_date: 2026-04-18
---

### 2.7 Self-Healing Tests, CI Integration, and Agentic Test Management

**Self-healing mechanics (mature, 2025):**
- Detect structural change (broken locator, renamed method, changed API signature)
- Re-synthesize the test reference automatically
- Most mature in end-to-end web testing (Mabl, Testim, Functionize, Testsigma)
- Core principle is generalizable to any test type: *when the code structure changes, detect and update dependent tests*

**AI in CI pipelines — best practices (2026):**
- **Hard quality gates:** block merge if tests don't compile, mutation score falls below threshold on changed files, or unexpected snapshot diffs appear
- **Tiered model strategy:** small/fast models for style/labeling; large reasoning models for semantic code review
- **Policy-as-code:** every agent action logged (actor, intent, tool invoked, outcome) for auditability (SOC 2)
- **"First reviewer" pattern:** AI as the first code reviewer, not auto-merger; human always approves before landing

**AI-native TDD workflow (2026 standard practice):**
1. Human or agent writes a *failing* test (RED phase)
2. Agent generates minimal code to make it pass (GREEN phase)
3. Agent refactors with test suite as safety net (REFACTOR phase)
4. Agent runs mutation testing to verify test suite effectiveness
5. Human reviews the diff; approves or requests adjustments

The phrase **"use red/green TDD"** in prompts is now a recognized behavioral signal in major LLMs — they understand to follow the structured cycle rather than generating an entire implementation upfront.

**LSP integration for inline tests (the developer experience layer):**
- `textDocument/codeLens` — "Run Test" / "Debug Test" annotations rendered above test definitions
- `textDocument/publishDiagnostics` — maps test failures to source positions (inline squiggles on failing assertions)
- Build Server Protocol (BSP) — handles build/test/run lifecycle; bridges LSP and the test runner
- The Vox LSP (`vox-lsp`) is the natural integration point for surfacing all of the above

---

## 3. Implications for the Vox Codebase

### 3.1 What We Already Have

| Component | Current Role | Testing Relevance |
|---|---|---|
| `vox-test-harness` | Shared test infrastructure | HIR builders, span dummies, pipeline helpers, assertions — foundation already exists |
| `vox-integration-tests` | Full pipeline tests: parse → HIR → typeck → codegen | Covers 10+ test files; the pattern (define Vox source as string → assert on output) is the scaffold for snapshot testing |
| `vox-eval` | Parse rate, construct coverage metrics for ML | Can be extended for test coverage metrics |
| `vox-skills` | Skill execution runtime (Pending → Succeeded/Failed) | Natural host for the test synthesis + repair loop |
| `vox-populi` | Native LLM training/inference (QLoRA on RTX 4080) | Can be fine-tuned on Vox test patterns; corpus generation for test examples |
| WASI/Sandbox backend | Greenfield architecture (compiler → WASI output) | Already exists; needs wiring to a controlled execution context for generated code |
| `vox-lsp` | Language server | Integration point for CodeLens ("Run Test") and publishDiagnostics (test failure inline markers) |
| `vox-compiler` | Full pipeline: parse → HIR → typecheck → codegen | Primary target for golden/snapshot testing and property-based testing |
| TOESTUB / quality gates | CI enforcement (G0-G3) | Already blocks skeleton code; can host mutation score gates |
| `vox-orchestrator` | Agent dispatch, model routing | Routes LLM calls for test generation to the right model based on task complexity |

### 3.2 Current Gaps

| Gap | Description | Priority |
|---|---|---|
| **No test syntax in the language** | `.vox` files have no native `test` block, `@test` annotation, or `assert` primitive | **HIGH** |
| **No snapshot/golden testing** | No mechanism to record compiler output as a reference and diff against it | **HIGH** |
| **No oracle definition** | No formal spec of what "correct" Vox compilation output looks like; without this, AI cannot generate meaningful assertions | **HIGH (foundational)** |
| **No property/fuzz testing** | No `@forall`, `@fuzz`, or arbitrary input generation for `.vox` programs | **HIGH** |
| **No mutation testing** | No mutant generator for Vox source; no mutation score tracking in CI | **MEDIUM** |
| **No AI test generation pipeline** | No ARS skill connecting model routing to test synthesis or repair | **MEDIUM** |
| **No sandbox execution for generated code** | WASI backend exists but not wired to a test agent execution context | **MEDIUM** |
| **No coverage instrumentation** | `vox-compiler` doesn't emit branch coverage data for `.vox` programs | **LOW** |

### 3.3 The Oracle Problem is Vox's Hardest Challenge

For *user-written Vox code*, the oracle is relatively tractable — the user specifies expected behavior via assertions or `@spec` annotations. For **the Vox compiler pipeline itself**, three oracle types are needed:

1. **Golden reference oracle** — record the HIR/codegen output of a known-correct program; future runs must match it (snapshot testing)
2. **Differential oracle** — output of version N must match version N-1 except for intentional changes (regression detection)
3. **Semantic oracle** — the generated Rust/TypeScript code must behave as the Vox source specifies (hardest; requires formal verification or extensive property-based testing)

Option 3 — semantic correctness of codegen — is where Verus (formal verification for Rust) becomes relevant for the Vox compiler codebase itself, not for user programs. LLM-assisted annotation of Verus specs for `vox-compiler` functions is a viable long-term path, enabled by the "vericoding" trend.

**Practical near-term oracle strategy:**
- Use **metamorphic testing** for stable properties (parsing is idempotent, lowering is monotone)
- Use **snapshot testing** for regression prevention
- Use **`@spec` annotations** on Vox functions as generation hints for the AI synthesis skill
- Reserve semantic correctness proofs for the highest-risk compiler invariants

archived_date: 2026-04-18
---

## 4. Proposed Roadmap: Four Waves

### Wave T1 — Language-Native Test Syntax (Foundation)
*Estimated effort: Medium. No AI required. Very high value.*

Add first-class test support to the Vox language itself:

- `test "description" { ... }` block syntax (like Zig's `test` keyword, but string-named like Go)
- Compile-time stripping from production builds (conditional compilation, like Rust's `#[cfg(test)]`)
- `vox test` CLI subcommand via `vox-cli`
- Basic inline assertions: `assert`, `assert_eq`, `assert_ne`, `assert_err`, `assert_ok`
- **Doctests:** extract `vox` code blocks from `///` documentation comments; run them as part of `vox test` (like Rust's `rustdoc` integration)
- Wire results into `vox-lsp`: CodeLens ("▶ Run test") above each `test` block; `publishDiagnostics` for inline failure messages
- Persist test outcomes in Arca: new `test_runs` schema table (result, duration, timestamp, file, test name)
- `vox ci test` gate in the CI pipeline

**Outcome:** Any `.vox` file becomes self-validating. Agents can generate `.vox` programs and verify them inline without a separate test framework. Documentation examples are automatically tested.

---

### Wave T2 — Golden Testing, Property Testing, and Fuzzing
*Estimated effort: Medium. Builds on T1.*

Add structural testing capabilities:

**Snapshot/Golden Testing:**
- `vox test --update-snapshots` records HIR output, codegen output, and diagnostic output as `.snap` files
- Stored in `crates/vox-integration-tests/snapshots/`
- CI comparison: any unexpected diff blocks merge; intentional changes require explicit `--update-snapshots` and commit
- Snapshots become the "differential oracle" for all compiler pipeline changes

**Property-Based Testing:**
- `@forall(x: Type) { ... }` annotation triggers PBT for that function
- `vox-runtime` generates arbitrary inputs using a strategy model inspired by `proptest`
- Shrinking: minimal counterexample reported in diagnostic output with the failing input value
- Properties are checkable by both humans and the AI synthesis skill

**Fuzzing Entry Points:**
- `@fuzz fn entry(data: Bytes) { ... }` designates a fuzzing target function
- `vox ci fuzz` integration with `cargo-fuzz` / libFuzzer
- Primary targets: parser, lexer, HIR lowerer, expression evaluator
- Crash-reproducer files saved to `crates/vox-compiler/fuzz/corpus/`

**Mutation Testing (Async/Nightly):**
- New `vox-mutagen` crate: Vox-specific mutant generator
  - Operators: swap `+`↔`-`, `*`↔`/`, `&&`↔`||`
  - Statements: remove `return`, invert `if` condition, delete assignment
  - Targets: `vox-compiler`, `vox-runtime`, `vox-type-checker`
- `vox ci mutation-score --path crates/vox-compiler` (nightly CI job)
- Mutation score tracked in Arca; trend charted over time

archived_date: 2026-04-18
---

### Wave T3 — AI-Driven Test Generation and Sandbox Execution
*Estimated effort: High. Requires ARS + WASI + orchestrator integration.*

The core of the agentic testing vision:

**T3a: Sandbox Execution Gate**
- Wire the WASI backend into a controlled execution context
- Agent-generated `.vox` program → compile in sandbox → run test block in sandbox
- Hard resource limits per sandbox instance: CPU time cap, memory cap, file I/O syscall allowlist
- Sandbox escapes or resource exhaustion reported as test failures, not host crashes

**T3b: ARS Test Synthesis Skill**
New skill: `vox.testing.synthesize`
- **Input:** `.vox` source file + optional `@spec` annotations + coverage gaps from last test run
- **Output:** `.vox` test file with unit tests, `@forall` properties, and one `@fuzz` entry point per public function
- Uses orchestrator model routing (complex semantic reasoning → large model; boilerplate → small model)
- Generated tests validated through T1/T2 infrastructure before being proposed

New skill: `vox.testing.repair`
- **Input:** failing test + compiler diagnostics + sandbox output
- **Output:** patched `.vox` source or updated test assertions
- Implements the standard agent loop: Diagnose → Generate → Execute → Evaluate
- Hard cap: **5 repair iterations per session** before escalating to human
- Budget tracked via `vox-scaling-policy`

**T3c: Oracle Infrastructure (`@spec` annotations)**
```vox
// vox:skip
@spec(
    requires: input.len() > 0,
    ensures: result.len() >= input.len()
)
fn process(input: list[str]) -> list[str] { ... }
```
- `vox-compiler` validates `@spec` annotations as `debug_assert!` in debug mode
- `@spec` annotations fed to the test synthesis skill as generation hints — the AI knows what the function promises
- Long-term: SMT solver validation of `@spec` invariants (formal verification direction)

**T3d: Coverage-Guided Generation**
- Instrument `.vox` programs for branch coverage during `vox test --coverage`
- Coverage report fed back to synthesis skill: "these branches are uncovered; generate tests for them"

---

### Wave T4 — Continuous Autonomous Testing in CI
*Estimated effort: Medium. Orchestration, governance, and corpus work.*

Close the feedback loop from generation to production:

**CI Quality Gates (`vox ci test-gate`):**
- Block merge if: new `.vox` files have no test blocks, mutation score on changed files < 70%, unexpected snapshot diff
- AI-generated tests are a **first-pass reviewer** only — human approves before landing
- Low-risk PRs (docs-only, test-only): auto-approvable via policy
- High-risk PRs (compiler, runtime, type system): mandatory human review + mutation gate

**Test Corpus for `vox-populi` Fine-Tuning:**
- All human-reviewed, passing Vox test files fed into `vox-corpus` pipeline
- Fine-tune the native Populi model on Vox-specific test patterns
- This closes the flywheel: better AI → better generated tests → better review data → better AI

**Telemetry and Audit Trail:**
- Every generated test logged: model used, timestamp, review status, pass/fail history
- Wire into existing telemetry SSOT (`docs/src/architecture/telemetry-trust-ssot.md`)
- Agents are logged with a synthetic `AgentIdentity` so their contributions are distinguishable in audit logs

**Regression Auto-Fix Loop:**
- When a new PR causes `vox ci test` to regress, the repair skill triggers automatically
- A branch is created with the candidate fix; a PR is opened for human review
- Human merges or rejects; outcome feeds back into the repair skill's training signal

archived_date: 2026-04-18
---

## 5. Risk Analysis

### 5.1 Failure Modes and Mitigations

| Risk | Likelihood | Severity | Mitigation |
|---|---|---|---|
| Hallucinated tests (pass but assert nothing) | HIGH | HIGH | Mutation testing as quality gate; `@spec` as oracle; human review |
| Runaway repair loop (infinite iteration on unfixable error) | MEDIUM | HIGH | Hard 5-iteration cap; ARS budget tracking via `vox-scaling-policy` |
| Flaky AI-generated tests eroding CI trust | HIGH | MEDIUM | Human review gate before landing; stabilization period before snapshot commit |
| Oracle problem — asserting wrong expected behavior | MEDIUM | HIGH | Prefer metamorphic testing; use `@spec` annotations; formal review for critical paths |
| Build time explosion from mutation testing | HIGH | MEDIUM | Nightly only; selective mutation; parallel execution |
| WASI sandbox performance overhead | LOW | MEDIUM | Profile before mandating; sandbox only agent-synthesized code, not hand-written |
| Bad training signal from AI-reviewed-AI tests | MEDIUM | MEDIUM | Curated human review before corpus inclusion; TOESTUB checks on test files |
| Test synthesis skill generates tests that teach the wrong behavior | LOW | HIGH | `@spec` annotations as ground truth; never synthesize tests for undocumented functions without `@spec` |

### 5.2 Is This Too Much?

**No — but order matters enormously.**

Waves T1 and T2 are conventional engineering work with high immediate value and zero dependence on AI. They establish the foundation that the AI layer (T3) requires: a compilable test format, a snapshot oracle, and property specifications that the AI can target.

Jumping to T3 without T1/T2 is the failure mode: AI-generated tests with no compilation target, no oracle, and no quality gate. The output would be noise.

**Recommendation:** Start with T1 (language test syntax). Ship it. Then add snapshot testing to `vox-integration-tests` (T2). Then pilot T3 on *one subsystem only* — the HIR lowerer — before generalizing. If the repair loop produces useful diffs on real regressions, scale. If it produces noise, invest more in the oracle infrastructure first.

---

## 6. Test Taxonomy for Vox

Clarifying the terminology from the original question:

| Term (Original) | Standard Name | Vox Implementation |
|---|---|---|
| Unit tests | Unit tests | `test` block in `.vox` files (T1) |
| Integration tests | Integration tests | `vox-integration-tests` crate (already exists); extend with snapshots (T2) |
| Send-in tests | Fuzz / acceptance tests | `@fuzz` annotation targeting parser/runtime (T2); E2E tests with known good inputs |
| Folding tests | Idempotency / metamorphic tests | `@forall` property: `parse(unparse(ast)) == ast` (T2) |
| AI-generated tests | LLM synthesis tests | `vox.testing.synthesize` ARS skill output (T3) |
| Doctests | Documentation tests | Extracted from `///` blocks, run by `vox test` (T1) |
| Mutation tests | Mutation tests | `vox-mutagen` crate; nightly CI (T2) |
| Snapshot/golden tests | Regression snapshots | `.snap` files for HIR/codegen output diffs (T2) |
| Contract/spec tests | Design-by-Contract assertions | `@spec(requires:, ensures:)` annotations (T3c) |

archived_date: 2026-04-18
---

## 7. Decision Framework: Immediate Next Actions

Given current codebase state (April 2026):

1. **[T1, Now] Implement `test` block syntax in the Vox language.**  
   Parser → HIR → codegen strip → `vox test` CLI → `vox-lsp` CodeLens. Unambiguously valuable.

2. **[T2, Soon] Add snapshot/golden testing to `vox-integration-tests`.**  
   One `.snap` file per integration test. Zero AI required. High regression safety.

3. **[T2, Soon] Add `@fuzz` annotation and wire to `cargo-fuzz`.**  
   Parser and lexer are obvious first targets.

4. **[Oracle, Parallel] Document semantic invariants of Vox compilation.**  
   What properties must always hold? These become `@spec` annotations and mutation targets.  
   Example invariants:
   - "Lowering a nil-safe expression never produces a nullable codegen output"
   - "A type-checked HIR module always has no unresolved type variables"
   - "codegen(lower(parse(source))) is stable under whitespace normalization"

5. **[T3, Pilot] Wire one ARS skill to the WASI sandbox for a single `.vox` compile-and-test.**  
   Prove the execution path works before building the full repair loop.

---

## 8. Related Prior Art and Key References

| System | What It Demonstrates |
|---|---|
| Meta's ACH (Automated Compliance Hardening, 2025) | LLM + mutation-guided test generation; mutation score 80% → 95% |
| Cover-Agent (open-source) | Iterative LLM coverage improvement via execution feedback loop |
| Mutahunter | Mutation testing integrated with LLM test synthesis |
| RepairAgent (ICSE 2025) | Autonomous Java repair agent with sandboxed patch execution |
| Microsoft Wassette + MCP | WASM component distribution for sandboxed AI agent tools |
| Cloudflare Dynamic Workers (April 2026) | Ephemeral isolated V8 contexts for AI-generated code |
| Dafny / Verus | Formal verification via SMT; "vericoding" with LLMs annotating invariants |
| Python Hypothesis | Mature PBT framework; model for Vox `@forall` annotation design |
| Rust `proptest` | Strategy-based PBT with superior shrinking; model for Vox PBT strategy layer |
| Zig `test` + `comptime` | Closest analog to proposed T1 inline test syntax |
| Diffblue Cover | RL-based autonomous test generation; no LLM prompts; maintains tests as code changes |

archived_date: 2026-04-18
---

## 9. Connections to Existing Vox Architecture Documents

- **Telemetry and observability SSOT:** `docs/src/architecture/telemetry-trust-ssot.md`
- **Skills runtime:** `crates/vox-skills/src/runtime.rs`
- **WASI sandbox backend:** `docs/src/architecture/architecture-index.md` (Greenfield architecture diagram)
- **TOESTUB enforcement:** `crates/vox-toestub/`
- **Corpus pipeline:** `crates/vox-corpus/`
- **Quality gates (G0–G3):** Greenfield Wave 6 (`docs/src/architecture/`)
- **Vox eval metrics (parse rate, construct coverage):** `crates/vox-eval/`
- **ARS implementation plan:** `docs/src/architecture/` (Phase 2)
- **Completion policy (Tier A/B/C):** `contracts/operations/completion-policy.v1.yaml`

---

*Document created: 2026-04-04. Last updated: 2026-04-04.*  
*Copy to canonical location when ready: `docs/src/architecture/automated-testing-research-2026.md`*  
*Track implementation progress in `task.md` under the testing initiative.*


