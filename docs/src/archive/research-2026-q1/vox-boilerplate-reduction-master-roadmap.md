---
title: "Vox boilerplate reduction master roadmap"
description: "Execution roadmap for reducing accidental complexity and boilerplate in Vox language and full-stack surfaces."
category: "Architecture SSOTs"
last_updated: "2026-03-25"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox boilerplate reduction master roadmap

## Purpose
This is the persistent execution plan for reducing boilerplate and accidental complexity across Vox language features, compiler pipeline, and full-stack web surfaces. It is designed so smaller models can execute tasks safely with clear complexity and token expectations.

## Scope
- Language ergonomics and syntax ceremony reduction
- Parser/AST/HIR normalization
- Typechecker and diagnostics ergonomics
- Error propagation and effect-like ergonomics
- Shared full-stack contract surfaces (Rust + TS emitters)
- Data layer duplication reduction
- CLI/MCP registry and dispatch duplication reduction
- Autofix and developer-loop tooling
- Validation, migration, governance, and KPI tracking

## Complexity rubric
- `C1` low: 200-600 tokens, local changes, low integration risk
- `C2` medium: 700-1600 tokens, 2-4 files, moderate integration
- `C3` high: 1700-3200 tokens, cross-module changes + tests/docs
- `C4` very high: 3300-6000 tokens, architecture refactor + migration

## Risk rubric
- `low`: isolated change, straightforward rollback
- `medium`: cross-file behavior coupling
- `high`: architectural or semantic compatibility impact

## Task assignment guidance for smaller models
- Keep one stream-focused branch per task family.
- Always implement tests in the same task when behavior changes.
- Never collapse high-risk tasks into single mega-PRs.
- For `C3/C4`, require pre/post behavior assertions and migration notes.

## 200-task catalog (canonical)

