# Semantic Behavior Map — `vox-compiler`

Deterministically synthesized from 989 distinct proven-behavior claims (of 989 extracted) across 563 symbols. 98 symbols have an explicit error-path proof; **371 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `compile_component`  (happy, invariant; EXTRACTED)
- [happy] component compilation emits JSX elements (<button>) not JS function calls (button())  (crates/vox-compiler/tests/golden_dashboard_chrome_test.rs)
- [happy] component compilation correctly includes all NavItem component calls and string labels in emitted TSX  (crates/vox-compiler/tests/golden_dashboard_chrome_test.rs)
- [happy] component compilation emits both branches of conditional raw_class expressions (w-14 and w-[200px])  (crates/vox-compiler/tests/golden_dashboard_chrome_test.rs)
- [happy] component compilation preserves static string values (e.g., model names, costs) and color tokens in emitted TSX  (crates/vox-compiler/tests/golden_dashboard_chrome_test.rs)
- [happy] component compilation emits nested component calls (TopBar, LeftRail, StatusBar) as JSX and preserves min-h-screen layout class  (crates/vox-compiler/tests/golden_dashboard_chrome_test.rs)
- [happy] component compilation emits JSX elements not JS function calls and preserves all color tokens (emerald, amber, rose) in output  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] button component compilation emits <button> JSX element and onClick prop, not button() JS function call  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] component compilation emits conditional color tokens and onClick event handler in JSX output  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] Vox panel() and text() component calls do not emit as raw JavaScript function calls in the compiled TypeScript output  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] Vox conditional expressions (if/else) in component attributes compile and output both conditional branches in the TypeScript  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] Vox column() and text() component calls do not emit as raw JavaScript function calls  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] Vox Tailwind utility class sizing (2xl) passes through to the compiled TypeScript output  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- … +13 more claims

### `validate_web_ir()`  (edge, error, happy, invariant; EXTRACTED)
- [happy] validate_web_ir accepts a valid StyleNode::Rule with non-empty property name and returns no diagnostics  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] validate_web_ir returns empty diagnostics after JSON serialization round-trip via serde_json  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.style.empty_property' when StyleDeclarationValue has empty property name  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir returns empty diagnostics for valid WebIrModule created from parsed component source  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.interop.empty_import_source' when InteropNode::ReactComponentRef has empty import_source  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir accepts WebIrModule with RouteNode::RouteTree and matching view_roots without diagnostics  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.route.empty_loader_id' when RouteNode::LoaderContract has empty route_id  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir returns empty diagnostics for default (empty) WebIrModule  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.interop.empty_external_specifier' when InteropNode::ExternalModuleRef has empty specifier  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir accepts WebIrModule with both StyleNode::Rule and RouteNode::RouteTree without diagnostics  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir accepts BehaviorNode::StateDecl with Required optionality and non-None initial value without diagnostics  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir() emits a diagnostic with code 'web_ir_validate.a11y.interactive_missing_label' when a button element lacks a label  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- … +12 more claims

### `validate_web_ir`  (edge, error, happy, invariant; EXTRACTED)
- [error] validate_web_ir emits diagnostic with code web_ir_validate.route.duplicate_contract_id when routes have duplicate ids  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code web_ir_validate.behavior.required_state_without_initial when Required state has no initial value  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir allows Optional and Defaulted state without initial value  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] validate_web_ir diagnostic codes start with prefix web_ir_validate. and include category field  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir rejects StyleNode::Rule with empty declarations list emitting code web_ir_validate.style.empty_declarations  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir produces empty diagnostics when processing a valid parity chain fixture  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir produces empty diagnostics for a simple Gate component with state and text view  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.style.literal_color_value' when a raw hex color is used  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.style.literal_color_value' when CssColor::Hex variant is used  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.style.literal_dimension_value' when literal Length value is used in padding  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] validate_web_ir does not emit literal_color_value or literal_dimension_value diagnostics when StyleDeclarationValue::TokenRef is used  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir emits diagnostic with code 'web_ir_validate.route.missing_component' when route specifies a component not in view_roots  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- … +8 more claims

### `vox_codegen::codegen_ts::generate()`  (happy, invariant; EXTRACTED)
- [invariant] Codegen produces identical TypeScript output across multiple lowering runs on the same module  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Generated TypeScript for React components includes useState hook usage  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Reactive component codegen produces Web IR view emission statistics  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Reactive view emission completes without failures  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [invariant] Web IR view emission stats are recorded even when environment variable VOX_WEBIR_EMIT_REACTIVE_VIEWS is disabled  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [invariant] Each reactive view codegen produces exactly one bridge decision (either web_ir_view_emitted or web_ir_view_emitted_parity_mismatch)  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Reactive view emission completes without failures when environment variable VOX_WEBIR_EMIT_REACTIVE_VIEWS is enabled  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Codegen produces Shell.tsx file for branch-registry fixture  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Codegen produces Home.tsx file for branch-registry fixture  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Codegen produces CSS file with style declarations when raw_css block is present  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Generated TSX imports CSS module when component has raw_css block  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Generated TypeScript component matches snapshot showing useState hooks for reactive state  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- … +2 more claims

### `Report`  (edge, error, happy; EXTRACTED)
- [happy] parse_success is true for valid Vox function declarations  (crates/vox-compiler/src/ast_eval.rs)
- [happy] construct_histogram contains entry for 'fn' construct  (crates/vox-compiler/src/ast_eval.rs)
- [happy] coverage_score() returns value greater than 0.0 for valid code  (crates/vox-compiler/src/ast_eval.rs)
- [error] parse_success is false for invalid Vox code  (crates/vox-compiler/src/ast_eval.rs)
- [error] error_span is Some for invalid code  (crates/vox-compiler/src/ast_eval.rs)
- [error] coverage_score() returns exactly 0.0 for parse errors  (crates/vox-compiler/src/ast_eval.rs)
- [happy] parse_success is true for test declarations  (crates/vox-compiler/src/ast_eval.rs)
- [happy] has_tests is true when @test decorator is present  (crates/vox-compiler/src/ast_eval.rs)
- [edge] node_count is 0 for empty input string  (crates/vox-compiler/src/ast_eval.rs)
- [happy] parse_success is true for code with multiple construct types  (crates/vox-compiler/src/ast_eval.rs)
- [happy] construct_histogram contains keys for 'fn', 'type', and 'test' when all are present  (crates/vox-compiler/src/ast_eval.rs)
- [happy] has_tests is true when test declarations are present alongside other constructs  (crates/vox-compiler/src/ast_eval.rs)
- … +1 more claims

### `typecheck_ast_module`  (edge, error, happy; EXTRACTED)
- [error] typechecking detects zero dimensions in @embed decorator and emits 'vox/embed/zero-dimensions' diagnostic code  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] typechecking does not emit 'vox/embed/zero-dimensions' diagnostic when @embed has valid non-zero dimensions  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [error] duplicate import alias names (serde_json as j, chrono as j) produce an Error diagnostic with 'Import alias conflict' message  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [error] same crate with conflicting version specs produces an Error diagnostic with category Lowering and message about conflicting dependency specifications  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [happy] same crate with identical spec but two different aliases produces no Error diagnostics  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] rust imports without version/path/git pin produce a Warning diagnostic  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] importing internal runtime crates (tokio) produces a Warning diagnostic mentioning internal_runtime_only  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] importing deferred crates (sqlx) produces a Warning diagnostic mentioning deferred status  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [error] rust import with both path and git sources produces an Error diagnostic with category Lowering mentioning conflicting sources  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] importing escape hatch only crates (chrono) produces a Warning diagnostic mentioning escape_hatch_only  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] importing planned semantics crates (time) produces a Warning diagnostic mentioning planned status  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- [edge] template-managed app dependencies (reqwest) with explicit version pin produce a Warning diagnostic mentioning template  (crates/vox-compiler/tests/rust_import_diagnostics_test.rs)
- … +1 more claims

### `parse()`  (happy; EXTRACTED)
- [happy] parse successfully converts lexed tokens to Module AST when source is valid  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] parse produces Module AST from component source with state and text view  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] parse() successfully parses Vox source code containing a component with a stack primitive  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] parse() successfully parses Vox source containing a button primitive with variant attribute  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] parse() successfully parses @versioned function declaration  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] parse() successfully parses @tracked function declaration  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] Parser can re-parse formatted source without errors after format() and lex()  (crates/vox-compiler/tests/property_tests.rs)
- [happy] K-metric branch registry fixture parses successfully  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Parses simple function declaration with body and creates single Decl::Function via assert_eq! on length  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] Parses multiple import paths separated by comma via assert! with matches!  (crates/vox-compiler/src/parser/descent/tests.rs)

### `DiagnosticExcerpt::from_source`  (edge, error, happy, invariant; EXTRACTED)
- [happy] returns excerpt with lines 2..=8 (context ±3 around line 5) and text containing both line2 and line8  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [edge] clips context to file start so first line is 1 and text starts with 'line1'  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [edge] clips context to file end so last line is 10 and text contains 'line10'  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] returns excerpt with lines [2,3,4,5,6] and text containing both line3 and line5 for span covering lines 3-5 with context ±1  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [error] returns None when source is empty string  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [error] returns None when start_line is 0  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] returns excerpt with lines=[1] and text equal to 'single line only' for single-line file  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [invariant] returns excerpt text that does not end with newline character  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [invariant] returns excerpt where lines vector length equals the count of actual lines in text  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `Token`  (happy; EXTRACTED)
- [happy] Lexer emits Token::Eof, Token::LBrace, Token::RBrace, and Token::Return tokens for brace-delimited function blocks  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer emits Token::Newline tokens to be cosmetic and non-structural, alongside Token::LBrace and Token::RBrace for block delimiters  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer produces Token::Lt and Token::JsxCloseStart tokens when lexing JSX markup  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer emits Token::Match, Token::Arrow, Token::LBrace, and Token::RBrace for match expression syntax  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer produces Token::Http as first token and Token::Ident for HTTP method names in route declarations  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer filters out Token::Comment tokens and produces only valid syntax tokens ending with Token::Eof  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer recognizes Token::Agent, Token::Env, and Token::Migrate keywords in agent environment declarations  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer tokenizes complex chatbot source with Import, AtComponent, Fn, Match, Http, Spawn, and brace delimiters, ending with Token::Eof  (crates/vox-compiler/src/lexer/cursor.rs)
- [happy] Lexer produces exactly [Token::Activity, Token::With, Token::Workflow, Token::Eof] when parsing durable execution keywords  (crates/vox-compiler/src/lexer/cursor.rs)

### `VoxCompilerDiagnosticPayload::from_diagnostic`  (happy; EXTRACTED)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic generates a vox-lang.org explain URL for diagnostic codes with vox/ prefix  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic returns None for explain_url when diagnostic code is numeric (E0001)  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic returns None for explain_url when diagnostic code has lint. prefix  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic defaults missing diagnostic code to E0000 and has no explain_url  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic explain_url starts with https://vox-lang.org/diag/vox/ for vox/ codes  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic generates explain URL for vox/types/type-mismatch code  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic generates explain URL for vox/types/undefined-variable code  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] VoxCompilerDiagnosticPayload::from_diagnostic generates explain URL for vox/types/method-not-found code  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `generate`  (happy; EXTRACTED)
- [happy] column and row primitives emit as JSX elements, not as JS function calls  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] text children emit within JSX elements and preserve content ('left', 'right')  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] PlayIcon component lowers view_box to viewBox and stroke_width to strokeWidth in SVG attributes  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] polygon emits as JSX child <polygon ...> not as JS function call polygon()  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] HaloRect outer svg emits viewBox and preserveAspectRatio attributes in camelCase  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] HaloRect nested SVG elements defs, radialGradient, and stop emit as JSX children  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] MeshNode component emits viewBox, defs, radialGradient, stop JSX elements  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)
- [happy] MeshNode component lowers stop_color to stopColor and stop_opacity to stopOpacity attributes  (crates/vox-compiler/tests/golden_svg_vuv_test.rs)

### `has_determinism_diag()`  (error, happy, invariant; EXTRACTED)
- [error] Workflows using std.time.now_ms() trigger a determinism diagnostic with code 'lint.workflow.non_deterministic'  (crates/vox-compiler/tests/determinism_lint.rs)
- [error] Workflows using std.random() trigger a determinism diagnostic with code 'lint.workflow.non_deterministic'  (crates/vox-compiler/tests/determinism_lint.rs)
- [happy] Activities using std.time.now_ms() do not trigger determinism diagnostics  (crates/vox-compiler/tests/determinism_lint.rs)
- [happy] Plain functions using std.time.now_ms() do not trigger determinism diagnostics  (crates/vox-compiler/tests/determinism_lint.rs)
- [error] Workflows transitively calling plain functions that use std.time.now_ms() trigger determinism diagnostics  (crates/vox-compiler/tests/determinism_lint.rs)
- [happy] Workflows calling activities that use std.time.now_ms() do not trigger determinism diagnostics due to activity journaling boundary  (crates/vox-compiler/tests/determinism_lint.rs)
- [error] Multi-hop transitive call chains (workflow → outer → inner → non_det) trigger determinism diagnostics  (crates/vox-compiler/tests/determinism_lint.rs)
- [invariant] Mutually-recursive functions in workflow transitive chains detect non-deterministic calls without infinite-looping  (crates/vox-compiler/tests/determinism_lint.rs)

### `lower_hir_to_web_ir()`  (edge, happy; EXTRACTED)
- [happy] lower_hir_to_web_ir transforms a valid HIR module to WebIrModule that passes validation  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir produces valid WebIrModule from component with text view and string literal  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir() lowers a Vox stack primitive with gap=4 to a DOM div element with className containing 'flex', 'flex-col', and 'gap-4'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir() lowers a Vox button primitive with variant='default' to a button element with className containing 'bg-primary'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir() lowers a Vox row primitive to a DOM div element with className containing 'flex-row'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [edge] lower_hir_to_web_ir() lowers a Vox column primitive without injecting primitive-specific Tailwind classes (flex, bg-) to non-primitive div elements  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir() marks style rules parsed from raw_css blocks with is_raw_css=true flag  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir() lowers a panel primitive with surface='primary' to a div with data-vox-surface attribute and style attrs containing CSS variable references like --vox-surface-primary-fg  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `HirImport`  (happy; EXTRACTED)
- [happy] Mixed slash/dot separators in import paths parse and lower correctly, with slashes and dots both accepted as segment separators  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] Legacy dotted form imports parse and lower without change, preserving es_module_specifier as None  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] React component imports with ES module specifiers parse and lower with empty module_path and populated es_module_specifier  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] React component imports coexist with legacy dotted-form React imports in the same source file, each lowering to separate HirImport objects  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] React named imports with braces expand to one HirImport per imported name, preserving es_import_kind=Named with original exported names for aliased imports  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] React namespace imports ('* as X') parse and lower to HirImport with es_import_kind=Some(EsImportKind::Namespace)  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] React default imports ('import react X from ...') lower to HirImport with es_import_kind=Some(EsImportKind::Default)  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `RepoStore`  (error, happy, invariant; EXTRACTED)
- [happy] RepoStore::snapshot() assigns monotone IDs (0, 1, ...) and records changes  (crates/vox-compiler/src/eval/repo.rs)
- [happy] RepoStore::changes() returns a collection with length equal to number of snapshots  (crates/vox-compiler/src/eval/repo.rs)
- [happy] RepoStore::undo() removes the last change and returns its ID  (crates/vox-compiler/src/eval/repo.rs)
- [happy] RepoStore default constructor creates an empty store with no changes  (crates/vox-compiler/src/eval/repo.rs)
- [happy] RepoStore::undo() on empty store returns None  (crates/vox-compiler/src/eval/repo.rs)
- [invariant] RepoStore snapshot IDs are monotone and never reused after undo (maintain monotone invariant)  (crates/vox-compiler/src/eval/repo.rs)
- [error] RepoStore snapshot operation with non-string argument raises TypeError  (crates/vox-compiler/src/eval/repo.rs)

### `lex()`  (edge, happy; EXTRACTED)
- [happy] lex accepts valid Vox component source and produces tokens  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lex tokenizes component source with state and text view successfully  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lex() tokenizes Vox source containing a component declaration with nested stack primitive  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] Lexer tokenizes raw strings with Windows paths as RawStringLit preserving backslashes  (crates/vox-compiler/tests/raw_string_test.rs)
- [happy] Double-hash-padded raw strings allow embedded quote-hash sequences in the body  (crates/vox-compiler/tests/raw_string_test.rs)
- [happy] Source 'let x = 5' lexes to sequence [Token::Let, Token::Ident(x), Token::Eq, Token::IntLit(5), Token::Eof]  (crates/vox-compiler/src/lexer/cursor.rs)
- [edge] Backticks and colons inside string literals do not break lexing and are preserved in token  (crates/vox-compiler/src/lexer/cursor.rs)

### `App`  (happy; EXTRACTED, INFERRED)
- [happy] project_app_contract() groups endpoints by kind (server/query/mutation), maintains source order, preserves names and route paths, and correctly reports signatures  (crates/vox-compiler/src/app_contract.rs)
- [happy] mutation endpoints have wraps_db_transaction=true when @table declarations exist  (crates/vox-compiler/src/app_contract.rs)
- [happy] mutation endpoints have wraps_db_transaction=false when no @table declarations exist  (crates/vox-compiler/src/app_contract.rs)
- [happy] endpoints maintain correct identity and routing when declared in interleaved kind order rather than grouped by kind  (crates/vox-compiler/src/app_contract.rs)
- [happy] App contract schema_version is 2  (crates/vox-compiler/tests/projection_parity_test.rs)
- [happy] App contract contains non-empty server_fns, query_fns, and mutation_fns collections when decorators present  (crates/vox-compiler/tests/projection_parity_test.rs)

### `ParseErrorClass`  (error; EXTRACTED)
- [error] Parse of 'actor' keyword block produces error with Tombstoned class containing 'actor' and 'tombstoned' in message  (crates/vox-compiler/tests/tombstone_test.rs)
- [error] Parse of 'workflow' keyword block produces error with Tombstoned class  (crates/vox-compiler/tests/tombstone_test.rs)
- [error] Parse of '@component' decorator produces error with Tombstoned class  (crates/vox-compiler/tests/tombstone_test.rs)
- [error] Parse of 'http' keyword produces error with Tombstoned class  (crates/vox-compiler/tests/tombstone_test.rs)
- [error] ParseErrorClass::TopLevel is assigned to malformed top-level tokens like '@@@bad@@@'  (crates/vox-compiler/tests/parser_recovery_test.rs)
- [error] ParseErrorClass::Declaration is used for malformed declaration syntax in pub-bogus.vox  (crates/vox-compiler/tests/parser_recovery_test.rs)

### `call_builtin_method()`  (happy; EXTRACTED)
- [happy] call_builtin_method() dispatches "basename" on path namespace to extract filename from path  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] call_builtin_method() dispatches "dirname" on path namespace to extract directory from path  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] call_builtin_method() dispatches "matches" on Regex tagged value to test pattern matching  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] call_builtin_method() dispatches "find" on Regex tagged value returning Some(Match)  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] call_builtin_method() dispatches "find_all" on Regex tagged value returning a List of matches  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] call_builtin_method() dispatches "group" on Match value to extract capture groups by index  (crates/vox-compiler/src/eval/builtins.rs)

### `codegen_ts::generate`  (happy; EXTRACTED)
- [happy] Match on Result type does not emit case _ literal patterns  (crates/vox-compiler/tests/bug_a_match_arms_repro.rs)
- [happy] Match arms on Result type use _tag-discriminated dispatch  (crates/vox-compiler/tests/bug_a_match_arms_repro.rs)
- [happy] Match bound variables are bound before use in emitted code  (crates/vox-compiler/tests/bug_a_match_arms_repro.rs)
- [happy] Speech.transcribe_microphone() does not lower to mobile.transcribe_microphone()  (crates/vox-compiler/tests/bug_b_speech_repro.rs)
- [happy] .length() method calls are not emitted for array length access  (crates/vox-compiler/tests/bug_handler_lambda_repro.rs)
- [happy] .length property access is emitted in TypeScript output  (crates/vox-compiler/tests/bug_handler_lambda_repro.rs)

### `compile_components`  (happy; EXTRACTED)
- [happy] PlayIcon component does not emit raw snake_case SVG attributes like view_box=  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)
- [happy] PlayIcon component emits camelCase SVG attributes: viewBox, strokeWidth, strokeLinecap, strokeLinejoin  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)
- [happy] Halo component emits <radialGradient> camelCase SVG tag, not <radial_gradient>  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)
- [happy] Halo component emits camelCase attributes patternUnits, preserveAspectRatio, stopColor, stopOpacity  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)
- [happy] Filtered component emits <linearGradient>, <feGaussianBlur>, <foreignObject> as camelCase SVG tags  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)
- [happy] Filtered component emits <filter> tag and stdDeviation= attribute  (crates/vox-compiler/tests/golden_svg_snake_case_test.rs)

### `emit_main`  (happy; EXTRACTED)
- [happy] Emitted main.rs contains starts_with("/api") check to reserve /api prefix  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] Emitted main.rs explicitly routes /api/ping endpoint  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] Query GET routes are registered in router before fallback  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] Query handler decodes JSON query parameters as BTreeMap  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] Mutation with @table schema wraps in transaction and emits JSON error envelope  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] @server functions without schema do not use JSON error envelope  (crates/vox-compiler/tests/axum_emit_contract_test.rs)

### `lower_hir_to_web_ir`  (happy; EXTRACTED)
- [happy] Event handler attributes lower on_click to onClick in DomNode elements  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] Component style blocks lower to StyleNode::Rule with unparsed selector and CSS declarations  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] routes block lowers to RouteNode::RouteTree with pattern matching routes declaration  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir successfully lowers a simple Gate component with state into WebIrModule  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir successfully converts component HIR to WebIR representation  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] JSX event handlers on_click in Vox source lower to onClick attribute in WebIR DOM elements  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `Decl::Function`  (happy; EXTRACTED)
- [happy] Parses function name 'add' correctly via assert! with matches! pattern  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] Parses pipe operator syntax in function body via assert! with matches! pattern  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] The parsed declarations include the original function (helper) as first declaration and a synthetic main function as second declaration  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] Pure declaration file produces exactly the declared functions without synthetic main  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] pub keyword on function declaration sets is_pub flag to true and preserves name field  (crates/vox-compiler/src/parser/descent/tests.rs)

### `FileKind::from_path`  (edge, happy, invariant; EXTRACTED)
- [happy] FileKind::from_path returns FileKind::Source for files with .vox extension regardless of path (absolute, relative, or simple filename)  (crates/vox-compiler/src/module.rs)
- [happy] FileKind::from_path returns FileKind::ReactiveModule for files with .vox.ui extension  (crates/vox-compiler/src/module.rs)
- [happy] FileKind::from_path returns FileKind::Unknown for files without .vox or .vox.ui extensions  (crates/vox-compiler/src/module.rs)
- [invariant] FileKind::from_path only examines the file name component, ignoring directory names that contain .vox or .vox.ui patterns  (crates/vox-compiler/src/module.rs)
- [edge] FileKind::from_path prioritizes .vox.ui suffix over .vox suffix when both patterns match (e.g. x.vox.ui returns ReactiveModule not Source)  (crates/vox-compiler/src/module.rs)

