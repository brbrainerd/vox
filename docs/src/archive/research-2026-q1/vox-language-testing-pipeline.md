---
title: "Vox Language Testing Pipeline"
description: "Embedding tests into .vox format and LLM to Vox delivery pipeline with five-stage gate validation."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-04"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Language Testing Pipeline
## Embedding Tests Into the .vox Format & the LLM → Vox Delivery Pipeline

> **Status:** Research + Design Specification — April 2026  
> **Depends on:** `automated-testing-research-2026.md` (general survey)  
> **Canonical path:** `docs/src/architecture/vox-language-testing-pipeline.md`  
> **Relevant AST:** `crates/vox-compiler/src/ast/decl/fundecl.rs`

---

## 1. The Core Question

You asked two things that are actually three interlocking layers:

**Layer A:** Can the `.vox` language format natively express tests, contracts, and invariants — embedded directly in source files so that any valid `.vox` program is also partially self-validating?

**Layer B:** When an LLM writes Vox code, can we apply testing at the generation point — before the code is ever shown to a user — so that what is delivered is not just syntactically valid but also *logically correct*?

**Layer C:** Should the test mode be *optional at runtime* — so the user can choose to run their Vox program with assertions enabled, and the language makes this easy?

The answer to all three is **yes**, and critically: **the Vox AST already has most of the structure needed**. This document specifies what to build next.

archived_date: 2026-04-18
---

## 2. What the AST Already Gives Us

Reading `crates/vox-compiler/src/ast/decl/fundecl.rs` reveals:

```rust
pub struct FnDecl {
    // ...
    pub is_llm: bool,              // ← function body implemented by an LLM
    pub llm_model: Option<String>, // ← which model
    pub preconditions: Vec<Expr>,  // ← @require(expr) already parsed
    pub is_pure: bool,             // ← pure function flag (no side effects)
    pub is_traced: bool,           // ← observability
    // ...
}

pub struct TestDecl { pub func: FnDecl }      // ← @test already in AST
pub struct FixtureDecl { pub func: FnDecl }   // ← @fixture already in AST
pub struct MockDecl { pub target: String, ... } // ← @mock already in AST
```

This means **the parser and AST nodes already exist** for `@test`, `@fixture`, `@mock`, and `@require`. What is missing is:

1. **`@ensure` / postconditions** on `FnDecl` (only `preconditions` exists today)
2. **`@invariant`** on type/struct declarations
3. **`@forall` / property-based test annotations**
4. **The compiler pass that enforces contracts at the right level** (debug vs. release vs. runtime-optional)
5. **The AI synthesis skill that uses these annotations as oracle hints**
6. **The `vox test` CLI command** that collects and runs all `TestDecl` nodes in a file

---

## 3. Layer A: What the `.vox` Format Should Express

### 3.1 The Testing Surface in `.vox` Files

Here is the complete proposed surface — showing what Vox code looks like when fully annotated for testing. Everything here maps to an AST node or a trivial extension of one.

```vox
// vox:skip
/// Parse and validate a user email address.
/// Returns the normalized address or an error.
@require(email.len() > 0)
@require(!email.contains(" "))
@ensure(result.is_ok() implies result.unwrap().contains("@"))
@pure
fn parse_email(email: str) -> Result[str, str] {
    // Logic here
}

@test("empty string is rejected")
fn test_parse_email_empty() {
    let r = parse_email("");
    assert_err(r);
}

@test("valid email round-trips correctly")
fn test_parse_email_valid() {
    let r = parse_email("user@example.com");
    assert_ok(r);
    assert_eq(r.unwrap(), "user@example.com");
}

@forall(email: str)
fn prop_parse_email_no_spaces(email: str) {
    let clean = email.replace(" ", "");
    assert_eq(parse_email(clean), parse_email(email.trim()));
}

@fixture
fn sample_emails() -> list[str] {
    ["user@example.com", "admin@vox.dev", "test+tag@mail.co"]
}

@fuzz
fn fuzz_parse_email(data: Bytes) {
    let s = str.from_utf8_lossy(data);
    let _ = parse_email(s); 
}
```

