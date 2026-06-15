//! Structural pipeline-gap regression tests — guard the *named headline bug* of the
//! semantic-coverage initiative, not a leaf utility.
//!
//! Pattern #1 (silent-drop catch-all) from the pipeline-gap audit: the parser emits
//! [`crate::ast::decl::Decl::Const`] for BOTH `const Name = …` and top-level `let name = …`
//! (see `parser/descent/mod.rs`). HIR lowering MUST route that node into `hir.consts`
//! (`hir/lower/mod.rs`), never into the `legacy_ast_nodes` `_ =>` catch-all that emits a
//! `LOWER_UNLOWERED_DECL` "silently dropped" warning. These tests pin that wiring so the
//! binding (and its initializer value) cannot vanish between stages again.
//!
//! Every test carries a `// Catches:` comment naming the concrete regression it guards.

#[cfg(test)]
mod semcov_struct_pipeline_tests {
    use crate::ast::decl::Decl;
    use crate::hir::lower::lower_module;
    use crate::hir::nodes::{HirExpr, HirStmt};
    use crate::lexer::cursor::lex;
    use crate::parser::parse;

    fn lower(src: &str) -> crate::hir::nodes::HirModule {
        let m = parse(lex(src)).unwrap_or_else(|e| panic!("expected parse ok, got: {e:?}"));
        lower_module(&m)
    }

    #[test]
    fn top_level_let_lowers_to_const_not_swallowed_by_catch_all() {
        // Catches: top-level `let` (parsed as Decl::Const) falling into the
        // `legacy_ast_nodes` `_ =>` catch-all in hir/lower/mod.rs and silently vanishing —
        // the named headline pipeline-gap bug.
        let hir = lower("let answer = 42;");
        assert_eq!(
            hir.consts.len(),
            1,
            "top-level let must lower to exactly one HirConst, got {:?}",
            hir.consts
        );
        assert_eq!(hir.consts[0].name, "answer");
        assert!(
            !hir.legacy_ast_nodes
                .iter()
                .any(|d| matches!(d, Decl::Const(_))),
            "the binding must NOT land in the legacy_ast_nodes catch-all"
        );
        assert!(
            !hir.lower_warnings
                .iter()
                .any(|w| w.contains("silently dropped")),
            "no unlowered-decl warning should fire for a top-level let: {:?}",
            hir.lower_warnings
        );
    }

    #[test]
    fn top_level_let_initializer_value_survives_lowering() {
        // Catches: the binding being lowered by NAME while the initializer is dropped or
        // zeroed — a "half-lowered" variant of the silent-drop bug that a name-only
        // assertion would miss.
        let hir = lower("let answer = 42;");
        assert_eq!(hir.consts.len(), 1);
        assert!(
            matches!(hir.consts[0].value, HirExpr::IntLit(42, _)),
            "initializer literal 42 must survive lowering, got {:?}",
            hir.consts[0].value
        );
    }

    #[test]
    fn let_type_annotation_survives_lowering() {
        // Catches: the optional `: Type` annotation on a top-level `let` being dropped
        // during lowering (lower_const maps c.type_ann through lower_type) — a binding can
        // survive by name+value while its declared type silently vanishes.
        let hir = lower("let titled: Int = 7;");
        assert_eq!(
            hir.consts.len(),
            1,
            "annotated let must lower to one HirConst"
        );
        assert_eq!(hir.consts[0].name, "titled");
        assert!(
            hir.consts[0].type_ann.is_some(),
            "declared type annotation must survive lowering, got None"
        );
        assert!(
            matches!(hir.consts[0].value, HirExpr::IntLit(7, _)),
            "initializer must survive, got {:?}",
            hir.consts[0].value
        );
    }