### `HirModule`  (happy; EXTRACTED)
- [happy] HirModule serializes to JSON with stringified span keys in the format "start-end"  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)
- [happy] HirModule deserializes from JSON preserving inferred_types entry count  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)
- [happy] HirModule inferred types round-trip through JSON serialization with type equality preserved  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)
- [happy] Empty HirModule serializes to JSON and deserializes with inferred_types.len() == 0  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)
- [happy] lower_module correctly categorizes source components into HirModule.imports, HirModule.components with one import and one component named Home  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `RouteId`  (happy; EXTRACTED)
- [happy] Home route_id has url_pattern of '/' and empty params list  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] Home route_id analytics_slug is converted to snake_case 'home'  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] UserProfile route_id has url_pattern of '/users/:id' with params containing ('id', 'string')  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] UserProfile route_id analytics_slug is converted to snake_case 'user_profile'  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] Single home route_id has analytics_slug of 'home'  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Snapshot`  (error, happy; EXTRACTED)
- [happy] Interpreter auto-generates exactly one snapshot per @versioned function on successful completion  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] Non-decorated functions do not auto-generate snapshots on completion  (crates/vox-compiler/tests/versioned_decorator.rs)
- [error] @versioned function that raises exception does not record a snapshot  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] Nested @versioned calls each snapshot independently, inner before outer  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] Snapshot struct has an id field that is retrieved by snapshot() method  (crates/vox-compiler/src/eval/repo.rs)

### `Token::StringLit`  (happy; EXTRACTED)
- [happy] JSON object literal {"key":1} is lexed as StringLit token, not TemplateStringLit  (crates/vox-compiler/tests/lexer_json_template_test.rs)
- [happy] complex JSON with nested objects and arrays is lexed as StringLit  (crates/vox-compiler/tests/lexer_json_template_test.rs)
- [happy] empty JSON object {} is lexed as StringLit token  (crates/vox-compiler/tests/lexer_json_template_test.rs)
- [happy] JSON object with underscore-prefixed keys is lexed as StringLit  (crates/vox-compiler/tests/lexer_json_template_test.rs)
- [happy] String literal "hello world" lexes to Token::StringLit with correct content  (crates/vox-compiler/src/lexer/cursor.rs)

### `WireType`  (happy; EXTRACTED)
- [happy] WireType has primitive variants (Number, String, Bool, Unit, Unknown) that can be passed to wire_type_to_zod()  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] WireType has a DateTimeString variant for datetime string wire types  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] WireType has a Ref variant that wraps a type name string  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] WireType has an Array variant wrapping an inner type  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] WireType::Ref can be pattern-matched to extract the wrapped name  (crates/vox-compiler/src/contract_ir/tests.rs)

### `compact()`  (happy; EXTRACTED)
- [happy] Function definition compacts to single-line form preserving braces, name, and statements  (crates/vox-compiler/src/lexer/compact.rs)
- [happy] Compaction preserves brace tokens and produces single-line output without spurious newlines  (crates/vox-compiler/src/lexer/compact.rs)
- [happy] Multi-line Vox source with if-else compacts to a single line with function name and conditions intact  (crates/vox-compiler/src/lexer/compact.rs)
- [happy] Function with let and return statements compacts to exact golden output 'fn main(){let x=10 return x}'  (crates/vox-compiler/src/lexer/compact.rs)
- [happy] Compacted source code output is valid and parseable by the parser  (crates/vox-compiler/src/lexer/compact.rs)

### `lower_module`  (happy; EXTRACTED)
- [happy] modules containing only let bindings parse and lower without panicking  (crates/vox-compiler/tests/golden_dashboard_composites_test.rs)
- [happy] HirModule endpoint_fns contains query endpoints  (crates/vox-compiler/tests/axum_emit_contract_test.rs)
- [happy] lower_module parses and lowers a realistic app.vox import block with 8 import statements (3 chrome + 7 surface items) to exactly 10 HirImports  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] lower_module and parse successfully process @table type declarations with @index  (crates/vox-compiler/tests/voxdb_schema_hir_parity_test.rs)
- [happy] lower_module processes component with state and derived fields  (crates/vox-compiler/tests/web_ir_environment_gates_test.rs)

### `parse`  (error, happy; EXTRACTED)
- [happy] parse succeeds on real app.vox import block syntax without errors  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [error] parse rejects empty braces in React named imports ('import react { } from ...'), returning Err  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [error] parse collects multiple disjoint parse errors in a single source (two valid functions separated by garbage), returning non-empty error vector  (crates/vox-compiler/tests/parser_recovery_test.rs)
- [error] parse rejects pub-bogus.vox file, returning error vector with at least one ParseErrorClass::Declaration error  (crates/vox-compiler/tests/parser_recovery_test.rs)
- [error] parse returns Err on nested-unclosed.vox (file with unclosed blocks) without panicking  (crates/vox-compiler/tests/parser_recovery_test.rs)

### `parse_imports`  (happy; EXTRACTED)
- [happy] parse_imports on 'import surfaces/mesh.MeshSurface' produces exactly one HirImport with item='MeshSurface' and module_path=['surfaces', 'mesh']  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] parse_imports on 'import react.use_state' produces one import with module_path=['react'], item='use_state', and es_module_specifier=None  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] parse_imports on 'import react MyButton from "../ui/MyButton.tsx"' produces one import with item='MyButton', empty module_path, and es_module_specifier=Some('..ui/MyButton.tsx')  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] parse_imports handles mixed dotted and ES-module imports, producing 2 imports with different kinds in the same block  (crates/vox-compiler/tests/parser_import_syntax_test.rs)
- [happy] parse_imports on 'import react * as Dialog from "@radix-ui/react-dialog"' produces one import with item='Dialog' and es_import_kind=Some(EsImportKind::Namespace)  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `synthesise_json_as_fns()`  (happy; EXTRACTED)
- [happy] synthesise_json_as_fns() returns empty vec when type definition has no json_as annotation  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] synthesise_json_as_fns() emits two functions named <Type>_from_json and <Type>_to_json for annotated struct  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] synthesise_json_as_fns() generates from_json function with return type Result<T>  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] synthesise_json_as_fns() generates to_json function with return type Json  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] synthesise_json_as_fns() generates function body with exactly 2 statements (let + return) for optional field, with no additional guard  (crates/vox-compiler/src/hir/lower/json_as.rs)

### `Fragment`  (happy; EXTRACTED)
- [happy] fragment declaration with no parameters parses and preserves the name 'Greeting'  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] fragment declaration params list is empty when no parameters are declared  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] fragment declaration with typed parameters parses and preserves the name 'Row'  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] fragment declaration params list contains two parameters with names 'item' and 'idx' when declared with two typed parameters  (crates/vox-compiler/src/parser/descent/tests.rs)

### `LayerTier::default_for_primitive()`  (happy; EXTRACTED)
- [happy] Maps 'Tooltip' to LayerTier::Popover via assert_eq!  (crates/vox-compiler/src/hir/nodes/layer.rs)
- [happy] Maps 'Dialog' to LayerTier::Modal via assert_eq!  (crates/vox-compiler/src/hir/nodes/layer.rs)
- [happy] Maps 'Toast' to LayerTier::Toast via assert_eq!  (crates/vox-compiler/src/hir/nodes/layer.rs)
- [happy] Maps unknown strings like 'MyCustomThing' to LayerTier::Content as default via assert_eq!  (crates/vox-compiler/src/hir/nodes/layer.rs)

### `Module`  (happy; EXTRACTED)
- [happy] lex(), parse(), and lower_module() successfully convert source code through the pipeline  (crates/vox-compiler/src/app_contract.rs)
- [happy] Parsed module from fn declaration contains exactly one declaration  (crates/vox-compiler/tests/property_tests.rs)
- [happy] Fixture module contains at least 3 declarations (import, routes, and reactive components)  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] module parses import and function declarations as separate entries in declarations array with correct count  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Shape_from_json`  (error, happy; EXTRACTED)
- [happy] json_as ADT generator dispatches to correct variant based on tag field value  (crates/vox-compiler/tests/json_as_test.rs)
- [happy] json_as ADT generator correctly constructs second variant of tagged enum from JSON  (crates/vox-compiler/tests/json_as_test.rs)
- [error] json_as ADT generator returns Err when required tag field is missing from JSON  (crates/vox-compiler/tests/json_as_test.rs)
- [error] json_as ADT generator returns Err when tag field value does not match any variant  (crates/vox-compiler/tests/json_as_test.rs)

### `VoxCompilerDiagnosticPayload::from_diagnostic()`  (edge, happy; EXTRACTED)
- [happy] Generates a minimal repro excerpt with 3 lines of context before and after the error span, bounded by file limits  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [edge] Clips minimal repro excerpt to start of file without underflowing when error is near file start  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [edge] Clips minimal repro excerpt to end of file without overflowing when error is near file end  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [edge] Generates minimal repro excerpt for single-line file containing only the line text  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `apply_naming()`  (happy; EXTRACTED)
- [happy] apply_naming() returns input string unchanged when naming convention is snake_case  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] apply_naming() converts snake_case to camelCase correctly  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] apply_naming() converts snake_case to PascalCase correctly  (crates/vox-compiler/src/hir/lower/json_as.rs)
- [happy] apply_naming() converts snake_case to kebab-case correctly  (crates/vox-compiler/src/hir/lower/json_as.rs)

### `check_async_view`  (edge, error, happy, invariant; EXTRACTED)
- [happy] check_async_view returns None for exhaustive async views with all arms (fetching, empty, error, ok) present  (crates/vox-compiler/src/typeck/async_exhaustiveness.rs)
- [error] check_async_view detects missing arms in async views and returns diagnostic with code 'vox/async/missing-arm' listing missing cases  (crates/vox-compiler/src/typeck/async_exhaustiveness.rs)
- [edge] check_async_view reports all 4 missing cases when async view has no arms present  (crates/vox-compiler/src/typeck/async_exhaustiveness.rs)
- [invariant] check_async_view uses stable diagnostic code 'vox/async/missing-arm' per Phase 1 SSOT policy  (crates/vox-compiler/src/typeck/async_exhaustiveness.rs)

### `check_file_errors`  (error, happy; EXTRACTED)
- [happy] Typecheck passes without errors when calling public imported function  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [error] Typecheck reports error when calling private function from imported module  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [happy] Typecheck passes when calling aliased namespace method via import alias  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [error] Typecheck reports error when calling unknown method on import alias  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `check_semantic_ui()`  (error, happy; EXTRACTED)
- [error] check_semantic_ui emits exactly one diagnostic with code 'vox/a11y/dialog-missing-label' when Dialog is created without a label  (crates/vox-compiler/src/typeck/semantic_ui.rs)
- [happy] check_semantic_ui returns empty result when Dialog is created with a label  (crates/vox-compiler/src/typeck/semantic_ui.rs)
- [error] check_semantic_ui emits one diagnostic for each of Menu, Listbox, Combobox, and Tabs when created without a label  (crates/vox-compiler/src/typeck/semantic_ui.rs)
- [happy] check_semantic_ui returns empty result for unknown or custom primitive names regardless of label presence  (crates/vox-compiler/src/typeck/semantic_ui.rs)

### `generate_with_options`  (error, happy, invariant; EXTRACTED)
- [happy] generate_with_options produces output files when VOX_WEBIR_VALIDATE is unset  (crates/vox-compiler/tests/web_ir_environment_gates_test.rs)
- [error] generate_with_options returns error when VOX_WEBIR_VALIDATE=1 detects literal CSS color values in style blocks  (crates/vox-compiler/tests/web_ir_environment_gates_test.rs)
- [happy] codegen generates routes.manifest.ts file including nested routes, loaders, pending UI, and boundary exports  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] generated output does not contain legacy TanStack router imports or createServerFn APIs; vox-client.ts uses transport mechanisms instead  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `lower_hir_to_vox_ir`  (happy, invariant; EXTRACTED)
- [invariant] IR lowering produces non-empty source_hash metadata  (crates/vox-compiler/tests/ir_emission_test.rs)
- [happy] IR lowering includes regular functions from HIR in generated IR module  (crates/vox-compiler/tests/ir_emission_test.rs)
- [happy] IR lowering includes endpoint functions from HIR in generated IR module  (crates/vox-compiler/tests/ir_emission_test.rs)
- [happy] IR lowering includes scheduled jobs from @scheduled-decorated functions in web_ir  (crates/vox-compiler/tests/ir_emission_test.rs)

### `map_jsx_attr_name()`  (happy, invariant; EXTRACTED)
- [happy] map_jsx_attr_name() is publicly exported and produces equivalent output to the compat module version  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] hir_emit::map_jsx_attr_name and jsx::map_jsx_attr_name and hir_emit::compat::map_jsx_attr_name produce identical results for JSX attributes  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [invariant] Attr mapping is consistent across hir_emit, jsx, and compat implementations for edge cases like for/htmlFor, tab_index/tabIndex, class/className  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [invariant] jsx::map_jsx_attr_name and hir_emit::map_jsx_attr_name match GuiCompatibilityContract specifications  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `typecheck_module`  (error, happy; EXTRACTED)
- [error] db.table.query(clause) in @query function emits Error-level Lint diagnostic  (crates/vox-compiler/tests/db_query_safety_test.rs)
- [error] @query declaration rejects db insert operations with 'must be read-only' error  (crates/vox-compiler/tests/db_query_safety_test.rs)
- [happy] db.table.filter().limit(n) chaining does not report unsupported chain error  (crates/vox-compiler/tests/db_query_safety_test.rs)
- [happy] db.table.all().select() with all non-optional columns produces no typecheck errors  (crates/vox-compiler/tests/db_query_safety_test.rs)

### `vox_compiler::parser::renames::RenameRegistry::parse_json`  (error, happy; EXTRACTED)
- [happy] RenameRegistry accepts JSON with empty entries array  (crates/vox-compiler/tests/rename_alias_test.rs)
- [error] RenameRegistry rejects JSON with version != 1  (crates/vox-compiler/tests/rename_alias_test.rs)
- [error] RenameRegistry rejects entries with empty 'from' field  (crates/vox-compiler/tests/rename_alias_test.rs)
- [error] RenameRegistry rejects entries where 'from' equals 'to' (self-rename)  (crates/vox-compiler/tests/rename_alias_test.rs)

### `wcag21_contrast_ratio()`  (happy; EXTRACTED)
- [happy] wcag21_contrast_ratio() returns approximately 21.0 for maximum contrast (black #000000 on white #ffffff)  (crates/vox-compiler/src/tokens/contrast.rs)
- [happy] wcag21_contrast_ratio() returns approximately 1.0 for identical colors (white #ffffff on white #ffffff)  (crates/vox-compiler/src/tokens/contrast.rs)
- [happy] wcag21_contrast_ratio() accepts shorthand hex color notation (#000, #fff) and computes correct contrast ratio  (crates/vox-compiler/src/tokens/contrast.rs)
- [happy] wcag21_contrast_ratio() for navy #1d3557 on white #ffffff returns >= 4.5:1, meeting WCAG body text threshold  (crates/vox-compiler/src/tokens/contrast.rs)

### `@form decorator`  (happy; EXTRACTED)
- [happy] @form decorated declarations lower to HirModule with correct field count  (crates/vox-compiler/tests/form_hir_test.rs)
- [happy] @form with basic fields, constraints, and metadata parses and exposes field properties  (crates/vox-compiler/tests/form_parse_test.rs)
- [happy] @form fields support hidden modifier and default() constraint  (crates/vox-compiler/tests/form_parse_test.rs)

### `EventRow`  (happy; EXTRACTED)
- [happy] EventRow component references ts prop in compiled output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)
- [happy] EventRow component references kind prop in compiled output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)
- [happy] EventRow component references label prop in compiled output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)

### `Expr::Call`  (edge, happy; EXTRACTED)
- [happy] Parses function call with string argument containing backticks via pattern matching  (crates/vox-compiler/src/parser/descent/tests.rs)
- [edge] capitalized calls with positional arguments remain Expr::Call not JSX, preserving constructor semantics  (crates/vox-compiler/src/parser/descent/tests.rs)
- [edge] function calls with positional arguments parse as Expr::Call with args array where positional args have name=None  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Expr::JsxSelfClosing`  (happy; EXTRACTED)
- [happy] attr_ prefix on attribute names is stripped to base name during parsing  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] capitalized calls with no block and no arguments lower to Expr::JsxSelfClosing with tag field  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] capitalized calls with all-named arguments lower to Expr::JsxSelfClosing with attributes array populated  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Function`  (happy; EXTRACTED)
- [happy] AI decorator sets is_llm flag to true on function  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] AI decorator max_iterations parameter is parsed and stored as ai_max_iterations = 5  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] AI decorator structured_output parameter is parsed and stored as ai_structured_output_type = 'Plan'  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `HirCorsPolicy::allows_origin`  (happy; EXTRACTED)
- [happy] CORS policy with wildcard origin pattern allows any origin to pass  (crates/vox-compiler/src/hir/nodes/http_ergonomics.rs)
- [happy] CORS policy with specific origin allows that origin but denies requests from other origins  (crates/vox-compiler/src/hir/nodes/http_ergonomics.rs)
- [happy] CORS policy with empty origins list denies all origins  (crates/vox-compiler/src/hir/nodes/http_ergonomics.rs)

### `RunRow`  (happy; EXTRACTED)
- [happy] RunRow component emits at least 6 <p> elements in compiled TypeScript output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)
- [happy] RunRow emits either border-l-blue-500 or border-l-2 class for active state highlight  (crates/vox-compiler/tests/golden_runs_surface_test.rs)
- [happy] RunRow emits hover:bg-zinc-900 class for inactive state  (crates/vox-compiler/tests/golden_runs_surface_test.rs)

### `Scrape`  (happy, invariant; EXTRACTED)
- [invariant] builtin_registry_entries() contains exactly 4 Scrape namespace entries  (crates/vox-compiler/src/builtin_registry.rs)
- [happy] all Scrape builtin entries have runtime_symbol starting with 'vox_actor_runtime::builtins::vox_scrape_'  (crates/vox-compiler/src/builtin_registry.rs)
- [invariant] all Scrape builtins have returns_unit=false (return Result[str] not unit)  (crates/vox-compiler/src/builtin_registry.rs)

### `VoxValue`  (happy; EXTRACTED)
- [happy] VoxValue::Str wraps string values that can be passed to builtin methods  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] VoxValue has a Tagged variant with name and fields that can represent Regex and Match types  (crates/vox-compiler/src/eval/builtins.rs)
- [happy] VoxValue::list() constructs list values; VoxValue::List can be mutated via Rc::make_mut  (crates/vox-compiler/src/eval/env.rs)

### `WebIrModule`  (happy, invariant; EXTRACTED)
- [invariant] WebIrModule serializes to JSON with stable top-level field names: dom_nodes, view_roots, behavior_nodes, style_nodes, route_nodes, scheduled_jobs, interop_nodes, diagnostic_nodes, spans, version  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] WebIrModule with all node families serializes and deserializes through JSON roundtrip while maintaining validation cleanliness  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] WebIrModule contains diagnostic_nodes with category and code fields for lowering errors  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `call_builtin_method`  (happy, invariant; EXTRACTED)
- [happy] call_builtin_method successfully dispatches to implementation for every builtin in NAMESPACE_BUILTINS marked with surface::INTERP  (crates/vox-compiler/src/builtin_registry.rs)
- [invariant] call_builtin_method returns values whose shapes match the typecheck return type for pure/string-input builtins (path, regex, json, csv, toml, yaml, time, env, agentos namespaces)  (crates/vox-compiler/src/builtin_registry.rs)
- [invariant] call_builtin_method returns values whose shapes match the typecheck return type for fs and process namespace builtins with real file fixtures  (crates/vox-compiler/src/builtin_registry.rs)

### `canonical_app_contract_bytes()`  (invariant; EXTRACTED)
- [invariant] App contract canonical bytes are deterministic across multiple calls  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] App contract bytes remain deterministic when @back_button decorator is present  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] App contract bytes are deterministic across independent bundle creation calls  (crates/vox-compiler/tests/projection_parity_test.rs)

### `canonical_runtime_projection_bytes()`  (invariant; EXTRACTED)
- [invariant] Runtime projection canonical bytes are deterministic across multiple calls  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] Runtime projection bytes remain deterministic when @back_button decorator is present  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] Runtime projection bytes are deterministic across independent bundle creation calls  (crates/vox-compiler/tests/projection_parity_test.rs)

### `canonical_shell_projection_bytes()`  (invariant; EXTRACTED)
- [invariant] canonical_shell_projection_bytes() is idempotent: calling it twice on the same shell projection yields identical byte sequences  (crates/vox-compiler/src/shell_projection.rs)
- [invariant] returns identical bytes for the same shell projection  (crates/vox-compiler/tests/shell_projection_smoke_test.rs)
- [invariant] Shell projection bytes are deterministic across independent bundle creation calls  (crates/vox-compiler/tests/projection_parity_test.rs)

### `canonical_web_ir_bytes()`  (invariant; EXTRACTED)
- [invariant] Web IR canonical bytes are deterministic across multiple calls on the same bundle  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] Web IR bytes remain deterministic when @back_button decorator is present  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] Web IR bytes are deterministic across independent project_bundle_from_hir calls  (crates/vox-compiler/tests/projection_parity_test.rs)

### `canonicalize_vox()`  (error, happy, invariant; EXTRACTED)
- [invariant] canonicalize_vox() is idempotent: applying it twice yields the same result as applying once  (crates/vox-compiler/src/serialization.rs)
- [happy] canonicalize_vox() produces deterministic output 'fn main(){let x=10 return x}' for a canonical function definition  (crates/vox-compiler/src/serialization.rs)
- [error] canonicalize_vox() rejects malformed input and returns error containing 'parse validation'  (crates/vox-compiler/src/serialization.rs)

### `check_effect_compliance`  (happy; EXTRACTED)
- [happy] check_effect_compliance returns a vector of Diagnostic and parses through lex, parse, and lower_module stages  (crates/vox-compiler/src/typeck/effect_check.rs)
- [happy] check_effect_compliance allows a pure function with no calls and uses nothing annotation to return empty diagnostics  (crates/vox-compiler/src/typeck/effect_check.rs)
- [happy] check_effect_compliance does not enforce effect requirements on unannotated function caller even when callee requires net  (crates/vox-compiler/src/typeck/effect_check.rs)

### `codes::ALL_PHASE_1`  (invariant; EXTRACTED)
- [invariant] Every diagnostic code in ALL_PHASE_1 follows format vox/<lowercase-kebab-category>/<lowercase-kebab-id> with no leading/trailing hyphens  (crates/vox-compiler/tests/diagnostic_id_namespace.rs)
- [invariant] Every diagnostic code category in ALL_PHASE_1 is one of: types, effect, workflow, remote, api  (crates/vox-compiler/tests/diagnostic_id_namespace.rs)
- [invariant] No diagnostic code appears more than once in ALL_PHASE_1  (crates/vox-compiler/tests/diagnostic_id_namespace.rs)

### `emit_component_view_tsx()`  (happy; EXTRACTED)
- [happy] emit_component_view_tsx produces valid TSX output via snapshot assertion for component with view  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] emit_component_view_tsx successfully emits TSX for component with text() view containing string literal  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] emit_component_view_tsx() emits element attributes in lexicographic order (className before id)  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `generate()`  (happy; EXTRACTED)
- [happy] Web IR view emission succeeds even when VOX_WEBIR_EMIT_REACTIVE_VIEWS env var is explicitly set to 0  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Codegen produces no reactive_view_emit_failures when legacy env var is cleared  (crates/vox-compiler/tests/reactive_smoke_test.rs)
- [happy] Reactive component codegen produces valid TypeScript matching Counter.tsx snapshot  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `list subscript evaluation`  (edge, happy; EXTRACTED)
- [happy] List subscript with valid index evaluates to the element at that position when unwrapped  (crates/vox-compiler/tests/typed_subscript_test.rs)
- [edge] List subscript with out-of-bounds index evaluates to None, falling back to unwrap_or default  (crates/vox-compiler/tests/typed_subscript_test.rs)
- [edge] List subscript with negative index evaluates to None (no wraparound), falling back to unwrap_or default  (crates/vox-compiler/tests/typed_subscript_test.rs)

### `lower_module()`  (happy; EXTRACTED)
- [happy] lower_module produces valid HIR when parsing Vox component source succeeds  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_module creates valid HIR from parsed component with state and text view  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] Module with component, state, derived, on mount, and view blocks successfully lowers to HIR  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `parse_with_kind()`  (error, happy; EXTRACTED)
- [error] parsing module-scope state in a regular .vox file with FileKind::Source returns Err  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] parsing module-scope state with FileKind::ReactiveModule succeeds and produces a module with one ReactiveModule declaration  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] parsing module-scope state, derived, and effect in a .vox.ui file succeeds and produces a ReactiveModule with three members  (crates/vox-compiler/src/parser/descent/tests.rs)

### `run_file`  (edge, happy; EXTRACTED)
- [happy] Runtime successfully executes code calling imported public function  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [happy] Runtime successfully executes code calling aliased imported public function  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [edge] Runtime resolves cyclic imports without infinite loop and executes successfully  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `run_frontend_str`  (error, happy; EXTRACTED)
- [error] run_frontend_str returns Ok with diagnostics containing exactly one error with code E091 when given macro_rules! source  (crates/vox-compiler/src/pipeline.rs)
- [happy] Frontend pipeline infers return type 'int' for untyped integer expression  (crates/vox-compiler/tests/ir_emission_test.rs)
- [happy] Frontend accepts @scheduled decorator and processes scheduled function  (crates/vox-compiler/tests/ir_emission_test.rs)

### `run_frontend_str_with_options`  (edge, happy; EXTRACTED)
- [happy] Pipeline inlines public function bodies from imported modules into HIR  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [edge] Pipeline inlines functions through cyclic import dependencies without infinite recursion  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [happy] Pipeline inlines aliased imports under namespace-prefixed naming convention (<alias>__<fn>)  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `stale closure capture lint`  (edge, error, happy; EXTRACTED)
- [error] emits lint.closure.stale_capture warning when closure in on_mount reads state  (crates/vox-compiler/tests/stale_capture_test.rs)
- [edge] suppresses lint.closure.stale_capture when effect has explicit depends_on clause  (crates/vox-compiler/tests/stale_capture_test.rs)
- [happy] does not warn for closures used as event handlers in view expressions  (crates/vox-compiler/tests/stale_capture_test.rs)