### 3.2 The Contract Annotations (`@require`, `@ensure`, `@invariant`)

These implement **Design by Contract** — the gold standard established by Eiffel, now recognized as essential for AI-generated code verification.

| Annotation | Position | Meaning | Runtime Mode |
|---|---|---|---|
| `@require(expr)` | Function | Precondition: caller's obligation | Assert on call |
| `@ensure(expr)` | Function | Postcondition: function's promise | Assert on return |
| `@invariant(expr)` | Type/struct | Class invariant: must hold before+after every method | Assert on entry/exit |
| `@pure` | Function | No observable side effects | Enables memoization, property testing |

**Key design decision — runtime modes (like Eiffel):**

```vox
// vox:skip
// In vox.config or via CLI flag:
// test-mode = "full"     -> all @require, @ensure, @invariant checked
// test-mode = "precond"  -> only @require checked (production-safe default)  
// test-mode = "off"      -> all annotations stripped (maximum performance)
```

This means the annotations cost nothing in production unless the user opts in. They serve three simultaneous purposes:
1. **Documentation** — a human reading a function immediately knows what it expects and promises
2. **Runtime safety net** — in debug/test mode, violations terminate early with a precise error
3. **AI oracle** — the test synthesis skill reads `@ensure` as the ground truth for what to assert in generated test cases

**Critical insight from research (AIware 2025):** Providing the full function context (including `@require`/`@ensure`) -> the LLM when generating test oracles produces significantly better assertions than providing only the function signature. The annotations *are* the oracle.

### 3.3 The `@test` and `@fixture` Blocks

`TestDecl` and `FixtureDecl` already exist in the AST. What needs to happen:

**Compiler behavior:**
- In `release`/`production` codegen: `TestDecl` nodes are completely elided — zero overhead, no inclusion in output
- In `test` mode: `TestDecl` nodes are compiled and registered in a test runner registry
- `FixtureDecl` nodes are only compiled in `test` mode; their names are injectable into `TestDecl` function parameters

**Naming convention (like Rust):**
```vox
// vox:skip
@test("description drives the name")
fn test_anything() { 
    // Logic here
}
```

**Discovery model:** `vox test` walks all `.vox` files in the project, collects every `TestDecl`, and runs them as a flat list (with optional filter by name pattern: `vox test --filter="email"`).

### 3.4 The `@forall` Property-Based Test Annotation

This is the Vox-native version of QuickCheck / proptest / Hypothesis. The compiler generates a driver that:
1. Creates a strategy for each parameter type (integers, strings, lists, enums)
2. Generates N random instances (default: 1000)
3. Runs the annotated function body with each instance
4. On failure, shrinks the input to the minimal counterexample
5. Reports the failing case in diagnostics

```vox
// vox:skip
@forall(x: int, y: int)
fn prop_addition_commutative(x: int, y: int) {
    assert_eq(x + y, y + x);
}

@forall(s: str)
fn prop_trim_idempotent(s: str) {
    assert_eq(s.trim().trim(), s.trim());
}
```

The strategy for each type is defined in `vox-runtime` and is automatically inferred from the type annotation. Custom strategies can be specified:

```vox
// vox:skip
@forall(email: str using email_strategy())
fn prop_parse_valid_email(email: str) {
    assert_ok(parse_email(email));
}
```

### 3.5 The `@fuzz` Entry Point

For security-critical and parser-facing functions, `@fuzz` creates an entry point for coverage-guided fuzzing:

```vox
// vox:skip
@fuzz
fn fuzz_parse_vox_module(data: Bytes) {
    let src = str.from_utf8_lossy(data);
    let _ = Parser.parse(src); 
}
```

**Compiler behavior:** `@fuzz` functions are only compiled when building for a fuzzing target (`vox ci fuzz`). They are completely excluded from normal builds. The generated harness integrates with `cargo-fuzz` / libFuzzer via the WASI compilation target.

