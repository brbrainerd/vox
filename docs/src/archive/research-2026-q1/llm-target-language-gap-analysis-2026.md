---
title: "LLM Target Language Gap Analysis (April 2026)"
description: "Comprehensive audit of gaps across syntax, AST, HIR, type checking, codegen, grammar-constrained decoding, diagnostics, testing, contract system, training, and LSP — with specific, actionable tasks for making Vox the premier LLM target language."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-17"
correction_pass: "2026-04-17"
training_eligible: false
training_rationale: "Cross-cutting gap analysis directly driving LLM-native language quality improvements."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# LLM Target Language Gap Analysis (April 2026)

> [!WARNING]
> **Correction Pass (2026-04-17):** Gaps B-1, B-2, B-4, and B-6 were false positives — the described items were already implemented. Each section has been updated with a correction notice. The **authoritative superseding document** is [llm-target-language-implementation-plan-2026.md](llm-target-language-implementation-plan-2026.md), which reflects the verified codebase state and provides the explicit file-level remediation plan.

> **Scope:** This document is the output of a full codebase review covering `crates/vox-compiler/`, `crates/vox-grammar-export/`, `crates/vox-lsp/`, `crates/vox-runtime/`, `crates/vox-test-harness/`, `tree-sitter-vox/`, and the full `docs/src/architecture/` research tree. It identifies **77 concrete gaps** across 13 dimensions, each with a specific solution and crate owner.
>
> Research foundations:
> - [LLM-Native Language Design](research-llm-native-lang-design-2026.md)
> - [Vox as the First AI-Native Language: K-Complexity](vox-llm-native-language-research-2026.md)
> - [Grammar-Constrained Decoding](research-grammar-constrained-decoding-2026.md)
> - [GRPO Reward Shaping](research-grpo-reward-shaping-2026.md)
> - [Vox Language Testing Pipeline](vox-language-testing-pipeline.md)
> - [Fuzzy & Partial Parsing](research-fuzzy-parsing-2026.md)
> - [Zero-Shot Invariants](research-ts-hallucination-zero-shot-invariants-2026.md)

---

## Dimension A — Syntax & Lexer

### Gap A-1: `GRAMMAR_SSOT.md` and `token.rs` are out of sync

**Finding:** `tree-sitter-vox/GRAMMAR_SSOT.md` lists keywords `message`, `bind`, `routes`, `style` that do not appear as `Token` variants in `crates/vox-compiler/src/lexer/token.rs`. Conversely, `token.rs` has `Loop`, `Break`, `Continue`, `Environment`, `Migrate`, `Cleanup`, `Mount`, `Effect`, `View`, `Derived`, `State`, `Pub`, `With`, `On`, `Agent`, `Component`, `Spawn`, `Workflow`, `Activity`, `Actor` — none of which are in the SSOT doc.

**Impact:** Tree-sitter syntax highlighting diverges from the actual grammar. LLMs trained on the SSOT doc will generate tokens the compiler cannot lex.

**Solution:**
- Add `vox ci grammar-ssot-parity` CI check: parse `GRAMMAR_SSOT.md` keyword table and assert every entry maps to a `Token` variant.
- Update `GRAMMAR_SSOT.md` to be generated from `token.rs` via `vox grammar ssot-export` (new subcommand).
- Owner: `vox-compiler`, `vox-cli` CI | **Severity: Critical** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap A-2: `ret` keyword deprecation not enforced across codebase

**Finding:** `crates/vox-compiler/src/lexer/token.rs` still has both `Ret` and `Return` tokens. `docs/src/architecture/vox-lang-training-ssot-2026.md` explicitly states "`ret` keyword is deprecated — `return` is the sole canonical keyword." However, the parser tests in `crates/vox-compiler/src/parser/descent/tests.rs` still use `ret` extensively (e.g., `"fn add(a, b) to int { ret a + b }"`).

**Impact:** LLMs trained on Vox code will see both forms, creating split-brain ambiguity in the tokenizer fertility budget (exactly the problem the training SSOT was written to prevent).

**Solution:**
- Emit a `Warn` diagnostic for `ret` usage in the parser: "deprecated keyword `ret`; use `return`".
- Add autofix in `vox fmt`: rewrite `ret` → `return` in source.
- Update all parser test fixtures to use `return`.
- Add `vox ci retired-keyword-guard` check that rejects new `.vox` files using `ret` in `examples/`.
- Owner: `vox-compiler` lexer/parser, `vox-cli` | **Severity: High** | **Effort: Small**

---

### Gap A-3: No string interpolation

**Finding:** The lexer only supports `StringLit(String)` and single-quoted strings. There is no f-string / template string interpolation (e.g., `f"Hello {name}"`). LLMs trained on Python, JavaScript, and Rust habitually generate interpolated strings; forcing them to use concatenation inflates K-complexity and produces hallucinated syntax.

**Solution:**
- Add `TemplateLit(Vec<TemplateSegment>)` to `Token` where `TemplateSegment` is either `Literal(String)` or `Expr(Box<Expr>)`.
- Lex with backtick delimiters (`` `Hello {name}` ``), consistent with JS/Rust format strings.
- Extend AST `Expr` with `TemplateLit(Vec<TemplateSegment>, Span)`.
- Extend HIR `HirExpr` to lower to concatenation or target-language interpolation.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap A-4: No multiline string literals

**Finding:** String literals require escaped `\n` sequences. There is no raw/multiline string (e.g., `"""..."""` or `r"..."`). LLMs generating SQL, HTML fragments, or doc strings will either escape incorrectly or hallucinate non-existent syntax.

**Solution:**
- Add `r"..."` raw string and `"""..."""` multiline string tokens.
- Lexer priority: raw strings have lower priority than regular strings.
- Owner: `vox-compiler` | **Severity: Medium** | **Effort: Small**

---

### Gap A-5: No integer literal variants

**Finding:** `token.rs` only supports decimal integer literals (`[0-9]+`). No hex (`0xFF`), binary (`0b1010`), octal (`0o77`), or underscore-separated (`1_000_000`) forms exist. This forces LLMs handling bit manipulation, configuration constants, or large numbers to use decimal forms that are harder to read and reason about.

**Solution:**
- Extend `IntLit` lexer regex to support `0x[0-9a-fA-F]+`, `0b[01]+`, `0o[0-7]+`, and `[0-9][0-9_]*` (underscore separators).
- Store as `i64` after parsing; add lexer callback that strips underscores.
- Owner: `vox-compiler` | **Severity: Low** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap A-6: Decorator namespace divergence

**Finding:** `GRAMMAR_SSOT.md` lists `@action`, `@skill`, `@agent_def`, `@storage` as decorators. None of these appear in `token.rs`. Meanwhile `token.rs` has `@island`, `@loading`, `@require`, `@ensure`, `@invariant`, `@forall`, `@fuzz`, `@pure`, `@scheduled`, `@deprecated` — not in SSOT. The SSOT is an authoritative training surface; divergence here directly causes LLM decorator hallucination.

**Solution:**
- Resolve which decorators are canonical vs. retired in the language surface (cross-ref AGENTS.md retired-symbol table).
- Regenerate SSOT from lexer (see Gap A-1 solution).
- Remove `AtComponent` from token.rs (retired per AGENTS.md) and add parser error with migration hint.
- Owner: `vox-compiler`, docs | **Severity: High** | **Effort: Small**

---

### Gap A-7: Optional chaining `?.` in SSOT but missing from lexer

**Finding:** `GRAMMAR_SSOT.md` operator list includes `?.` (optional chaining). `token.rs` has no `OptionalChain` variant. LLMs will hallucinate `?.` access on `Option` values; without a token, those programs will fail at lex time with a confusing error.