    #[test]
    fn multiple_top_level_lets_none_swallowed() {
        // Catches: the lowering loop dropping a binding when several siblings are present
        // (e.g. an early `continue`/overwrite that loses all but one) — each must survive.
        let hir = lower("let keep = 1;\nlet also = 2;\nlet third = 3;");
        let names: Vec<&str> = hir.consts.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            hir.consts.len(),
            3,
            "all three top-level lets must lower; got {names:?}"
        );
        assert!(
            names.contains(&"keep") && names.contains(&"also") && names.contains(&"third"),
            "every binding name must be present; got {names:?}"
        );
    }

    #[test]
    fn let_string_initializer_survives_lowering() {
        // Catches: a non-numeric initializer (string literal) being dropped or mistyped
        // while numeric ones survive — the value must round-trip through lowering intact,
        // not just for IntLit.
        let hir = lower("let greeting = \"hi\";");
        assert_eq!(hir.consts.len(), 1);
        assert_eq!(hir.consts[0].name, "greeting");
        assert!(
            matches!(hir.consts[0].value, HirExpr::StringLit(ref s, _) if s == "hi"),
            "string initializer must survive lowering, got {:?}",
            hir.consts[0].value
        );
    }

    // ── Pattern #5: half-wired `when {}` blocks ─────────────────────────────
    // A Vox `when src { fetching => … empty => … error e => … ok x => … }`
    // parses to `Expr::AsyncView` (4 distinct Option arm slots) and lowers via
    // independent `.map(...)` lines in hir/lower/expr.rs. Dropping or mis-wiring
    // any single arm during a refactor would silently lose that branch with NO
    // parse error — exactly the "half-wired" gap. This pins all four arms.

    fn lower_when_view() -> crate::hir::nodes::HirAsyncView {
        let hir = lower(
            "fn render(data: Async[Int]) -> Int {\n\
             when data {\n\
             fetching => 0\n\
             empty => 1\n\
             error e => 2\n\
             ok x => 3\n\
             }\n\
             }",
        );
        let f = hir
            .functions
            .iter()
            .find(|f| f.name == "render")
            .expect("render fn must lower");
        f.body
            .iter()
            .find_map(|s| match s {
                HirStmt::Expr {
                    expr: HirExpr::AsyncView(v),
                    ..
                } => Some((**v).clone()),
                _ => None,
            })
            .expect("when{} must lower to HirExpr::AsyncView in the fn body")
    }

    #[test]
    fn when_block_all_four_arms_survive_lowering() {
        // Catches: a refactor dropping or mis-routing one of the four `when` arms'
        // `.map(self.lower_expr)` lines in hir/lower/expr.rs — the branch vanishes
        // with no parse error (half-wired when{}).
        let view = lower_when_view();
        assert!(
            view.missing_arms().is_empty(),
            "no when-arm may be dropped in lowering; missing: {:?}",
            view.missing_arms()
        );
    }

    #[test]
    fn when_block_error_and_ok_bindings_survive_lowering() {
        // Catches: the `error e` / `ok x` arm BODY surviving while its binding name
        // is dropped — the arm would lower but reference an unbound variable.
        let view = lower_when_view();
        assert_eq!(
            view.error_binding.as_deref(),
            Some("e"),
            "error-arm binding must survive lowering"
        );
        assert_eq!(
            view.ok_binding.as_deref(),
            Some("x"),
            "ok-arm binding must survive lowering"
        );
    }

    // ── Pattern #3: context-dependent silent drop ──────────────────────────
    // The `@pure` marker must survive IDENTICALLY whether it decorates a free
    // `fn` (context A) or an `@example`-wrapped fn (context B). The two routes
    // through lowering are different (free fn -> hir.functions; example ->
    // hir.examples via lower_fn), so a decorator-threading refactor could
    // preserve purity in one context and silently drop it in the other.
    // (Note: `to int`, not `-> int`; and the wrapper decorator must precede
    // `@pure` — `@pure` first is a parse error.)

    #[test]
    fn pure_marker_survives_in_both_fn_and_example_contexts() {
        // Catches: @pure honored on a free fn but silently dropped when the same
        // fn is @example-wrapped (context-dependent drop) — asserted as a parity
        // relationship so it fires on divergence in either direction.
        let a = lower("@pure\nfn f() to int { 1 }");
        let fa = a
            .functions
            .iter()
            .find(|f| f.name == "f")
            .expect("free fn f must lower");
        assert!(fa.is_pure, "context A: @pure dropped on free fn");

        let b = lower("@example\n@pure\nfn ef() to int { 1 }");
        assert_eq!(b.examples.len(), 1, "context B: @example fn must lower");
        let eb = &b.examples[0];
        assert_eq!(eb.name, "ef");
        assert_eq!(
            fa.is_pure, eb.is_pure,
            "pattern #3: @pure survives on free fn (A) but is dropped in @example context (B)"
        );
        assert!(
            eb.is_pure,
            "context B: @pure silently dropped on @example fn"
        );
    }

    // ── R5: decorator-order asymmetry (known limitation, executable TODO) ────
    // `@pure` AFTER `@example`/`@test` parses (see the test above); `@pure` BEFORE
    // them is a hard parse error because the top-level dispatch in
    // parser/descent/mod.rs routes a leading `@pure` straight to `parse_fn_decl`,
    // whose decorator loop has no arm for `@example`/`@test`. The real fix is to
    // collect ALL leading decorators first, then dispatch on the decl-kind keyword.
    // This test pins the DESIRED behavior; remove `#[ignore]` once the refactor lands.
    #[test]
    #[ignore = "R5: needs collect-all-leading-decorators-then-dispatch refactor in parser/descent/mod.rs"]
    fn pure_before_example_should_parse_and_stay_pure() {
        // Catches (post-fix): decorator order changing acceptance or dropping @pure.
        let m = parse(lex("@pure\n@example\nfn ef() to int { 1 }")).expect(
            "@pure before @example should parse once decorator collection is order-independent",
        );
        assert_eq!(m.declarations.len(), 1, "must lower to exactly one decl");
    }

    #[test]
    fn traced_marker_survives_lowering() {
        // Catches: @traced parsed but dropped during lowering (no HirFn.is_traced).
        let hir = lower("@traced\nfn f() to int { return 1 }");
        let f = hir.functions.iter().find(|f| f.name == "f").expect("fn f");
        assert!(
            f.is_traced,
            "@traced must survive lowering into HirFn.is_traced"
        );
    }

    #[test]
    fn at_traced_sets_fndecl_is_traced() {
        // Catches: @traced parsed but FnDecl.is_traced not set (decorator-loop gap).
        let m = parse(lex("@traced\nfn f() to int { return 1 }")).expect("parse");
        let f = m
            .declarations
            .iter()
            .find_map(|d| match d {
                Decl::Function(f) => Some(f),
                _ => None,
            })
            .expect("fn decl");
        assert!(f.is_traced, "@traced must set FnDecl.is_traced");
    }
}