archived_date: 2026-04-18
---

## 4. Layer B: The LLM → Vox Delivery Pipeline

This is the heart of the second part of your question: **how do we ensure that code written by an LLM is correct before it reaches the user?**

The answer is a **five-stage delivery gate** that runs automatically whenever `is_llm: true` on a `FnDecl` in the AST — or whenever a Vox Orchestrator agent generates a `.vox` file.

### 4.1 The Five-Stage Delivery Gate

```
LLM generates .vox code
        │
        ▼
┌───────────────────────┐
│  Stage 1: Parse Gate  │  Lexer + Parser → must produce valid AST
│                       │  If fail: surface diagnostic → LLM repairs
└───────────┬───────────┘
            │ PASS
            ▼
┌───────────────────────┐
│  Stage 2: Type Gate   │  HIR lowering + typeck → no unresolved types
│                       │  @require / @ensure syntactically valid
│                       │  If fail: surface diagnostic → LLM repairs
└───────────┬───────────┘
            │ PASS
            ▼
┌─────────────────────────────┐
│  Stage 3: Contract Gate     │  Any @require annotations run against
│                             │  a set of canonical "probe inputs"   
│                             │  (type-derived edge cases: null, empty,
│                             │  zero, MAX_INT, etc.)
│                             │  If @require violated → LLM reconsiders
└───────────┬─────────────────┘
            │ PASS
            ▼
┌───────────────────────────────┐
│  Stage 4: Test Execution Gate │  Run any @test blocks in a WASI sandbox
│                               │  Run @forall properties (100 cases)
│                               │  Report pass/fail per test
│  If fail: repair loop (max 5) │  → LLM sees: failing test + diagnostics
└───────────┬───────────────────┘
            │ PASS
            ▼
┌────────────────────────────────┐
│  Stage 5: Human Review Signal  │  Tag generated code in output with:
│                                │  - Which tests passed
│                                │  - Which @ensure annotations exist
│                                │  - Coverage percentage (if available)
│                                │  - "AI-generated, pipeline-validated"
│                                │    badge in vox-lsp gutter
└────────────────────────────────┘
            │
            ▼
      Delivered to user
```

### 4.2 Who Triggers the Gate?

The gate runs in three contexts:

**Context 1: Inline LLM function (`is_llm: true`)**
```vox
// vox:skip
@llm(model = "claude-sonnet")
@require(items.len() > 0)
@ensure(result.total > 0)
fn calculate_order_total(items: list[LineItem]) -> OrderTotal {
    // body generated at runtime by the LLM
}
```
When the Vox runtime encounters `is_llm: true`, it:
1. Routes to the orchestrator model selection
2. Gets back generated `.vox` body text
3. Runs it through the parse + type + contract gates
4. If it passes, inlines and executes

**Context 2: Agent-generated `.vox` files (via ARS skill)**
The `vox.testing.synthesize` ARS skill wraps any generated file in the full five-stage gate before returning the file to the caller.

**Context 3: Agentic coding sessions (Orchestrator task)**
When an orchestrator agent completes a coding task (writes `.vox` files), the delivery step automatically runs the full gate before marking the task as `Succeeded`.

### 4.3 The Repair Loop (Stages 1–4)

Each failing stage triggers a targeted repair prompt to the originating model. The prompt structure is:

```
CONTEXT: This Vox function was generated to satisfy: <original request>

PROBLEM: The function failed Stage <N> of the delivery gate.
Error: <exact diagnostic from vox-compiler>
Failing test: <test name + assertion that failed>
Failing input: <minimal counterexample from shrinking>

CURRENT FUNCTION:
<generated .vox source>

CONTRACT:
@require: <precondition exprs>
@ensure: <postcondition exprs>

TASK: Fix the function so it passes the gate. Output only the corrected
function body. Do not change the @require or @ensure annotations.
```