**Solution:**
- Add `#[token("?.")] OptionalChain` to `token.rs` before `Question` (higher priority to avoid split).
- Add parser rule: `expr?.field` → `HirExpr::OptionalChain(Box<HirExpr>, String, Span)`.
- Lower to HIR: emit `match expr { Some(v) => Some(v.field), None => None }`.
- Owner: `vox-compiler` | **Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

## Dimension B — AST

### ~~Gap B-1: `@ensure` postconditions missing from `FnDecl`~~ — **CORRECTION: IMPLEMENTED**

> [!NOTE]
> **Correction (2026-04-17):** Code audit of `crates/vox-ast/src/decl/fundecl.rs` confirmed `pub postconditions: Vec<Expr>` **exists**. `pub verify_mode: VerifyMode` also exists, as does `pub enum VerifyMode { Off, RequireOnly, Full }`. This gap was a false positive based on stale documentation. The real bug is in HIR lowering: `crates/vox-compiler/src/hir/lower/contracts.rs` injects postconditions as a flat append after the full function body, **not at each return site**. See [implementation plan](llm-target-language-implementation-plan-2026.md#bug-1-contracts-postcondition-injection) for the real fix.

**Finding (corrected):** `postconditions` and `VerifyMode` are implemented in the AST. However, `crates/vox-compiler/src/hir/lower/contracts.rs` `inject_contracts` has a bug in the `VerifyMode::Full` branch: postconditions are appended at the end of the function body with `new_body.append(&mut body); for post in &f.postconditions { ... new_body.push(HirStmt::Expr{...}) }`. Functions with early `return` statements never trigger postcondition checks.

**Solution:** Traverse `body: Vec<HirStmt>`, find every `HirStmt::Return { .. }`, and inject the postcondition assertions immediately before each one. See [implementation plan](llm-target-language-implementation-plan-2026.md#bug-1-contracts-postcondition-injection).

- Owner: `vox-compiler` HIR lower | **Severity: Critical** | **Effort: Small**

---

### ~~Gap B-2: `VerifyMode` enum and `verify_mode` field missing from `FnDecl`~~ — **CORRECTION: IMPLEMENTED**

> [!NOTE]
> **Correction (2026-04-17):** `pub enum VerifyMode { Off, RequireOnly, Full }` and `pub verify_mode: VerifyMode` both exist in `crates/vox-ast/src/decl/fundecl.rs`. False positive from stale documentation.

archived_date: 2026-04-18
---

### Gap B-3: `is_fuzz` flag missing from `FnDecl`

**Finding:** `@fuzz` is in `token.rs` (`AtFuzz`) and the testing pipeline spec requires `pub is_fuzz: bool` on `FnDecl`. The parser does not set this flag when `@fuzz` is encountered.

**Solution:**
- Add `pub is_fuzz: bool` to `FnDecl`.
- Parser: set `is_fuzz = true` when `@fuzz` decorator is parsed.
- Codegen: `@fuzz` functions emit a `libFuzzer` entry point only in `vox ci fuzz` target.
- Owner: `vox-compiler` | **Severity: Medium** | **Effort: Small**

---

### ~~Gap B-4: `ForallDecl` AST node missing~~ — **CORRECTION: IMPLEMENTED**

> [!NOTE]
> **Correction (2026-04-17):** `pub struct ForallDecl { pub label: String, pub func: FnDecl, pub iterations: u32 }` exists in `crates/vox-ast/src/decl/fundecl.rs`. `Decl::Forall(ForallDecl)` variant is present. `vox-lsp/src/code_lens.rs` uses it correctly, emitting "▶ Run property (N iters)" code lenses. False positive from stale documentation.

archived_date: 2026-04-18
---

### Gap B-5: `@llm` annotation not in AST

**Finding:** `vox-language-testing-pipeline.md` defines `is_llm: bool` and `llm_model: Option<String>` on `FnDecl` for inline LLM-implemented functions. These fields are referenced in the doc but their presence in the actual `fundecl.rs` is unverified and almost certainly absent (no `AtLlm` token in `token.rs`).

**Impact:** The most novel LLM-native feature — functions whose bodies are generated at runtime — has no language surface.

**Solution:**
- Add `#[token("@llm")] AtLlm` to `token.rs`.
- Add `pub is_llm: bool`, `pub llm_model: Option<String>`, `pub llm_verify: LlmVerifyMode`, `pub llm_cache: bool` to `FnDecl`.
- Parser: parse `@llm(model = "...", verify = "strict", cache = true)` attribute syntax.
- Runtime: `vox-runtime` LLM dispatch when `is_llm = true` at call site.
- Owner: `vox-compiler`, `vox-runtime` | **Severity: Critical** | **Effort: Medium**

---

### ~~Gap B-6: Label string missing from `TestDecl`~~ — **CORRECTION: IMPLEMENTED**

> [!NOTE]
> **Correction (2026-04-17):** `pub struct TestDecl { pub label: String, pub func: FnDecl }` exists in `crates/vox-ast/src/decl/fundecl.rs`. `vox-lsp/src/code_lens.rs` uses `t.label` correctly. False positive from stale documentation.

archived_date: 2026-04-18
---

## Dimension C — HIR

### Gap C-1: `Option<T>` not structurally distinct in `HirType`

**Finding:** `HirType` in `stmt_expr.rs` is `Named(String) | Generic(String, Vec<HirType>) | Function | Tuple | Unit | Decimal`. An `Option<str>` is just `Generic("Option", [Named("str")])` — indistinguishable from any other generic at the type-checker level. There is no `HirType::Option(Box<HirType>)` variant that the type-checker can use to enforce non-null safety.

**Impact:** The non-null policy ([Zero-Shot Invariants research](research-ts-hallucination-zero-shot-invariants-2026.md)) cannot be enforced. LLMs can pass `None` to non-optional parameters without compile error.

**Solution:**
- Add `HirType::Option(Box<HirType>)` variant to `HirType`.
- Update HIR lowering to produce this variant when `Option[T]` is in surface type syntax.
- Update `typeck` to reject assigning `None`-typed expressions to non-`Option` bindings.
- Owner: `vox-compiler` HIR + typeck | **Severity: Critical** | **Effort: Medium**

---

### Gap C-2: `legacy_ast_nodes` still carries raw AST for components, hooks, and pages

**Finding:** `HirModule.legacy_ast_nodes: Vec<crate::ast::decl::Decl>` is marked `MigrationOnly` and owned by the TS codegen. Components, hooks, pages, contexts, and error boundaries pass through as raw AST nodes rather than typed HIR. This means the type-checker does not validate component props, hook parameter types, or page return shapes.

**Impact:** LLM-generated components can have wrong prop types that slip past the compiler. The WebIR projection (future) cannot be typed correctly.

**Solution:**
- Implement full HIR lowering for `HirComponent`, `HirHook`, `HirPage`, `HirContext`, `HirErrorBoundary`.
- Wire TS codegen to read from typed HIR vectors, not `legacy_ast_nodes`.
- Gate `legacy_ast_nodes.len() == 0` in CI (`vox ci run-body` already has hooks for this; surface it as hard failure).
- Owner: `vox-compiler` HIR + codegen_ts | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap C-3: No HIR-level representation of contract annotations

**Finding:** `@require(expr)` is parsed to `FnDecl.preconditions: Vec<Expr>`. HIR lowering in `lower/decl.rs` copies these as raw expressions but there is no dedicated HIR node type (`HirPrecondition`, `HirPostcondition`) and no `lower/contracts.rs` pass (the file is referenced in the testing pipeline doc but does not exist in `crates/vox-compiler/src/hir/lower/`).

**Solution:**
- Create `crates/vox-compiler/src/hir/lower/contracts.rs` implementing the three-mode contract emission (Off / RequireOnly / Full).
- Add `HirContract { kind: ContractKind, expr: HirExpr, span: Span }` to `hir/nodes/`.
- Thread contracts through `HirFn` as `pub preconditions: Vec<HirContract>`, `pub postconditions: Vec<HirContract>`.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Medium**

---

### Gap C-4: No cross-file name resolution

**Finding:** `HirImport { module_path: Vec<String>, item: String, span: Span }` stores import paths as unresolved strings. The `def_map.rs` module describes resolution maps but these operate only within a single module. There is no multi-file resolver that binds `import react.use_state` to the actual `HirFn` definition of `use_state`.

**Impact:** LLMs generating multi-file Vox programs will write imports that the compiler accepts but never validates. Type errors across file boundaries are invisible.

**Solution:**
- Implement `ModuleResolver` in `hir/def_map.rs` that takes a `Vec<HirModule>` and resolves cross-module references.
- Add `ResolvedImport { source_module: PathBuf, def_id: DefId }` to HIR.
- Run resolver pass in `pipeline.rs` after all modules are lowered.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap C-5: `HirValidationError.correction_hint` always `None`

**Finding:** `hir/validate.rs` defines `correction_hint: Option<String>` on `HirValidationError`. In the 380-line implementation, `correction_hint` is set to `Some(...)` in exactly **one** place (the `@scheduled` interval check). All other 30+ error paths emit `correction_hint: None`.

**Impact:** The [research directive](research-ts-hallucination-frontier-2026.md) is explicit: "The Vox compiler must output highly structured, exact error payloads optimized for LLM self-repair." A `None` hint means the LLM sees only "Table name is empty" with no suggestion.

**Solution:**
- For every `HirValidationError` call site, add a domain-specific `correction_hint`:
  - Empty name → `Some("Provide a non-empty identifier, e.g. 'my_table'")`
  - Empty route path → `Some("Add a path string, e.g. '/api/users'")`
  - Duplicate MCP resource URI → `Some("Each @mcp.resource must have a unique URI scheme and path")`
  - etc.
- Add `suggestion_code: Option<String>` field for a corrected code snippet.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Small** (mechanical, high ROI)

---

## Dimension D — Type Checker

### Gap D-1: Match exhaustiveness checker not verified to cover all `HirExpr::Match` sites

**Finding:** `typeck/checker/match_exhaust.rs` exists. However, the HIR has `HirExpr::Match` and `HirPattern` used in both function bodies and reactive component view expressions. It is unclear whether the exhaustiveness checker is called for match expressions inside reactive component views (which still go through `legacy_ast_nodes` in some paths).

**Solution:**
- Add a CI test: a `.vox` file with a non-exhaustive match on an ADT must produce a `TypeErrorKind::NonExhaustive` diagnostic.
- Add a fixture for match inside a `component` view body.
- Add the check to `vox-integration-tests/tests/typeck_test.rs`.
- Owner: `vox-compiler` typeck | **Severity: High** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap D-2: No non-null enforcement at the type level

**Finding:** The `typeck` unifier (`typeck/unify.rs`) unifies `HirType` values. Since `Option<T>` is just `Generic("Option", [T])` (see Gap C-1), the type checker has no special handling. A literal `None` assigned to `str` would need to be typed as `Option<str>` but without the structural distinction the unifier cannot catch this.

**Solution:** Depends on Gap C-1 (add `HirType::Option`). Once distinct, update `typeck/checker/expr.rs` to:
- Type `None` literal as `HirType::Option(HirType::Unit)` initially, then unify.
- Reject `HirType::Option(T)` unifying with non-Option types.
- Owner: `vox-compiler` typeck | **Severity: Critical** | **Effort: Medium** (after C-1)

---

### Gap D-3: `@pure` is metadata only — purity is never verified

**Finding:** `HirFn.is_pure: bool` is set from `@pure` annotations. There is no compiler pass that verifies pure functions have no side effects (no `db.*` calls, no `spawn`, no mutable state writes, no I/O).

**Solution:**
- Add `PurityChecker` pass in `typeck/` that walks `HirStmt` and `HirExpr` of `@pure` functions.
- Flag `DbTableOp`, `Spawn`, `MethodCall` on mutable state, and HTTP calls as impure.
- Emit `Diagnostic::PureViolation` with correction hint.
- Owner: `vox-compiler` typeck | **Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap D-4: No implicit coercion detection

**Finding:** The [zero-shot invariants research](research-ts-hallucination-zero-shot-invariants-2026.md) identifies zero implicit coercion as a core LLM reliability enhancer. The Vox type checker has no explicit pass that catches and rejects cases like `let x: int = "42"` or `fn f(x: str) ... f(42)`.

**Solution:**
- In `typeck/unify.rs`, when two types fail to unify, check if one is a "coercible" source for the other (e.g., `IntLit` → `str`).
- If coercible, emit a `Diagnostic::ImplicitCoercion` error (not a warning) requiring explicit `as str` or `to_string()`.
- Define the coercion matrix in `typeck/policy.rs`.
- Owner: `vox-compiler` typeck | **Severity: High** | **Effort: Medium**

---

## Dimension E — Codegen

### Gap E-1: TS codegen still reads raw AST for many constructs

**Finding:** `codegen_ts/` reads from `HirModule.legacy_ast_nodes`, `HirComponent` (which wraps raw `ComponentDecl`), `HirV0Component`, `HirHook`, etc. These are wrapper newtypes over raw AST types. The codegen emits TypeScript from unvalidated AST rather than typed HIR.

**Impact:** Type errors in component bodies escape the compiler pipeline and surface only as TypeScript compiler errors downstream, breaking the LLM's self-repair loop.

**Solution:** Depends on Gap C-2. As HIR lowering is completed for each construct, update `codegen_ts/component.rs`, `codegen_ts/reactive.rs` to read from `HirReactiveComponent`, `HirHook`, etc.

- Owner: `vox-compiler` codegen_ts | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap E-2: No HIR-to-WASM codegen for test sandboxing

**Finding:** The [five-stage delivery gate](vox-language-testing-pipeline.md#42-who-triggers-the-gate) requires `@test` functions to run in a WASI sandbox. The current codegen targets Rust (via codegen_rust) and TypeScript (via codegen_ts). There is no WASM/WASI compilation path.

**Solution:**
- Add `vox-compiler/src/codegen_wasm/` using `wasm-bindgen` or direct `wasm32-wasi` target via Rust codegen backend.
- For test execution: use existing Rust codegen path with `--target wasm32-wasi`; the test harness wasmtime-executes the output.
- Owner: `vox-compiler`, `vox-test-harness` | **Severity: High** | **Effort: Large**

---

## Dimension F — Grammar Export & Constrained Decoding

### Gap F-1: GBNF path is actively dangerous for Vox grammar

**Finding:** `crates/vox-grammar-export/src/gbnf.rs` exists and `llm_prompt.rs` delegates to `vox_grammar_export::compact_prompt`. The [grammar-constrained decoding research](research-grammar-constrained-decoding-2026.md) documents CVE-2026-2069: llama.cpp's GBNF engine has a stack-based buffer overflow on nested repetition patterns. Vox's grammar (nested JSX, recursive match, block expressions) will reliably trigger this.

**Solution:**
- Deprecate `gbnf.rs` as a serving target; keep for reference only.
- Add `vox-grammar-export/src/xgrammar.rs`: export the Vox EBNF in XGrammar-2-compatible format.
- Add `vox-grammar-export/src/llguidance.rs`: export Lark-compatible format for llguidance serving.
- Document the recommended serving stack (XGrammar-2 on vLLM for batch; llguidance for Rust-native serving).
- Owner: `vox-grammar-export` | **Severity: Critical** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap F-2: Grammar prompt is compact text, not a structured JSON repair payload

**Finding:** `vox_grammar_prompt()` emits a compact text prompt. LLM self-repair needs structured JSON: exact AST node path, error kind, permitted alternatives at the failure point, and a corrected skeleton.

**Solution:**
- Add `vox_grammar_export::repair_payload::emit_repair_payload(error: &Diagnostic) -> serde_json::Value`.
- Payload schema: `{ "error_kind": "...", "span": { "start": N, "end": N }, "context": "...", "permitted_next_tokens": [...], "suggested_correction": "...", "grammar_rule": "..." }`.
- Publish schema to `contracts/grammar/repair-payload.v1.schema.json`.
- Owner: `vox-grammar-export`, `vox-compiler` | **Severity: High** | **Effort: Medium**

---

### Gap F-3: No grammar-breaking-change CI guard

**Finding:** `vox-grammar-export/src/versioning.rs` tracks grammar versions. But there is no CI check that alerts when a token is added/removed from the lexer and the grammar export is not updated. An LLM fine-tuned on grammar version N will generate invalid code after a silent grammar version N+1 change.

**Solution:**
- Add `vox ci grammar-version-check`: hash the EBNF export and compare to `contracts/grammar/grammar.v<N>.sha256`.
- On mismatch: require a manual `vox grammar bump-version --reason="..."` before CI passes.
- Store bump history in `contracts/grammar/CHANGELOG.md`.
- Owner: `vox-grammar-export`, CI | **Severity: High** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap F-4: Grammar export omits decorator semantics

**Finding:** The EBNF and compact prompt exports describe the syntactic grammar (tokens, rules) but do not encode the *semantic* constraints of decorators. For example, `@table type T` implies the type must have only primitive/scalar fields; `@mcp.resource fn f()` implies f takes no parameters. These are enforced in `hir/validate.rs` but not expressed in the grammar prompt.

**Impact:** LLMs can generate syntactically valid but semantically invalid decorated declarations that compile only to validation errors.

**Solution:**
- Add `vox-grammar-export/src/semantic_hints.rs`: a structured JSON document listing per-decorator constraints.
- Include in `vox_grammar_prompt()` output as a `"semantic_constraints"` section.
- Example: `{ "@mcp.resource": { "param_count": 0, "return_type": "str | list[str]" } }`.
- Owner: `vox-grammar-export` | **Severity: Medium** | **Effort: Small**

---

### Gap F-5: No Stream of Revision / mid-generation backtracking support

**Finding:** The grammar-constrained decoding research recommends ["Stream of Revision"](research-grammar-constrained-decoding-2026.md#62-stream-of-revision-and-orchestrated-inference) — a special revision token that lets the LLM backtrack and edit its own generated history mid-generation. No such mechanism exists in Vox's serving or grammar export infrastructure.

**Solution:**
- Define a special `// vox:revise` comment token that the grammar allows at statement boundaries.
- In constrained decoding mode: when the serving engine sees `// vox:revise`, activate edit-mode allowing deletion of the last N tokens.
- Document protocol in `contracts/grammar/stream-of-revision.v1.md`.
- Owner: `vox-grammar-export`, `vox-runtime` inference | **Severity: Medium** | **Effort: Large**

archived_date: 2026-04-18
---

## Dimension G — Diagnostics & LLM Feedback

### Gap G-1: Diagnostics have no machine-readable JSON export path

**Finding:** `typeck/diagnostics.rs` defines `Diagnostic { category, message, span, severity, correction_hint }`. The CLI's `vox check` outputs diagnostics as human-readable text. There is no `--format=json` flag that emits diagnostics as a JSON array suitable for an LLM self-repair loop.

**Solution:**
- Add `vox check --format=json` flag emitting `Vec<DiagnosticJson>` where `DiagnosticJson` includes byte-offset span, category enum value, severity, correction_hint, and a suggested_fix field.
- Use the output in the [five-stage delivery gate](vox-language-testing-pipeline.md) Stage 1 and Stage 2 repair prompts.
- Owner: `vox-cli`, `vox-compiler` | **Severity: Critical** | **Effort: Small**

---

### Gap G-2: Pattern match missing-case enumeration not in diagnostics

**Finding:** When `match_exhaust.rs` finds a non-exhaustive match, it emits a diagnostic. However, the diagnostic message does not enumerate the *specific* missing patterns. For an LLM repairing a non-exhaustive match, seeing "non-exhaustive match" is useless; it needs "missing arms: `Err(e)`, `None`".

**Solution:**
- In `typeck/checker/match_exhaust.rs`, collect `missing_patterns: Vec<String>` during exhaustiveness checking.
- Include in diagnostic: `"Non-exhaustive match on Result[str]: missing arms: Err(e)"`.
- Include in `correction_hint`: a code template for the missing arms.
- Owner: `vox-compiler` typeck | **Severity: High** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap G-3: Parser errors do not produce partial/skeleton AST

**Finding:** The parser returns `Result<Module, Vec<ParseError>>`. On failure, the entire AST is discarded. `research-fuzzy-parsing-2026.md` describes a planned "Skeleton AST" where successfully parsed nodes are retained even when a later error occurs. The `parse_str` test helper calls `.unwrap_or_else(|e| panic!())` confirming no partial output is available.

**Impact:** LLMs generating 200-line `.vox` files where a single brace is wrong lose all diagnostic anchoring on the 199 correct lines. The repair prompt has no AST context.

**Solution:**
- Refactor the descent parser to accumulate parsed declarations into a `Module` even when individual declarations fail.
- Failed declaration: emit a `Decl::Error { span, error: ParseError }` sentinel node.
- Return `(Module, Vec<ParseError>)` — always return a module, even if partial.
- Update all callers to handle `(module, errors)` pair.
- Owner: `vox-compiler` parser | **Severity: High** | **Effort: Large**

---

### Gap G-4: No streaming incremental parse

**Finding:** The parser operates on a complete token stream. For real-time LLM generation feedback (sending each line to the compiler as the LLM streams output), there is no incremental or streaming parse API.

**Solution:**
- Add `IncrementalParser` in `parser/` that accepts tokens one at a time and returns partial diagnostics per statement boundary.
- Initially: detect unclosed blocks and emit `Diagnostic::UnclosedBlock` while still accumulating.
- Expose via `vox-lsp` for live diagnostics during typing.
- Owner: `vox-compiler` parser, `vox-lsp` | **Severity: Medium** | **Effort: Large**

archived_date: 2026-04-18
---

## Dimension H — Testing Infrastructure

### Gap H-1: No `vox test` CLI command

**Finding:** `crates/vox-cli/src/commands/` has no `test.rs`. The [testing pipeline](vox-language-testing-pipeline.md#73-cli-vox-test) specifies a complete `vox test` command. Without it, `@test` decorated functions are parsed and lowered but never executed.

**Solution:**
- Add `crates/vox-cli/src/commands/test.rs` implementing:
  - Collect all `HirFn` with `is_test = true` from the module.
  - Compile in `dev` mode (contracts as `debug_assert!`).
  - Run each test function; capture pass/fail.
  - Print results in a format matching `cargo test` output.
- Add `vox test --filter=<pattern>`, `--forall-iterations=N`, `--coverage` flags.
- Owner: `vox-cli` | **Severity: Critical** | **Effort: Medium**

---

### Gap H-2: No property-based test runner for `@forall`

**Finding:** `HirForall { label, iterations, func }` is in `hir/nodes/decl.rs`. There is no execution engine for property-based tests. `vox-test-harness/` has `assertions.rs`, `barriers.rs` but no PBT strategy generators.

**Solution:**
- Add `vox-test-harness/src/pbt/` with `Strategy<T>` trait and built-in generators for `int`, `str`, `bool`, `list[T]`.
- Shrinker: on failure, bisect the input space to find the minimal counterexample.
- Wire into `vox test` command via `ForallRunner`.
- Owner: `vox-test-harness`, `vox-cli` | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap H-3: No fuzz harness CI integration

**Finding:** `@fuzz` token exists. `research-fuzzy-parsing-2026.md` references fuzz corpus work. There are scratch `*.vox` files at the repo root (`error_test.vox`, `scratch_test.vox`, etc.) but no structured fuzz corpus and no `vox ci fuzz` command.

**Solution:**
- Move root-level scratch `.vox` files to `tests/fuzz-corpus/` with a naming convention.
- Add `crates/vox-cli/src/commands/ci/fuzz.rs` implementing `vox ci fuzz`.
- `@fuzz fn` declarations compile to `cargo-fuzz` harness entry points under `fuzz/` directory.
- Owner: `vox-cli` | **Severity: Medium** | **Effort: Medium**

---

### Gap H-4: Formatter has no golden output or idempotency tests

**Finding:** `crates/vox-compiler/src/fmt/` implements expression, statement, and printer formatting. There are no tests in this module's `tests` submodule and no golden output fixtures. The corpus research references `canonicalize_vox` but its idempotency (fmt(fmt(x)) == fmt(x)) is untested.

**Solution:**
- Add `crates/vox-compiler/src/fmt/tests.rs` with:
  - Roundtrip test: `parse(fmt(parse(src))) == parse(src)` for 20+ representative programs.
  - Idempotency test: `fmt(src) == fmt(fmt(src))`.
  - Golden fixtures for each decl type stored in `tests/fixtures/fmt/`.
- Owner: `vox-compiler` | **Severity: Medium** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap H-5: No end-to-end LLM-generation-then-compile integration test

**Finding:** There are integration tests for the compiler (`vox-integration-tests/tests/typeck_test.rs`) but no test that exercises the full loop: LLM output → parse → HIR lower → typeck → diagnostic → repair prompt → second LLM output → parse.

**Solution:**
- Add `crates/vox-integration-tests/tests/llm_roundtrip_test.rs`:
  - Use a set of "intentionally broken" `.vox` fixtures (wrong type, missing arm, etc.).
  - Run through the compiler; assert expected diagnostic categories.
  - Simulate a repair prompt; apply a pre-written correction; assert the second pass succeeds.
- Gate in CI as `vox ci llm-roundtrip`.
- Owner: `vox-integration-tests`, CI | **Severity: High** | **Effort: Medium**

---

### Gap H-6: GRAMMAR_SSOT.md not tested against token.rs in CI

**Finding:** See Gap A-1 for the divergence detail. There is no CI step that programmatically verifies the SSOT keyword table matches the lexer.

**Solution:** See Gap A-1 solution (`vox ci grammar-ssot-parity`).

archived_date: 2026-04-18
---

## Dimension I — Contract System

### Gap I-1: `@ensure` annotation not parsed

**Finding:** `@ensure` token exists (`AtEnsure`) but the parser does not produce AST nodes for it. See Gap B-1 for full context and solution.

---

### Gap I-2: `@invariant` on type declarations not parsed

**Finding:** `@invariant` token exists (`AtInvariant`) but there is no `invariants: Vec<Expr>` field on `TypeDefDecl` or the ADT definition in `ast/decl/typedef.rs`. The testing pipeline spec requires type invariants for Design by Contract.

**Solution:**
- Add `pub invariants: Vec<Expr>` to `TypeDefDecl`.
- Parser: accumulate `@invariant(expr)` before the `type` keyword into this field.
- HIR: `HirTypeDef` gains `invariants: Vec<HirExpr>`.
- Typeck: emit invariant checks at construction sites of the type.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap I-3: `contracts.rs` HIR lowering pass does not exist

**Finding:** `crates/vox-compiler/src/hir/lower/` contains `async_flags.rs`, `contracts.rs` (referenced in the testing pipeline doc as a planned file), `db_select_normalize.rs`, `decl.rs`, `expr.rs`, `expr_db.rs`, `mod.rs`, `stmt.rs`. Checking the actual listing: `contracts.rs` IS in the directory listing (line 106 of the earlier file list). This file needs to be verified for completeness.

**Solution:**
- Read `crates/vox-compiler/src/hir/lower/contracts.rs` and verify it implements the three-mode emission.
- If stub-only: implement the full `Off | RequireOnly | Full` contract emission modes.
- Add integration test: a function with `@require(x > 0)` in `Full` mode must emit a runtime assertion that panics on `x = -1`.
- Owner: `vox-compiler` | **Severity: High** | **Effort: Medium**

---

### Gap I-4: No five-stage delivery gate skill

**Finding:** `vox-skills/` (formerly `vox-ars`) contains skill definitions but no `vox.testing.validate` skill implementing the five-stage gate from the [testing pipeline](vox-language-testing-pipeline.md#74-ars-skill-voxtestingvalidate-delivery-gate).

**Solution:**
- Add `crates/vox-skills/src/skills/testing_validate.rs` implementing `ValidateVoxCodeSkill`.
- Stage 1: parse. Stage 2: HIR lower + typeck. Stage 3: contract probe on edge inputs. Stage 4: WASI sandbox test execution. Stage 5: tag output.
- Repair loop: 5 iterations max; frozen `@require`/`@ensure`; structured repair prompt via Gap F-2's repair payload.
- Owner: `vox-skills`, `vox-compiler`, `vox-test-harness` | **Severity: Critical** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap I-5: No WASI sandbox for test execution

**Finding:** The delivery gate Stage 4 requires executing `@test` functions in a WASI sandbox. No WASI execution engine is wired into `vox-test-harness/` or `vox-runtime/`.

**Solution:**
- Add `wasmtime = "..."` dependency to `vox-test-harness`.
- `WasiSandbox::run_tests(wasm_bytes: &[u8]) -> Vec<TestResult>` using `wasmtime::Engine` in isolated memory.
- Sandbox has no network access, no filesystem write access.
- Owner: `vox-test-harness` | **Severity: High** | **Effort: Large**

---

## Dimension J — Training / MENS

### Gap J-1: GRPO reward function uses additive structure — should be multiplicative gating

**Finding:** The reward is `0.45 × r_syntax + 0.25 × r_test + 0.10 × r_coverage + 0.20 × r_routing_efficiency`. The [GRPO gap analysis](research-grpo-gaps-and-adjustments-2026.md) concludes this is wrong: a 60% syntax weight causes reward hacking and gradient stagnation. The correct form is multiplicative: `R = r_syntax × (w₁ × r_test + w₂ × r_coverage)`.

**Solution:**
- Update `mens/config/reward_weights.yaml` (or wherever reward weights live) to implement multiplicative gating.
- If `r_syntax = 0`, the total reward is 0 regardless of test/coverage scores.
- Remove `r_routing_efficiency` as a primary reward component; use it as a secondary shaping signal only.
- Add regression test: a syntactically invalid output must receive `reward = 0`.
- Owner: `vox-cli` mens pipeline | **Severity: Critical** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap J-2: No DAPO mechanics / median-centered advantage estimation

**Finding:** Vanilla GRPO with k=8 is statistically unstable per the [GRPO research](research-grpo-gaps-and-adjustments-2026.md). The research recommends DAPO with median-centering to insulate gradient updates from reward hacks.

**Solution:**
- Replace mean-based advantage baseline with median across k=8 rollouts.
- Eliminate KL-divergence penalty (conserves VRAM on RTX 4080).
- Add `use_median_baseline: bool` config flag in MENS training config.
- Owner: `vox-populi` training pipeline | **Severity: High** | **Effort: Medium**

---

### Gap J-3: No negative sample reinforcement for failed parses

**Finding:** The current pipeline separates invalid-parse outputs into a "separate SFT pipeline." The GRPO gaps research recommends ingesting failures directly as hard negatives in the RL loop with `reward = 0`.

**Solution:**
- Include failed-parse rollouts in the GRPO batch.
- Assign reward `0` (not filtered out) so the advantage estimator produces negative advantages.
- This implements Negative Sample Reinforcement (NSR) for syntax errors.
- Owner: `vox-populi` training pipeline | **Severity: High** | **Effort: Small**

archived_date: 2026-04-18
---

### Gap J-4: Training dataset below GRPO stability threshold

**Finding:** `organic_vox.jsonl` has fewer than the 100K examples needed for CPT (per `vox-lang-training-ssot-2026.md`). The GRPO gaps research estimates GRPO needs 8K–50K prompts for stability; a 500-pair dataset will overfit catastrophically.

**Solution:**
- Implement curriculum generative seeding: use Qwen2.5-Coder to mutate existing high-quality pairs.
- Add `vox corpus expand --strategy=mutate --target-count=8000` command.
- Implement Anna Karenina sampling to balance batch distribution with known negatives.
- Track corpus size in `vox mens corpus stats` output with a threshold warning.
- Owner: `vox-cli` corpus, `vox-populi` | **Severity: Critical** | **Effort: Medium**

---

### Gap J-5: AST-aware tokenization (syntax-weighted CE loss) not implemented

**Finding:** `docs/src/architecture/ast-token-alignment-2026.md` (referenced in research-index.md) specifies mapping AST spans to token streams for syntax-critical loss weighting (identifiers, types, control flow at k=2.5–5.0). This is listed as research, not implementation.

**Solution:**
- Implement `AstTokenAligner` in `vox-populi/src/training/` that:
  1. Parses the `.vox` source to get AST spans.
  2. Aligns byte offsets with HuggingFace tokenizer offsets.
  3. Emits per-token `loss_weight` values in the JSONL training pairs.
- Wire into the CE loss computation during QLoRA fine-tuning.
- Owner: `vox-populi` | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap J-6: Grammar-constrained decoding not integrated with inference server

**Finding:** `vox-grammar-export` exports EBNF/GBNF/Lark formats. But there is no code in `vox-runtime/src/inference_env.rs` or `vox-cli/src/commands/ai/serve/` that activates grammar-constrained decoding during MENS inference.

**Solution:**
- Add `grammar_constrained: bool` and `grammar_engine: GrammarEngine` (XGrammar2 | LLGuidance | Off) to inference config.
- When `grammar_constrained = true`, pass the EBNF export to the serving backend as a structured output constraint.
- Wire into `vox ai serve` handler.
- Owner: `vox-runtime`, `vox-cli` | **Severity: High** | **Effort: Large**

---

### Gap J-7: No phonetic keyword matching for mens-training mode

**Finding:** `research-fuzzy-parsing-2026.md` describes a planned "Phonetic Similarity" feature: if an LLM emits `compnent` instead of `component`, the lexer identifies the high-probability intent and emits a `Warn` instead of hard `Error` in `mens-training` mode. This feature does not exist.

**Solution:**
- Add `LexerMode { Strict, MensTraining }` enum to the lexer.
- In `MensTraining` mode: after a lex error, run edit-distance check against all keywords.
- If nearest keyword is within Levenshtein distance 2: emit `Warn(SuggestedKeyword)` and continue.
- Gate via `VOX_LEXER_MODE=mens-training` env var resolved at parse time.
- Owner: `vox-compiler` lexer | **Severity: Medium** | **Effort: Medium**

archived_date: 2026-04-18
---

## Dimension K — LSP

### Gap K-1: No semantic token provider

**Finding:** `crates/vox-lsp/src/` has `completions.rs`, `symbols.rs`, `code_lens.rs`, `grammar.rs`, but no `semantic_tokens.rs`. Without semantic tokens, the LSP only provides basic TextMate grammar highlighting (if the VSCode extension is installed). Semantic tokens enable LLM-aware highlighting (e.g., marking `@llm`-generated functions with a different color).

**Solution:**
- Add `vox-lsp/src/semantic_tokens.rs` implementing `textDocument/semanticTokens/full`.
- Token types: `function`, `decorator`, `keyword`, `type`, `llm_generated`, `contract_annotation`.
- Owner: `vox-lsp` | **Severity: Medium** | **Effort: Medium**

---

### Gap K-2: No go-to-definition across files

**Finding:** `vox-lsp/src/grammar.rs` provides hover and completion within a single file. Cross-file go-to-definition requires the multi-file resolver from Gap C-4. Without it, clicking on an imported function does nothing.

**Solution:** Depends on Gap C-4. Once `ModuleResolver` is implemented, wire into LSP `textDocument/definition` handler.

- Owner: `vox-lsp`, `vox-compiler` | **Severity: Medium** | **Effort: Medium** (after C-4)

archived_date: 2026-04-18
---

### Gap K-3: `@test` CodeLens not implemented

**Finding:** `vox-lsp/src/code_lens.rs` exists. The [testing pipeline](vox-language-testing-pipeline.md#75-lsp-test-codelens-and-validation-badge) specifies "▶ Run test" lenses above each `TestDecl`. This is not implemented.

**Solution:**
- In `code_lens.rs`, iterate `HirModule.tests` and emit a `CodeLens { range: test_span, command: "vox.runTest" }` for each.
- Register the `vox.runTest` command in the VSCode extension.
- Owner: `vox-lsp`, `vox-vscode` | **Severity: Medium** | **Effort: Small**

---

### Gap K-4: No AI-validated badge in LSP

**Finding:** Functions with `is_llm: true` that have passed the delivery gate should show `✓ AI-validated (model · tests passed · @ensure verified)` in the LSP gutter. This requires Gap B-5 (@llm annotation) and the delivery gate (Gap I-4) to exist first.

**Solution:** Depends on Gap B-5 and Gap I-4. Add badge emission to `code_lens.rs` when `HirFn.is_llm && delivery_gate_passed`.

- Owner: `vox-lsp` | **Severity: Medium** | **Effort: Small** (after B-5 and I-4)

archived_date: 2026-04-18
---

### Gap K-5: No inlay hints for inferred types

**Finding:** The LSP has no `textDocument/inlayHint` implementation. Variables declared as `let x = ...` with no type annotation show no hint for the inferred type. This is particularly important for LLM-generated code where type annotations are often omitted.

**Solution:**
- Add `vox-lsp/src/inlay_hints.rs`.
- For each `HirStmt::Let` with no explicit `type_ann` but an inferred type from typeck: emit an inlay hint `": <type>"`.
- Owner: `vox-lsp` | **Severity: Low** | **Effort: Medium**

---

## Dimension L — LLM-Native Language Features

### Gap L-1: No IR-first / JSON AST export path

**Finding:** The [research frontier](research-ts-hallucination-frontier-2026.md) argues the ideal LLM-native language should operate as a semantic graph or structured IR (e.g., JSON), with human-readable text as a projection. Currently Vox is purely text-in / text-out. There is no path for an LLM to directly emit a JSON AST that bypasses text parsing.

**Solution:**
- Define `vox-ast-ir.v1.schema.json` in `contracts/` as the canonical JSON representation of `HirModule`.
- The `HirModule` struct already derives `serde::Serialize`/`Deserialize` — expose as `vox check --emit-hir-json`.
- Add `vox build --from-hir-json <file>` to codegen from a JSON AST directly.
- This enables an LLM serving path: LLM → JSON HIR → compiler validation → codegen.
- Owner: `vox-cli`, `vox-compiler` | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap L-2: No contract-first generation prompt template

**Finding:** The [testing pipeline](vox-language-testing-pipeline.md#44-what-logically-correct-means-the-oracle-problem-solved-practically) identifies contract-first generation (LLM writes `@require`/`@ensure` before the body) as the strongest oracle pattern. There is no canonical prompt template in `vox-grammar-export/` or `vox-runtime/` that implements this pattern.

**Solution:**
- Add `vox-grammar-export/src/prompt_templates.rs` with `ContractFirstTemplate` that:
  1. Shows the function signature.
  2. Instructs the LLM to write `@require` and `@ensure` annotations first.
  3. Then generate the body satisfying those contracts.
- Wire into `vox ai generate --contract-first` flag.
- Owner: `vox-grammar-export`, `vox-cli` | **Severity: High** | **Effort: Small**

---

### Gap L-3: `@llm` annotation has no runtime dispatch

**Finding:** Even if Gap B-5 adds the `@llm` annotation to the AST, `vox-runtime/src/llm/` would need to intercept calls to `is_llm = true` functions, generate a body via the LLM, run the delivery gate, and cache the result. None of this exists.

**Solution:**
- Add `LlmFunctionDispatcher` in `vox-runtime/src/llm/dispatch.rs`.
- On call to an `is_llm = true` HirFn: construct a contract-first prompt, invoke the configured model, run five-stage gate, cache in Arca on success.
- Cache key: hash of function signature + preconditions + model version.
- Owner: `vox-runtime` | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

### Gap L-4: No inline LLM function result caching (Arca integration)

**Finding:** `vox-runtime/src/store.rs` and `storage.rs` handle persistence. But there is no "Arca" cache layer for `@llm` function results that survives across invocations and invalidates on model version change.

**Solution:**
- Add `ArcaCache` in `vox-runtime/src/arca.rs`: key = `(fn_signature_hash, model_version)`, value = generated function body + delivery gate result.
- Cache miss: generate fresh. Cache hit: skip generation, re-run contract probe as sanity check.
- Add `vox clavis doctor` check for Arca storage health.
- Owner: `vox-runtime` | **Severity: Medium** | **Effort: Medium**

---

### Gap L-5: No multi-file project-level type coherence check

**Finding:** The compiler validates individual `.vox` files. But when an LLM generates a multi-file Vox project (e.g., `routes.vox`, `models.vox`, `api.vox`), there is no cross-file coherence check. A `@table type User` in `models.vox` might be referenced as `User` in `api.vox` but the compiler sees them as separate compilations.

**Solution:**
- Implement `ProjectCompiler` in `vox-cli/src/build_service.rs` that:
  1. Discovers all `.vox` files in a project.
  2. Parses and lowers each to `HirModule`.
  3. Runs `ModuleResolver` (Gap C-4) across all modules.
  4. Runs typeck with the unified symbol table.
- Expose as `vox check --project` flag.
- Owner: `vox-cli`, `vox-compiler` | **Severity: High** | **Effort: Large**

archived_date: 2026-04-18
---

## Dimension M — Documentation & SSOT

### Gap M-1: `vox-lang-training-ssot` says `ret` deprecated but test fixtures still use it

**Finding:** `docs/src/architecture/vox-lang-training-ssot-2026.md` states `return` is canonical. `crates/vox-compiler/src/parser/descent/tests.rs` uses `ret` in 15+ test strings. `crates/vox-cli/tests/fixtures/golden_rust_import_lowering.vox` may also use `ret`.

**Solution:**
- Run `rg "ret " crates/ examples/ tests/ --glob "*.vox"` and update all occurrences to `return`.
- Add negative fixture: `tests/fixtures/negative/ret_deprecated.vox` that asserts `ret` produces a deprecation warning.
- Owner: docs, `vox-compiler` | **Severity: High** | **Effort: Small**

---

### Gap M-2: Tree-sitter grammar may not match compiler grammar

**Finding:** `tree-sitter-vox/GRAMMAR_SSOT.md` is a documentation file, not a generated artifact. The actual `tree-sitter-vox/grammar.js` (or equivalent) needs to be verified against the compiler's EBNF export.

**Solution:**
- Add `vox ci tree-sitter-parity`: generate EBNF from `vox-grammar-export` and diff it against the tree-sitter grammar rules.
- On diff: CI fails with a specific "tree-sitter grammar is out of sync with compiler grammar" error.
- Owner: `vox-grammar-export`, CI | **Severity: High** | **Effort: Medium**

archived_date: 2026-04-18
---

### Gap M-3: Research index missing crosslinks to several key architecture documents

**Finding:** `research-index.md` does not link to: `vox-language-testing-pipeline.md`, `research-fuzzy-parsing-2026.md`, `research-grammar-constrained-decoding-2026.md`, `ast-token-alignment-2026.md` (referenced inline but listed separately), or this document.

**Solution:**
- Add a new section "LLM Target Language Pipeline" to `research-index.md` linking all these documents.
- Update crosslinks in each document to reference this gap analysis.
- Owner: docs | **Severity: Low** | **Effort: Small**

---

## Summary Priority Matrix

| # | Gap | Dimension | Severity | Effort |
|---|-----|-----------|----------|--------|
| A-1 | GRAMMAR_SSOT / token.rs divergence | Syntax | **Critical** | Small |
| B-1 | `@ensure` postconditions missing from FnDecl | AST | **Critical** | Small |
| B-5 | `@llm` annotation not in AST | AST | **Critical** | Medium |
| C-1 | `Option<T>` not structurally distinct in HIR | HIR | **Critical** | Medium |
| F-1 | GBNF path is dangerous for Vox grammar | Grammar | **Critical** | Medium |
| G-1 | No machine-readable JSON diagnostic export | Diagnostics | **Critical** | Small |
| I-4 | No five-stage delivery gate skill | Contracts | **Critical** | Large |
| J-1 | GRPO reward additive not multiplicative | Training | **Critical** | Small |
| J-4 | Training dataset below GRPO threshold | Training | **Critical** | Medium |
| H-1 | No `vox test` CLI command | Testing | **Critical** | Medium |
| A-2 | `ret` deprecation not enforced | Syntax | **High** | Small |
| A-6 | Decorator namespace divergence | Syntax | **High** | Small |
| B-2 | `VerifyMode` enum missing from FnDecl | AST | **High** | Small |
| B-4 | `ForallDecl` AST node missing | AST | **High** | Small |
| C-2 | `legacy_ast_nodes` still carries raw AST | HIR | **High** | Large |
| C-3 | No HIR-level contract annotations | HIR | **High** | Medium |
| C-4 | No cross-file name resolution | HIR | **High** | Large |
| C-5 | `correction_hint` always None | HIR | **High** | Small |
| D-1 | Match exhaustiveness not verified to cover all sites | Typeck | **High** | Small |
| D-2 | No non-null enforcement at type level | Typeck | **Critical** | Medium |
| D-4 | No implicit coercion detection | Typeck | **High** | Medium |
| E-2 | No WASM/WASI codegen for test sandboxing | Codegen | **High** | Large |
| F-2 | Grammar prompt not a structured JSON repair payload | Grammar | **High** | Medium |
| F-3 | No grammar-breaking-change CI guard | Grammar | **High** | Small |
| G-2 | Pattern match missing-case not enumerated in diagnostics | Diagnostics | **High** | Small |
| G-3 | Parser returns binary fail, not skeleton AST | Diagnostics | **High** | Large |
| H-2 | No property-based test runner for `@forall` | Testing | **High** | Large |
| H-5 | No LLM-roundtrip integration test | Testing | **High** | Medium |
| I-1 | `@ensure` not parsed (see B-1) | Contracts | **Critical** | Small |
| I-2 | `@invariant` on types not parsed | Contracts | **High** | Medium |
| I-3 | `contracts.rs` pass needs verification | Contracts | **High** | Medium |
| I-5 | No WASI sandbox for test execution | Contracts | **High** | Large |
| J-2 | No DAPO / median-centered advantage | Training | **High** | Medium |
| J-3 | No negative sample reinforcement | Training | **High** | Small |
| J-5 | AST-aware tokenization not implemented | Training | **High** | Large |
| J-6 | Grammar-constrained decoding not on inference server | Training | **High** | Large |
| L-1 | No IR-first / JSON AST export path | LLM-Native | **High** | Medium |
| L-2 | No contract-first generation prompt template | LLM-Native | **High** | Small |
| L-3 | `@llm` runtime dispatch does not exist | LLM-Native | **High** | Large |
| L-5 | No multi-file project-level type coherence check | LLM-Native | **High** | Large |
| M-1 | Tests still use deprecated `ret` keyword | Docs | **High** | Small |
| M-2 | Tree-sitter grammar not verified against compiler | Docs | **High** | Medium |
| A-3 | No string interpolation | Syntax | **High** | Medium |
| A-7 | `?.` optional chaining missing | Syntax | **Medium** | Medium |
| B-3 | `is_fuzz` missing from FnDecl | AST | **Medium** | Small |
| B-6 | Label string missing from TestDecl | AST | **Medium** | Small |
| D-3 | `@pure` purity never verified | Typeck | **Medium** | Medium |
| E-1 | TS codegen reads raw AST | Codegen | **High** | Large |
| F-4 | Grammar export omits decorator semantics | Grammar | **Medium** | Small |
| F-5 | No Stream of Revision support | Grammar | **Medium** | Large |
| G-4 | No streaming incremental parse | Diagnostics | **Medium** | Large |
| H-3 | No fuzz harness CI integration | Testing | **Medium** | Medium |
| H-4 | Formatter has no golden tests | Testing | **Medium** | Small |
| J-7 | No phonetic keyword matching (mens mode) | Training | **Medium** | Medium |
| K-1 | No semantic token provider in LSP | LSP | **Medium** | Medium |
| K-2 | No go-to-definition across files | LSP | **Medium** | Medium |
| K-3 | `@test` CodeLens not implemented | LSP | **Medium** | Small |
| K-5 | No inlay hints for inferred types | LSP | **Low** | Medium |
| L-4 | No Arca cache for `@llm` results | LLM-Native | **Medium** | Medium |
| A-4 | No multiline string literals | Syntax | **Medium** | Small |
| A-5 | No integer literal variants | Syntax | **Low** | Small |
| H-6 | GRAMMAR_SSOT not CI-tested against token.rs (see A-1) | Testing | **Critical** | Small |
| K-4 | No AI-validated badge in LSP | LSP | **Medium** | Small |
| M-3 | Research index missing crosslinks | Docs | **Low** | Small |

archived_date: 2026-04-18
---

## Recommended Execution Waves

### Wave 0 — Quick Critical Wins (1–3 days each, unblock everything else)

- **A-1 / H-6:** `vox ci grammar-ssot-parity` CI check + generate SSOT from token.rs
- **A-2:** `ret` deprecation warning + autofix + fixture updates
- **A-6:** Decorator namespace audit + AGENTS.md retired-symbol alignment
- **B-1 / I-1:** Add `postconditions: Vec<Expr>` to FnDecl + parser wiring
- **B-6:** Add `label: String` to TestDecl
- **C-5:** Add `correction_hint` strings to all `HirValidationError` call sites
- **G-1:** `vox check --format=json` diagnostic export
- **G-2:** Enumerate missing match arms in exhaustiveness error
- **J-1:** Multiplicative GRPO reward gating
- **J-3:** Negative sample reinforcement for failed parses

### Wave 1 — Language Surface Hardening (1–2 weeks)

- **B-2:** `VerifyMode` enum + `verify_mode` field on FnDecl
- **B-3:** `is_fuzz` flag on FnDecl
- **B-4:** `ForallDecl` AST node + HIR lowering
- **B-5:** `@llm` annotation (token + AST + HIR)
- **C-1 / D-2:** `HirType::Option` distinct variant + non-null typeck enforcement
- **D-1:** Exhaustiveness checker coverage test + reactive component fixture
- **D-4:** Implicit coercion detection in typeck
- **H-1:** `vox test` CLI command (minimal: collect + run `@test` fns)
- **I-2:** `@invariant` on type declarations
- **M-1:** Update all `ret` → `return` in test fixtures

### Wave 2 — Diagnostics & Repair Loop (1–2 weeks)

- **F-2:** Structured JSON repair payload from diagnostics
- **F-3:** Grammar-breaking-change CI guard
- **F-4:** Decorator semantic hints in grammar export
- **G-3:** Skeleton AST (partial module on parse failure)
- **L-2:** Contract-first generation prompt template
- **M-2:** Tree-sitter grammar CI parity check

### Wave 3 — Grammar Constrained Decoding Migration (1 week)

- **F-1:** Deprecate GBNF path; add XGrammar-2 and llguidance export formats
- **J-6:** Wire grammar-constrained decoding into `vox ai serve` inference handler
- **J-7:** Phonetic keyword matching in mens-training lexer mode

### Wave 4 — Contract System & Delivery Gate (2–3 weeks)

- **I-3:** Verify and complete `contracts.rs` HIR lowering pass
- **I-5:** WASI sandbox for test execution (wasmtime integration)
- **I-4:** Five-stage delivery gate skill (`vox.testing.validate`)
- **H-2:** Property-based test runner for `@forall`
- **E-2:** WASM/WASI codegen path for test compilation

### Wave 5 — Training Pipeline Upgrades (2 weeks)

- **J-2:** DAPO mechanics + median-centered advantage estimation
- **J-4:** Curriculum seeding to 8K+ corpus size
- **J-5:** AST-aware tokenization (syntax-weighted CE loss)
- **A-3:** String interpolation (template literals)
- **A-4:** Multiline string literals

### Wave 6 — LSP & IDE Polish (1 week)

- **K-1:** Semantic token provider
- **K-3:** `@test` CodeLens
- **K-5:** Inlay hints for inferred types
- **K-4:** AI-validated badge (after B-5 + I-4)

### Wave 7 — Multi-File & IR-First (2–3 weeks)

- **C-4:** `ModuleResolver` for cross-file name resolution
- **L-5:** `ProjectCompiler` for project-level type coherence
- **K-2:** Go-to-definition across files (after C-4)
- **L-1:** JSON HIR export / IR-first path
- **C-2 / E-1:** Full HIR lowering for components/hooks/pages; TS codegen from typed HIR

### Wave 8 — LLM-Native Runtime Features (2–3 weeks)

- **L-3:** `@llm` runtime dispatch
- **L-4:** Arca cache for `@llm` results
- **H-5:** LLM-roundtrip integration test
- **F-5:** Stream of Revision protocol
- **G-4:** Streaming incremental parse for LSP

---

## Appendix: Cross-References

| Concern | SSOT Document | Owner Crate |
|---------|--------------|-------------|
| LLM hallucination research | [research-llm-native-lang-design-2026.md](research-llm-native-lang-design-2026.md) | research |
| K-complexity & language design | [vox-llm-native-language-research-2026.md](vox-llm-native-language-research-2026.md) | research |
| Grammar-constrained decoding | [research-grammar-constrained-decoding-2026.md](research-grammar-constrained-decoding-2026.md) | `vox-grammar-export` |
| GRPO reward shaping | [research-grpo-reward-shaping-2026.md](research-grpo-reward-shaping-2026.md) | `vox-populi` |
| Contract system design | [vox-language-testing-pipeline.md](vox-language-testing-pipeline.md) | `vox-compiler`, `vox-skills` |
| Fuzzy parsing | [research-fuzzy-parsing-2026.md](research-fuzzy-parsing-2026.md) | `vox-compiler` |
| Zero-shot type invariants | [research-ts-hallucination-zero-shot-invariants-2026.md](research-ts-hallucination-zero-shot-invariants-2026.md) | research |
| GRPO gap analysis | [research-grpo-gaps-and-adjustments-2026.md](research-grpo-gaps-and-adjustments-2026.md) | `vox-populi` |
| Corpus lab | [vox-corpus-lab-research-2026.md](vox-corpus-lab-research-2026.md) | `vox-cli` mens |
| Training SSOT | [vox-lang-training-ssot-2026.md](vox-lang-training-ssot-2026.md) | `vox-populi` |
| Scientia gap analysis | [scientia-gap-analysis-2026.md](scientia-gap-analysis-2026.md) | `vox-publisher` |
| Architecture index | [architecture-index.md](architecture-index.md) | docs |
| Research index | [research-index.md](research-index.md) | docs |

