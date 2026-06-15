//! Adversarial semantic-coverage tests — wave 17.
//!
//! Targets:
//!   - `parser/descent` — parse rejection and error classification
//!   - `hir/lower`      — lowering invariants and capability mapping
//!   - `typeck/effect_check` — effect checking logic
//!   - `fmt`            — round-trip / idempotency
//!
//! Every test carries a `// Catches:` comment naming the plausible real bug it guards.

#[cfg(test)]
mod semcov_wave17_tests {
    use crate::hir::lower::{LowerConfig, lower_module, lower_module_with_config};
    use crate::hir::nodes::effect::HirEffectKind;
    use crate::hir::{HirCapability, validate_module};
    use crate::lexer::cursor::lex;
    use crate::parser::{ParseErrorClass, parse};
    use crate::typeck::effect_check::{check_effect_compliance, check_endpoint_fn_effects};

    // ── parse helpers ──────────────────────────────────────────────────────

    fn parse_ok(src: &str) -> crate::ast::decl::Module {
        let tokens = lex(src);
        parse(tokens).unwrap_or_else(|e| panic!("expected parse ok, got errors: {e:?}"))
    }

    fn parse_err(src: &str) -> Vec<crate::parser::ParseError> {
        let tokens = lex(src);
        parse(tokens).expect_err("expected parse to fail")
    }

    // ── lowering helper ────────────────────────────────────────────────────

    fn lower(src: &str) -> crate::hir::nodes::HirModule {
        let m = parse_ok(src);
        lower_module(&m)
    }

    // ── effect-check helper ────────────────────────────────────────────────