**Key design choices:**
- `@require` and `@ensure` are **frozen** during repair — they represent the specification, not the implementation. The LLM must satisfy them, not change them.
- The repair prompt includes the **shrunk minimal counterexample** — the smallest input that causes the failure — making the LLM's reasoning task as tractable as possible.
- Hard cap: **5 repair iterations**. After that, the task is marked `Failed` and surfaced to a human with full diagnostic context.

### 4.4 What "Logically Correct" Means (The Oracle Problem, Solved Practically)

The research is clear: there is no perfect automated oracle. But here is the practical hierarchy Vox should use, from strongest to weakest:

| Oracle Type | How Strong | Source | Cost |
|---|---|---|---|
| `@ensure` annotation | ✅✅✅ Strong | Author-specified postcondition | Zero (already written) |
| Metamorphic property (`@forall`) | ✅✅ Good | Structural relationship | Low |
| Docstring-derived assertion | ✅ Moderate | LLM reads `///` comments | Low |
| Type-derived probe (edge cases) | ✅ Moderate | Compiler infers from types | Zero |
| Snapshot diff vs. previous version | ✅ Moderate | Regression only | Low |
| Mutation score > threshold | ✅ Slow | Full mutation run (nightly) | High |

**The key insight:** `@ensure` annotations written alongside a function are the best oracle. The design principle is therefore:

> **When an LLM generates a function, it should also be prompted to write `@ensure` annotations for it.** These then become the oracle for testing the function.

This is the "contract-first" generation pattern:

```
Prompt to LLM:
  "Write a Vox function that <user intent>.
   First write the @require and @ensure annotations.
   Then implement the body."
```

The LLM writing its own contracts before writing its own body is the Vox equivalent of test-driven development for AI — it forces the model to reason about correctness before implementation, and produces machine-checkable oracles as a side effect.

### 4.5 The `@llm` Annotation and Runtime Generation

The most novel surface in the Vox AST is `is_llm: bool` and `llm_model: Option<String>`. This enables **inline LLM-implemented functions** — functions whose body is generated at runtime by a language model. The delivery gate makes this safe.

Extended design for the `@llm` annotation:

```vox
// vox:skip
@llm(
    model = "claude-sonnet",      
    verify = "strict",            
    cache = true,                 
    on_fail = "raise"             
)
@require(query.len() > 0)
@ensure(result.items.len() >= 0)
fn search_products(query: str, filters: SearchFilters) -> SearchResult {
    // body generated at runtime
}
```

With `verify = "strict"`, the first call to this function:
1. Sends the function signature + `@require`/`@ensure` + doc comment to the LLM
2. Gets back a `.vox` function body
3. Runs it through all five gate stages
4. If it passes, caches the generated body in Arca and uses it for this and future calls
5. If it fails after 5 repair attempts, raises an error or executes the `on_fail` strategy

This is **the most powerful form of AI-integrated programming Vox can offer** — functions that write themselves, but are contractually verified before they execute.

---

## 5. Layer C: Optional Runtime Test Mode

The key question: should users be able to run their Vox programs in a mode where tests and contracts are active at runtime, optionally?

**Yes. Three modes, controlled by `vox.config` and/or a CLI flag:**

### Mode 1: `build` (default, production)
- All `@test`, `@fixture`, `@forall`, `@fuzz` blocks are stripped from codegen
- `@require`/`@ensure`/`@invariant` are compiled to **no-ops** (zero runtime cost)
- No testing overhead whatsoever

### Mode 2: `dev` (development default)
- All `@test`, `@fixture`, `@forall` blocks are compiled and registered
- `@require` / `@ensure` are compiled to **runtime assertions** (panic on failure with diagnostic message)
- `vox run` in dev mode runs tests before starting the program; fail → exit before launch
- This is like Rust's `debug_assert!` — costs nothing in production, catches bugs in development

### Mode 3: `verify` (explicit opt-in for runtime safety)
- `@require` / `@ensure` / `@invariant` are compiled to **recoverable `Result`-returning checks**
- Instead of panicking, a contract violation returns `Result::Err(ContractError)` to the caller
- This is the "production-safe contract checking" mode — like Eiffel's configurable assertion monitoring
- Useful for high-stakes functions where you want runtime safety without crashes