### `ts_string_literal()`  (happy; EXTRACTED)
- [happy] ts_string_literal() escapes inner double quotes in JSON objects to produce valid TypeScript string literals  (crates/vox-compiler/src/lowering_shared/jsx.rs)
- [happy] ts_string_literal() properly escapes backslashes and control characters (\n, \t) in string literals for TypeScript output  (crates/vox-compiler/src/lowering_shared/jsx.rs)
- [happy] ts_string_literal() wraps plain ASCII strings in double quotes without modification  (crates/vox-compiler/src/lowering_shared/jsx.rs)

### `typecheck_ast_module()`  (happy; EXTRACTED)
- [happy] typecheck_ast_module() emits diagnostic with code 'lint.handler.uncancellable_async' for unannotated handler calling async endpoint and assigning state  (crates/vox-compiler/tests/async_handler_test.rs)
- [happy] typecheck_ast_module() does not emit 'lint.handler.uncancellable_async' for handler annotated @cancellable calling async endpoint  (crates/vox-compiler/tests/async_handler_test.rs)
- [happy] typecheck_ast_module() does not emit 'lint.handler.uncancellable_async' for synchronous state-only handler without async endpoint calls  (crates/vox-compiler/tests/async_handler_test.rs)

### `validate_token_registry`  (error, happy; EXTRACTED)
- [happy] validate_token_registry emits a diagnostic with code 'token.registry.empty' for empty token registries  (crates/vox-compiler/src/tokens/validate.rs)
- [happy] validate_token_registry returns empty diagnostics list for valid registries with color tokens  (crates/vox-compiler/src/tokens/validate.rs)
- [error] validate_token_registry rejects token registry keys containing whitespace with code 'token.registry.invalid_key' and messages containing the key name  (crates/vox-compiler/src/tokens/validate.rs)

### `validate_web_ir diagnostic`  (invariant; EXTRACTED)
- [invariant] literal_color_value diagnostic has category 'style' and message containing 'color'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] literal_color_value diagnostic has category 'style'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [invariant] literal_dimension_value diagnostic has category 'style' and message containing 'padding'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `vox_ir`  (happy, invariant; EXTRACTED)
- [invariant] Generated VoxIrModule serializes to JSON and conforms to vox-ir.schema.json  (crates/vox-compiler/tests/ir_emission_test.rs)
- [happy] Scheduled jobs in web_ir include correct name and interval attributes  (crates/vox-compiler/tests/ir_emission_test.rs)
- [invariant] Generated VoxIrModule with scheduled jobs conforms to vox-ir.schema.json  (crates/vox-compiler/tests/ir_emission_test.rs)

### `wasi_unsupported_rust_imports()`  (happy, invariant; EXTRACTED)
- [happy] wasi_unsupported_rust_imports() returns a collection containing 'reqwest'  (crates/vox-compiler/src/rust_interop_support.rs)
- [invariant] wasi_unsupported_rust_imports() returns a set matching the registry.wasi_unsupported_rust_imports entries from ecosystem-support.yaml  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)
- [invariant] crates supporting wasi are not in wasi_unsupported_rust_imports() and crates not supporting wasi are in it  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `wire_type_to_zod()`  (happy; EXTRACTED)
- [happy] wire_type_to_zod() maps WireType::Number to "z.number()", WireType::String to "z.string()", WireType::Bool to "z.boolean()", WireType::Unit to "z.void()", and WireType::Unknown to "z.any()"  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] wire_type_to_zod() maps WireType::DateTimeString to "z.string().datetime({ offset: true })"  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] wire_type_to_zod() appends "Schema" suffix to WireType::Ref type names  (crates/vox-compiler/src/contract_ir/tests.rs)

### `@example decorator`  (happy; EXTRACTED)
- [happy] @example decorated functions parse cleanly without error-level diagnostics  (crates/vox-compiler/tests/example_decorator_test.rs)
- [happy] @example decorator without a label argument parses successfully  (crates/vox-compiler/tests/example_decorator_test.rs)

### `AI decorator structured output validation`  (error, happy; EXTRACTED)
- [error] AI decorator with undeclared structured_output type emits vox/ai/return-shape-not-codec'd diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] AI decorator without structured_output does not emit return-shape-not-codec'd diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Browser`  (happy, invariant; EXTRACTED)
- [invariant] builtin_registry_entries() contains exactly 9 Browser namespace entries  (crates/vox-compiler/src/builtin_registry.rs)
- [happy] all Browser builtin entries have runtime_symbol starting with 'vox_actor_runtime::builtins::vox_browser_'  (crates/vox-compiler/src/builtin_registry.rs)

### `CORS decorator validation`  (error, happy; EXTRACTED)
- [error] CORS with wildcard origins and allow_credentials=true emits vox/cors/credentials-with-wildcard diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] CORS with explicit specific origin and allow_credentials=true does not emit credentials-with-wildcard diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Decl::V0Component`  (happy; EXTRACTED)
- [happy] V0Component declaration with quoted ID parses name, v0_id, and image_path as None  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] V0Component from image path parses name, v0_id as empty string, and image_path correctly  (crates/vox-compiler/src/parser/descent/tests.rs)

### `FnDecl`  (happy; EXTRACTED)
- [happy] @versioned decorator sets is_versioned flag on parsed function declaration  (crates/vox-compiler/tests/versioned_decorator.rs)
- [happy] @tracked decorator alias sets same is_versioned flag on function declaration  (crates/vox-compiler/tests/versioned_decorator.rs)

### `HirType`  (happy; EXTRACTED)
- [happy] HirType has a Generic variant that accepts a name string and vector of element types  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] HirType has a Named variant for user-defined type names  (crates/vox-compiler/src/contract_ir/tests.rs)

### `ImportPathKind::RustCrate()`  (happy; EXTRACTED)
- [happy] Parses Rust crate import with rust: prefix via pattern matching  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] Parses Rust crate metadata fields version, git, and rev via assert_eq!  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Json.pointer`  (edge, happy; EXTRACTED)
- [happy] pointer() method successfully navigates nested JSON structures using RFC 6901 JSON Pointer paths  (crates/vox-compiler/tests/json_ergonomics_test.rs)
- [edge] pointer() method returns None when path does not exist in JSON structure  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Layer decorator tier validation`  (error, happy; EXTRACTED)
- [error] Layer decorator with tier: system_overlay emits vox/layer/reserved-tier diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] Layer decorator with tier: modal does not emit reserved-tier diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `LayerTier::allows_child`  (happy; EXTRACTED)
- [happy] Popover tier cannot host Modal children, but Modal can host itself and Popover can be hosted by Modal  (crates/vox-compiler/src/hir/nodes/layer.rs)
- [happy] Stronger tier parents can host weaker-tier children but not stronger tiers; Content cannot host Modal or Popover children  (crates/vox-compiler/src/hir/nodes/layer.rs)

### `PII decorator validation for network effects`  (error, happy; EXTRACTED)
- [error] PII decorator without @uses(net) annotation emits vox/pii/unannotated-net-effect diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] PII decorator with @uses(net) annotation does not emit unannotated-net-effect diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Routes block lowering to route identifiers`  (happy; EXTRACTED)
- [happy] Routes block with two route declarations produces exactly 2 route_ids in HIR  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)
- [happy] Routes block with single route declaration produces at least one route_id  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Stmt::Let`  (happy; EXTRACTED)
- [happy] Parses let statement in function body via assert! with matches! pattern  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] The synthetic main function body contains the top-level statements as Let statements  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Token::TemplateStringLit`  (happy; EXTRACTED)
- [happy] template string with identifier in braces is lexed as TemplateStringLit  (crates/vox-compiler/tests/lexer_json_template_test.rs)
- [happy] template string with optional whitespace inside braces is lexed as TemplateStringLit  (crates/vox-compiler/tests/lexer_json_template_test.rs)

### `TokenRegistry`  (happy; EXTRACTED)
- [happy] TokenRegistry loads annotated token values (with metadata) from JSON and lookup() returns correct values  (crates/vox-compiler/src/tokens/mod.rs)
- [happy] TokenRegistry parses contrast pair definitions from JSON with correct foreground_key, background_key, and text_role fields  (crates/vox-compiler/src/tokens/mod.rs)

### `TokenRegistry::validate_contrast()`  (error, happy; EXTRACTED)
- [happy] TokenRegistry::validate_contrast() returns empty diagnostics array when all token contrast pairs meet threshold requirements  (crates/vox-compiler/src/tokens/mod.rs)
- [error] TokenRegistry::validate_contrast() produces a ContrastSeverity::Error diagnostic when color contrast ratio fails minimum threshold  (crates/vox-compiler/src/tokens/mod.rs)

### `VoxCompilerDiagnosticPayload`  (happy; EXTRACTED)
- [happy] minimal_repro field is None when source is empty string  (crates/vox-compiler/src/typeck/diagnostics.rs)
- [happy] minimal_repro contains excerpt_first_line, excerpt text, and local_span with correct start_line and end_line for multi-line diagnostic spans  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `check_ai_return_shape()`  (error, happy; EXTRACTED)
- [error] AI return type without codec generates diagnostic with code vox/ai/return-shape-not-codec'd  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] AI return type with codec passes validation  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_capability_leak`  (error, happy; EXTRACTED)
- [error] check_capability_leak detects capability leaks when required capability is not held by principal, emitting diagnostic with code 'vox/auth/capability-leak'  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] check_capability_leak returns empty diagnostics when required capability is present in principal's capability set  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_dangling_marks()`  (error, happy; EXTRACTED)
- [error] check_dangling_marks emits exactly one diagnostic with code 'vox/layer/dangling-mark' when a mark reference has no corresponding mark declaration  (crates/vox-compiler/src/typeck/layer.rs)
- [happy] check_dangling_marks returns empty result when all mark references resolve to existing mark declarations  (crates/vox-compiler/src/typeck/layer.rs)

### `check_duplicate_marks()`  (error, happy; EXTRACTED)
- [error] check_duplicate_marks emits exactly one diagnostic with code 'vox/layer/duplicate-mark' when duplicate HirMark labels exist  (crates/vox-compiler/src/typeck/layer.rs)
- [happy] check_duplicate_marks returns empty result when all HirMark labels are unique  (crates/vox-compiler/src/typeck/layer.rs)

### `check_file`  (error, happy; EXTRACTED)
- [error] check_file produces exactly one diagnostic with error code E091 containing 'SyntacticConfigurabilityNotAllowed' when given macro_rules! source  (crates/vox-compiler/src/pipeline.rs)
- [happy] check_file produces no Error-severity diagnostics for actor declaration (ADR-041 compatibility)  (crates/vox-compiler/src/pipeline.rs)

### `check_missing_effect_decl()`  (error, happy; EXTRACTED)
- [error] Missing net effect declaration generates diagnostic with code vox/effect/missing-net-decl  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] Declared Fs effect passes validation when used  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_pii_leak()`  (error, happy; EXTRACTED)
- [error] PII leak generates diagnostic with code vox/taint/pii-leak when not redacted  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] PII marker passes validation when either redacted or uses_internal flags are true  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_state_machines()`  (happy; EXTRACTED)
- [happy] check_state_machines returns empty diagnostics when parsing and lowering a valid module with state_machine definition  (crates/vox-compiler/src/typeck/state_machine_check.rs)
- [happy] check_state_machines returns empty result for a complete state_machine with exhaustive transition coverage  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `check_tier_inversions()`  (happy; EXTRACTED)
- [happy] check_tier_inversions returns an empty result when Tooltip is nested inside Dialog  (crates/vox-compiler/src/typeck/layer.rs)
- [happy] check_tier_inversions returns empty result when Tooltip with explicit Content tier is nested in Modal parent  (crates/vox-compiler/src/typeck/layer.rs)

### `check_tokens()`  (happy; EXTRACTED)
- [happy] Emits one diagnostic with code 'vox/tokens/contrast-violation' when color token light and dark values violate contrast ratio  (crates/vox-compiler/src/typeck/contrast.rs)
- [happy] Emits no diagnostics when color token light and dark values meet contrast requirements  (crates/vox-compiler/src/typeck/contrast.rs)

### `check_upload_type()`  (error; EXTRACTED)
- [error] Upload type with zero max bytes generates diagnostic with code vox/upload/zero-max-bytes  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [error] Upload type with empty mime pattern generates diagnostic with code vox/upload/empty-mime  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_url_decls()`  (error, happy; EXTRACTED)
- [happy] check_url_decls() returns empty vec when URL declaration has well-formed variants with no duplicates  (crates/vox-compiler/src/typeck/url_check.rs)
- [error] check_url_decls() emits diagnostic with code E201 when URL variants have duplicate names  (crates/vox-compiler/src/typeck/url_check.rs)

### `check_vector_dimension()`  (error, happy; EXTRACTED)
- [error] Vector dimension mismatch generates diagnostic with code vox/vector/dimension-mismatch  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] Vectors with identical dimensions pass validation  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_webhook_decl()`  (error, happy; EXTRACTED)
- [error] Webhook with Custom provider and empty secret_var generates diagnostic with code vox/webhook/missing-secret-var  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)
- [happy] Validates that webhook declarations with replay_window_secs out of range emit a diagnostic with code 'vox/webhook/replay-window-out-of-range'  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `classify_rust_crate()`  (invariant; EXTRACTED)
- [invariant] classify_rust_crate() maps serde_json and time to FirstClassWrapper, tokio to InternalRuntimeOnly  (crates/vox-compiler/src/rust_interop_support.rs)
- [invariant] classify_rust_crate() maps sqlx to Deferred and unknown crates to EscapeHatchOnly  (crates/vox-compiler/src/rust_interop_support.rs)

### `contrast_ratio()`  (edge, happy; EXTRACTED)
- [happy] Computes contrast ratio between black and white as approximately 21:1, meeting or exceeding MIN_CONTRAST_RATIO  (crates/vox-compiler/src/typeck/contrast.rs)
- [edge] Returns a ratio below MIN_CONTRAST_RATIO when comparing similar gray colors  (crates/vox-compiler/src/typeck/contrast.rs)

### `db.Table.insert`  (happy; EXTRACTED)
- [happy] Multiple insert operations successfully add rows to database  (crates/vox-compiler/tests/interpreter_db_test.rs)
- [happy] insert returns monotonically increasing _id values starting at 0  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `emit_component_view_tsx`  (happy; EXTRACTED)
- [happy] emit_component_view_tsx successfully emits TSX for Counter component with state and derived fields  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] web_ir emit produces identical TypeScript output to direct HIR emit for self-closing JSX elements  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `fmt::format`  (invariant; EXTRACTED, INFERRED)
- [invariant] Formatting the same source code twice produces the same result (format is idempotent)  (crates/vox-compiler/tests/fmt_idempotent.rs)
- [invariant] Formatting code and re-parsing produces an AST with equivalent structure (spans stripped)  (crates/vox-compiler/tests/format_round_trip.rs)

### `inline_imported_decls`  (invariant; EXTRACTED, INFERRED)
- [invariant] Pipeline does not inline private functions from imported modules, preserving privacy invariant  (crates/vox-compiler/tests/intra_project_imports_test.rs)
- [invariant] Pipeline loads each file at most once during transitive inlining across cycles  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `language_surface::LEXER_DECORATORS`  (error, invariant; EXTRACTED)
- [invariant] every entry in LSP_DECORATOR_DOCS is present in LEXER_DECORATORS  (crates/vox-compiler/tests/language_surface_ssot_test.rs)
- [error] @component decorator does not appear in LEXER_DECORATORS  (crates/vox-compiler/tests/language_surface_ssot_test.rs)

### `lint_ast_declarations()`  (happy; EXTRACTED)
- [happy] lint_ast_declarations() emits a diagnostic with code 'lint.pure_shallow_violation' and severity Warning when @pure function calls print()  (crates/vox-compiler/tests/ast_decl_lints_pure_test.rs)
- [happy] lint_ast_declarations() does not emit 'lint.pure_shallow_violation' for @pure function returning Unit without side effects  (crates/vox-compiler/tests/ast_decl_lints_pure_test.rs)

### `lower_hir_to_web_ir_with_summary`  (happy; EXTRACTED)
- [happy] Summary counts correctly distinguish @server, @query, and @mutation function contracts  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] scheduled jobs marked with @scheduled in Vox source lower to WebIrModule.scheduled_jobs with name and interval fields intact  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `lower_hir_to_web_ir_with_summary()`  (happy; EXTRACTED)
- [happy] lower_hir_to_web_ir_with_summary() returns a summary with lowering_diagnostics count tracking AST node diagnostics  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [happy] lower_hir_to_web_ir_with_summary() returns a summary tracking component count and dom_expr_fallback count  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `parse_hex_luminance()`  (error, happy; EXTRACTED)
- [error] Rejects invalid hex color strings by returning Err for malformed or non-color inputs  (crates/vox-compiler/src/typeck/contrast.rs)
- [happy] Parses shorthand hex color (#FFF) to the same luminance as full form (#FFFFFF)  (crates/vox-compiler/src/typeck/contrast.rs)

### `parse_script`  (happy; EXTRACTED)
- [happy] Top-level let statement wraps in synthetic main function with correct name, zero params, and Let statement body  (crates/vox-compiler/src/parser/descent/tests.rs)
- [happy] Top-level expression becomes Stmt::Expr inside synthetic main function  (crates/vox-compiler/src/parser/descent/tests.rs)

### `project_bundle_from_hir()`  (happy, invariant; EXTRACTED)
- [happy] Bundle capabilities contain net.http and notifications when @uses(net) and @push decorators present  (crates/vox-compiler/tests/projection_parity_test.rs)
- [invariant] Canonical projections produce pairwise-distinct SHA3 hashes across web, app, runtime, shell, and capabilities  (crates/vox-compiler/tests/projection_parity_test.rs)

### `project_type()`  (happy; EXTRACTED)
- [happy] project_type() maps generic list-like types (list, List, Vec, Array) containing an element type to WireType::Array  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] project_type() maps user-named HirType (non-generic) to WireType::Ref with the same name  (crates/vox-compiler/src/contract_ir/tests.rs)

### `required_capabilities`  (happy; EXTRACTED)
- [happy] Speech.transcribe() derives 'speech' capability but not 'microphone' when used in function return type  (crates/vox-compiler/tests/required_capabilities_test.rs)
- [happy] fs.read() in function body maps to 'fs.read' capability  (crates/vox-compiler/tests/required_capabilities_test.rs)

### `state machine is_partial flag`  (happy; EXTRACTED)
- [happy] is_partial is false for non-partial state machines  (crates/vox-compiler/tests/state_machine_integration_test.rs)
- [happy] is_partial is true when state machine is declared partial  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `std_namespace_method_ty`  (happy; EXTRACTED)
- [happy] std_namespace_method_ty returns Some for every namespace/method pair in NAMESPACE_BUILTINS  (crates/vox-compiler/src/builtin_registry.rs)
- [happy] std_namespace_method_ty returns Some for listed methods in owned namespaces (e.g. fs.read) and None for unlisted methods (e.g. fs.totally_not_a_real_fs_method)  (crates/vox-compiler/src/builtin_registry.rs)

### `stdlib http.get() capability requirements`  (error, happy; EXTRACTED)
- [error] Calling http.get() without 'uses net' produces one diagnostic error mentioning both 'net' and the call site  (crates/vox-compiler/src/typeck/effect_check.rs)
- [happy] Calling http.get() with 'uses net' produces no diagnostics  (crates/vox-compiler/src/typeck/effect_check.rs)

### `suggest_tokens()`  (edge, happy; EXTRACTED)
- [happy] suggest_tokens() performs fuzzy string matching to find similar token names (e.g., 'color-primaty' suggests 'color-primary')  (crates/vox-compiler/src/tokens/mod.rs)
- [edge] suggest_tokens() returns empty suggestions array for completely dissimilar keys with no fuzzy matches  (crates/vox-compiler/src/tokens/mod.rs)

### `supported_targets_for_rust_crate()`  (happy; EXTRACTED)
- [happy] supported_targets_for_rust_crate('serde_json') returns targets containing RustInteropTarget::Wasi  (crates/vox-compiler/src/rust_interop_support.rs)
- [happy] supported_targets_for_rust_crate('reqwest') returns targets not containing RustInteropTarget::Wasi, and 'future_crate' returns None  (crates/vox-compiler/src/rust_interop_support.rs)

### `validate_web_ir_with_registry()`  (error, happy; EXTRACTED)
- [happy] validate_web_ir_with_registry() does not emit diagnostic with code 'web_ir_validate.surface.unknown_surface' when data-vox-surface attribute contains a surface name defined in the token registry  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)
- [error] validate_web_ir_with_registry() emits diagnostic with code 'web_ir_validate.surface.unknown_surface' when data-vox-surface attribute contains a surface name not defined in the token registry  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `vox_compiler::required_capabilities::project_required_capabilities`  (happy; EXTRACTED)
- [happy] Endpoint function with 'uses net' effect maps to 'net.http' capability ID  (crates/vox-compiler/tests/required_capabilities_test.rs)
- [happy] Speech.transcribe_microphone call derives both 'speech' and 'microphone' capability IDs  (crates/vox-compiler/tests/required_capabilities_test.rs)

### `vox_compiler::typeck::typecheck_ast_module`  (error, happy; EXTRACTED)
- [happy] @remote fn with int and str parameters produces no vox/remote/non-serializable-param diagnostic  (crates/vox-compiler/tests/remote_fn.rs)
- [error] @remote fn with function-typed parameter produces vox/remote/non-serializable-param error diagnostic  (crates/vox-compiler/tests/remote_fn.rs)

### `wire_type_to_ts function`  (happy; EXTRACTED)
- [happy] wire_type_to_ts maps WireType::Number to 'number', WireType::String to 'string', WireType::Bool to 'boolean', WireType::Unit to 'void', and WireType::Unknown to 'unknown'  (crates/vox-compiler/src/contract_ir/tests.rs)
- [happy] wire_type_to_ts maps string-encoded WireTypes (DecimalString, BigIntString, DateTimeString) to the string literal 'string'  (crates/vox-compiler/src/contract_ir/tests.rs)

### `? operator error propagation`  (happy; EXTRACTED)
- [happy] ? operator on Result<Err> causes early return from enclosing function  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `? operator on Option`  (happy; EXTRACTED)
- [happy] ? operator on Option<None> causes early return from enclosing function  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `? operator on Result`  (happy; EXTRACTED)
- [happy] ? operator on Result<Ok> unwraps the inner value without early return  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `@ai decorator intent routing metadata parsing`  (happy; EXTRACTED)
- [happy] The @ai decorator parses and lowers task_category, strengths list, tier_max, and cost_ceiling_usd_per_call into HirAiFixture::IntentRouted  (crates/vox-compiler/tests/mens_decorators.rs)

### `@ai decorator tier validation`  (error; EXTRACTED)
- [error] The @ai decorator rejects unknown tier_max values by not storing them in the parsed result  (crates/vox-compiler/tests/mens_decorators.rs)

### `@field_name attribute in @json_as`  (happy; EXTRACTED)
- [happy] The @field_name attribute overrides the JSON key mapping, allowing deserialization from a different JSON property name  (crates/vox-compiler/tests/json_as_test.rs)

### `@inference decorator parsing`  (happy; EXTRACTED)
- [happy] The @inference decorator parses and stores the model parameter as a function attribute  (crates/vox-compiler/tests/mens_decorators.rs)

### `@pure decorator effect semantics`  (invariant; EXTRACTED)
- [invariant] The @pure decorator is treated as 'uses nothing' for effect-checking purposes, requiring pure functions to not call effectful operations  (crates/vox-compiler/src/typeck/effect_check.rs)

### `@test decorator`  (invariant; EXTRACTED)
- [invariant] @test blocks remain separate in HirModule::tests and do not leak into HirModule::examples  (crates/vox-compiler/tests/example_decorator_test.rs)

### `@tokens block parsing`  (happy; EXTRACTED)
- [happy] @tokens declaration block with color, spacing, font definitions parses successfully without errors  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `@uses decorator parsing`  (happy; EXTRACTED)
- [happy] @uses(net) decorator on function parses successfully and populates function effects with Net annotation  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `@uses with multiple effects`  (happy; EXTRACTED)
- [happy] @uses(net, fs) decorator populates function effects with both Net and Fs effect annotations  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `ADT type registration and constructor binding`  (happy; EXTRACTED)
- [happy] ADT registration creates ADT lookups and registers constructors as bindings; parameterized constructors have function type, nullary constructors have the ADT type directly  (crates/vox-compiler/src/typeck/env.rs)

### `ALL_COMPILER_DIAGNOSTIC_CODES`  (invariant; EXTRACTED)
- [invariant] every code in ALL_COMPILER_DIAGNOSTIC_CODES is unique with no duplicates  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `AtExample token`  (happy; EXTRACTED)
- [happy] @example token is recognized by the lexer and compiles without error-level diagnostics  (crates/vox-compiler/tests/example_decorator_test.rs)

### `BinOp`  (happy; EXTRACTED)
- [happy] Respects operator precedence with multiplication higher than addition via assert! with nested pattern matching  (crates/vox-compiler/src/parser/descent/tests.rs)