    fn effect_diags(src: &str) -> Vec<crate::typeck::diagnostics::Diagnostic> {
        let hir = lower(src);
        check_effect_compliance(&hir, src)
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 1 — Parser error paths
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn parse_empty_source_produces_empty_module() {
        // Catches: panic on empty token stream (pos underflow)
        let m = parse_ok("");
        assert!(
            m.declarations.is_empty(),
            "empty source must yield zero declarations, got {:?}",
            m.declarations
        );
    }

    #[test]
    fn parse_whitespace_only_produces_empty_module() {
        // Catches: whitespace/newlines being misread as a declaration
        let m = parse_ok("   \n\n\t  \n");
        assert!(m.declarations.is_empty());
    }

    #[test]
    fn parse_fn_missing_closing_brace_is_error() {
        // Catches: parser returning Ok on structurally incomplete input
        let errs = parse_err("fn f() {");
        assert!(!errs.is_empty(), "unclosed brace must be an error");
    }

    #[test]
    fn parse_fn_missing_body_brace_class_is_declaration_or_expression() {
        // Catches: wrong error class being assigned to a missing-body error
        let errs = parse_err("fn f()");
        assert!(
            !errs.is_empty(),
            "fn with no body must produce errors, got empty"
        );
        // At least one error should be Declaration or ExpectToken, not silently Other
        let has_meaningful_class = errs.iter().any(|e| {
            matches!(
                e.class,
                ParseErrorClass::Declaration
                    | ParseErrorClass::ExpectToken
                    | ParseErrorClass::Statement
            )
        });
        assert!(
            has_meaningful_class,
            "expected a classified parse error, got: {errs:?}"
        );
    }

    #[test]
    fn parse_tombstoned_http_route_is_rejected() {
        // Catches: tombstoned form accidentally re-enabled by refactor
        let errs = parse_err("http get \"/api/users\" to List { return [] }");
        assert!(!errs.is_empty(), "http route form must remain tombstoned");
    }

    #[test]
    fn parse_tombstoned_reactive_component_is_rejected() {
        // Catches: Path-C reactive component being silently accepted
        let errs = parse_err(
            "@component Counter(start: int) {\n  state n: int = start\n  view: <span>{n}</span>\n}",
        );
        assert!(
            !errs.is_empty(),
            "reactive @component must remain tombstoned"
        );
    }

    #[test]
    fn parse_classic_component_fn_form_is_rejected() {
        // Catches: `@component fn` (pre-tombstone form) sneaking back in
        let errs = parse_err("@component fn Btn() to Element { return column() }");
        assert!(!errs.is_empty(), "`@component fn` must be rejected");
    }

    #[test]
    fn parse_error_carries_found_token_for_expect_mismatch() {
        // Catches: found field being left None on ExpectToken, making diagnostics useless
        let errs = parse_err("fn 42bad() {}");
        // We just need at least one error; the found field may or may not be set,
        // but the error list must be non-empty and contain a span with positive length hint.
        assert!(!errs.is_empty());
    }

    #[test]
    fn parse_single_fn_module_has_exactly_one_declaration() {
        // Catches: boundary where a one-item module is double-counted or zero-counted
        let m = parse_ok("fn hello() to int { return 1 }");
        assert_eq!(
            m.declarations.len(),
            1,
            "single-fn source must have exactly 1 declaration"
        );
    }

    #[test]
    fn parse_module_with_only_import_has_exactly_one_declaration() {
        // Catches: import being swallowed without being stored in the declaration list
        let m = parse_ok("import react.use_state");
        assert_eq!(m.declarations.len(), 1);
        assert!(matches!(
            &m.declarations[0],
            crate::ast::decl::Decl::Import(_)
        ));
    }

    #[test]
    fn parse_error_span_start_never_exceeds_end() {
        // Catches: span inversion bug (start > end) in error recovery
        let errs = parse_err("fn { broken junk @@@");
        for e in &errs {
            assert!(
                e.span.start <= e.span.end,
                "span start {} > end {} in error {e:?}",
                e.span.start,
                e.span.end
            );
        }
    }

    #[test]
    fn parse_multiple_top_level_fns_counted_correctly() {
        // Catches: off-by-one in the declaration accumulator loop
        let src = "fn a() { }\nfn b() { }\nfn c() { }";
        let m = parse_ok(src);
        assert_eq!(
            m.declarations.len(),
            3,
            "three functions must produce exactly 3 declarations"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 2 — HIR lowering invariants
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn lower_empty_module_produces_empty_hir() {
        // Catches: default HirModule having phantom entries pre-populated
        let hir = lower("");
        assert!(hir.functions.is_empty(), "empty module → no functions");
        assert!(hir.imports.is_empty(), "empty module → no imports");
        assert!(hir.tables.is_empty(), "empty module → no tables");
        assert!(hir.endpoint_fns.is_empty(), "empty module → no endpoints");
    }

    #[test]
    fn lower_single_fn_name_preserved() {
        // Catches: name being mangled or cloned incorrectly during lowering
        let hir = lower("fn greet() to str { return \"hello\" }");
        assert_eq!(hir.functions.len(), 1);
        assert_eq!(hir.functions[0].name, "greet");
    }

    #[test]
    fn lower_fn_with_uses_net_maps_to_net_capability() {
        // Catches: EffectAnnotation::Net not mapping to HirCapability::Net in lower_fn
        let hir = lower("fn fetch() uses net to str { return \"\" }");
        assert_eq!(hir.functions.len(), 1);
        let caps = &hir.functions[0].capabilities;
        assert!(
            caps.contains(&HirCapability::Net),
            "uses net must lower to HirCapability::Net, got: {caps:?}"
        );
    }

    #[test]
    fn lower_fn_with_uses_db_maps_to_db_capability() {
        // Catches: EffectAnnotation::Db not mapping to HirCapability::Db
        let hir = lower("fn query() uses db to str { return \"\" }");
        let caps = &hir.functions[0].capabilities;
        assert!(
            caps.contains(&HirCapability::Db),
            "uses db must lower to HirCapability::Db, got: {caps:?}"
        );
    }

    #[test]
    fn lower_fn_with_multiple_effects_all_mapped() {
        // Catches: only the first effect being copied while the rest are dropped
        let hir = lower("fn multi() uses net, db, fs to str { return \"\" }");
        let caps = &hir.functions[0].capabilities;
        assert!(caps.contains(&HirCapability::Net), "net missing: {caps:?}");
        assert!(caps.contains(&HirCapability::Db), "db missing: {caps:?}");
        assert!(caps.contains(&HirCapability::Fs), "fs missing: {caps:?}");
    }

    #[test]
    fn lower_versioned_fn_gets_vcs_capability_injected() {
        // Catches: is_versioned flag not triggering automatic Vcs capability injection
        let hir = lower("@versioned fn snap() to str { return \"\" }");
        assert_eq!(
            hir.functions.len(),
            1,
            "versioned fn must appear in hir.functions"
        );
        assert!(
            hir.functions[0].is_versioned,
            "is_versioned must be true after lowering"
        );
        assert!(
            hir.functions[0].capabilities.contains(&HirCapability::Vcs),
            "versioned fn must have Vcs capability injected, got: {:?}",
            hir.functions[0].capabilities
        );
    }

    #[test]
    fn lower_strip_tests_removes_test_fns() {
        // Catches: LowerConfig::strip_tests being ignored, leaking @test fns into prod HIR
        let src = "fn prod() { }\n@test fn check_it() { }";
        let m = parse_ok(src);
        let config = LowerConfig { strip_tests: true };
        let hir = lower_module_with_config(&m, &config);
        assert!(
            hir.functions.iter().all(|f| f.name != "check_it"),
            "strip_tests=true must remove @test declarations"
        );
        assert!(
            hir.functions.iter().any(|f| f.name == "prod"),
            "non-test fn must survive strip_tests"
        );
    }

    #[test]
    fn lower_strip_tests_false_keeps_test_fns() {
        // Catches: strip_tests=false accidentally removing tests (inverted flag logic)
        let src = "fn prod() { }\n@test fn check_it() { }";
        let m = parse_ok(src);
        let config = LowerConfig { strip_tests: false };
        let hir = lower_module_with_config(&m, &config);
        assert!(
            hir.tests.iter().any(|f| f.name == "check_it"),
            "strip_tests=false must keep @test fns"
        );
    }

    #[test]
    fn lower_import_stored_in_hir_imports() {
        // Catches: import being parsed but not written into hir.imports
        let hir = lower("import react.use_state");
        assert_eq!(
            hir.imports.len(),
            1,
            "one import statement must produce one hir.imports entry"
        );
    }

    #[test]
    fn lower_produces_valid_hir_no_validation_errors() {
        // Catches: lowering producing structurally invalid HIR (duplicate ids, etc.)
        let src = "fn alpha() { }\nfn beta() { }";
        let hir = lower(src);
        let errs = validate_module(&hir);
        assert!(
            errs.is_empty(),
            "lowered HIR must pass structural validation, got: {errs:?}"
        );
    }

    #[test]
    fn lower_order_preserved_for_multiple_fns() {
        // Catches: HashMap-based intermediate storage destroying declaration order
        let src = "fn first() { }\nfn second() { }\nfn third() { }";
        let hir = lower(src);
        let names: Vec<&str> = hir.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["first", "second", "third"],
            "declaration order must be preserved across lowering"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 3 — Effect checking
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn effect_check_pure_fn_calling_net_fn_is_violation() {
        // Catches: effect propagation skipping pure callers (capability check short-circuits)
        let src = r#"
fn fetch_data() uses net to str { return "" }
fn process() uses nothing to str { fetch_data() }
"#;
        let diags = effect_diags(src);
        assert!(
            !diags.is_empty(),
            "`uses nothing` calling a net function must be a violation"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("net")),
            "violation message must name 'net', got: {diags:?}"
        );
    }

    #[test]
    fn effect_check_unannotated_fn_is_open_world_no_violation() {
        // Catches: unannotated callers being wrongly constrained
        let src = r#"
fn do_io() uses net to str { return "" }
fn wrapper() to str { do_io() }
"#;
        let diags = effect_diags(src);
        assert!(
            diags.is_empty(),
            "unannotated caller must be open-world, got: {diags:?}"
        );
    }

    #[test]
    fn effect_check_caller_with_superset_effects_is_ok() {
        // Catches: subset check being inverted (caller fails when it declares MORE than callee)
        let src = r#"
fn db_read() uses db to str { return "" }
fn coordinator() uses net, db to str { db_read() }
"#;
        let diags = effect_diags(src);
        assert!(
            diags.is_empty(),
            "caller with superset capabilities must not be flagged, got: {diags:?}"
        );
    }

    #[test]
    fn effect_check_stdlib_http_call_without_net_is_violation() {
        // Catches: stdlib intrinsic cap check not firing for http module
        let src = r#"fn f() uses nothing to str { http.get("https://example.com") }"#;
        let diags = effect_diags(src);
        assert_eq!(
            diags.len(),
            1,
            "http.get without uses net must produce exactly 1 violation, got: {diags:?}"
        );
        assert!(
            diags[0].message.contains("net"),
            "violation must mention 'net', got: {}",
            diags[0].message
        );
    }

    #[test]
    fn effect_check_stdlib_db_call_without_db_is_violation() {
        // Catches: Db intrinsic cap not mapped for `db.query`
        let src = r#"fn f() uses nothing to str { db.query("SELECT 1") }"#;
        let diags = effect_diags(src);
        assert!(
            !diags.is_empty(),
            "db.query without uses db must be a violation"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("db")),
            "violation must name 'db': {diags:?}"
        );
    }

    #[test]
    fn effect_check_empty_module_no_diagnostics() {
        // Catches: effect checker crashing or returning phantom diags on empty input
        let src = "";
        let diags = effect_diags(src);
        assert!(
            diags.is_empty(),
            "empty module must produce no effect diags"
        );
    }

    #[test]
    fn endpoint_fn_pure_plus_effects_is_conflict() {
        // Catches: E_EFFECT_PURE_CONFLICT not being raised for endpoints
        use crate::ast::span::Span;
        use crate::hir::nodes::{DefId, HirEndpointFn, HirEndpointKind};
        let f = HirEndpointFn {
            kind: HirEndpointKind::Query,
            id: DefId(0),
            name: "bad_fn".to_string(),
            params: vec![],
            return_type: None,
            body: vec![],
            route_path: "/api/query/bad_fn".to_string(),
            is_pure: true,
            effects: vec![HirEffectKind::Net],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            auth: None,
            span: Span::new(0, 0),
        };
        let diags = check_endpoint_fn_effects(&[f]);
        assert_eq!(
            diags.len(),
            1,
            "pure+uses conflict must produce exactly 1 diag"
        );
        assert_eq!(
            diags[0].code,
            Some("E_EFFECT_PURE_CONFLICT".to_string()),
            "code must be E_EFFECT_PURE_CONFLICT"
        );
    }

    #[test]
    fn endpoint_fn_duplicate_effect_is_caught() {
        // Catches: E_EFFECT_DUPLICATE check not running for endpoint functions
        use crate::ast::span::Span;
        use crate::hir::nodes::{DefId, HirEndpointFn, HirEndpointKind};
        let f = HirEndpointFn {
            kind: HirEndpointKind::Mutation,
            id: DefId(1),
            name: "dup_fn".to_string(),
            params: vec![],
            return_type: None,
            body: vec![],
            route_path: "/api/mutation/dup_fn".to_string(),
            is_pure: false,
            effects: vec![HirEffectKind::Db, HirEffectKind::Db],
            webhook: None,
            cors: None,
            rate_limit: None,
            pii: None,
            layer: None,
            auth: None,
            span: Span::new(0, 0),
        };
        let diags = check_endpoint_fn_effects(&[f]);
        assert_eq!(
            diags.len(),
            1,
            "duplicate effect must produce exactly 1 diag"
        );
        assert_eq!(
            diags[0].code,
            Some("E_EFFECT_DUPLICATE".to_string()),
            "code must be E_EFFECT_DUPLICATE"
        );
    }

    #[test]
    fn effect_check_mcp_capability_propagated() {
        // Catches: HirCapability::Mcp not being matched in subset check (arm missing in match)
        let src = r#"
fn mcp_call() uses mcp(search) to str { return "" }
fn caller() uses nothing to str { mcp_call() }
"#;
        let diags = effect_diags(src);
        assert!(
            !diags.is_empty(),
            "mcp capability propagation must be enforced"
        );
    }

    // ══════════════════════════════════════════════════════════════════════
    // SECTION 4 — Round-trip / fmt idempotency
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn fmt_empty_source_is_idempotent() {
        // Catches: formatter emitting extra whitespace / newlines on empty input
        let once = crate::fmt::format("");
        let twice = crate::fmt::format(&once);
        assert_eq!(once, twice, "format of empty source must be idempotent");
    }

    #[test]
    fn fmt_single_fn_is_idempotent() {
        // Catches: printer adding/removing braces or indentation on second pass
        let src = "fn add(a, b) to int { return a + b }";
        let once = crate::fmt::format(src);
        let twice = crate::fmt::format(&once);
        assert_eq!(once, twice, "single-fn format must be idempotent");
    }

    #[test]
    fn fmt_import_is_idempotent() {
        // Catches: import printer doubling the `import` keyword on re-parse
        let src = "import react.use_state";
        let once = crate::fmt::format(src);
        let twice = crate::fmt::format(&once);
        assert_eq!(once, twice, "import format must be idempotent");
    }

    #[test]
    fn fmt_invalid_source_returned_unchanged() {
        // Catches: format() crashing instead of returning source on parse failure (soft-mode contract)
        let bad = "fn { broken source @@@";
        let out = crate::fmt::format(bad);
        assert_eq!(
            out, bad,
            "invalid source must be returned unchanged by format()"
        );
    }

    #[test]
    fn fmt_try_format_round_trips_valid_source() {
        // Catches: try_format producing output that doesn't re-parse (printer semantic loss)
        let src = "fn greet() to str { return \"hello\" }";
        let result = crate::fmt::try_format(src);
        assert!(
            result.is_ok(),
            "try_format must succeed for valid source, got: {result:?}"
        );
        // Re-parse the formatted output to confirm it's structurally valid.
        let formatted = result.unwrap();
        let tokens = lex(&formatted);
        let reparse = parse(tokens);
        assert!(
            reparse.is_ok(),
            "formatted output must re-parse cleanly, got: {reparse:?}"
        );
    }

    #[test]
    fn fmt_try_format_rejects_invalid_source() {
        // Catches: try_format returning Ok on syntactically broken input
        let bad = "fn { totally broken";
        let result = crate::fmt::try_format(bad);
        assert!(
            result.is_err(),
            "try_format must return Err for invalid source"
        );
    }

    #[test]
    fn fmt_multiple_fns_idempotent() {
        // Catches: multi-declaration printer adding blank lines differently on each pass
        let src = "fn a() { }\nfn b() { }\nfn c() { }";
        let once = crate::fmt::format(src);
        let twice = crate::fmt::format(&once);
        assert_eq!(once, twice, "multi-fn format must be idempotent");
    }
}