```vox
// vox:skip
// vox.config
[build]
mode = "dev"          // or "build" or "verify"
contract-level = "require"  // "off" | "require" | "full"
```

This three-mode model directly addresses your question about whether testing is "optional" — yes, by default it is (mode = `build` in production), but it is trivially opt-in for development and testing scenarios.

archived_date: 2026-04-18
---

## 6. How the Pipeline Fits Together: The Complete Picture

```
┌─────────────────────────────────────────────────────────────────┐
│  USER / ORCHESTRATOR AGENT                                      │
│  "Write me a Vox function that does X"                          │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│  LLM GENERATION (via vox-orchestrator + model routing)          │
│                                                                 │
│  Prompt includes:                                               │
│  - Function signature (name, params, return type)               │
│  - "Write @require and @ensure annotations first"               │
│  - Any existing context from the .vox file                      │
│  - Vox syntax guide                                             │
└─────────────────┬───────────────────────────────────────────────┘
                  │  Generated: @require, @ensure, fn body
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│  FIVE-STAGE DELIVERY GATE (vox-skills skill: vox.testing.validate) │
│                                                                 │
│  Stage 1: Parse Gate      → AST valid?                         │
│  Stage 2: Type Gate       → HIR + typeck pass?                 │
│  Stage 3: Contract Gate   → @require holds on probe inputs?    │
│  Stage 4: Test Gate       → @test blocks pass in WASI sandbox? │
│  Stage 5: Review Signal   → Tag + report for human inspection  │
│                                                                 │
│  On failure at any stage: repair loop (max 5 iterations)        │
│  → model sees: error + minimal failing input + frozen contracts │
└─────────────────┬───────────────────────────────────────────────┘
                  │  PASS (or escalate to human after 5 retries)
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│  DELIVERED TO USER                                              │
│                                                                 │
│  .vox file with:                                                │
│  - Validated function body                                      │
│  - @require / @ensure annotations preserved                     │
│  - @test blocks for future regression                           │
│  - LSP gutter badge: "AI-generated · pipeline-validated"        │
│  - Arca trace: which model, which gate stages passed, timestamp │
└─────────────────────────────────────────────────────────────────┘
```

---

## 7. Concrete Implementation: What to Build and Where

### 7.1 AST Changes (Small — Most Already Exists)

**File: `crates/vox-compiler/src/ast/decl/fundecl.rs`**

Add to `FnDecl`:
```rust
// Missing today — needs to be added:
pub postconditions: Vec<Expr>,    // @ensure(expr) annotations
pub invariants: Vec<Expr>,        // @invariant(expr) on fn (for methods)
pub test_strategy: Option<String>, // @forall strategy override, if any
pub is_fuzz: bool,                // @fuzz annotation
pub verify_mode: VerifyMode,      // off | require | full (compile-time setting)
```

Add new enum:
```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VerifyMode { Off, RequireOnly, Full }
```

**`TestDecl` already exists.** Add a string label field:
```rust
pub struct TestDecl {
    pub label: String,   // ADD: the description string after @test("...")
    pub func: FnDecl,
}
```

**New: `ForallDecl`** for property-based tests:
```rust
pub struct ForallDecl {
    pub label: String,
    pub func: FnDecl,
    pub iterations: u32,  // default 1000
}
```

### 7.2 Compiler Pass: Contract Emission

**File: new `crates/vox-compiler/src/hir/lower/contracts.rs`**

A HIR lowering pass that converts `@require`/`@ensure` into one of three forms depending on `VerifyMode`:

- `Off` → emit nothing, elide all contract nodes from HIR
- `RequireOnly` → emit `debug_assert!(precondition, "...")` at function entry
- `Full` → emit `debug_assert!` for preconditions at entry + postconditions at every return site