### `BinOp::Add`  (happy; EXTRACTED)
- [happy] Parses binary addition in function call arguments via pattern matching assert!  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Config_to_json serialization`  (happy; EXTRACTED)
- [happy] Config_to_json emits a JSON object containing the correct keys and values from a Config record  (crates/vox-compiler/tests/json_as_test.rs)

### `Counter_from_json deserialization`  (happy; EXTRACTED)
- [happy] Counter_from_json successfully extracts integer field from JSON payload and unwraps to the correct int value  (crates/vox-compiler/tests/json_as_test.rs)

### `Decl`  (happy; EXTRACTED)
- [happy] Routes declaration parse_summary yields entry_count of 2 and paths [/, /health]  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `Decl::Activity`  (happy; EXTRACTED)
- [happy] activity keyword parses as Decl::Activity with name field accessible  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Actor`  (happy; EXTRACTED)
- [happy] Parses actor declaration with name 'Worker' via assert! with matches! pattern  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Agent`  (invariant; EXTRACTED)
- [invariant] No assertions execute because test is marked #[ignore]  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Endpoint`  (happy; EXTRACTED)
- [happy] Server-decorated function parses as Endpoint with Server kind and correct function metadata  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Function effects field when uses clause absent`  (happy; EXTRACTED)
- [happy] Functions without a uses clause have an empty effects vector  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Import`  (happy; EXTRACTED)
- [happy] Dotted-path imports (std.http) parse and coexist with function declarations in module  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Index`  (happy; EXTRACTED)
- [happy] Index declaration parses table_name, index_name, and column list correctly  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Loading`  (happy; EXTRACTED)
- [happy] Parses @loading decorator and extracts function name via assert! with matches! pattern  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Table`  (happy; EXTRACTED)
- [happy] Table declaration parses name and fields list with correct field count and names  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::TypeDef`  (happy; EXTRACTED)
- [happy] Parses type definition with name 'Shape' via assert_eq!  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Url`  (happy; EXTRACTED)
- [happy] A simple url declaration with one variant parses correctly with the variant name and empty argument list  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Url is_pub field`  (happy; EXTRACTED)
- [happy] By default, url declarations without pub keyword are not public  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Url optional arguments`  (happy; EXTRACTED)
- [happy] Url variant arguments prefixed with ? are parsed as optional parameters  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Url variant arguments`  (happy; EXTRACTED)
- [happy] Variant arguments have name and optional flag properties that can be queried  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Url with parameterized variants`  (happy; EXTRACTED)
- [happy] Url variants can have typed arguments with required (non-optional) parameters  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Workflow`  (happy; EXTRACTED)
- [happy] workflow keyword parses as Decl::Workflow with name field accessible  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Decl::Workflow, distributed_train_strategy, distributed_train_peers`  (happy; EXTRACTED)
- [happy] DistributedTrain decorator attributes parse from workflow source and store strategy and peers in Decl::Workflow  (crates/vox-compiler/tests/mens_decorators.rs)

### `Diagnostic::with_code`  (happy; EXTRACTED)
- [happy] Diagnostic::with_code attaches stable code string vox/types/type-mismatch that persists to payload.error_code  (crates/vox-compiler/src/typeck/diagnostics.rs)

### `Dialog accessibility check`  (error; EXTRACTED)
- [error] Dialog view without label prop emits vox/a11y/dialog-missing-label diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `Dialog accessibility fix`  (happy; EXTRACTED)
- [happy] Dialog view with label= property does not emit vox/a11y/dialog-missing-label diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `DomNode`  (invariant; EXTRACTED)
- [invariant] DomNode::Element can have a tag 'div' with attrs containing className key-value pairs  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `DurablePromise type constructor`  (error; EXTRACTED)
- [error] DurablePromise without a type argument emits a vox/types/durable-promise-arity diagnostic  (crates/vox-compiler/tests/durable_promise.rs)

### `Effect checker diagnostic messages`  (invariant; EXTRACTED)
- [invariant] Effect mismatch diagnostics contain the missing capability name in their message  (crates/vox-compiler/src/typeck/effect_check.rs)

### `Effect checker with annotated callers`  (error; EXTRACTED)
- [error] When a function annotated with specific effect capabilities calls a function requiring unannounced effects, the effect checker produces exactly one diagnostic error  (crates/vox-compiler/src/typeck/effect_check.rs)

### `Effect checker with superset capabilities`  (happy; EXTRACTED)
- [happy] When a caller declares a superset of callees' required effects, no diagnostics are emitted  (crates/vox-compiler/src/typeck/effect_check.rs)

### `EffectAnnotation with multiple effects`  (happy; EXTRACTED)
- [happy] Multiple comma-separated effects in a uses clause parse into a vector of corresponding EffectAnnotation values  (crates/vox-compiler/src/parser/descent/tests.rs)

### `EffectAnnotation::Env`  (happy; EXTRACTED)
- [happy] The @uses decorator with env keyword parses env as EffectAnnotation::Env effect  (crates/vox-compiler/src/parser/descent/tests.rs)

### `EffectAnnotation::Mcp with parameter`  (happy; EXTRACTED)
- [happy] The uses clause with mcp(name) syntax parses as EffectAnnotation::Mcp with the parameter string captured  (crates/vox-compiler/src/parser/descent/tests.rs)

### `EffectAnnotation::Net`  (happy; EXTRACTED)
- [happy] The uses clause with 'net' keyword parses as EffectAnnotation::Net effect  (crates/vox-compiler/src/parser/descent/tests.rs)

### `EffectAnnotation::Nothing`  (happy; EXTRACTED)
- [happy] The uses clause with 'nothing' keyword parses as EffectAnnotation::Nothing in the function's effects field  (crates/vox-compiler/src/parser/descent/tests.rs)

### `EsImportKind`  (happy; EXTRACTED)
- [happy] EsImportKind::Named tracks the original exported name separately from the local alias, allowing 'Trigger as DialogTrigger' to record imported='Trigger'  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `EvalError`  (error; EXTRACTED)
- [error] EvalError::TypeError variant is raised for type mismatches in repo operations  (crates/vox-compiler/src/eval/repo.rs)

### `Expr::Binary`  (happy; EXTRACTED)
- [happy] Parses binary operations with correct structure via pattern matching  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Expr::For`  (happy; EXTRACTED)
- [happy] for loop syntax parses as Expr::For with binding field containing loop variable name  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Expr::If`  (happy; EXTRACTED)
- [happy] else-if chains parse as nested Expr::If with single-statement else bodies, each containing another Expr::If  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Expr::Jsx`  (happy; EXTRACTED)
- [happy] view-call form `Ident(kwargs) { children }` lowers to Expr::Jsx with tag, attributes array, and children array  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Expr::With`  (happy; EXTRACTED)
- [happy] With expression contains operand and options fields both matching their expected expression types  (crates/vox-compiler/src/parser/descent/tests.rs)

### `FileKind::allows_module_scope_reactive_members`  (happy; EXTRACTED)
- [happy] FileKind::allows_module_scope_reactive_members returns true only for FileKind::ReactiveModule, and false for Source and Unknown variants  (crates/vox-compiler/src/module.rs)

### `Flag_from_json bool field deserialization`  (happy; EXTRACTED)
- [happy] Flag_from_json successfully deserializes bool fields from JSON and returns the correct boolean value  (crates/vox-compiler/tests/json_as_test.rs)

### `FnDecl.prompt_stage, FnDecl.prompt_schema, FnDecl.prompt_redact`  (happy; EXTRACTED)
- [happy] Prompt decorator attributes parse and lower from AST to HirAiFixture::Prompt with stage, schema, and redact fields preserved  (crates/vox-compiler/tests/mens_decorators.rs)

### `FnDecl.search_corpus, FnDecl.search_query, FnDecl.search_into, FnDecl.search_top_k, FnDecl.search_policy`  (happy; EXTRACTED)
- [happy] Search decorator attributes parse and lower from AST to HirAiFixture::Search with corpus, query, into_type, top_k, and policy fields preserved  (crates/vox-compiler/tests/mens_decorators.rs)

### `FnDecl.subagent_policy, FnDecl.subagent_max_depth, FnDecl.subagent_budget_usd, FnDecl.subagent_description`  (happy; EXTRACTED)
- [happy] Subagent decorator attributes parse and lower from AST to HirAiFixture::Subagent with policy, max_depth, budget_usd, and description preserved  (crates/vox-compiler/tests/mens_decorators.rs)

### `Future[T] deprecation fix suggestion`  (happy; EXTRACTED)
- [happy] Future[T] deprecation diagnostic provides a fix suggestion that replaces it with DurablePromise[T]  (crates/vox-compiler/tests/future_promise_deprecation.rs)

### `Future[T] type`  (happy; EXTRACTED)
- [happy] Future[T] type annotation produces vox/types/future-deprecated warning  (crates/vox-compiler/tests/future_promise_deprecation.rs)

### `GuiCompatibilityContract`  (invariant; EXTRACTED)
- [invariant] gui-compatibility.v1.yaml contract react_attr_matrix aligns with hir_emit::compat::map_jsx_attr_name implementation  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `HIR lowering of @query and @mutation declarations`  (happy; EXTRACTED)
- [happy] lower_module maps @query and @mutation declarations to HirEndpointFn entries with correct QUERY_FN_API_PREFIX and MUTATION_FN_API_PREFIX route_path prefixes  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of @table, http endpoint, @server, @component`  (invariant; EXTRACTED)
- [invariant] lower_module ensures legacy_ast_nodes is empty when lowering @table, http endpoint, @server fn, and @component declarations  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of collection, vector index, and search index declarations`  (invariant; EXTRACTED)
- [invariant] lower_module populates collections, vector_indexes, and search_indexes fields and ensures legacy_ast_nodes remains empty for these declaration types  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of db all().select() projection`  (happy; EXTRACTED)
- [happy] lower_module populates the projection field in HirDbTableOp when lowering db.User.all().select() chains  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of db chain capability modifiers`  (happy; EXTRACTED)
- [happy] lower_module populates HirDbQueryPlan.capabilities with sync, live_topic, orchestration_scope, and retrieval_mode when lowering db.User.filter().using().live().scope().sync().limit() chains  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of db filter chain`  (happy; EXTRACTED)
- [happy] lower_module lowers db.User.filter() calls to HirExpr::MethodCall with HirDbTableOp::FilterRecord in the query plan  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of db filter+count chain`  (happy; EXTRACTED)
- [happy] lower_module converts db.User.filter().count() to a count MethodCall with HirDbTableOp::Count and preserves filter arguments  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of db query chain with order_by and limit`  (happy; EXTRACTED)
- [happy] lower_module preserves order_by and limit modifiers in HirDbTableOp when lowering db.User.filter().order_by().limit() chains  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of environment declarations`  (happy; EXTRACTED)
- [happy] lower_module parses environment declarations and populates HirModule.environments with correct name, base_image, and packages fields  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of golden CRUD API example`  (invariant; EXTRACTED)
- [invariant] lower_module produces empty legacy_ast_nodes when lowering the golden crud_api.vox example  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HIR lowering of url declarations`  (invariant; EXTRACTED)
- [invariant] lower_module populates url_decls collection and does not allow url declarations to fall into legacy_ast_nodes, preserving variant structure and argument metadata  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HirAiFixture::Hole, typecheck_hir_module()`  (happy; EXTRACTED)
- [happy] Hole decorator with unfilled fixture generates diagnostic code vox/fixture/unfilled-hole during typecheck  (crates/vox-compiler/tests/mens_decorators.rs)

### `HirCapability`  (happy; EXTRACTED)
- [happy] @versioned decorator implicitly grants Vcs capability to function  (crates/vox-compiler/tests/versioned_decorator.rs)

### `HirExpr::WorkflowVersion`  (happy; EXTRACTED)
- [happy] When workflow.version() is parsed and lowered to HIR, the resulting body contains HirExpr::WorkflowVersion with the correct change_id  (crates/vox-compiler/tests/workflow_version.rs)

### `HirFn`  (happy; EXTRACTED)
- [happy] HirFn carries is_versioned flag after lowering  (crates/vox-compiler/tests/versioned_decorator.rs)

### `HirFn.capabilities, HirCapability::GpuCompute, HirCapability::Random, HirCapability::Net`  (happy; EXTRACTED)
- [happy] Inference decorator lowers to HirFn with capabilities containing GpuCompute, Random, and Net  (crates/vox-compiler/tests/mens_decorators.rs)

### `HirFn.distributed_train, HirCapability::Spawn, HirCapability::Net`  (happy; EXTRACTED)
- [happy] DistributedTrain decorator lowers to HirFn with distributed_train metadata and Spawn and Net capabilities  (crates/vox-compiler/tests/mens_decorators.rs)

### `HirFn.training_step, HirFn.capabilities, HirCapability::GpuCompute, HirCapability::Mutate`  (happy; EXTRACTED)
- [happy] TrainingStep decorator sets HirFn.training_step flag and adds GpuCompute and Mutate capabilities  (crates/vox-compiler/tests/mens_decorators.rs)

### `HirImport, destructured single-item import`  (happy; EXTRACTED)
- [happy] Destructured import with single item as { Name } creates one HirImport with correct module_path and item  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `HirImport.module_path, HirImport.item`  (happy; EXTRACTED)
- [happy] Forward slash separators in import paths parse identically to dot separators in module_path and item  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `HirModule deserialization`  (error; EXTRACTED)
- [error] HirModule deserialization rejects malformed span keys in JSON with an error (not panic)  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)

### `HirModule deserialization error message`  (happy; EXTRACTED)
- [happy] Deserialization error message contains descriptive text describing the malformed key  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)

### `HirModule endpoint population from golden example`  (happy; EXTRACTED)
- [happy] lower_module generates exactly 3 endpoint_fns and 1 table from the golden crud_api.vox file  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HirModule serialization`  (happy; EXTRACTED)
- [happy] HirModule with inferred types serializes to JSON without panic (regression gate for serde_json::to_string)  (crates/vox-compiler/tests/hir_module_json_roundtrip.rs)

### `HirModule.tables and HirModule.endpoint_fns population`  (happy; EXTRACTED)
- [happy] lower_module correctly populates tables (1) and endpoint_fns (1) collections when processing web declarations  (crates/vox-compiler/src/hir/lower/mod.rs)

### `HirModule::examples`  (happy; EXTRACTED)
- [happy] @example blocks land in HirModule::examples, not HirModule::tests  (crates/vox-compiler/tests/example_decorator_test.rs)

### `HirModule::field_ownership_map`  (invariant; EXTRACTED)
- [invariant] Field ownership map contains expected HirFieldOwnership assignments for named fields (endpoint_fns, functions, mcp_tools, mcp_resources, etc.)  (crates/vox-compiler/src/hir/nodes/decl.rs)

### `HirModule::to_semantic_hir`  (happy; EXTRACTED)
- [happy] Semantic HIR projection drops migration-only vectors, leaving the functions vector empty  (crates/vox-compiler/src/hir/nodes/decl.rs)

### `HirPiiMarker`  (happy; EXTRACTED)
- [happy] JSON serialization and deserialization of HirPiiMarker preserves the PiiClass::Email value  (crates/vox-compiler/src/hir/nodes/boilerplate_grafts.rs)

### `HirSmFrom`  (happy; EXTRACTED)
- [happy] State machine transition from 'any' keyword parses to HirSmFrom::Any variant  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `ImportPath`  (happy; EXTRACTED)
- [happy] Parses Rust crate import alias 'as json' correctly via assert_eq!  (crates/vox-compiler/src/parser/descent/tests.rs)

### `InferenceContext::unify field-order independence`  (happy; EXTRACTED)
- [happy] Table and Record types unify successfully regardless of field declaration order  (crates/vox-compiler/src/typeck/unify.rs)

### `InferenceContext::unify for Ty::Collection and Ty::Record`  (happy; EXTRACTED)
- [happy] Collection types (live query views) unify with matching Record types  (crates/vox-compiler/src/typeck/unify.rs)

### `InferenceContext::unify for Ty::Table and Ty::Record`  (happy; EXTRACTED)
- [happy] Table and Record types with matching fields unify successfully in both directions (symmetric unification)  (crates/vox-compiler/src/typeck/unify.rs)

### `InferenceContext::unify structural compatibility`  (error; EXTRACTED)
- [error] Table and Record types fail to unify when fields are missing, extra, or have conflicting types  (crates/vox-compiler/src/typeck/unify.rs)

### `InteropNode`  (invariant; EXTRACTED)
- [invariant] InteropNode serde serialization produces deterministic JSON output for the same value  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `Interpreter`  (happy; EXTRACTED)
- [happy] Interpreter successfully executes test functions with Result type matching and returns Ok() result  (crates/vox-compiler/tests/result_match_eval_repro.rs)

### `Interpreter execution with functions and loops`  (happy; EXTRACTED)
- [happy] Interpreter executes functions with parameters, mutable variable assignment, while loops, and function calls correctly  (crates/vox-compiler/tests/interpreter_test.rs)

### `Interpreter recovery after failed call`  (happy; EXTRACTED)
- [happy] After a failed call (EvalError) that would corrupt scope, a subsequent call still evaluates correctly and scope depth is clean at 1  (crates/vox-compiler/tests/eval_scope_restore_test.rs)

### `Interpreter scope depth restoration on EvalError`  (invariant; EXTRACTED)
- [invariant] When a function body raises an EvalError via early return inside a pushed block frame, the Interpreter.scope.depth() is restored to baseline, not left at the leaked inner depth  (crates/vox-compiler/tests/eval_scope_restore_test.rs)

### `Interpreter.run_module`  (happy; EXTRACTED)
- [happy] Interpreter successfully executes for loops with index binding  (crates/vox-compiler/tests/interpreter_test.rs)

### `Item_from_json error handling`  (error; EXTRACTED)
- [error] Item_from_json returns an Err result when a required field is missing from the JSON input  (crates/vox-compiler/tests/json_as_test.rs)

### `JSX attribute value expression containing Speech.transcribe_microphone()`  (happy; EXTRACTED)
- [happy] capability walker descends into JSX attribute value expressions to derive capabilities from Speech.transcribe_microphone()  (crates/vox-compiler/src/required_capabilities.rs)

### `JSX element children containing Speech.transcribe_microphone()`  (happy; EXTRACTED)
- [happy] capability walker descends into both JSX attributes and children to derive capabilities from Speech.transcribe_microphone()  (crates/vox-compiler/src/required_capabilities.rs)

### `Json.as_str`  (edge; EXTRACTED)
- [edge] as_str() coercion method returns None when JSON value is not a string  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Json.at`  (edge; EXTRACTED)
- [edge] at() method returns None for negative indices and out-of-bounds array indices  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Json.get`  (edge; EXTRACTED)
- [edge] get() method returns None for keys that do not exist in JSON object  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Json.has`  (happy; EXTRACTED)
- [happy] has() method reports JSON key membership without requiring unwrapping  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Json::as_str()`  (happy; EXTRACTED)
- [happy] as_str() on JSON null value returns None, not Some  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `Json::is_null()`  (happy; EXTRACTED)
- [happy] is_null() returns true when Json value is JSON null  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `JsxAttribute`  (happy; EXTRACTED)
- [happy] JSX attributes have name field and are indexable by position  (crates/vox-compiler/src/parser/descent/tests.rs)

### `KPI chip string literals`  (happy; EXTRACTED)
- [happy] string literal chip keys (NODES, ACTIVE, BLOCKED, ERRORS) are emitted verbatim in tsx output  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `LayerTier`  (invariant; EXTRACTED)
- [invariant] LayerTier variants have a total ordering: Background < Content < Chrome < Popover < Modal < Toast < SystemOverlay  (crates/vox-compiler/src/hir/nodes/layer.rs)

### `LayerTier::from_str`  (happy; EXTRACTED)
- [happy] All LayerTier variants convert to string and back correctly via from_str roundtrip  (crates/vox-compiler/src/hir/nodes/layer.rs)

### `List.filter()`  (happy; EXTRACTED)
- [happy] List.filter(closure) keeps only elements where the closure returns true  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `List.fold()`  (happy; EXTRACTED)
- [happy] list.fold() with closure accumulator evaluates to correct integer sum  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `List.map()`  (happy; EXTRACTED)
- [happy] List.map(closure) applies the closure to each element and returns a new list of the same length  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Module parser (script mode) with mixed declarations and statements`  (happy; EXTRACTED)
- [happy] When parsing a script with both declarations (e.g., fn helper) and statements, the parser creates a synthetic main function containing the statements, resulting in 2 declarations total  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Module parser (script mode) with multiple statements`  (happy; EXTRACTED)
- [happy] Multiple top-level statements are all collected into a single synthetic main function  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Module parser (script mode) with pure declarations`  (happy; EXTRACTED)
- [happy] When parsing a script containing only declarations with no top-level statements, no synthetic main function is created  (crates/vox-compiler/src/parser/descent/tests.rs)

### `Multiple stdlib calls effect checking`  (invariant; EXTRACTED)
- [invariant] When multiple stdlib methods are called, only those requiring unannounced effects produce diagnostics  (crates/vox-compiler/src/typeck/effect_check.rs)

### `Note_from_json Option field deserialization`  (happy; EXTRACTED)
- [happy] When an Option[T] field is present in JSON input, from_json stores Some(value) in the synthesized record  (crates/vox-compiler/tests/json_as_test.rs)

### `Note_from_json Option field handling`  (edge; EXTRACTED)
- [edge] When an Option[T] field is absent from JSON input, from_json stores None in the synthesized record  (crates/vox-compiler/tests/json_as_test.rs)

### `ObjectLit`  (happy; EXTRACTED)
- [happy] Object literal keys can include phonetic keywords (and, or, not, in) as field names  (crates/vox-compiler/src/parser/descent/tests.rs)

### `OfflineStrategy`  (invariant; EXTRACTED)
- [invariant] OfflineStrategy variants (StaleWhileRevalidate, CacheFirst, NetworkFirst) are distinct and not equal to each other  (crates/vox-compiler/src/hir/nodes/boilerplate_grafts.rs)

### `Open-world semantics for unannotated functions`  (invariant; EXTRACTED)
- [invariant] Functions without effect annotations are treated as open-world and not checked against stdlib effect requirements  (crates/vox-compiler/src/typeck/effect_check.rs)

### `Option equality comparison (mk() is None)`  (happy; EXTRACTED)
- [happy] An Option value returned from a function equals the None literal under equality comparison  (crates/vox-compiler/tests/interpreter_test.rs)

### `Option.expect()`  (error; EXTRACTED)
- [error] Option.expect(msg) on None panics with the supplied message surfaced in the error  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Option.map()`  (happy; EXTRACTED)
- [happy] option.map() with closure transforms Some and passes through None  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Option.unwrap()`  (error; EXTRACTED)
- [error] Option.unwrap() on None raises an EvalError containing both 'unwrap' and 'None' in the message  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Option.unwrap_or()`  (happy; EXTRACTED)
- [happy] Option.unwrap_or(default) returns the value if Some, or the default if None  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Ordering`  (invariant; EXTRACTED)
- [invariant] Snapshot ordering for nested @versioned calls is deterministic and innermost-first  (crates/vox-compiler/tests/versioned_decorator.rs)

### `Product_from_json deserialization`  (happy; EXTRACTED)
- [happy] Product_from_json successfully extracts string field from JSON and returns a Record containing the correct value  (crates/vox-compiler/tests/json_as_test.rs)

### `Promise[T] type deprecation warning`  (happy; EXTRACTED)
- [happy] Promise[T] return type produces vox/types/promise-deprecated diagnostic code  (crates/vox-compiler/tests/future_promise_deprecation.rs)

### `ReactiveMemberDecl`  (happy; EXTRACTED)
- [happy] reactive module declaration contains two members when parsing two state declarations  (crates/vox-compiler/src/parser/descent/tests.rs)

### `RenameRegistry::entries`  (invariant; EXTRACTED)
- [invariant] Canonical registry contains zero entries in VUV-9 (empty as of release)  (crates/vox-compiler/tests/rename_alias_test.rs)

### `Result ADT error value propagation`  (happy; EXTRACTED)
- [happy] Result type carries a real typed error value (ADT variant) through match expressions, extracting payload values  (crates/vox-compiler/tests/interpreter_test.rs)

