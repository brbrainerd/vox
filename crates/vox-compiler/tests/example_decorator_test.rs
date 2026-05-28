//! Coverage for the `@example` decorator added 2026-05-17.
//!
//! `@example` mirrors `@test`'s surface (optional `(label)`, then a
//! `fn name() to Unit { ... }` body) but lowers into
//! [`HirModule::examples`] rather than `tests` so corpus-mining tooling
//! can enumerate authored reference solutions without scanning the
//! regression-test set.

#[test]
fn example_decl_parses_typechecks_and_lowers_into_examples_vec() {
    let source = r#"
        fn double(n: int) to int {
            return n * 2
        }

        @example
        fn ex_double_works() to Unit {
            assert(double(3) is 6)
        }

        @example("triple via constant")
        fn ex_triple_works() to Unit {
            assert((3 * 3) is 9)
        }

        @test
        fn test_double_negative() to Unit {
            assert(double(-1) is -2)
        }
    "#;
    let res = vox_compiler::pipeline::run_frontend_str(source, "example_test.vox")
        .expect("frontend should succeed");

    // No error-level diagnostics on a clean @example surface.
    let errors: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| {
            matches!(
                d.severity,
                vox_compiler::typeck::diagnostics::TypeckSeverity::Error
            )
        })
        .collect();
    assert!(
        errors.is_empty(),
        "unexpected errors: {:?}",
        errors.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );

    assert_eq!(
        res.hir.examples.len(),
        2,
        "two @example blocks should land in hir.examples; got {}",
        res.hir.examples.len()
    );
    let names: Vec<&str> = res.hir.examples.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"ex_double_works"));
    assert!(names.contains(&"ex_triple_works"));

    // @test must land in hir.tests, not hir.examples — the two surfaces
    // are deliberately separate.
    assert_eq!(
        res.hir.tests.len(),
        1,
        "the @test block should still land in hir.tests"
    );
    assert_eq!(res.hir.tests[0].name, "test_double_negative");
    assert!(
        !res.hir
            .examples
            .iter()
            .any(|f| f.name == "test_double_negative"),
        "@test must not leak into hir.examples"
    );
}

#[test]
fn example_without_label_parses() {
    let source = r#"
        @example
        fn ex_no_label() to Unit {
            assert(true)
        }
    "#;
    let res = vox_compiler::pipeline::run_frontend_str(source, "ex.vox")
        .expect("frontend should succeed");
    assert_eq!(res.hir.examples.len(), 1);
    assert_eq!(res.hir.examples[0].name, "ex_no_label");
}

#[test]
fn example_pre_fix_would_have_failed_to_parse() {
    // Regression: before 2026-05-17 the lexer did not recognize @example,
    // and `vox check` rejected this fixture with "Unexpected token at top
    // level: xample". Keep this test alive so silent removal of the
    // AtExample token is caught.
    let source = r#"
        @example
        fn baseline() to Unit {
            assert(true)
        }
    "#;
    let payloads = vox_compiler::pipeline::check_file(source, "baseline.vox");
    let errors: Vec<_> = payloads
        .iter()
        .filter(|d| {
            matches!(
                d.severity,
                vox_compiler::typeck::diagnostics::TypeckSeverity::Error
            )
        })
        .collect();
    assert!(
        errors.is_empty(),
        "@example should compile clean: {:?}",
        errors.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}