For `verify` mode (recoverable contracts):
- Wrap function return type in `ContractResult<T>` 
- Precondition failure → early return `ContractResult::PreconditionFailed { ... }`
- Postcondition failure → wrap return value in `ContractResult::PostconditionFailed { ... }`

### 7.3 CLI: `vox test`

**File: `crates/vox-cli/src/commands/test.rs`** (new)

```
vox test                         → run all @test blocks in project
vox test --filter="email"        → only tests whose label matches
vox test --forall-iterations=5000 → increase PBT sample count
vox test --coverage              → instrument for branch coverage
vox test --update-snapshots      → update .snap golden files
```

Internally: compile in `dev` mode → collect `TestDecl` nodes → run test harness → print results → exit 0 or 1.

### 7.4 ARS Skill: `vox.testing.validate` (Delivery Gate)

**New skill in `crates/vox-skills/skills/`**

The five-stage delivery gate as an ARS skill:

```rust
pub struct ValidateVoxCodeSkill;

impl ArsSkill for ValidateVoxCodeSkill {
    fn id() -> &'static str { "vox.testing.validate" }
    
    fn execute(&self, input: &SkillInput, ctx: &ArsContext) -> SkillResult<SkillOutput> {
        let source = input.source_code();
        
        // Stage 1: Parse
        let ast = parse(source).map_err(|e| stage_fail(1, e))?;
        
        // Stage 2: Typecheck
        let hir = lower_and_typecheck(ast).map_err(|e| stage_fail(2, e))?;
        
        // Stage 3: Contract probing
        probe_contracts(&hir).map_err(|e| stage_fail(3, e))?;
        
        // Stage 4: Test execution in WASI sandbox
        run_tests_in_sandbox(&hir).map_err(|e| stage_fail(4, e))?;
        
        Ok(SkillOutput::validated(hir, stage_reports))
    }
}
```

### 7.5 LSP: Test CodeLens and Validation Badge

**File: `crates/vox-lsp/src/code_lens.rs`** (extend)

For each `TestDecl` node in the HIR: emit a CodeLens at the function definition line:
```
▶ Run test  🐛 Debug test
```

For functions with `is_llm: true` that have passed the delivery gate: emit a status indicator:
```
✓ AI-validated (claude-sonnet · 3 tests passed · @ensure verified)
```

For functions with `is_llm: true` that have NOT been validated yet: emit a warning lens:
```
⚠ AI-generated · not yet validated — run vox test
```

archived_date: 2026-04-18
---

## 8. The `@llm` Function: The Killer Feature

The most powerful combination is the `@llm` annotation working with the contract system. This enables:

```vox
// vox:skip
/// Sort a list of products by price.
@llm(verify = "strict", cache = true)
@require(products.len() >= 0)
@ensure(result.len() == products.len())
@ensure(result.is_sorted_by(|a, b| a.price <= b.price))
fn sort_products_by_price(products: list[Product]) -> list[Product] {
    // logic here
}
```

This function does something most programming languages cannot:
1. **It documents its own correctness properties** (`@ensure`)
2. **It generates its own implementation** (`@llm`)
3. **It verifies its implementation against the properties** (five-stage gate)
4. **It caches the verified implementation** (Arca, `cache = true`)
5. **It re-validates when the implementation is regenerated** (on cache miss or model update)

This is the Vox answer to the question "can we ensure LLM-written code is correct" — yes, by combining the language's contract system with the AI runtime in a closed loop.

---

## 9. Phased Implementation Plan

### Phase 1 — Language Foundation (No AI Required)
*Target: allows `vox test` to work on any `.vox` file*

1. Add `postconditions`, `is_fuzz`, `verify_mode` to `FnDecl` AST
2. Add label string to `TestDecl`
3. Add `ForallDecl` AST node
4. Parser: recognize `@ensure(expr)`, `@forall(...)`, `@fuzz` decorators
5. HIR lowering: `contracts.rs` pass for contract emission
6. `vox test` CLI command (collect `TestDecl` nodes, run, report)
7. `vox-lsp` CodeLens: "▶ Run test" above each `TestDecl`