### `Result error type parameter binding`  (happy; EXTRACTED)
- [happy] The Error arm of Result[T, E] binds the error payload to the declared E type, not hardcoded str  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `Result.is_err()`  (happy; EXTRACTED)
- [happy] Result.is_err() method dispatches correctly on Result types returned from fs operations  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Result.is_ok()`  (happy; EXTRACTED)
- [happy] Result.is_ok() method dispatches correctly on Result types returned from fs operations  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Result.map_err()`  (happy; EXTRACTED)
- [happy] result.map_err() with closure transforms Err and passes through Ok  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Result.unwrap()`  (error; EXTRACTED)
- [error] Result.unwrap() on Err carries the error message in the panic containing 'Err' and 'Result.unwrap'  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `Result.unwrap_err()`  (error; EXTRACTED)
- [error] Result.unwrap_err() on Ok(value) panics with an error containing both 'unwrap_err' and 'Ok'  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `RetryPolicy`  (happy; EXTRACTED)
- [happy] All RetryPolicy enum variants (None, ExpBackoff, FixedInterval) serialize and deserialize correctly via JSON roundtrip  (crates/vox-compiler/src/hir/nodes/boilerplate_grafts.rs)

### `RouteNode`  (happy; EXTRACTED)
- [happy] RouteNode::RouteTree round-trips through JSON serialization preserving routes length and pattern values  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `RoutesDecl::parse_summary`  (happy; EXTRACTED)
- [happy] Routes declaration parse_summary returns correct entry count and path strings matching parsed routes  (crates/vox-compiler/src/parser/descent/tests.rs)

### `RuleBasedAutoFixer::suggest_fixes`  (happy; EXTRACTED)
- [happy] RuleBasedAutoFixer::suggest_fixes generates component migration fixes with correct diff format containing source and target syntax  (crates/vox-compiler/src/typeck/autofix.rs)

### `RustInteropSemanticsState`  (invariant; EXTRACTED)
- [invariant] Schema JSON semantics_state enum contains exactly the same values as RustInteropSemanticsState variant labels  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `RustInteropSemanticsState::PartiallyImplemented.as_label()`  (happy; EXTRACTED)
- [happy] RustInteropSemanticsState::PartiallyImplemented.as_label() returns 'partially_implemented'  (crates/vox-compiler/src/rust_interop_support.rs)

### `RustInteropSupportClass`  (invariant; EXTRACTED)
- [invariant] Schema JSON decision enum contains exactly the same values as RustInteropSupportClass variant labels  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `SSOT golden roots`  (invariant; EXTRACTED)
- [invariant] examples SSOT manifest lists at least one golden root  (crates/vox-compiler/tests/examples_ssot_test.rs)

### `SSOT negative roots`  (invariant; EXTRACTED)
- [invariant] examples SSOT manifest lists at least one negative root (parser-inventory or successor)  (crates/vox-compiler/tests/examples_ssot_test.rs)

### `SSOT schema version`  (invariant; EXTRACTED)
- [invariant] examples SSOT manifest has schema_version equal to 1  (crates/vox-compiler/tests/examples_ssot_test.rs)

### `SVG points prop forwarding`  (happy; EXTRACTED)
- [happy] points attribute in SVG polygon is forwarded as a prop in generated tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `SVG viewBox attribute`  (happy; EXTRACTED)
- [happy] viewBox attribute in SVG is emitted in generated tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `Scope`  (happy; EXTRACTED)
- [happy] Scope implements copy-on-write semantics: cloned scopes share data until get_mut is called, then original remains unchanged  (crates/vox-compiler/src/eval/env.rs)

### `Snapshot repo operation`  (error; EXTRACTED)
- [error] Calling snapshot with more than 1 argument produces an ArityMismatch error  (crates/vox-compiler/src/eval/repo.rs)

### `SourceSpanTable`  (happy; EXTRACTED)
- [happy] SourceSpanTable.push_span returns sequential IDs starting at 0, and SourceSpanTable.get retrieves the correct span for each ID  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `Speech.transcribe(filepath)`  (happy; EXTRACTED)
- [happy] Speech.transcribe() with file path derives 'speech' capability but not 'microphone' capability  (crates/vox-compiler/src/required_capabilities.rs)

### `Speech.transcribe_microphone()`  (happy; EXTRACTED)
- [happy] Speech.transcribe_microphone() derives both 'microphone' and 'speech' capabilities  (crates/vox-compiler/src/required_capabilities.rs)

### `Stats_from_json`  (happy; EXTRACTED)
- [happy] json_as decorator generates function that extracts all declared struct fields from JSON object  (crates/vox-compiler/tests/json_as_test.rs)

### `StubAutoFixer::suggest_fixes`  (happy; EXTRACTED)
- [happy] StubAutoFixer::suggest_fixes generates exactly one fix per diagnostic with correct diff format  (crates/vox-compiler/src/typeck/autofix.rs)

### `StyleNode`  (happy; EXTRACTED)
- [happy] StyleNode::Declaration round-trips through JSON serialization preserving property name  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `TextRole`  (invariant; EXTRACTED)
- [invariant] TextRole::Body has warn_threshold=4.5 and error_threshold=3.0; TextRole::Large and TextRole::Ui have warn_threshold=3.0  (crates/vox-compiler/src/tokens/contrast.rs)

### `Token::AtComponent`  (happy; EXTRACTED)
- [happy] @component decorator lexes to Token::AtComponent  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtDeprecated`  (happy; EXTRACTED)
- [happy] @deprecated decorator lexes to Token::AtDeprecated  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtMcpResource`  (happy; EXTRACTED)
- [happy] @mcp.resource decorator lexes to Token::AtMcpResource  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtMcpTool`  (happy; EXTRACTED)
- [happy] @mcp.tool decorator lexes to Token::AtMcpTool  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtMutation`  (happy; EXTRACTED)
- [happy] @mutation decorator lexes to Token::AtMutation as a first-class distinct token  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtNative`  (happy; EXTRACTED)
- [happy] @native decorator lexes to Token::AtNative  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtPure`  (happy; EXTRACTED)
- [happy] @pure decorator lexes to Token::AtPure  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtQuery`  (happy; EXTRACTED)
- [happy] @query decorator lexes to Token::AtQuery as a first-class distinct token  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtResource`  (happy; EXTRACTED)
- [happy] @resource decorator lexes to Token::AtResource  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtScheduled`  (happy; EXTRACTED)
- [happy] @scheduled decorator lexes to Token::AtScheduled  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtTool`  (happy; EXTRACTED)
- [happy] @tool decorator lexes to Token::AtTool  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtTracked`  (happy; EXTRACTED)
- [happy] @tracked decorator lexes to Token::AtTracked  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::AtVersioned`  (happy; EXTRACTED)
- [happy] @versioned decorator lexes to Token::AtVersioned  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::BangInvalid`  (happy; EXTRACTED)
- [happy] Bang (!) character lexes as Token::BangInvalid, a distinct token separate from 'not' keyword  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::FloatLit`  (happy; EXTRACTED)
- [happy] Float literals lex to Token::FloatLit with correct numeric value  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::Ident`  (happy; EXTRACTED)
- [happy] Lowercase identifiers lex to Token::Ident variant  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::IntLit`  (happy; EXTRACTED)
- [happy] Integer literals lex to Token::IntLit with correct numeric value  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::NotEq`  (invariant; EXTRACTED)
- [invariant] Operator != lexes as single Token::NotEq, not as separate ! and = tokens  (crates/vox-compiler/src/lexer/cursor.rs)

### `Token::TypeIdent`  (happy; EXTRACTED)
- [happy] PascalCase identifiers lex to Token::TypeIdent variant, not Token::Ident  (crates/vox-compiler/src/lexer/cursor.rs)

### `TokenRegistry::load_from_str()`  (happy; EXTRACTED)
- [happy] TokenRegistry::load_from_str() parses flat token definitions and lookup() retrieves values by key correctly  (crates/vox-compiler/src/tokens/mod.rs)

### `TypeEnv scope push/pop behavior`  (happy; EXTRACTED)
- [happy] Variables defined in outer scope are visible in inner scope after push_scope(), and variable shadowing in inner scope is resolved after pop_scope() returns to outer type  (crates/vox-compiler/src/typeck/env.rs)

### `VoxValue list assignment semantics`  (invariant; EXTRACTED)
- [invariant] List assignment creates a copy, not an alias; mutating the original list via push does not affect the assigned binding  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `VoxValue list copy semantics`  (invariant; EXTRACTED)
- [invariant] Index assignment to one list binding does not affect a prior copy of that list, preserving value semantics  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `VoxValue list push mutation`  (happy; EXTRACTED)
- [happy] The push method on a list mutates the owner in-place, observable as growth of the list length  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `VoxValue object copy semantics`  (invariant; EXTRACTED)
- [invariant] An object passed by value is independent of its source binding and reads correct field values  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `VoxValue pass-by-value semantics`  (invariant; EXTRACTED)
- [invariant] A list passed by value to a function and mutated therein does not affect the caller's original list  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `VoxValue::Fn`  (happy; EXTRACTED)
- [happy] VoxValue::Fn registered in scope carries is_versioned flag for @versioned functions  (crates/vox-compiler/tests/versioned_decorator.rs)

### `VoxValue::list(), VoxValue::object(), VoxValue::tuple()`  (happy; EXTRACTED)
- [happy] Constructor functions create values matching their expected variant patterns  (crates/vox-compiler/src/eval/value.rs)

### `WebIrModule serde JSON encoding`  (invariant; EXTRACTED)
- [invariant] WebIrModule JSON encoding is deterministic - identical values always produce identical byte sequences  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `WebIrModule serde round-trip`  (invariant; EXTRACTED)
- [invariant] WebIrModule survives serde JSON round-trip (serialize to bytes, deserialize, re-serialize) without byte changes  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `Webhook decorator validation for Stripe provider`  (happy; EXTRACTED)
- [happy] Stripe webhook provider does not emit missing-secret-var or replay-window-out-of-range diagnostics with default configuration  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `WireType projection for Option`  (happy; EXTRACTED)
- [happy] Option<T> fields are marked as optional and the inner type T is projected to its corresponding WireType (str->String)  (crates/vox-compiler/src/contract_ir/tests.rs)

### `WireType projection for primitives`  (happy; EXTRACTED)
- [happy] HirType::Named('int') projects to WireType::Number and HirType::Named('str') projects to WireType::String  (crates/vox-compiler/src/contract_ir/tests.rs)

### `activity function generated_hash`  (happy; EXTRACTED)
- [happy] activity functions have non-None generated_hash field after HIR lowering  (crates/vox-compiler/tests/workflow_hash_stable.rs)

### `actor keyword`  (happy; EXTRACTED)
- [happy] actor keyword compiles without producing TypeckSeverity::Error diagnostics  (crates/vox-compiler/src/pipeline.rs)

### `actor registration and lookup`  (happy; EXTRACTED)
- [happy] Registered actor is findable via lookup() with BindingKind::Actor and via lookup_actor()  (crates/vox-compiler/src/typeck/env.rs)

### `agentos_mutation_kind_for_tool()`  (happy; EXTRACTED)
- [happy] std_namespace_runtime_call for agentos.mutation_kind_for_tool contains 'vox_foundation::primitives::agentos_mutation_kind_for_tool' in output  (crates/vox-compiler/src/builtin_registry.rs)

### `arithmetic subscript index`  (happy; EXTRACTED)
- [happy] subscript expression with arithmetic expression (items[i + 1]) is emitted in tsx  (crates/vox-compiler/tests/golden_props_test.rs)

### `assert_golden_file`  (happy; EXTRACTED)
- [happy] All golden .vox example files can be parsed and lowered without assertion failures  (crates/vox-compiler/tests/golden_vox_examples_test.rs)

### `backward compatibility for string error types`  (happy; EXTRACTED)
- [happy] Result[T, str] with string literal Error construction maintains backward compatibility and typechecks  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `bang operator (!) error message`  (error; EXTRACTED)
- [error] bang operator (!) parse error message indicates it is not a valid operator and suggests 'not' as canonical form  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `canonical_required_capabilities_bytes()`  (invariant; EXTRACTED)
- [invariant] Required capabilities bytes are deterministic across independent bundle creation calls  (crates/vox-compiler/tests/projection_parity_test.rs)

### `canonical_runtime_projection_bytes`  (invariant; EXTRACTED)
- [invariant] Canonical runtime projection bytes roundtrip through sort_json_value_keys and serialize identity  (crates/vox-compiler/tests/runtime_projection_smoke_test.rs)

### `canonical_web_ir_bytes`  (invariant; EXTRACTED)
- [invariant] SHA3 hash of canonical web IR bytes is stable across multiple calls on same input  (crates/vox-compiler/tests/runtime_projection_smoke_test.rs)

### `changes repo operation`  (error; EXTRACTED)
- [error] Calling changes with extra arguments produces an ArityMismatch error  (crates/vox-compiler/src/eval/repo.rs)

### `check_pure_violation`  (error; EXTRACTED)
- [error] check_pure_violation detects when a pure function uses Net effect, returning diagnostic with code 'vox/effect/pure-violation'  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_pure_violation()`  (happy; EXTRACTED)
- [happy] Pure caller calling pure callee returns no diagnostic (is_none)  (crates/vox-compiler/src/typeck/boilerplate_grafts.rs)

### `check_state_machines function and E_SM_UNKNOWN_STATE diagnostic code`  (error; EXTRACTED)
- [error] Transitions referencing undeclared states produce E_SM_UNKNOWN_STATE diagnostic with the state name in the message  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `check_training_cuda_tier()`  (happy; EXTRACTED)
- [happy] Returns empty diagnostics list when VOX_CUDA_TIER environment variable is unset (default tier allows training)  (crates/vox-compiler/src/typeck/cuda_gate.rs)

### `chip filter labels`  (happy; EXTRACTED)
- [happy] string literal filter chip labels ('all', 'ok', 'error') are emitted in generated tsx output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)

### `circle SVG element passthrough`  (happy; EXTRACTED)
- [happy] <circle> SVG element is emitted as JSX element, not as a function call  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `classify_rust_crate`  (happy; EXTRACTED)
- [happy] classify_rust_crate() returns decision label matching ecosystem-support.yaml registry for each named crate  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `closure capture`  (happy; EXTRACTED)
- [happy] Closures capture enclosing scope variables by clone and use them in the closure body  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `closure environment capture`  (happy; EXTRACTED)
- [happy] A closure captures its enclosing lexical environment by value, allowing it to read captured variables (n=10) correctly across multiple invocations (add_n(5)=15, add_n(100)=110, sum=125)  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `closure literal syntax`  (happy; EXTRACTED)
- [happy] Closure literal syntax fn(x: Type) to ReturnType { body } parses and evaluates to the correct result  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `collect_golden_vox_files`  (happy; EXTRACTED)
- [happy] Golden example collection recursively traverses examples/golden/ subdirectories and locates .vox files  (crates/vox-compiler/tests/golden_examples_strict_parse.rs)

### `collect_golden_vox_files()`  (happy; EXTRACTED)
- [happy] collect_golden_vox_files recursively traverses examples/golden/** subdirectories and includes nested vox files  (crates/vox-compiler/tests/golden_examples_strict_parse.rs)

### `color()`  (error; INFERRED)
- [error] WebIR validate rejects literal color string values in CSS style declarations  (crates/vox-compiler/tests/web_ir_environment_gates_test.rs)

### `compiled regex object and group extraction`  (happy; EXTRACTED)
- [happy] Compiled regex objects support .find() and .group(i) methods to extract match groups under interpreter mode  (crates/vox-compiler/tests/interpreter_test.rs)

### `component call count in tsx`  (happy; EXTRACTED)
- [happy] MeshKpi component call count in generated tsx matches the number of component invocations in source (6)  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `component instance count for OrchNode`  (happy; EXTRACTED)
- [happy] multiple OrchNode component instances in source are emitted as JSX elements with correct count (2)  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `component instance count in svg`  (happy; EXTRACTED)
- [happy] multiple AgentNode component instances in source are emitted as JSX elements with correct count (5)  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `conditional stroke color emission`  (happy; EXTRACTED)
- [happy] conditional stroke colors in component are emitted as string literals (#3b82f6, #52525b) in tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `conditional stroke color in polygon`  (happy; EXTRACTED)
- [happy] conditional stroke color (#60a5fa) in polygon is emitted as string literal in tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `cross-numeric equality (1 is 1.0)`  (happy; EXTRACTED)
- [happy] Equality comparison between int and float operands evaluates based on numeric value equivalence  (crates/vox-compiler/tests/interpreter_test.rs)

### `dashed line styling`  (happy; EXTRACTED)
- [happy] SVG line with stroke-dasharray attribute emits strokeDasharray in generated tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `db query method chaining`  (happy; EXTRACTED)
- [happy] Chained db query methods (.where, .order_by, .limit) typecheck without errors  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `db.Table.all`  (happy; EXTRACTED)
- [happy] all() returns all inserted rows and len() counts them correctly  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.all.order_by.limit`  (happy; EXTRACTED)
- [happy] limit() on an ordered query restricts result count to specified number  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.count`  (happy; EXTRACTED)
- [happy] count() returns the correct count of rows in the table  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.delete`  (happy; EXTRACTED)
- [happy] delete() removes the specified row from the table by _id  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.filter`  (happy; EXTRACTED)
- [happy] filter() with boolean equality predicate returns matching rows  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.get`  (happy; EXTRACTED)
- [happy] get() with valid id returns Some containing the row  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.where`  (happy; EXTRACTED)
- [happy] where clause with gte predicate correctly filters rows by numeric comparison  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.where.order_by.limit`  (happy; EXTRACTED)
- [happy] Fused where+order_by+limit chain composes correctly to filter, sort, and limit rows  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.Table.where.select`  (happy; EXTRACTED)
- [happy] Fused where().select() chain applies filter before projection  (crates/vox-compiler/tests/interpreter_db_test.rs)

### `db.User.all().select()`  (happy; EXTRACTED)
- [happy] Selecting a subset of columns with select() from a table query returns no TypeckSeverity::Error  (crates/vox-compiler/tests/db_query_safety_test.rs)

### `decimal arithmetic (0.1dec + 0.2dec is 0.3dec)`  (happy; EXTRACTED)
- [happy] Decimal arithmetic in interpreter produces exact results without floating-point precision loss  (crates/vox-compiler/tests/interpreter_test.rs)

### `defaults: true in @json_as`  (happy; EXTRACTED)
- [happy] When defaults: true is specified, missing fields are populated with zero values (0 for int, empty string for str) instead of returning an error  (crates/vox-compiler/tests/json_as_test.rs)

### `defined_in_current_scope() scope boundary detection`  (happy; EXTRACTED)
- [happy] defined_in_current_scope() returns true only for variables directly defined in current scope, not for inherited bindings from outer scope after push_scope()  (crates/vox-compiler/src/typeck/env.rs)

### `destructured import as { } syntax, HirImport collection`  (happy; EXTRACTED)
- [happy] Destructured import with as { Name1, Name2, ... } expands to one HirImport per name with same module_path  (crates/vox-compiler/tests/parser_import_syntax_test.rs)

### `diags()`  (invariant; EXTRACTED)
- [invariant] Empty diagnostics list indicates valid WebIR for Counter component with view  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `doc pipeline .vox includes`  (invariant; EXTRACTED)
- [invariant] Doc pipeline .vox includes must target paths under the golden directory  (crates/vox-compiler/tests/examples_ssot_test.rs)

### `effect violation detection for @pure functions`  (error; EXTRACTED)
- [error] A @pure function that calls an unannotated helper which uses http.* emits an effect violation diagnostic via bottom-up effect inference  (crates/vox-compiler/tests/effect_inference.rs)

### `effect violation detection for declared effect sets`  (error; EXTRACTED)
- [error] A function declared with uses db that calls an unannotated helper using http.* emits an effect violation because net is not in its declared effect set  (crates/vox-compiler/tests/effect_inference.rs)

### `effect violation detection for unannotated functions`  (edge; EXTRACTED)
- [edge] An unannotated function calling another unannotated function does not emit an effect violation diagnostic (open-world assumption)  (crates/vox-compiler/tests/effect_inference.rs)

### `effect with explicit depends_on clause`  (happy; EXTRACTED)
- [happy] An explicit depends_on clause suppresses the lint.effect.unresolvable_deps diagnostic regardless of whether the called function is a builtin  (crates/vox-compiler/tests/effect_deps_test.rs)

### `effect with only state reads and assignments`  (happy; EXTRACTED)
- [happy] An effect that only reads and assigns state variables does not emit lint.effect.unresolvable_deps diagnostic  (crates/vox-compiler/tests/effect_deps_test.rs)

### `effect with unresolvable dependency tracking`  (error; EXTRACTED)
- [error] An effect containing calls to non-builtin/non-state-setter functions emits lint.effect.unresolvable_deps diagnostic  (crates/vox-compiler/tests/effect_deps_test.rs)

### `effect_check`  (happy; INFERRED)
- [happy] Effect checker permits bare repo.* calls in @versioned functions due to implicit vcs capability  (crates/vox-compiler/tests/versioned_decorator.rs)

### `embed`  (happy; EXTRACTED)
- [happy] embed decorator attributes (model, dimensions, source_field) are captured and available on function declarations  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `emit_component_view_tsx_with_stats()`  (happy; EXTRACTED)
- [happy] emit_component_view_tsx_with_stats() returns stats object tracking nodes_visited count during emission  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `empty function without speech calls`  (invariant; EXTRACTED)
- [invariant] function without Speech API calls does not derive 'microphone' or 'speech' capabilities  (crates/vox-compiler/src/required_capabilities.rs)

### `endpoint clean effect validation`  (happy; EXTRACTED)
- [happy] An endpoint with a single declared effect produces no diagnostics  (crates/vox-compiler/src/typeck/effect_check.rs)

### `endpoint duplicate effect detection`  (error; EXTRACTED)
- [error] An endpoint declaring the same effect twice (Net, Net) produces exactly one diagnostic with code E_EFFECT_DUPLICATE  (crates/vox-compiler/src/typeck/effect_check.rs)

### `endpoint pure effect conflict detection`  (error; EXTRACTED)
- [error] An endpoint marked pure with a Db effect produces exactly one diagnostic with code E_EFFECT_PURE_CONFLICT and severity Error  (crates/vox-compiler/src/typeck/effect_check.rs)

### `exhaustive match patterns`  (happy; EXTRACTED)
- [happy] Exhaustive Option/Result matches including binding-as-wildcard patterns are accepted without E0301 errors  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `for loop body rendering`  (happy; EXTRACTED)
- [happy] for-loop body containing markup emits as JSX span elements in tsx output  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for loop compilation`  (happy; EXTRACTED)
- [happy] for-loop without explicit index compiles to .map((r, _i) => ...) with synthetic underscore index  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for loop field access`  (happy; EXTRACTED)
- [happy] for-loop body with field access (r.name) emits as JSX expression {r.name}  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for loop field references`  (happy; EXTRACTED)
- [happy] for-loop body can reference multiple fields from loop variable (r.id, r.duration)  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for loop with explicit index`  (happy; EXTRACTED)
- [happy] for-loop with user-named index variable preserves the index name in .map((item, i) => ...) call  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for loop with index binding`  (happy; EXTRACTED)
- [happy] for v, i in list { ... } correctly binds both value and index in loop body  (crates/vox-compiler/tests/interpreter_test.rs)

### `for loop without index`  (happy; EXTRACTED)
- [happy] for-loop omitting index variable emits synthetic _i parameter in .map() call  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `for-loop iteration over maps and strings`  (happy; EXTRACTED)
- [happy] For loops iterate over maps (yielding tuples) and strings (yielding characters) correctly  (crates/vox-compiler/tests/interpreter_test.rs)

### `form endpoint resolution`  (error; EXTRACTED)
- [error] Forms with unknown on_submit endpoints produce lint.form.unknown_endpoint diagnostics  (crates/vox-compiler/tests/form_hir_test.rs)

### `form field type checking`  (error; EXTRACTED)
- [error] Forms with field type mismatches produce lint.form.field_type_mismatch diagnostics  (crates/vox-compiler/tests/form_hir_test.rs)

### `format()`  (edge; EXTRACTED)
- [edge] Formatting invalid source code returns the input unchanged  (crates/vox-compiler/src/fmt/mod.rs)

### `from any transition pattern`  (happy; EXTRACTED)
- [happy] Transitions using 'from any' satisfy exhaustiveness requirements for all non-terminal states  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `fs namespace method dispatch`  (happy; EXTRACTED)
- [happy] fs.cwd() returns Result[str] with is_ok() method, fs.exists(path) returns bool; both dispatch without Method-not-found errors  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `generate_voxdb_schema`  (invariant; EXTRACTED)
- [invariant] generate_voxdb_schema from AST produces identical output to generate_voxdb_schema_from_hir  (crates/vox-compiler/tests/voxdb_schema_hir_parity_test.rs)

### `generated output.files`  (happy; EXTRACTED)
- [happy] Codegen produces Counter.tsx file in output files map  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `hir.legacy_ast_nodes`  (invariant; EXTRACTED)
- [invariant] HIR constructed from parity chain fixture has no legacy AST nodes (is_empty returns true)  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `import resolution`  (error; EXTRACTED)
- [error] Runtime raises UndefinedVariable error when calling private function from imported module  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `integer addition overflow handling`  (error; EXTRACTED)
- [error] integer addition overflow produces clean EvalError containing 'overflow' rather than panic  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `integer division by zero handling`  (error; EXTRACTED)
- [error] integer division by zero produces clean EvalError containing 'division by zero'  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `interp_csv_parse() and interp_csv_render()`  (happy; EXTRACTED)
- [happy] CSV data roundtrips through render and parse functions preserving array structure  (crates/vox-compiler/src/eval/shell_stdlib.rs)