### Stream A - Language surface ergonomics (A001-A020)
- A001 (C2, 900): Define concise syntax principles and anti-ceremony rules in compiler docs.
- A002 (C2, 1000): Add grammar proposal for explicit-but-compact function signatures.
- A003 (C3, 2200): Design `let-else` style early-exit syntax for Vox.
- A004 (C2, 1100): Design destructuring declarations for tuples/records.
- A005 (C3, 2000): Specify partial record matching syntax with exhaustiveness constraints.
- A006 (C2, 1000): Specify optional chaining/null propagation simplifications.
- A007 (C3, 2500): Design ergonomic pipeline chaining with named placeholders.
- A008 (C2, 900): Add shorthand lambda syntax options and parsing constraints.
- A009 (C2, 850): Add function argument label elision rules for common cases.
- A010 (C3, 2100): Design argument defaults semantics (evaluation order, purity, scope).
- A011 (C2, 950): Define immutable update shorthand for nested fields.
- A012 (C3, 2400): Introduce pattern guards for match branches.
- A013 (C2, 1200) { Define composable `with` options shorthand for APIs/workflows.
- A014 (C3, 2800): Add ergonomic async/await sugar for common sequential flows.
- A015 (C2, 1300): Define concise import aliases and grouped imports.
- A016 (C2, 1400): Add naming and readability lint rules for concise syntax.
- A017 (C1, 500): Write sample corpus snippets for each new syntax concept.
- A018 (C2, 1200): Add parser ambiguity tests for every new shorthand.
- A019 (C1, 450): Add feature-gate strategy for staged rollout.
- A020 (C2, 1100): Document migration examples old->new syntax.

### Stream B - Parser and AST unification (B001-B020)
- B001 (C2, 1200): Audit parser coverage against language docs.
- B002 (C3, 2100): Add parser support plan for currently out-of-scope full-stack declarations.
- B003 (C3, 2300): Introduce AST nodes for missing decorator declarations.
- B004 (C3, 2000): Normalize decorator parsing entrypoints.
- B005 (C2, 1300): Add parser tests for `@page/@layout/@action` declarations.
- B006 (C2, 1100): Add robust error-recovery sync points for new declarations.
- B007 (C2, 900): Improve parser diagnostics for decorator misuse.
- B008 (C3, 2400): Parse `?` error-propagation operator explicitly (if absent).
- B009 (C2, 1200): Parse default arguments with deterministic AST representation.
- B010 (C3, 2200): Add parser support for pattern guards and nested destructuring.
- B011 (C2, 950): Add serialization/debug dump for AST nodes to aid tooling.
- B012 (C2, 1000): Ensure AST nodes carry stable spans for autofix operations.
- B013 (C1, 500): Add unit tests for malformed shorthand syntax.
- B014 (C2, 1000): Harden Pratt precedence interactions with new operators.
- B015 (C2, 1400): Add parse-time lint hooks for ambiguous constructs.
- B016 (C1, 600): Expand fixtures for parser regression testing.
- B017 (C2, 1000): Add doc comments in parser modules for each new rule.
- B018 (C2, 900): Add parser benchmark cases to monitor complexity cost.
- B019 (C3, 1800): Refactor parser module boundaries for maintainability.
- B020 (C2, 1200): Publish parser feature matrix in docs.

### Stream C - HIR lowering debt elimination (C001-C020)
- C001 (C2, 1000): Inventory all declarations entering `legacy_ast_nodes`.
- C002 (C3, 2300): Define typed HIR structs for each legacy declaration class.
- C003 (C3, 2500): Lower `@page` declarations into typed HIR vectors.
- C004 (C3, 2500): Lower `@layout` declarations into typed HIR vectors.
- C005 (C3, 2500): Lower `@action` declarations into typed HIR vectors.
- C006 (C3, 2100): Lower `@theme` declarations into typed HIR vectors.
- C007 (C3, 2100): Lower `@partial` declarations into typed HIR vectors.
- C008 (C2, 1200): Add cross-reference links among typed HIR nodes.
- C009 (C2, 1100): Remove fallthrough lowering paths where now covered.
- C010 (C2, 1500): Add invariants: prohibit web declarations in `legacy_ast_nodes`.
- C011 (C2, 1300): Add HIR snapshot tests for full-stack declarations.
- C012 (C3, 2100): Add compatibility adapters for existing codegen callers.
- C013 (C2, 1400): Update HIR validation to enforce typed-only constraints.
- C014 (C2, 1200): Add debug traces for lowering decisions.
- C015 (C2, 1300): Add explicit lowerer error messages for unsupported constructs.
- C016 (C1, 500): Add unit tests for each lowered declaration variant.
- C017 (C2, 1500): Audit performance impact of expanded HIR nodes.
- C018 (C2, 1100): Remove dead/unused legacy lowering helpers.
- C019 (C1, 600): Document HIR migration strategy.
- C020 (C3, 2600): Complete `legacy_ast_nodes` minimization gate in CI.

### Stream D - Type system and inference ergonomics (D001-D020)
- D001 (C2, 1100): Define local inference boundaries for readability.
- D002 (C3, 2200): Improve inference for defaulted parameters at call sites.
- D003 (C3, 2300): Improve inference in chained pipeline expressions.
- D004 (C2, 1200): Improve inference for destructured bindings.
- D005 (C2, 1400): Add diagnostics for inference ambiguity with clear fixes.
- D006 (C3, 2600): Expand ADT exhaustiveness checking for nested patterns.
- D007 (C2, 1300): Add compile-time hints for non-exhaustive UI states.
- D008 (C2, 1200): Improve match-arm type narrowing and messages.
- D009 (C3, 2400): Add row-like record flexibility design (safe subset).
- D010 (C2, 1100): Add nominal marker type escape hatch for critical domains.
- D011 (C2, 900): Add lints for over-annotation and redundant type hints.
- D012 (C2, 1400): Add smarter expected/found rendering for complex types.
- D013 (C1, 500): Add micro-tests for inference edge cases.
- D014 (C2, 1300): Add checker perf metrics for larger generic signatures.
- D015 (C2, 1000): Add strict-mode option for teams preferring explicit annotations.
- D016 (C3, 1900): Add option/result combinator typing improvements.
- D017 (C2, 1400): Add `with` option-bag type validation enhancements.
- D018 (C2, 1200): Add type-driven quickfix metadata in diagnostics.
- D019 (C1, 450): Update language guide with inference examples.
- D020 (C2, 1300): Add inference regression test suite.

### Stream E - Error handling and effect ergonomics (E001-E020)
- E001 (C2, 1200): Validate doc/code parity for `?` operator semantics.
- E002 (C3, 2400): Implement/complete `?` lowering through HIR.
- E003 (C3, 2200): Implement typechecking rules for `?` in Result/Option contexts.
- E004 (C3, 2200): Add Rust codegen for `?` propagation semantics.
- E005 (C3, 2200): Add TS codegen equivalent propagation patterns.
- E006 (C2, 1300): Add diagnostics for invalid `?` usage with fix suggestions.
- E007 (C2, 900): Add ergonomic helper APIs for wrapping/annotating errors.
- E008 (C3, 2000): Add typed domain error enums generation pattern.
- E009 (C2, 1500): Add optional effect annotation draft syntax.
- E010 (C3, 2800): Prototype lightweight effect inference for async/db/network usage.
- E011 (C2, 1400): Add compiler warning for swallowed errors.
- E012 (C2, 1200): Add structured error metadata for frontend rendering.
- E013 (C2, 1000): Add workflow error-handling sugar for retries/backoff.
- E014 (C2, 1200): Add pattern helpers for error classification.
- E015 (C1, 550): Add tests for nested `?` in pipeline chains.
- E016 (C2, 1300): Add docs on recoverable vs unrecoverable failures.
- E017 (C2, 1400): Add compile-time checks for panic-prone branches.
- E018 (C2, 1000): Add generated error-handling snippets in templates.
- E019 (C1, 450): Add migration lint for manual early-return boilerplate.
- E020 (C2, 1500): Add end-to-end examples in docs and goldens.

### Stream F - Shared full-stack contract pipeline (F001-F020)
- F001 (C3, 2200): Define unified route IR consumed by Rust and TS emitters.
- F002 (C3, 2600): Refactor Rust HTTP emitter to consume shared route IR.
- F003 (C3, 2600): Refactor TS routes emitter to consume shared route IR.
- F004 (C2, 1400): Centralize route prefix policy usage.
- F005 (C3, 2400): Add contract-first schema source for request/response payloads.
- F006 (C3, 2400): Generate validation schemas from one source for both sides.
- F007 (C2, 1500): Add client SDK generation from unified contract model.
- F008 (C2, 1300): Add server stub generation minimizing handler boilerplate.
- F009 (C2, 1200): Add path/param normalization and validation pass.
- F010 (C2, 1200): Add openapi parity checks for generated endpoints.
- F011 (C2, 1100): Add smoke tests for contract drift failures.
- F012 (C3, 2100): Add hot-reload safe regeneration flow for contract changes.
- F013 (C2, 1400): Add feature gates for contract pipeline rollout.
- F014 (C2, 1000): Add migration command for legacy route definitions.
- F015 (C2, 900): Add docs for contract-first authoring patterns.
- F016 (C3, 1800): Add auth metadata in contracts for consistent security checks.
- F017 (C2, 1300): Add typed form/action helpers from same contract source.
- F018 (C2, 1300): Add compile-time duplicate route detection.
- F019 (C1, 500): Add golden fixtures for generated contracts.
- F020 (C3, 2400): Integrate route IR checks into CI.

### Stream G - Data-layer boilerplate collapse (G001-G020)
- G001 (C2, 1300): Audit current table/query/mutation declaration friction.
- G002 (C3, 2200): Add concise query DSL wrappers for common filters/sorts.
- G003 (C3, 2300): Add typed projection helpers to avoid DTO duplication.
- G004 (C2, 1400): Add pagination primitives with one-liner defaults.
- G005 (C2, 1400): Add reusable mutation transaction helpers.
- G006 (C3, 2000): Add generated relation-loading helpers with N+1 linting.
- G007 (C2, 1200): Add schema-derived validation for db-bound inputs.
- G008 (C2, 1300): Add safer dynamic query builder with typed constraints.
- G009 (C2, 1000): Add common index declaration shortcuts.
- G010 (C2, 1000): Add db migration-generation ergonomics improvements.
- G011 (C3, 1900): Add upsert patterns and conflict-resolution shorthand.
- G012 (C2, 1200): Add query explain hooks for developer diagnostics.
- G013 (C2, 1000): Add typed aggregation helpers.
- G014 (C2, 900): Add conventions for id/timestamp defaults.
- G015 (C2, 1400): Add compile-time checks for unsafe raw query patterns.
- G016 (C2, 1300): Add dataset fixtures for query DSL tests.
- G017 (C2, 1200): Add codemods for migrating legacy db boilerplate.
- G018 (C1, 500): Add examples for full-stack feed/query patterns.
- G019 (C2, 1200): Add docs for preferred data-access patterns.
- G020 (C3, 2200): Add CI gate for query safety + boilerplate regressions.

### Stream H - CLI and MCP boilerplate reduction (H001-H020)
- H001 (C2, 1200): Map duplicated metadata across clap, registry, docs.
- H002 (C3, 2600): Design single-definition command metadata generation path.
- H003 (C3, 2600): Generate clap stubs/metadata from registry model where possible.
- H004 (C2, 1400): Expand command compliance to stricter drift prevention.
- H005 (C3, 2200): Convert MCP dispatch to table-driven registration model.
- H006 (C3, 2400): Generate MCP input schema from typed param structures.
- H007 (C2, 1400): Derive MCP subset lists from canonical tool tags.
- H008 (C2, 1200): Add compile-time assertions for unregistered tool handlers.
- H009 (C2, 1300): Add alias lifecycle/deprecation metadata automation.
- H010 (C2, 1100): Add one-command docs sync for command/tool surfaces.
- H011 (C2, 1200): Add tests ensuring every registry entry has examples.
- H012 (C2, 1200): Add command UX linting (naming/description consistency).
- H013 (C2, 1400): Add machine-readable changelog for command surface changes.
- H014 (C1, 600): Add fixtures for command-catalog baseline testing.
- H015 (C2, 1500): Add performance checks for startup/dispatch overhead.
- H016 (C2, 1000): Add migration docs for deprecated commands/tools.
- H017 (C3, 1900): Add scoped plugin model for future command expansion.
- H018 (C2, 1000): Add CI artifact comparing generated vs committed registries.
- H019 (C1, 500): Add docs for single-source command authoring workflow.
- H020 (C3, 2300): Finalize fully automated command/tool sync pipeline.

### Stream I - Autofix, LSP, and developer workflow (I001-I020)
- I001 (C2, 1200): Replace `StubAutoFixer` with rule-based fixer architecture.
- I002 (C3, 2200): Add fix rule for missing imports.
- I003 (C3, 2200): Add fix rule for type-annotation insertion.
- I004 (C3, 2200): Add fix rule for non-exhaustive matches.
- I005 (C2, 1400): Add fix rule for redundant boilerplate constructs.
- I006 (C2, 1300): Add fix confidence scoring.
- I007 (C2, 1200): Add safe-preview mode for autofixes.
- I008 (C2, 1200): Add LSP code-action integration with fix rules.
- I009 (C2, 1000): Add quick docs links in diagnostics payloads.
- I010 (C2, 1200): Add parser/typecheck debug logging toggles for diagnosis.
- I011 (C2, 1300): Add periodic progress logging in long-running compile checks.
- I012 (C2, 1400): Add command-level explain mode (why this diagnostic appears).
- I013 (C1, 500): Add tests for autofix no-op safety.
- I014 (C2, 1400): Add conflict detection for overlapping fix edits.
- I015 (C2, 1200): Add rollback checkpoints for failed fix application.
- I016 (C2, 1100): Add telemetry counters for most-used fixes.
- I017 (C2, 1300): Add docs for fixer authoring guidelines.
- I018 (C1, 450): Add sample playground scenarios for fix demonstrations.
- I019 (C2, 1200): Add CI checks for fixer determinism.
- I020 (C3, 2000): Ship first stable autofix bundle.

### Stream J - Validation, docs, migration, and governance (J001-J020)
- J001 (C2, 1200): Create boilerplate-reduction KPI framework.
- J002 (C2, 1200): Define baseline metrics (LOC/feature, files touched/feature, compile diagnostics).
- J003 (C2, 1200): Add benchmark corpus for web-stack feature implementation speed.
- J004 (C2, 1300): Add regression dashboards for complexity trends.
- J005 (C2, 1400): Add docs/code drift checker for language claims.
- J006 (C2, 1200): Add migration playbooks per syntax/feature wave.
- J007 (C2, 900): Add release notes template for ergonomics changes.
- J008 (C2, 1100): Add compatibility policy for phased syntax deprecations.
- J009 (C2, 1400): Add golden examples for full-stack CRUD with minimal ceremony.
- J010 (C1, 600): Add contributor checklist for anti-boilerplate changes.
- J011 (C2, 1200): Add architecture decision records for major ergonomics shifts.
- J012 (C2, 1300): Add training-data updates for new syntax examples.
- J013 (C2, 1200): Add CI gates on docs freshness for new features.
- J014 (C2, 1000): Add style conventions to prevent syntactic over-compression.
- J015 (C2, 1200): Add rollout scorecard per feature gate.
- J016 (C2, 1200): Add risk register and rollback criteria per stream.
- J017 (C1, 550): Add cookbook patterns for common full-stack tasks.
- J018 (C2, 1200): Add anti-pattern catalog (what not to add as sugar).
- J019 (C2, 1300): Add post-merge adoption tracking process.
- J020 (C3, 1800): Publish v1 ergonomic core completion report criteria.

## Wave execution
- Wave 1 (foundation): B001-B010, C001-C010, E001-E006, H001-H006, I001-I004, J001-J006
- Wave 2 (leverage): A001-A012, D001-D010, F001-F010, G001-G010, I005-I012
- Wave 3 (scale): all remaining tasks with CI hardening, migration, and governance closure

## Completion criteria
- `legacy_ast_nodes` reduced to intentional residuals only (or removed).
- `?` operator and default-argument ergonomics are fully documented and verified end-to-end.
- Shared route IR drives both Rust and TS route emission.
- MCP/CLI metadata drift is minimized through generation/parity gates.
- Autofix delivers practical, safe fixes for top repetitive error classes.
- Docs and training corpus match shipped implementation without major drift.