### Phase 2 — Property Testing and Snapshots
*Target: property-based testing and golden regression*

1. `vox-runtime`: strategy generators for built-in types (Int, String, List, etc.)
2. `ForallDecl` execution driver: generate N inputs, run, shrink on failure
3. Snapshot testing: `.snap` files for codegen output, `--update-snapshots` flag
4. `@fuzz` harness: generate libFuzzer entry point from `@fuzz` declarations

### Phase 3 — LLM Delivery Gate
*Target: AI-generated Vox code validates before delivery*

1. `vox.testing.validate` ARS skill (five-stage gate)
2. WASI sandbox wiring for test execution (connect existing sandbox backend)
3. Repair loop: targeted repair prompt with frozen contracts, max 5 iterations
4. Budget tracking via `vox-scaling-policy`
5. `@llm` annotation execution: runtime generation → gate → cache in Arca
6. LSP badge: "AI-validated" / "AI-generated · not validated" status

### Phase 4 — Corpus and Flywheel
*Target: validated tests feed `vox-populi` training*

1. All human-reviewed, pipeline-validated `.vox` files enter `vox-corpus`
2. `vox-populi` fine-tuned on Vox-specific contract + test patterns
3. Model learns to write `@ensure` annotations as naturally as function bodies
4. Mutation testing (nightly): `vox ci mutation-score` on critical subsystems
5. `vox clavis doctor` integration: validate that `@llm` cache entries are still valid

archived_date: 2026-04-18
---

## 10. What This Means For Users of Vox

From a user's perspective, the experience should feel like this:

**Writing code (human author):**
```vox
// vox:skip
@require(x > 0)
@ensure(result > x)
fn grow(x: int) -> int { return x * 2; }

@test("doubles positive numbers")
fn test_grow() {
    assert_eq(grow(3), 6);
}
```
→ `vox test` runs automatically in `vox dev` mode  
→ LSP shows "▶ Run test" lens above the test  
→ Mutation testing (nightly) verifies the test would catch bugs

**Delegating to the LLM:**
```vox
// vox:skip
@llm
@require(name.len() > 0 && name.len() < 100)
@ensure(result.starts_with("Dear "))
fn format_greeting(name: str) -> str { }
```
→ At runtime, the LLM writes a body  
→ Five-stage gate validates it silently  
→ If it fails, it repairs itself up to 5 times  
→ If still failing, surfaces a clear diagnostic to the user  
→ User sees a validated function, not a raw LLM output

**Running in production:**
```
vox build --mode=build   → all tests stripped, contracts elided, zero overhead
vox build --mode=dev     → tests included, contracts as debug_assert! 
vox build --mode=verify  → contracts as recoverable Result errors
```

---

## 11. Connections to Existing Docs and Code

| Reference | Location |
|---|---|
| General testing research survey | `docs/src/architecture/automated-testing-research-2026.md` |
| `FnDecl` AST (current state) | `crates/vox-compiler/src/ast/decl/fundecl.rs` |
| ARS runtime | `crates/vox-skills/src/runtime.rs` |
| WASI sandbox backend | Greenfield arch → `docs/src/architecture/architecture-index.md` |
| `vox-test-harness` (Rust harness) | `crates/vox-test-harness/src/lib.rs` |
| `vox-integration-tests` (pipeline tests) | `crates/vox-integration-tests/README.md` |
| Orchestrator model routing | `crates/vox-orchestrator/` |
| `vox-scaling-policy` (budget) | `crates/vox-scaling-policy/` |
| Clavis secret management | `crates/vox-clavis/` |
| Telemetry SSOT | `docs/src/architecture/telemetry-trust-ssot.md` |

archived_date: 2026-04-18
---

*Document created: 2026-04-04. Track implementation in `task.md` under "Testing Pipeline" initiative.*  
*Phase 1 begins with the `postconditions` field addition to `FnDecl` and the `@ensure` parser change.*