### `interp_csv_parse_records(), interp_yaml_parse(), interp_yaml_render(), interp_toml_parse(), interp_toml_render()`  (happy; EXTRACTED)
- [happy] Format conversion roundtrips through CSV->YAML->TOML->parse chain complete without error  (crates/vox-compiler/src/eval/shell_stdlib.rs)

### `is_template_managed_app_dependency()`  (happy; EXTRACTED)
- [happy] is_template_managed_app_dependency() returns true for 'reqwest' and 'vox-db'  (crates/vox-compiler/src/rust_interop_support.rs)

### `is_template_managed_script_native_dependency()`  (happy; EXTRACTED)
- [happy] is_template_managed_script_native_dependency() returns true for 'tokio'  (crates/vox-compiler/src/rust_interop_support.rs)

### `is_template_managed_script_wasi_dependency()`  (happy; EXTRACTED)
- [happy] is_template_managed_script_wasi_dependency() returns true for 'serde' and false for 'reqwest'  (crates/vox-compiler/src/rust_interop_support.rs)

### `is_wasi_unsupported_rust_import()`  (happy; EXTRACTED)
- [happy] is_wasi_unsupported_rust_import() returns true for 'reqwest' and 'turso', false for 'serde_json' and 'uuid'  (crates/vox-compiler/src/rust_interop_support.rs)

### `json.parse`  (happy; EXTRACTED)
- [happy] json.parse returns Ok(Json) for valid JSON and get() retrieves nested values  (crates/vox-compiler/tests/json_ergonomics_test.rs)

### `json_as annotation synthesis`  (happy; EXTRACTED)
- [happy] When a type is annotated with @json_as, both Widget_from_json and Widget_to_json functions are synthesized and appear in hir.functions  (crates/vox-compiler/tests/json_as_test.rs)

### `json_as field synthesis`  (happy; EXTRACTED)
- [happy] synthesise_json_as_fns generates 4 statements in the from function body for a required field (let _f, guard, unwrap, return)  (crates/vox-compiler/src/hir/lower/json_as.rs)

### `json_as synthesis gating`  (invariant; EXTRACTED)
- [invariant] Types without @json_as annotation do not synthesize from_json or to_json functions  (crates/vox-compiler/tests/json_as_test.rs)

### `key prop in for loop`  (happy; EXTRACTED)
- [happy] for-loop body with key={{expr}} prop emits as key={expr} in JSX  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `language_surface::LEXER_KEYWORDS`  (error; EXTRACTED)
- [error] ret keyword is not in LEXER_KEYWORDS  (crates/vox-compiler/tests/language_surface_ssot_test.rs)

### `lex and parse`  (invariant; EXTRACTED)
- [invariant] All golden example .vox files in examples/golden/ directory tree can be lexed and parsed without errors  (crates/vox-compiler/tests/golden_examples_strict_parse.rs)

### `lexer token recognition`  (invariant; EXTRACTED)
- [invariant] lexes all keywords, decorators, and punctuators from grammar artifact as non-identifier tokens  (crates/vox-compiler/tests/speech_grammar_artifact_test.rs)

### `list.map() immutability`  (invariant; EXTRACTED)
- [invariant] list.map(fn) with closure returns a new list without mutating the source; source retains original length 4 after map operation  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `literal subscript index`  (happy; EXTRACTED)
- [happy] subscript expression with literal index (items[0]) is emitted verbatim in tsx  (crates/vox-compiler/tests/golden_props_test.rs)

### `match exhaustiveness checking for Option`  (error; EXTRACTED)
- [error] Matches on Option missing the None arm are rejected with E0301 error  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `match exhaustiveness checking for Result`  (error; EXTRACTED)
- [error] Matches on Result missing the Error arm are rejected with E0301 error  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `mixed integer and float arithmetic type promotion`  (happy; EXTRACTED)
- [happy] Arithmetic operations promote mixed int and float operands to float and execute without type errors  (crates/vox-compiler/tests/interpreter_test.rs)

### `multi-decorator function parsing`  (happy; EXTRACTED)
- [happy] Functions with @auth, @cors, @rate_limit, @webhook, @layer decorators parse successfully  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `namespace_builtin_owned`  (happy; EXTRACTED)
- [happy] namespace_builtin_owned returns true for owned namespaces (fs) and false for non-owned namespaces (mobile)  (crates/vox-compiler/src/builtin_registry.rs)

### `naming: camelCase in @json_as`  (happy; EXTRACTED)
- [happy] When naming: camelCase is specified, Event_from_json reads JSON keys in camelCase and maps them to snake_case struct fields  (crates/vox-compiler/tests/json_as_test.rs)

### `nested component rendering`  (happy; EXTRACTED)
- [happy] StateChip component is properly emitted in generated tsx output  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `nested error ADT match exhaustiveness`  (error; EXTRACTED)
- [error] Non-exhaustive matches on an ADT inside Result Error(e) arm are rejected with missing variant errors  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `nested for loops`  (happy; EXTRACTED)
- [happy] nested for-loops compile to nested .map() calls with independent variables per level  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `nested loop variable access`  (happy; EXTRACTED)
- [happy] inner for-loop can reference inner loop variable in body expressions (row.map((cell, j) => {cell}))  (crates/vox-compiler/tests/golden_for_loop_test.rs)

### `not operator boolean inversion`  (happy; EXTRACTED)
- [happy] The `not` operator correctly inverts boolean values and composes multiple inversions  (crates/vox-compiler/tests/interpreter_test.rs)

### `nullary variant pattern matching dispatch`  (happy; EXTRACTED)
- [happy] Nullary variant patterns in match expressions dispatch to their correct arm without catching all cases  (crates/vox-compiler/tests/interpreter_test.rs)

### `onClick handler emission`  (happy; EXTRACTED)
- [happy] onClick event handler attribute is emitted in generated jsx for components with interaction  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `overlay() component with toast child elements`  (happy; EXTRACTED)
- [happy] overlay() primitive lowers to DomNode with data-vox-overlay="true" attribute and nested toast children with data-vox-z and data-vox-pos attributes  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `parse error message`  (error; EXTRACTED)
- [error] Parser error message suggests 'not' as the canonical form for the ! operator  (crates/vox-compiler/tests/interpreter_test.rs)

### `parse() and lower_module()`  (happy; EXTRACTED)
- [happy] all .vox files in tests/llm_fixtures parse successfully and lower without errors  (crates/vox-compiler/tests/llm_fixtures_test.rs)

### `parser::descent::parse`  (error; EXTRACTED)
- [error] Parser rejects ! operator and reports error mentioning invalid operator form  (crates/vox-compiler/tests/interpreter_test.rs)

### `partial state_machine attribute`  (happy; EXTRACTED)
- [happy] Partial state machines skip exhaustiveness checks and produce no diagnostics  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `path.extension()`  (happy; EXTRACTED)
- [happy] path.extension() returns the file extension string correctly from a file path  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `path.stem()`  (happy; EXTRACTED)
- [happy] path.stem() returns Option[str] that can be matched and unwrapped to get the filename stem  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `polygon SVG element`  (happy; EXTRACTED)
- [happy] <polygon> SVG element is emitted as JSX element, not as a JavaScript function call  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `polymorphic Result Error construction`  (happy; EXTRACTED)
- [happy] Result[T, MyErr] with typed ADT error arm construction typechecks with exhaustive nested matches  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `polymorphic Result Ok construction`  (happy; EXTRACTED)
- [happy] Ok(v) constructed in functions declared to Result[T, MyErr] unifies with the declared ADT error type  (crates/vox-compiler/tests/match_exhaustiveness_test.rs)

### `preserveAspectRatio camelCase conversion`  (happy; EXTRACTED)
- [happy] preserve_aspect_ratio snake_case attribute is converted to preserveAspectRatio camelCase in tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `process.run() return type parity`  (happy; EXTRACTED)
- [happy] process.run(cmd, args) returns Option[Record] (not bare Record) wrapping execution results; unwrap() succeeds when process exits with code 0  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `process.which()`  (happy; EXTRACTED)
- [happy] process.which() returns Option[str] that is Some for executables like cargo and None for non-existent commands  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `project::type_def with struct`  (happy; EXTRACTED)
- [happy] Projecting a struct type preserves field names ('id', 'name') and maps Vox HirTypes to WireTypes (int->Number, str->String)  (crates/vox-compiler/src/contract_ir/tests.rs)

### `project::type_def with sum type`  (happy; EXTRACTED)
- [happy] Projecting a sum type emits ContractTypeKind::Sum with variants tagged by variant name and preserving field names  (crates/vox-compiler/src/contract_ir/tests.rs)

### `project_runtime_from_core`  (happy; EXTRACTED)
- [happy] Module task capability hints infer prefer_gpu_compute=true and vector/training labels from db.using() and scope()  (crates/vox-compiler/tests/runtime_projection_smoke_test.rs)

### `project_type for Decimal`  (happy; EXTRACTED)
- [happy] HirType::Decimal and HirType::Named('Decimal') both project to WireType::DecimalString  (crates/vox-compiler/src/contract_ir/tests.rs)

### `project_type for datetime types`  (happy; EXTRACTED)
- [happy] DateTime type aliases ('Date', 'DateTime', 'Instant', 'Timestamp') all project to WireType::DateTimeString  (crates/vox-compiler/src/contract_ir/tests.rs)

### `project_type for large integer types`  (happy; EXTRACTED)
- [happy] HirType::Named('bigint') and HirType::Named('i128') both project to WireType::BigIntString  (crates/vox-compiler/src/contract_ir/tests.rs)

### `reactive_smoke_test module`  (happy; EXTRACTED)
- [happy] Test runs with CARGO_MANIFEST_DIR environment variable set  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `record.get() None case`  (happy; EXTRACTED)
- [happy] record.get(key) returns Option.None when key is missing; is_none() evaluates to true  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `record.get() return type parity`  (happy; EXTRACTED)
- [happy] record.get(key) returns Option[T] (not bare T) compatible with .unwrap(); unwrapping a present key yields the correct value  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `rect SVG element`  (happy; EXTRACTED)
- [happy] <rect> SVG elements are emitted in generated tsx output  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `recursive function frame independence`  (happy; EXTRACTED)
- [happy] Recursive function calls maintain independent stack frames in the new cactus/Rc-frame scope representation, computing Fibonacci(10)=55 correctly  (crates/vox-compiler/tests/eval_cow_semantics_test.rs)

### `regex namespace method dispatch`  (happy; EXTRACTED)
- [happy] regex.is_match(), regex.replace() dispatch correctly; is_match(text, pattern) returns true for matches, replace(text, old, new) substitutes correctly  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `regex.captures() method dispatch`  (happy; EXTRACTED)
- [happy] regex.captures(text, pattern) returns Option[list[str]] with capture groups; is_some() is true for matching patterns, is_none() for non-matches  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `regex.find() method dispatch`  (happy; EXTRACTED)
- [happy] regex.find(text, pattern) returns Option[str]; is_some() is true for matching patterns, is_none() is true when pattern does not match  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `repo.changes()`  (invariant; EXTRACTED)
- [invariant] Snapshot label includes @versioned decorator name for auto-generated snapshots  (crates/vox-compiler/tests/versioned_decorator.rs)

### `resolve_key()`  (happy; EXTRACTED)
- [happy] resolve_key() uses explicit field_name override when present, ignoring naming convention  (crates/vox-compiler/src/hir/lower/json_as.rs)

### `routes`  (happy; INFERRED)
- [happy] Routes block is processed in code generation output  (crates/vox-compiler/tests/web_ir_environment_gates_test.rs)

### `row/column layout functions`  (happy; EXTRACTED)
- [happy] row() and column() functions do not emit as JavaScript function calls in generated tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `search placeholder text`  (happy; EXTRACTED)
- [happy] input placeholder text 'Search runs' is emitted in generated tsx output  (crates/vox-compiler/tests/golden_runs_surface_test.rs)

### `semantics_state_for_rust_crate`  (happy; EXTRACTED)
- [happy] semantics_state_for_rust_crate() returns label matching ecosystem-support.yaml semantics_state field for each crate  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `semantics_state_for_rust_crate()`  (invariant; EXTRACTED)
- [invariant] semantics_state_for_rust_crate() maps serde_json to Implemented, time to Planned, sqlx to DocsOnly, and unknown crates to PartiallyImplemented  (crates/vox-compiler/src/rust_interop_support.rs)

### `shell back_button field`  (happy; EXTRACTED)
- [happy] can be populated from parsed back_button decorator  (crates/vox-compiler/tests/shell_projection_smoke_test.rs)

### `side_effect block in workflow`  (happy; EXTRACTED)
- [happy] suppresses vox/workflow/non-deterministic-call diagnostic  (crates/vox-compiler/tests/side_effect_block.rs)

### `side_effect block syntax`  (happy; EXTRACTED)
- [happy] parses without E_PARSE errors inside workflow body  (crates/vox-compiler/tests/side_effect_block.rs)

### `side_effect block synthesis`  (happy; EXTRACTED)
- [happy] creates distinct synthesized activities for each side_effect block  (crates/vox-compiler/tests/side_effect_block.rs)

### `side_effect block usage`  (error; EXTRACTED)
- [error] emits vox/workflow/side-effect-outside-workflow diagnostic when used outside workflow  (crates/vox-compiler/tests/side_effect_block.rs)

### `span()`  (invariant; EXTRACTED)
- [invariant] Span field serializes and deserializes through JSON round-trip in WebIrModule  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `state machine codegen for terminal states`  (happy; EXTRACTED)
- [happy] Code emission excludes terminal state cases from the reducer switch statement  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `state machine exhaustiveness check for missing transitions`  (error; EXTRACTED)
- [error] State machines require all states to have transitions for all events; missing transitions produce error diagnostics  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `state machine exhaustiveness checking`  (error; EXTRACTED)
- [error] Typecheck produces Error severity diagnostic for non-exhaustive state machine transitions, containing both source and target state names  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `state machine field parsing`  (happy; EXTRACTED)
- [happy] Parsed state with field declarations populates the fields array with correct name and count  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `state machine parsing`  (happy; EXTRACTED)
- [happy] parses simple state machines with correct state count and transition count  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `state name uniqueness`  (error; EXTRACTED)
- [error] Duplicate state names in a state machine produce error diagnostics containing 'duplicate'  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `state variable emission`  (happy; EXTRACTED)
- [happy] state variable declarations are emitted in generated tsx component  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `std.http.get_text() performs real HTTP requests`  (happy; EXTRACTED)
- [happy] std.http.get_text() performs real HTTP requests in interpreter mode, not a stub, returning actual transport errors  (crates/vox-compiler/tests/interpreter_test.rs)

### `std.time.now_ms() builtin function`  (happy; EXTRACTED)
- [happy] std.time.now_ms() returns a positive integer epoch timestamp under interpreter mode  (crates/vox-compiler/tests/interpreter_test.rs)

### `std_namespace_method_ty()`  (happy; EXTRACTED)
- [happy] std_namespace_method_ty('agentos', 'mutation_kind_for_tool') returns Some  (crates/vox-compiler/src/builtin_registry.rs)

### `std_root_field_ty()`  (happy; EXTRACTED)
- [happy] std_root_field_ty('agentos') returns Some(Ty::Named('StdAgentosNs'))  (crates/vox-compiler/src/builtin_registry.rs)

### `stdlib db.insert() capability requirements`  (happy; EXTRACTED)
- [happy] Calling db.insert() with 'uses db' produces no diagnostics  (crates/vox-compiler/src/typeck/effect_check.rs)

### `stdlib db.query() capability requirements`  (error; EXTRACTED)
- [error] Calling db.query() without 'uses db' produces one diagnostic error mentioning 'db'  (crates/vox-compiler/src/typeck/effect_check.rs)

### `stdlib env.get() capability requirements`  (error; EXTRACTED)
- [error] Calling env.get() without 'uses env' produces one diagnostic error mentioning 'env'  (crates/vox-compiler/src/typeck/effect_check.rs)

### `stdlib fs.read() capability requirements`  (error; EXTRACTED)
- [error] Calling fs.read() without 'uses fs' produces one diagnostic error mentioning 'fs'  (crates/vox-compiler/src/typeck/effect_check.rs)

### `stdlib repo.snapshot() capability requirements`  (error; EXTRACTED)
- [error] Calling repo.snapshot() without 'uses vcs' produces one diagnostic error mentioning both 'vcs' and the call site  (crates/vox-compiler/src/typeck/effect_check.rs)

### `string subscript evaluation`  (happy; EXTRACTED)
- [happy] String subscript with valid index evaluates to the character at that position as a single-character string  (crates/vox-compiler/tests/typed_subscript_test.rs)

### `svg element emission`  (happy; EXTRACTED)
- [happy] <svg> JSX element is emitted in generated component output  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `template_managed_app_dependencies`  (happy; EXTRACTED)
- [happy] template_managed_{app,script_native,script_wasi}_dependencies() return sets matching ecosystem-support.yaml registry  (crates/vox-compiler/tests/rust_ecosystem_support_parity_test.rs)

### `terminal state parsing`  (happy; EXTRACTED)
- [happy] marks states as terminal when declared with terminal keyword  (crates/vox-compiler/tests/state_machine_integration_test.rs)

### `terminal state reachability`  (error; EXTRACTED)
- [error] Terminal states that cannot be reached from any transition produce unreachable-error diagnostics  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `terminal state transitions`  (error; EXTRACTED)
- [error] Terminal states cannot have outgoing transitions; violating this produces error diagnostics  (crates/vox-compiler/src/typeck/state_machine_check.rs)

### `text()`  (happy; EXTRACTED)
- [happy] text() view element is correctly lowered and emitted in TSX output  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `tier inversion detection for Dialog inside RootView`  (error; EXTRACTED)
- [error] A Dialog (Modal tier) nested inside a RootView (Content tier) produces exactly one diagnostic with code vox/layer/tier-inversion  (crates/vox-compiler/src/typeck/layer.rs)

### `tier inversion detection for Dialog inside Tooltip`  (error; EXTRACTED)
- [error] A Dialog (Modal tier) nested inside a Tooltip produces exactly one diagnostic with code vox/layer/tier-inversion  (crates/vox-compiler/src/typeck/layer.rs)

### `time.now() in regular function`  (happy; EXTRACTED)
- [happy] time.now() call within regular (non-workflow) function does not generate non-determinism diagnostic  (crates/vox-compiler/tests/workflow_determinism.rs)

### `toast() element lowering`  (happy; EXTRACTED)
- [happy] toast element with z and position attributes lowers to DomNode with data-vox-z and data-vox-pos string attributes  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `tokens contrast validation`  (error; EXTRACTED)
- [error] @tokens block with low-contrast color pairs emits vox/tokens/contrast-violation diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `try-postfix operator ? on Speech.transcribe_microphone()`  (happy; EXTRACTED)
- [happy] try-postfix ? operator on Speech.transcribe_microphone() preserves derivation of both 'speech' and 'microphone' capabilities  (crates/vox-compiler/src/required_capabilities.rs)

### `try-postfix operator ? on fs.read()`  (happy; EXTRACTED)
- [happy] try-postfix ? operator on fs.read() derives the 'fs.read' capability  (crates/vox-compiler/src/required_capabilities.rs)

### `try_format() and format()`  (happy; EXTRACTED)
- [happy] try_format() produces identical output to soft format() when input is valid  (crates/vox-compiler/src/fmt/mod.rs)

### `tuple literal evaluation`  (happy; EXTRACTED)
- [happy] tuple literal parses, lowers, and evaluates to VoxValue::Tuple with correct element values  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `typecheck`  (happy; EXTRACTED)
- [happy] Typecheck produces no errors for inlined public imported functions  (crates/vox-compiler/tests/intra_project_imports_test.rs)

### `typecheck_hir_module(), diagnostic code vox/train/cuda-required`  (edge; EXTRACTED)
- [edge] Training step decorator with low VOX_CUDA_TIER env var generates vox/train/cuda-required diagnostic  (crates/vox-compiler/tests/mens_decorators.rs)

### `typeck diagnostic message formatting`  (invariant; EXTRACTED)
- [invariant] typecheck error on unresolved method call does not leak internal TypeVar IDs  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `typeck diagnostic suggestion`  (happy; EXTRACTED)
- [happy] typecheck error on unresolved method call suggests canonical closure annotation form or uses user-facing <unknown> placeholder  (crates/vox-compiler/tests/eval_typeck_parity_test.rs)

### `undo repo operation`  (error; EXTRACTED)
- [error] Calling undo with extra arguments produces an ArityMismatch error  (crates/vox-compiler/src/eval/repo.rs)

### `useState hook generation`  (happy; EXTRACTED)
- [happy] state variable declarations compile to React useState hook calls in generated tsx  (crates/vox-compiler/tests/golden_mesh_surface_test.rs)

### `validate_overlay`  (error; EXTRACTED)
- [error] validate_overlay detects duplicate z-index values on overlay children and emits diagnostic with code 'web_ir_validate.overlay.duplicate_z'  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `validate_web_ir_with_metrics`  (happy; EXTRACTED)
- [happy] validate_web_ir_with_metrics returns metrics showing view_roots_walked and dom_nodes_traversed counts >= 1  (crates/vox-compiler/tests/web_ir_lower_emit_test.rs)

### `variable subscript index`  (happy; EXTRACTED)
- [happy] subscript expression with identifier index (items[i]) is emitted in tsx  (crates/vox-compiler/tests/golden_props_test.rs)

### `variant_names() method behavior`  (happy; EXTRACTED)
- [happy] variant_names() returns the list of variant names for a registered ADT and returns empty list for non-existent ADT  (crates/vox-compiler/src/typeck/env.rs)

### `vcs effect check`  (happy; EXTRACTED)
- [happy] A function using vcs effect can call repo.snapshot() without diagnostic errors  (crates/vox-compiler/src/typeck/effect_check.rs)

### `vcs effect requirement with try operator`  (error; EXTRACTED)
- [error] Try operator does not mask vcs effect requirement; calling repo.snapshot() with try in a function without vcs effect produces a vcs violation diagnostic  (crates/vox-compiler/src/typeck/effect_check.rs)

### `vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name()`  (invariant; EXTRACTED)
- [invariant] hir_emit attribute name mapping for 'on:click' matches jsx attribute name mapping for 'on_click'  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_codegen::codegen_ts::reactive::normalize_reactive_view_jsx_ws + vox_codegen::web_ir::emit_tsx::emit_component_view_tsx`  (invariant; EXTRACTED)
- [invariant] Legacy JSX emit and Web IR preview produce equivalent output after whitespace normalization  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_codegen::web_ir::emit_tsx::emit_component_view_tsx()`  (happy; EXTRACTED)
- [happy] TSX emission maps raw_class attributes to JSX className property  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_codegen::web_ir::validate::validate_web_ir`  (invariant; EXTRACTED)
- [invariant] Web IR validation returns empty diagnostics for properly formed reactive component  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_codegen::web_ir::validate::validate_web_ir()`  (happy; EXTRACTED)
- [happy] Web IR validation produces no blocking errors for branch-registry fixture after lowering  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_compiler::parser::Parser::warnings`  (edge; EXTRACTED)
- [edge] Parsing deprecated primitive emits exactly one Warning containing old name, new name, version, and migration suggestion  (crates/vox-compiler/tests/rename_alias_test.rs)

### `vox_compiler::parser::parse + vox_compiler::hir::lower_module + vox_codegen::codegen_ts::generate`  (happy; EXTRACTED)
- [happy] Component source with state, derived values, and view hierarchy parses, lowers to HIR, and generates TypeScript without error  (crates/vox-compiler/tests/reactive_smoke_test.rs)

### `vox_compiler::parser::parse_with_registry`  (happy; EXTRACTED)
- [happy] parse_with_registry resolves deprecated primitive name (Box) to canonical name (panel)  (crates/vox-compiler/tests/rename_alias_test.rs)

### `vox_compiler::parser::renames::RenameRegistry::load_canonical`  (happy; EXTRACTED)
- [happy] RenameRegistry loads from contracts/naming/renames.v1.json canonical path without error  (crates/vox-compiler/tests/rename_alias_test.rs)

### `vox_compiler::typeck::diagnostics::codes::ALL_COMPILER_DIAGNOSTIC_CODES`  (invariant; EXTRACTED)
- [invariant] Audit rule IDs do not collide with compiler diagnostic codes  (crates/vox-compiler/tests/audit_rule_collision.rs)

### `webhook custom provider validation`  (error; EXTRACTED)
- [error] @webhook(provider: custom) without explicit secret parameter emits vox/webhook/missing-secret-var diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `webhook custom provider with secret`  (happy; EXTRACTED)
- [happy] @webhook(provider: custom, secret: "...") with explicit secret parameter does not emit vox/webhook/missing-secret-var diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `webhook replay window validation`  (error; EXTRACTED)
- [error] @webhook with replay_window_secs value below recommended 5..=3600 range emits vox/webhook/replay-window-out-of-range diagnostic  (crates/vox-compiler/tests/ga_boilerplate_grafts_test.rs)

### `wire_type_to_ts for Array`  (happy; EXTRACTED)
- [happy] wire_type_to_ts generates readonly arrays in TypeScript (WireType::Array emits 'readonly' prefix)  (crates/vox-compiler/src/contract_ir/tests.rs)

### `wire_type_to_ts for Ref`  (happy; EXTRACTED)
- [happy] wire_type_to_ts maps WireType::Ref('User') to the bare name 'User' (not qualified)  (crates/vox-compiler/src/contract_ir/tests.rs)

### `wire_type_to_ts for Tuple`  (happy; EXTRACTED)
- [happy] wire_type_to_ts converts WireType::Tuple([Number, String]) to TypeScript tuple syntax '[number, string]'  (crates/vox-compiler/src/contract_ir/tests.rs)

### `with operator on Result type`  (happy; EXTRACTED)
- [happy] with operator on Result[int] type with timeout and retries parameters produces no type errors  (crates/vox-compiler/tests/with_expression_typecheck_test.rs)

### `with operator type checking`  (error; EXTRACTED)
- [error] with operator on non-Result type (int) produces Error diagnostic containing message "'with' operand must have type Result[T]"  (crates/vox-compiler/tests/with_expression_typecheck_test.rs)

### `workflow generated hash stability`  (invariant; EXTRACTED)
- [invariant] workflow functions with identical structure but different whitespace produce identical generated hashes  (crates/vox-compiler/tests/workflow_hash_stable.rs)

### `workflow hash differentiation`  (invariant; EXTRACTED)
- [invariant] workflow functions with different body content (return value) produce different generated hashes  (crates/vox-compiler/tests/workflow_hash_stable.rs)

### `workflow non-determinism check for random.int()`  (error; EXTRACTED)
- [error] random.int() call within workflow function generates vox/workflow/non-deterministic-call diagnostic  (crates/vox-compiler/tests/workflow_determinism.rs)

### `workflow non-determinism check for time.now()`  (error; EXTRACTED)
- [error] time.now() call within workflow function generates vox/workflow/non-deterministic-call diagnostic  (crates/vox-compiler/tests/workflow_determinism.rs)

### `workflow without non-deterministic calls`  (happy; EXTRACTED)
- [happy] workflow function with no non-deterministic calls produces empty diagnostics list  (crates/vox-compiler/tests/workflow_determinism.rs)

### `workflow.version() parsing`  (happy; EXTRACTED)
- [happy] workflow.version call with string change_id and integer min/max arguments parses and collects with correct field values  (crates/vox-compiler/tests/workflow_version.rs)

### `write_scaffold_if_missing()`  (happy; EXTRACTED)
- [happy] skips overwriting existing user files when called idempotently  (crates/vox-compiler/tests/scaffold_idempotent_test.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`? operator error propagation`** — only: _? operator on Result<Err> causes early return from enclosing function_
- **`? operator on Option`** — only: _? operator on Option<None> causes early return from enclosing function_
- **`? operator on Result`** — only: _? operator on Result<Ok> unwraps the inner value without early return_
- **`@ai decorator intent routing metadata parsing`** — only: _The @ai decorator parses and lowers task_category, strengths list, tier_max, and cost_ceiling_usd_per_call into HirAiFixture::IntentRouted_
- **`@example decorator`** — only: _@example decorated functions parse cleanly without error-level diagnostics_
- **`@field_name attribute in @json_as`** — only: _The @field_name attribute overrides the JSON key mapping, allowing deserialization from a different JSON property name_
- **`@form decorator`** — only: _@form decorated declarations lower to HirModule with correct field count_
- **`@inference decorator parsing`** — only: _The @inference decorator parses and stores the model parameter as a function attribute_
- **`@tokens block parsing`** — only: _@tokens declaration block with color, spacing, font definitions parses successfully without errors_
- **`@uses decorator parsing`** — only: _@uses(net) decorator on function parses successfully and populates function effects with Net annotation_
- **`@uses with multiple effects`** — only: _@uses(net, fs) decorator populates function effects with both Net and Fs effect annotations_
- **`ADT type registration and constructor binding`** — only: _ADT registration creates ADT lookups and registers constructors as bindings; parameterized constructors have function type, nullary constructors have the ADT type directly_
- **`App`** — only: _project_app_contract() groups endpoints by kind (server/query/mutation), maintains source order, preserves names and route paths, and correctly reports signatures_
- **`AtExample token`** — only: _@example token is recognized by the lexer and compiles without error-level diagnostics_
- **`BinOp`** — only: _Respects operator precedence with multiplication higher than addition via assert! with nested pattern matching_
- **`BinOp::Add`** — only: _Parses binary addition in function call arguments via pattern matching assert!_
- **`Config_to_json serialization`** — only: _Config_to_json emits a JSON object containing the correct keys and values from a Config record_
- **`Counter_from_json deserialization`** — only: _Counter_from_json successfully extracts integer field from JSON payload and unwraps to the correct int value_
- **`Decl`** — only: _Routes declaration parse_summary yields entry_count of 2 and paths [/, /health]_
- **`Decl::Activity`** — only: _activity keyword parses as Decl::Activity with name field accessible_
- **`Decl::Actor`** — only: _Parses actor declaration with name 'Worker' via assert! with matches! pattern_
- **`Decl::Endpoint`** — only: _Server-decorated function parses as Endpoint with Server kind and correct function metadata_
- **`Decl::Function`** — only: _Parses function name 'add' correctly via assert! with matches! pattern_
- **`Decl::Function effects field when uses clause absent`** — only: _Functions without a uses clause have an empty effects vector_
- **`Decl::Import`** — only: _Dotted-path imports (std.http) parse and coexist with function declarations in module_
- **`Decl::Index`** — only: _Index declaration parses table_name, index_name, and column list correctly_
- **`Decl::Loading`** — only: _Parses @loading decorator and extracts function name via assert! with matches! pattern_
- **`Decl::Table`** — only: _Table declaration parses name and fields list with correct field count and names_
- **`Decl::TypeDef`** — only: _Parses type definition with name 'Shape' via assert_eq!_
- **`Decl::Url`** — only: _A simple url declaration with one variant parses correctly with the variant name and empty argument list_
- **`Decl::Url is_pub field`** — only: _By default, url declarations without pub keyword are not public_
- **`Decl::Url optional arguments`** — only: _Url variant arguments prefixed with ? are parsed as optional parameters_
- **`Decl::Url variant arguments`** — only: _Variant arguments have name and optional flag properties that can be queried_
- **`Decl::Url with parameterized variants`** — only: _Url variants can have typed arguments with required (non-optional) parameters_
- **`Decl::V0Component`** — only: _V0Component declaration with quoted ID parses name, v0_id, and image_path as None_
- **`Decl::Workflow`** — only: _workflow keyword parses as Decl::Workflow with name field accessible_
- **`Decl::Workflow, distributed_train_strategy, distributed_train_peers`** — only: _DistributedTrain decorator attributes parse from workflow source and store strategy and peers in Decl::Workflow_
- **`Diagnostic::with_code`** — only: _Diagnostic::with_code attaches stable code string vox/types/type-mismatch that persists to payload.error_code_
- **`Dialog accessibility fix`** — only: _Dialog view with label= property does not emit vox/a11y/dialog-missing-label diagnostic_
- **`Effect checker with superset capabilities`** — only: _When a caller declares a superset of callees' required effects, no diagnostics are emitted_
- **`EffectAnnotation with multiple effects`** — only: _Multiple comma-separated effects in a uses clause parse into a vector of corresponding EffectAnnotation values_
- **`EffectAnnotation::Env`** — only: _The @uses decorator with env keyword parses env as EffectAnnotation::Env effect_
- **`EffectAnnotation::Mcp with parameter`** — only: _The uses clause with mcp(name) syntax parses as EffectAnnotation::Mcp with the parameter string captured_
- **`EffectAnnotation::Net`** — only: _The uses clause with 'net' keyword parses as EffectAnnotation::Net effect_
- **`EffectAnnotation::Nothing`** — only: _The uses clause with 'nothing' keyword parses as EffectAnnotation::Nothing in the function's effects field_
- **`EsImportKind`** — only: _EsImportKind::Named tracks the original exported name separately from the local alias, allowing 'Trigger as DialogTrigger' to record imported='Trigger'_
- **`EventRow`** — only: _EventRow component references ts prop in compiled output_
- **`Expr::Binary`** — only: _Parses binary operations with correct structure via pattern matching_
- **`Expr::For`** — only: _for loop syntax parses as Expr::For with binding field containing loop variable name_
- **`Expr::If`** — only: _else-if chains parse as nested Expr::If with single-statement else bodies, each containing another Expr::If_
- **`Expr::Jsx`** — only: _view-call form `Ident(kwargs) { children }` lowers to Expr::Jsx with tag, attributes array, and children array_
- **`Expr::JsxSelfClosing`** — only: _attr_ prefix on attribute names is stripped to base name during parsing_
- **`Expr::With`** — only: _With expression contains operand and options fields both matching their expected expression types_
- **`FileKind::allows_module_scope_reactive_members`** — only: _FileKind::allows_module_scope_reactive_members returns true only for FileKind::ReactiveModule, and false for Source and Unknown variants_
- **`Flag_from_json bool field deserialization`** — only: _Flag_from_json successfully deserializes bool fields from JSON and returns the correct boolean value_
- **`FnDecl`** — only: _@versioned decorator sets is_versioned flag on parsed function declaration_
- **`FnDecl.prompt_stage, FnDecl.prompt_schema, FnDecl.prompt_redact`** — only: _Prompt decorator attributes parse and lower from AST to HirAiFixture::Prompt with stage, schema, and redact fields preserved_
- **`FnDecl.search_corpus, FnDecl.search_query, FnDecl.search_into, FnDecl.search_top_k, FnDecl.search_policy`** — only: _Search decorator attributes parse and lower from AST to HirAiFixture::Search with corpus, query, into_type, top_k, and policy fields preserved_
- **`FnDecl.subagent_policy, FnDecl.subagent_max_depth, FnDecl.subagent_budget_usd, FnDecl.subagent_description`** — only: _Subagent decorator attributes parse and lower from AST to HirAiFixture::Subagent with policy, max_depth, budget_usd, and description preserved_
- **`Fragment`** — only: _fragment declaration with no parameters parses and preserves the name 'Greeting'_
- **`Function`** — only: _AI decorator sets is_llm flag to true on function_
- **`Future[T] deprecation fix suggestion`** — only: _Future[T] deprecation diagnostic provides a fix suggestion that replaces it with DurablePromise[T]_
- **`Future[T] type`** — only: _Future[T] type annotation produces vox/types/future-deprecated warning_
- **`HIR lowering of @query and @mutation declarations`** — only: _lower_module maps @query and @mutation declarations to HirEndpointFn entries with correct QUERY_FN_API_PREFIX and MUTATION_FN_API_PREFIX route_path prefixes_
- **`HIR lowering of db all().select() projection`** — only: _lower_module populates the projection field in HirDbTableOp when lowering db.User.all().select() chains_
- **`HIR lowering of db chain capability modifiers`** — only: _lower_module populates HirDbQueryPlan.capabilities with sync, live_topic, orchestration_scope, and retrieval_mode when lowering db.User.filter().using().live().scope().sync().limit() chains_
- **`HIR lowering of db filter chain`** — only: _lower_module lowers db.User.filter() calls to HirExpr::MethodCall with HirDbTableOp::FilterRecord in the query plan_
- **`HIR lowering of db filter+count chain`** — only: _lower_module converts db.User.filter().count() to a count MethodCall with HirDbTableOp::Count and preserves filter arguments_
- **`HIR lowering of db query chain with order_by and limit`** — only: _lower_module preserves order_by and limit modifiers in HirDbTableOp when lowering db.User.filter().order_by().limit() chains_
- **`HIR lowering of environment declarations`** — only: _lower_module parses environment declarations and populates HirModule.environments with correct name, base_image, and packages fields_
- **`HirAiFixture::Hole, typecheck_hir_module()`** — only: _Hole decorator with unfilled fixture generates diagnostic code vox/fixture/unfilled-hole during typecheck_
- **`HirCapability`** — only: _@versioned decorator implicitly grants Vcs capability to function_
- **`HirCorsPolicy::allows_origin`** — only: _CORS policy with wildcard origin pattern allows any origin to pass_
- **`HirExpr::WorkflowVersion`** — only: _When workflow.version() is parsed and lowered to HIR, the resulting body contains HirExpr::WorkflowVersion with the correct change_id_
- **`HirFn`** — only: _HirFn carries is_versioned flag after lowering_
- **`HirFn.capabilities, HirCapability::GpuCompute, HirCapability::Random, HirCapability::Net`** — only: _Inference decorator lowers to HirFn with capabilities containing GpuCompute, Random, and Net_
- **`HirFn.distributed_train, HirCapability::Spawn, HirCapability::Net`** — only: _DistributedTrain decorator lowers to HirFn with distributed_train metadata and Spawn and Net capabilities_
- **`HirFn.training_step, HirFn.capabilities, HirCapability::GpuCompute, HirCapability::Mutate`** — only: _TrainingStep decorator sets HirFn.training_step flag and adds GpuCompute and Mutate capabilities_
- **`HirImport`** — only: _Mixed slash/dot separators in import paths parse and lower correctly, with slashes and dots both accepted as segment separators_
- **`HirImport, destructured single-item import`** — only: _Destructured import with single item as { Name } creates one HirImport with correct module_path and item_
- **`HirImport.module_path, HirImport.item`** — only: _Forward slash separators in import paths parse identically to dot separators in module_path and item_
- **`HirModule`** — only: _HirModule serializes to JSON with stringified span keys in the format "start-end"_
- **`HirModule deserialization error message`** — only: _Deserialization error message contains descriptive text describing the malformed key_
- **`HirModule endpoint population from golden example`** — only: _lower_module generates exactly 3 endpoint_fns and 1 table from the golden crud_api.vox file_
- **`HirModule serialization`** — only: _HirModule with inferred types serializes to JSON without panic (regression gate for serde_json::to_string)_
- **`HirModule.tables and HirModule.endpoint_fns population`** — only: _lower_module correctly populates tables (1) and endpoint_fns (1) collections when processing web declarations_
- **`HirModule::examples`** — only: _@example blocks land in HirModule::examples, not HirModule::tests_
- **`HirModule::to_semantic_hir`** — only: _Semantic HIR projection drops migration-only vectors, leaving the functions vector empty_
- **`HirPiiMarker`** — only: _JSON serialization and deserialization of HirPiiMarker preserves the PiiClass::Email value_
- **`HirSmFrom`** — only: _State machine transition from 'any' keyword parses to HirSmFrom::Any variant_
- **`HirType`** — only: _HirType has a Generic variant that accepts a name string and vector of element types_
- **`ImportPath`** — only: _Parses Rust crate import alias 'as json' correctly via assert_eq!_
- **`ImportPathKind::RustCrate()`** — only: _Parses Rust crate import with rust: prefix via pattern matching_
- **`InferenceContext::unify field-order independence`** — only: _Table and Record types unify successfully regardless of field declaration order_
- **`InferenceContext::unify for Ty::Collection and Ty::Record`** — only: _Collection types (live query views) unify with matching Record types_
- **`InferenceContext::unify for Ty::Table and Ty::Record`** — only: _Table and Record types with matching fields unify successfully in both directions (symmetric unification)_
- **`Interpreter`** — only: _Interpreter successfully executes test functions with Result type matching and returns Ok() result_
- **`Interpreter execution with functions and loops`** — only: _Interpreter executes functions with parameters, mutable variable assignment, while loops, and function calls correctly_
- **`Interpreter recovery after failed call`** — only: _After a failed call (EvalError) that would corrupt scope, a subsequent call still evaluates correctly and scope depth is clean at 1_
- **`Interpreter.run_module`** — only: _Interpreter successfully executes for loops with index binding_
- **`JSX attribute value expression containing Speech.transcribe_microphone()`** — only: _capability walker descends into JSX attribute value expressions to derive capabilities from Speech.transcribe_microphone()_
- **`JSX element children containing Speech.transcribe_microphone()`** — only: _capability walker descends into both JSX attributes and children to derive capabilities from Speech.transcribe_microphone()_
- **`Json.has`** — only: _has() method reports JSON key membership without requiring unwrapping_
- **`Json::as_str()`** — only: _as_str() on JSON null value returns None, not Some_
- **`Json::is_null()`** — only: _is_null() returns true when Json value is JSON null_
- **`JsxAttribute`** — only: _JSX attributes have name field and are indexable by position_
- **`KPI chip string literals`** — only: _string literal chip keys (NODES, ACTIVE, BLOCKED, ERRORS) are emitted verbatim in tsx output_
- **`LayerTier::allows_child`** — only: _Popover tier cannot host Modal children, but Modal can host itself and Popover can be hosted by Modal_
- **`LayerTier::default_for_primitive()`** — only: _Maps 'Tooltip' to LayerTier::Popover via assert_eq!_
- **`LayerTier::from_str`** — only: _All LayerTier variants convert to string and back correctly via from_str roundtrip_
- **`List.filter()`** — only: _List.filter(closure) keeps only elements where the closure returns true_
- **`List.fold()`** — only: _list.fold() with closure accumulator evaluates to correct integer sum_
- **`List.map()`** — only: _List.map(closure) applies the closure to each element and returns a new list of the same length_
- **`Module`** — only: _lex(), parse(), and lower_module() successfully convert source code through the pipeline_
- **`Module parser (script mode) with mixed declarations and statements`** — only: _When parsing a script with both declarations (e.g., fn helper) and statements, the parser creates a synthetic main function containing the statements, resulting in 2 declarations total_
- **`Module parser (script mode) with multiple statements`** — only: _Multiple top-level statements are all collected into a single synthetic main function_
- **`Module parser (script mode) with pure declarations`** — only: _When parsing a script containing only declarations with no top-level statements, no synthetic main function is created_
- **`Note_from_json Option field deserialization`** — only: _When an Option[T] field is present in JSON input, from_json stores Some(value) in the synthesized record_
- **`ObjectLit`** — only: _Object literal keys can include phonetic keywords (and, or, not, in) as field names_
- **`Option equality comparison (mk() is None)`** — only: _An Option value returned from a function equals the None literal under equality comparison_
- **`Option.map()`** — only: _option.map() with closure transforms Some and passes through None_
- **`Option.unwrap_or()`** — only: _Option.unwrap_or(default) returns the value if Some, or the default if None_
- **`Product_from_json deserialization`** — only: _Product_from_json successfully extracts string field from JSON and returns a Record containing the correct value_
- **`Promise[T] type deprecation warning`** — only: _Promise[T] return type produces vox/types/promise-deprecated diagnostic code_
- **`ReactiveMemberDecl`** — only: _reactive module declaration contains two members when parsing two state declarations_
- **`Result ADT error value propagation`** — only: _Result type carries a real typed error value (ADT variant) through match expressions, extracting payload values_
- **`Result error type parameter binding`** — only: _The Error arm of Result[T, E] binds the error payload to the declared E type, not hardcoded str_
- **`Result.is_err()`** — only: _Result.is_err() method dispatches correctly on Result types returned from fs operations_
- **`Result.is_ok()`** — only: _Result.is_ok() method dispatches correctly on Result types returned from fs operations_
- **`Result.map_err()`** — only: _result.map_err() with closure transforms Err and passes through Ok_
- **`RetryPolicy`** — only: _All RetryPolicy enum variants (None, ExpBackoff, FixedInterval) serialize and deserialize correctly via JSON roundtrip_
- **`RouteId`** — only: _Home route_id has url_pattern of '/' and empty params list_
- **`RouteNode`** — only: _RouteNode::RouteTree round-trips through JSON serialization preserving routes length and pattern values_
- **`Routes block lowering to route identifiers`** — only: _Routes block with two route declarations produces exactly 2 route_ids in HIR_
- **`RoutesDecl::parse_summary`** — only: _Routes declaration parse_summary returns correct entry count and path strings matching parsed routes_
- **`RuleBasedAutoFixer::suggest_fixes`** — only: _RuleBasedAutoFixer::suggest_fixes generates component migration fixes with correct diff format containing source and target syntax_
- **`RunRow`** — only: _RunRow component emits at least 6 <p> elements in compiled TypeScript output_
- **`RustInteropSemanticsState::PartiallyImplemented.as_label()`** — only: _RustInteropSemanticsState::PartiallyImplemented.as_label() returns 'partially_implemented'_
- **`SVG points prop forwarding`** — only: _points attribute in SVG polygon is forwarded as a prop in generated tsx_
- **`SVG viewBox attribute`** — only: _viewBox attribute in SVG is emitted in generated tsx_
- **`Scope`** — only: _Scope implements copy-on-write semantics: cloned scopes share data until get_mut is called, then original remains unchanged_
- **`SourceSpanTable`** — only: _SourceSpanTable.push_span returns sequential IDs starting at 0, and SourceSpanTable.get retrieves the correct span for each ID_
- **`Speech.transcribe(filepath)`** — only: _Speech.transcribe() with file path derives 'speech' capability but not 'microphone' capability_
- **`Speech.transcribe_microphone()`** — only: _Speech.transcribe_microphone() derives both 'microphone' and 'speech' capabilities_
- **`Stats_from_json`** — only: _json_as decorator generates function that extracts all declared struct fields from JSON object_
- **`Stmt::Let`** — only: _Parses let statement in function body via assert! with matches! pattern_
- **`StubAutoFixer::suggest_fixes`** — only: _StubAutoFixer::suggest_fixes generates exactly one fix per diagnostic with correct diff format_
- **`StyleNode`** — only: _StyleNode::Declaration round-trips through JSON serialization preserving property name_
- **`Token`** — only: _Lexer emits Token::Eof, Token::LBrace, Token::RBrace, and Token::Return tokens for brace-delimited function blocks_
- **`Token::AtComponent`** — only: _@component decorator lexes to Token::AtComponent_
- **`Token::AtDeprecated`** — only: _@deprecated decorator lexes to Token::AtDeprecated_
- **`Token::AtMcpResource`** — only: _@mcp.resource decorator lexes to Token::AtMcpResource_
- **`Token::AtMcpTool`** — only: _@mcp.tool decorator lexes to Token::AtMcpTool_
- **`Token::AtMutation`** — only: _@mutation decorator lexes to Token::AtMutation as a first-class distinct token_
- **`Token::AtNative`** — only: _@native decorator lexes to Token::AtNative_
- **`Token::AtPure`** — only: _@pure decorator lexes to Token::AtPure_
- **`Token::AtQuery`** — only: _@query decorator lexes to Token::AtQuery as a first-class distinct token_
- **`Token::AtResource`** — only: _@resource decorator lexes to Token::AtResource_
- **`Token::AtScheduled`** — only: _@scheduled decorator lexes to Token::AtScheduled_
- **`Token::AtTool`** — only: _@tool decorator lexes to Token::AtTool_
- **`Token::AtTracked`** — only: _@tracked decorator lexes to Token::AtTracked_
- **`Token::AtVersioned`** — only: _@versioned decorator lexes to Token::AtVersioned_
- **`Token::BangInvalid`** — only: _Bang (!) character lexes as Token::BangInvalid, a distinct token separate from 'not' keyword_
- **`Token::FloatLit`** — only: _Float literals lex to Token::FloatLit with correct numeric value_
- **`Token::Ident`** — only: _Lowercase identifiers lex to Token::Ident variant_
- **`Token::IntLit`** — only: _Integer literals lex to Token::IntLit with correct numeric value_
- **`Token::StringLit`** — only: _JSON object literal {"key":1} is lexed as StringLit token, not TemplateStringLit_
- **`Token::TemplateStringLit`** — only: _template string with identifier in braces is lexed as TemplateStringLit_
- **`Token::TypeIdent`** — only: _PascalCase identifiers lex to Token::TypeIdent variant, not Token::Ident_
- **`TokenRegistry`** — only: _TokenRegistry loads annotated token values (with metadata) from JSON and lookup() returns correct values_
- **`TokenRegistry::load_from_str()`** — only: _TokenRegistry::load_from_str() parses flat token definitions and lookup() retrieves values by key correctly_
- **`TypeEnv scope push/pop behavior`** — only: _Variables defined in outer scope are visible in inner scope after push_scope(), and variable shadowing in inner scope is resolved after pop_scope() returns to outer type_
- **`VoxCompilerDiagnosticPayload`** — only: _minimal_repro field is None when source is empty string_
- **`VoxCompilerDiagnosticPayload::from_diagnostic`** — only: _VoxCompilerDiagnosticPayload::from_diagnostic generates a vox-lang.org explain URL for diagnostic codes with vox/ prefix_
- **`VoxValue`** — only: _VoxValue::Str wraps string values that can be passed to builtin methods_
- **`VoxValue list push mutation`** — only: _The push method on a list mutates the owner in-place, observable as growth of the list length_
- **`VoxValue::Fn`** — only: _VoxValue::Fn registered in scope carries is_versioned flag for @versioned functions_
- **`VoxValue::list(), VoxValue::object(), VoxValue::tuple()`** — only: _Constructor functions create values matching their expected variant patterns_
- **`Webhook decorator validation for Stripe provider`** — only: _Stripe webhook provider does not emit missing-secret-var or replay-window-out-of-range diagnostics with default configuration_
- **`WireType`** — only: _WireType has primitive variants (Number, String, Bool, Unit, Unknown) that can be passed to wire_type_to_zod()_
- **`WireType projection for Option`** — only: _Option<T> fields are marked as optional and the inner type T is projected to its corresponding WireType (str->String)_
- **`WireType projection for primitives`** — only: _HirType::Named('int') projects to WireType::Number and HirType::Named('str') projects to WireType::String_
- **`activity function generated_hash`** — only: _activity functions have non-None generated_hash field after HIR lowering_
- **`actor keyword`** — only: _actor keyword compiles without producing TypeckSeverity::Error diagnostics_
- **`actor registration and lookup`** — only: _Registered actor is findable via lookup() with BindingKind::Actor and via lookup_actor()_
- **`agentos_mutation_kind_for_tool()`** — only: _std_namespace_runtime_call for agentos.mutation_kind_for_tool contains 'vox_foundation::primitives::agentos_mutation_kind_for_tool' in output_
- **`apply_naming()`** — only: _apply_naming() returns input string unchanged when naming convention is snake_case_
- **`arithmetic subscript index`** — only: _subscript expression with arithmetic expression (items[i + 1]) is emitted in tsx_
- **`assert_golden_file`** — only: _All golden .vox example files can be parsed and lowered without assertion failures_
- **`backward compatibility for string error types`** — only: _Result[T, str] with string literal Error construction maintains backward compatibility and typechecks_
- **`call_builtin_method()`** — only: _call_builtin_method() dispatches "basename" on path namespace to extract filename from path_
- **`check_effect_compliance`** — only: _check_effect_compliance returns a vector of Diagnostic and parses through lex, parse, and lower_module stages_
- **`check_pure_violation()`** — only: _Pure caller calling pure callee returns no diagnostic (is_none)_
- **`check_state_machines()`** — only: _check_state_machines returns empty diagnostics when parsing and lowering a valid module with state_machine definition_
- **`check_tier_inversions()`** — only: _check_tier_inversions returns an empty result when Tooltip is nested inside Dialog_
- **`check_tokens()`** — only: _Emits one diagnostic with code 'vox/tokens/contrast-violation' when color token light and dark values violate contrast ratio_
- **`check_training_cuda_tier()`** — only: _Returns empty diagnostics list when VOX_CUDA_TIER environment variable is unset (default tier allows training)_
- **`chip filter labels`** — only: _string literal filter chip labels ('all', 'ok', 'error') are emitted in generated tsx output_
- **`circle SVG element passthrough`** — only: _<circle> SVG element is emitted as JSX element, not as a function call_
- **`classify_rust_crate`** — only: _classify_rust_crate() returns decision label matching ecosystem-support.yaml registry for each named crate_
- **`closure capture`** — only: _Closures capture enclosing scope variables by clone and use them in the closure body_
- **`closure environment capture`** — only: _A closure captures its enclosing lexical environment by value, allowing it to read captured variables (n=10) correctly across multiple invocations (add_n(5)=15, add_n(100)=110, sum=125)_
- **`closure literal syntax`** — only: _Closure literal syntax fn(x: Type) to ReturnType { body } parses and evaluates to the correct result_
- **`codegen_ts::generate`** — only: _Match on Result type does not emit case _ literal patterns_
- **`collect_golden_vox_files`** — only: _Golden example collection recursively traverses examples/golden/ subdirectories and locates .vox files_
- **`collect_golden_vox_files()`** — only: _collect_golden_vox_files recursively traverses examples/golden/** subdirectories and includes nested vox files_
- **`compact()`** — only: _Function definition compacts to single-line form preserving braces, name, and statements_
- **`compile_components`** — only: _PlayIcon component does not emit raw snake_case SVG attributes like view_box=_
- **`compiled regex object and group extraction`** — only: _Compiled regex objects support .find() and .group(i) methods to extract match groups under interpreter mode_
- **`component call count in tsx`** — only: _MeshKpi component call count in generated tsx matches the number of component invocations in source (6)_
- **`component instance count for OrchNode`** — only: _multiple OrchNode component instances in source are emitted as JSX elements with correct count (2)_
- **`component instance count in svg`** — only: _multiple AgentNode component instances in source are emitted as JSX elements with correct count (5)_
- **`conditional stroke color emission`** — only: _conditional stroke colors in component are emitted as string literals (#3b82f6, #52525b) in tsx_
- **`conditional stroke color in polygon`** — only: _conditional stroke color (#60a5fa) in polygon is emitted as string literal in tsx_
- **`cross-numeric equality (1 is 1.0)`** — only: _Equality comparison between int and float operands evaluates based on numeric value equivalence_
- **`dashed line styling`** — only: _SVG line with stroke-dasharray attribute emits strokeDasharray in generated tsx_
- **`db query method chaining`** — only: _Chained db query methods (.where, .order_by, .limit) typecheck without errors_
- **`db.Table.all`** — only: _all() returns all inserted rows and len() counts them correctly_
- **`db.Table.all.order_by.limit`** — only: _limit() on an ordered query restricts result count to specified number_
- **`db.Table.count`** — only: _count() returns the correct count of rows in the table_
- **`db.Table.delete`** — only: _delete() removes the specified row from the table by _id_
- **`db.Table.filter`** — only: _filter() with boolean equality predicate returns matching rows_
- **`db.Table.get`** — only: _get() with valid id returns Some containing the row_
- **`db.Table.insert`** — only: _Multiple insert operations successfully add rows to database_
- **`db.Table.where`** — only: _where clause with gte predicate correctly filters rows by numeric comparison_
- **`db.Table.where.order_by.limit`** — only: _Fused where+order_by+limit chain composes correctly to filter, sort, and limit rows_
- **`db.Table.where.select`** — only: _Fused where().select() chain applies filter before projection_
- **`db.User.all().select()`** — only: _Selecting a subset of columns with select() from a table query returns no TypeckSeverity::Error_
- **`decimal arithmetic (0.1dec + 0.2dec is 0.3dec)`** — only: _Decimal arithmetic in interpreter produces exact results without floating-point precision loss_
- **`defaults: true in @json_as`** — only: _When defaults: true is specified, missing fields are populated with zero values (0 for int, empty string for str) instead of returning an error_
- **`defined_in_current_scope() scope boundary detection`** — only: _defined_in_current_scope() returns true only for variables directly defined in current scope, not for inherited bindings from outer scope after push_scope()_
- **`destructured import as { } syntax, HirImport collection`** — only: _Destructured import with as { Name1, Name2, ... } expands to one HirImport per name with same module_path_
- **`effect with explicit depends_on clause`** — only: _An explicit depends_on clause suppresses the lint.effect.unresolvable_deps diagnostic regardless of whether the called function is a builtin_
- **`effect with only state reads and assignments`** — only: _An effect that only reads and assigns state variables does not emit lint.effect.unresolvable_deps diagnostic_
- **`effect_check`** — only: _Effect checker permits bare repo.* calls in @versioned functions due to implicit vcs capability_
- **`embed`** — only: _embed decorator attributes (model, dimensions, source_field) are captured and available on function declarations_
- **`emit_component_view_tsx`** — only: _emit_component_view_tsx successfully emits TSX for Counter component with state and derived fields_
- **`emit_component_view_tsx()`** — only: _emit_component_view_tsx produces valid TSX output via snapshot assertion for component with view_
- **`emit_component_view_tsx_with_stats()`** — only: _emit_component_view_tsx_with_stats() returns stats object tracking nodes_visited count during emission_
- **`emit_main`** — only: _Emitted main.rs contains starts_with("/api") check to reserve /api prefix_
- **`endpoint clean effect validation`** — only: _An endpoint with a single declared effect produces no diagnostics_
- **`exhaustive match patterns`** — only: _Exhaustive Option/Result matches including binding-as-wildcard patterns are accepted without E0301 errors_
- **`for loop body rendering`** — only: _for-loop body containing markup emits as JSX span elements in tsx output_
- **`for loop compilation`** — only: _for-loop without explicit index compiles to .map((r, _i) => ...) with synthetic underscore index_
- **`for loop field access`** — only: _for-loop body with field access (r.name) emits as JSX expression {r.name}_
- **`for loop field references`** — only: _for-loop body can reference multiple fields from loop variable (r.id, r.duration)_
- **`for loop with explicit index`** — only: _for-loop with user-named index variable preserves the index name in .map((item, i) => ...) call_
- **`for loop with index binding`** — only: _for v, i in list { ... } correctly binds both value and index in loop body_
- **`for loop without index`** — only: _for-loop omitting index variable emits synthetic _i parameter in .map() call_
- **`for-loop iteration over maps and strings`** — only: _For loops iterate over maps (yielding tuples) and strings (yielding characters) correctly_
- **`from any transition pattern`** — only: _Transitions using 'from any' satisfy exhaustiveness requirements for all non-terminal states_
- **`fs namespace method dispatch`** — only: _fs.cwd() returns Result[str] with is_ok() method, fs.exists(path) returns bool; both dispatch without Method-not-found errors_
- **`generate`** — only: _column and row primitives emit as JSX elements, not as JS function calls_
- **`generate()`** — only: _Web IR view emission succeeds even when VOX_WEBIR_EMIT_REACTIVE_VIEWS env var is explicitly set to 0_
- **`generated output.files`** — only: _Codegen produces Counter.tsx file in output files map_
- **`interp_csv_parse() and interp_csv_render()`** — only: _CSV data roundtrips through render and parse functions preserving array structure_
- **`interp_csv_parse_records(), interp_yaml_parse(), interp_yaml_render(), interp_toml_parse(), interp_toml_render()`** — only: _Format conversion roundtrips through CSV->YAML->TOML->parse chain complete without error_
- **`is_template_managed_app_dependency()`** — only: _is_template_managed_app_dependency() returns true for 'reqwest' and 'vox-db'_
- **`is_template_managed_script_native_dependency()`** — only: _is_template_managed_script_native_dependency() returns true for 'tokio'_
- **`is_template_managed_script_wasi_dependency()`** — only: _is_template_managed_script_wasi_dependency() returns true for 'serde' and false for 'reqwest'_
- **`is_wasi_unsupported_rust_import()`** — only: _is_wasi_unsupported_rust_import() returns true for 'reqwest' and 'turso', false for 'serde_json' and 'uuid'_
- **`json.parse`** — only: _json.parse returns Ok(Json) for valid JSON and get() retrieves nested values_
- **`json_as annotation synthesis`** — only: _When a type is annotated with @json_as, both Widget_from_json and Widget_to_json functions are synthesized and appear in hir.functions_
- **`json_as field synthesis`** — only: _synthesise_json_as_fns generates 4 statements in the from function body for a required field (let _f, guard, unwrap, return)_
- **`key prop in for loop`** — only: _for-loop body with key={{expr}} prop emits as key={expr} in JSX_
- **`lint_ast_declarations()`** — only: _lint_ast_declarations() emits a diagnostic with code 'lint.pure_shallow_violation' and severity Warning when @pure function calls print()_
- **`literal subscript index`** — only: _subscript expression with literal index (items[0]) is emitted verbatim in tsx_
- **`lower_hir_to_web_ir`** — only: _Event handler attributes lower on_click to onClick in DomNode elements_
- **`lower_hir_to_web_ir_with_summary`** — only: _Summary counts correctly distinguish @server, @query, and @mutation function contracts_
- **`lower_hir_to_web_ir_with_summary()`** — only: _lower_hir_to_web_ir_with_summary() returns a summary with lowering_diagnostics count tracking AST node diagnostics_
- **`lower_module`** — only: _modules containing only let bindings parse and lower without panicking_
- **`lower_module()`** — only: _lower_module produces valid HIR when parsing Vox component source succeeds_
- **`mixed integer and float arithmetic type promotion`** — only: _Arithmetic operations promote mixed int and float operands to float and execute without type errors_
- **`multi-decorator function parsing`** — only: _Functions with @auth, @cors, @rate_limit, @webhook, @layer decorators parse successfully_
- **`namespace_builtin_owned`** — only: _namespace_builtin_owned returns true for owned namespaces (fs) and false for non-owned namespaces (mobile)_
- **`naming: camelCase in @json_as`** — only: _When naming: camelCase is specified, Event_from_json reads JSON keys in camelCase and maps them to snake_case struct fields_
- **`nested component rendering`** — only: _StateChip component is properly emitted in generated tsx output_
- **`nested for loops`** — only: _nested for-loops compile to nested .map() calls with independent variables per level_
- **`nested loop variable access`** — only: _inner for-loop can reference inner loop variable in body expressions (row.map((cell, j) => {cell}))_
- **`not operator boolean inversion`** — only: _The `not` operator correctly inverts boolean values and composes multiple inversions_
- **`nullary variant pattern matching dispatch`** — only: _Nullary variant patterns in match expressions dispatch to their correct arm without catching all cases_
- **`onClick handler emission`** — only: _onClick event handler attribute is emitted in generated jsx for components with interaction_
- **`overlay() component with toast child elements`** — only: _overlay() primitive lowers to DomNode with data-vox-overlay="true" attribute and nested toast children with data-vox-z and data-vox-pos attributes_
- **`parse()`** — only: _parse successfully converts lexed tokens to Module AST when source is valid_
- **`parse() and lower_module()`** — only: _all .vox files in tests/llm_fixtures parse successfully and lower without errors_
- **`parse_imports`** — only: _parse_imports on 'import surfaces/mesh.MeshSurface' produces exactly one HirImport with item='MeshSurface' and module_path=['surfaces', 'mesh']_
- **`parse_script`** — only: _Top-level let statement wraps in synthetic main function with correct name, zero params, and Let statement body_
- **`partial state_machine attribute`** — only: _Partial state machines skip exhaustiveness checks and produce no diagnostics_
- **`path.extension()`** — only: _path.extension() returns the file extension string correctly from a file path_
- **`path.stem()`** — only: _path.stem() returns Option[str] that can be matched and unwrapped to get the filename stem_
- **`polygon SVG element`** — only: _<polygon> SVG element is emitted as JSX element, not as a JavaScript function call_
- **`polymorphic Result Error construction`** — only: _Result[T, MyErr] with typed ADT error arm construction typechecks with exhaustive nested matches_
- **`polymorphic Result Ok construction`** — only: _Ok(v) constructed in functions declared to Result[T, MyErr] unifies with the declared ADT error type_
- **`preserveAspectRatio camelCase conversion`** — only: _preserve_aspect_ratio snake_case attribute is converted to preserveAspectRatio camelCase in tsx_
- **`process.run() return type parity`** — only: _process.run(cmd, args) returns Option[Record] (not bare Record) wrapping execution results; unwrap() succeeds when process exits with code 0_
- **`process.which()`** — only: _process.which() returns Option[str] that is Some for executables like cargo and None for non-existent commands_
- **`project::type_def with struct`** — only: _Projecting a struct type preserves field names ('id', 'name') and maps Vox HirTypes to WireTypes (int->Number, str->String)_
- **`project::type_def with sum type`** — only: _Projecting a sum type emits ContractTypeKind::Sum with variants tagged by variant name and preserving field names_
- **`project_runtime_from_core`** — only: _Module task capability hints infer prefer_gpu_compute=true and vector/training labels from db.using() and scope()_
- **`project_type for Decimal`** — only: _HirType::Decimal and HirType::Named('Decimal') both project to WireType::DecimalString_
- **`project_type for datetime types`** — only: _DateTime type aliases ('Date', 'DateTime', 'Instant', 'Timestamp') all project to WireType::DateTimeString_
- **`project_type for large integer types`** — only: _HirType::Named('bigint') and HirType::Named('i128') both project to WireType::BigIntString_
- **`project_type()`** — only: _project_type() maps generic list-like types (list, List, Vec, Array) containing an element type to WireType::Array_
- **`reactive_smoke_test module`** — only: _Test runs with CARGO_MANIFEST_DIR environment variable set_
- **`record.get() None case`** — only: _record.get(key) returns Option.None when key is missing; is_none() evaluates to true_
- **`record.get() return type parity`** — only: _record.get(key) returns Option[T] (not bare T) compatible with .unwrap(); unwrapping a present key yields the correct value_
- **`rect SVG element`** — only: _<rect> SVG elements are emitted in generated tsx output_
- **`recursive function frame independence`** — only: _Recursive function calls maintain independent stack frames in the new cactus/Rc-frame scope representation, computing Fibonacci(10)=55 correctly_
- **`regex namespace method dispatch`** — only: _regex.is_match(), regex.replace() dispatch correctly; is_match(text, pattern) returns true for matches, replace(text, old, new) substitutes correctly_
- **`regex.captures() method dispatch`** — only: _regex.captures(text, pattern) returns Option[list[str]] with capture groups; is_some() is true for matching patterns, is_none() for non-matches_
- **`regex.find() method dispatch`** — only: _regex.find(text, pattern) returns Option[str]; is_some() is true for matching patterns, is_none() is true when pattern does not match_
- **`required_capabilities`** — only: _Speech.transcribe() derives 'speech' capability but not 'microphone' when used in function return type_
- **`resolve_key()`** — only: _resolve_key() uses explicit field_name override when present, ignoring naming convention_
- **`routes`** — only: _Routes block is processed in code generation output_
- **`row/column layout functions`** — only: _row() and column() functions do not emit as JavaScript function calls in generated tsx_
- **`search placeholder text`** — only: _input placeholder text 'Search runs' is emitted in generated tsx output_
- **`semantics_state_for_rust_crate`** — only: _semantics_state_for_rust_crate() returns label matching ecosystem-support.yaml semantics_state field for each crate_
- **`shell back_button field`** — only: _can be populated from parsed back_button decorator_
- **`side_effect block in workflow`** — only: _suppresses vox/workflow/non-deterministic-call diagnostic_
- **`side_effect block syntax`** — only: _parses without E_PARSE errors inside workflow body_
- **`side_effect block synthesis`** — only: _creates distinct synthesized activities for each side_effect block_
- **`state machine codegen for terminal states`** — only: _Code emission excludes terminal state cases from the reducer switch statement_
- **`state machine field parsing`** — only: _Parsed state with field declarations populates the fields array with correct name and count_
- **`state machine is_partial flag`** — only: _is_partial is false for non-partial state machines_
- **`state machine parsing`** — only: _parses simple state machines with correct state count and transition count_
- **`state variable emission`** — only: _state variable declarations are emitted in generated tsx component_
- **`std.http.get_text() performs real HTTP requests`** — only: _std.http.get_text() performs real HTTP requests in interpreter mode, not a stub, returning actual transport errors_
- **`std.time.now_ms() builtin function`** — only: _std.time.now_ms() returns a positive integer epoch timestamp under interpreter mode_
- **`std_namespace_method_ty`** — only: _std_namespace_method_ty returns Some for every namespace/method pair in NAMESPACE_BUILTINS_
- **`std_namespace_method_ty()`** — only: _std_namespace_method_ty('agentos', 'mutation_kind_for_tool') returns Some_
- **`std_root_field_ty()`** — only: _std_root_field_ty('agentos') returns Some(Ty::Named('StdAgentosNs'))_
- **`stdlib db.insert() capability requirements`** — only: _Calling db.insert() with 'uses db' produces no diagnostics_
- **`string subscript evaluation`** — only: _String subscript with valid index evaluates to the character at that position as a single-character string_
- **`supported_targets_for_rust_crate()`** — only: _supported_targets_for_rust_crate('serde_json') returns targets containing RustInteropTarget::Wasi_
- **`svg element emission`** — only: _<svg> JSX element is emitted in generated component output_
- **`synthesise_json_as_fns()`** — only: _synthesise_json_as_fns() returns empty vec when type definition has no json_as annotation_
- **`template_managed_app_dependencies`** — only: _template_managed_{app,script_native,script_wasi}_dependencies() return sets matching ecosystem-support.yaml registry_
- **`terminal state parsing`** — only: _marks states as terminal when declared with terminal keyword_
- **`text()`** — only: _text() view element is correctly lowered and emitted in TSX output_
- **`time.now() in regular function`** — only: _time.now() call within regular (non-workflow) function does not generate non-determinism diagnostic_
- **`toast() element lowering`** — only: _toast element with z and position attributes lowers to DomNode with data-vox-z and data-vox-pos string attributes_
- **`try-postfix operator ? on Speech.transcribe_microphone()`** — only: _try-postfix ? operator on Speech.transcribe_microphone() preserves derivation of both 'speech' and 'microphone' capabilities_
- **`try-postfix operator ? on fs.read()`** — only: _try-postfix ? operator on fs.read() derives the 'fs.read' capability_
- **`try_format() and format()`** — only: _try_format() produces identical output to soft format() when input is valid_
- **`ts_string_literal()`** — only: _ts_string_literal() escapes inner double quotes in JSON objects to produce valid TypeScript string literals_
- **`tuple literal evaluation`** — only: _tuple literal parses, lowers, and evaluates to VoxValue::Tuple with correct element values_
- **`typecheck`** — only: _Typecheck produces no errors for inlined public imported functions_
- **`typecheck_ast_module()`** — only: _typecheck_ast_module() emits diagnostic with code 'lint.handler.uncancellable_async' for unannotated handler calling async endpoint and assigning state_
- **`typeck diagnostic suggestion`** — only: _typecheck error on unresolved method call suggests canonical closure annotation form or uses user-facing <unknown> placeholder_
- **`useState hook generation`** — only: _state variable declarations compile to React useState hook calls in generated tsx_
- **`validate_web_ir_with_metrics`** — only: _validate_web_ir_with_metrics returns metrics showing view_roots_walked and dom_nodes_traversed counts >= 1_
- **`variable subscript index`** — only: _subscript expression with identifier index (items[i]) is emitted in tsx_
- **`variant_names() method behavior`** — only: _variant_names() returns the list of variant names for a registered ADT and returns empty list for non-existent ADT_
- **`vcs effect check`** — only: _A function using vcs effect can call repo.snapshot() without diagnostic errors_
- **`vox_codegen::web_ir::emit_tsx::emit_component_view_tsx()`** — only: _TSX emission maps raw_class attributes to JSX className property_
- **`vox_codegen::web_ir::validate::validate_web_ir()`** — only: _Web IR validation produces no blocking errors for branch-registry fixture after lowering_
- **`vox_compiler::parser::parse + vox_compiler::hir::lower_module + vox_codegen::codegen_ts::generate`** — only: _Component source with state, derived values, and view hierarchy parses, lowers to HIR, and generates TypeScript without error_
- **`vox_compiler::parser::parse_with_registry`** — only: _parse_with_registry resolves deprecated primitive name (Box) to canonical name (panel)_
- **`vox_compiler::parser::renames::RenameRegistry::load_canonical`** — only: _RenameRegistry loads from contracts/naming/renames.v1.json canonical path without error_
- **`vox_compiler::required_capabilities::project_required_capabilities`** — only: _Endpoint function with 'uses net' effect maps to 'net.http' capability ID_
- **`wcag21_contrast_ratio()`** — only: _wcag21_contrast_ratio() returns approximately 21.0 for maximum contrast (black #000000 on white #ffffff)_
- **`webhook custom provider with secret`** — only: _@webhook(provider: custom, secret: "...") with explicit secret parameter does not emit vox/webhook/missing-secret-var diagnostic_
- **`wire_type_to_ts for Array`** — only: _wire_type_to_ts generates readonly arrays in TypeScript (WireType::Array emits 'readonly' prefix)_
- **`wire_type_to_ts for Ref`** — only: _wire_type_to_ts maps WireType::Ref('User') to the bare name 'User' (not qualified)_
- **`wire_type_to_ts for Tuple`** — only: _wire_type_to_ts converts WireType::Tuple([Number, String]) to TypeScript tuple syntax '[number, string]'_
- **`wire_type_to_ts function`** — only: _wire_type_to_ts maps WireType::Number to 'number', WireType::String to 'string', WireType::Bool to 'boolean', WireType::Unit to 'void', and WireType::Unknown to 'unknown'_
- **`wire_type_to_zod()`** — only: _wire_type_to_zod() maps WireType::Number to "z.number()", WireType::String to "z.string()", WireType::Bool to "z.boolean()", WireType::Unit to "z.void()", and WireType::Unknown to "z.any()"_
- **`with operator on Result type`** — only: _with operator on Result[int] type with timeout and retries parameters produces no type errors_
- **`workflow without non-deterministic calls`** — only: _workflow function with no non-deterministic calls produces empty diagnostics list_
- **`workflow.version() parsing`** — only: _workflow.version call with string change_id and integer min/max arguments parses and collects with correct field values_
- **`write_scaffold_if_missing()`** — only: _skips overwriting existing user files when called idempotently_
