//! Adversarial tests for vox-ast pure functions.
//!
//! Each test targets a specific plausible bug in span arithmetic, type-expr helpers,
//! scalar mapping, pattern/expr span accessors, and display-adjacent invariants.

#[cfg(test)]
mod semcov_wave35_tests {
    use crate::expr::{
        BinOp, Expr, JsxElement, JsxSelfClosingElement, MatchArm, Param, StringPart, UnOp,
        WorkflowVersionCall,
    };
    use crate::pattern::Pattern;
    use crate::scalar_mapping::VoxScalar;
    use crate::span::{Span, byte_offset_to_line_col_zero_based};
    use crate::stmt::Stmt;
    use crate::types::TypeExpr;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn sp(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    fn int_lit(v: i64, start: usize, end: usize) -> Expr {
        Expr::IntLit {
            value: v,
            span: sp(start, end),
        }
    }

    fn ident_expr(name: &str, start: usize, end: usize) -> Expr {
        Expr::Ident {
            name: name.to_owned(),
            span: sp(start, end),
        }
    }

    // ── Span::merge ──────────────────────────────────────────────────────────

    #[test]
    fn span_merge_picks_outer_bounds() {
        // Catches: merge using max(start) / min(end) instead of min(start) / max(end)
        let a = sp(5, 20);
        let b = sp(10, 30);
        let m = a.merge(b);
        assert_eq!(m.start, 5, "merge start should be the smaller start");
        assert_eq!(m.end, 30, "merge end should be the larger end");
    }

    #[test]
    fn span_merge_identical_is_identity() {
        // Catches: merge returning wrong span when both sides are equal
        let a = sp(7, 14);
        assert_eq!(a.merge(a), a);
    }

    #[test]
    fn span_merge_left_contained_in_right() {
        // Catches: off-by-one if merge uses strict inequalities
        let inner = sp(10, 15);
        let outer = sp(0, 100);
        let m = inner.merge(outer);
        assert_eq!(
            m, outer,
            "merging inner into outer must return outer bounds"
        );
    }

    #[test]
    fn span_merge_adjacent_spans_produce_contiguous_range() {
        // Catches: merge not handling adjacent (non-overlapping) spans
        let a = sp(0, 5);
        let b = sp(5, 10);
        let m = a.merge(b);
        assert_eq!(m, sp(0, 10));
    }

    #[test]
    fn span_merge_is_commutative() {
        // Catches: asymmetric merge implementation
        let a = sp(3, 17);
        let b = sp(1, 9);
        assert_eq!(a.merge(b), b.merge(a));
    }

    #[test]
    fn span_zero_length_at_origin() {
        // Catches: special-casing start==end==0 or treating zero-length as invalid
        let z = sp(0, 0);
        assert_eq!(z.start, z.end);
        // merging with itself is a no-op
        assert_eq!(z.merge(z), z);
    }

    // ── byte_offset_to_line_col_zero_based ───────────────────────────────────

    #[test]
    fn line_col_offset_zero_yields_origin() {
        // Catches: off-by-one that returns (0,1) or (1,0) for the first char
        let (line, col) = byte_offset_to_line_col_zero_based("hello", 0);
        assert_eq!((line, col), (0, 0));
    }

    #[test]
    fn line_col_after_newline_resets_col() {
        // Catches: col counter not resetting after '\n'
        let src = "ab\ncd";
        let (line, col) = byte_offset_to_line_col_zero_based(src, 4); // 'c'
        assert_eq!((line, col), (1, 1));
    }

    #[test]
    fn line_col_offset_beyond_len_clamps_to_end() {
        // Catches: panic on out-of-bounds byte_index instead of clamping
        let src = "hi";
        let (line, col) = byte_offset_to_line_col_zero_based(src, 9999);
        // clamped to len=2 → "hi" has no newline, so still line 0
        assert_eq!(line, 0);
        assert_eq!(col, 2);
    }

    #[test]
    fn line_col_multi_byte_char_counts_as_one_column() {
        // Catches: treating multi-byte UTF-8 as multiple columns
        // "é" is 2 bytes (U+00E9); byte offset 2 = character after it
        let src = "é!";
        let (line, col) = byte_offset_to_line_col_zero_based(src, 2); // byte after 'é'
        assert_eq!(
            (line, col),
            (0, 1),
            "multi-byte scalar should count as one col"
        );
    }

    #[test]
    fn line_col_empty_string_offset_zero() {
        // Catches: indexing empty string or off-by-one on char_indices
        let (line, col) = byte_offset_to_line_col_zero_based("", 0);
        assert_eq!((line, col), (0, 0));
    }

    // ── TypeExpr::span ───────────────────────────────────────────────────────

    #[test]
    fn type_expr_span_all_variants_round_trip() {
        // Catches: missing arm in TypeExpr::span() match (would be compile error but
        // also catches logic errors if a new variant defaults to wrong span)
        let s = sp(1, 5);
        let variants: &[TypeExpr] = &[
            TypeExpr::Named {
                name: "int".into(),
                span: s,
            },
            TypeExpr::Generic {
                name: "list".into(),
                args: vec![],
                span: s,
            },
            TypeExpr::Function {
                params: vec![],
                return_type: Box::new(TypeExpr::Unit { span: s }),
                span: s,
            },
            TypeExpr::Tuple {
                elements: vec![],
                span: s,
            },
            TypeExpr::Unit { span: s },
            TypeExpr::Infer { span: s },
            TypeExpr::Decimal { span: s },
        ];
        for v in variants {
            assert_eq!(v.span(), s, "TypeExpr variant span() mismatch for {:?}", v);
        }
    }

    // ── VoxScalar::parse ─────────────────────────────────────────────────────

    #[test]
    fn scalar_parse_rejects_uppercase_names() {
        // Catches: case-insensitive match that would accept "Int" or "INT"
        assert!(VoxScalar::parse("Int").is_none());
        assert!(VoxScalar::parse("INT").is_none());
        assert!(VoxScalar::parse("BOOL").is_none());
    }

    #[test]
    fn scalar_parse_rejects_partial_names() {
        // Catches: prefix match instead of exact match
        assert!(VoxScalar::parse("in").is_none());
        assert!(VoxScalar::parse("boolean").is_none());
        assert!(
            VoxScalar::parse("decimal").is_none(),
            "'decimal' should not match; the keyword is 'dec'"
        );
    }

    #[test]
    fn scalar_parse_dec_not_decimal() {
        // Catches: accepting "decimal" when only "dec" is the Vox keyword
        assert_eq!(VoxScalar::parse("dec"), Some(VoxScalar::Decimal));
        assert!(VoxScalar::parse("decimal").is_none());
    }

    #[test]
    fn scalar_ts_primitive_int_and_float_both_number() {
        // Catches: returning "integer" or "int" for Int in TS target
        assert_eq!(VoxScalar::Int.as_ts_primitive(), "number");
        assert_eq!(VoxScalar::Float.as_ts_primitive(), "number");
    }

    #[test]
    fn scalar_sqlite_bool_maps_to_integer_not_boolean() {
        // Catches: emitting "BOOLEAN" (non-standard in SQLite) instead of "INTEGER"
        assert_eq!(VoxScalar::Bool.as_sqlite_affinity(), "INTEGER");
    }

    #[test]
    fn scalar_sqlite_decimal_maps_to_text() {
        // Catches: mapping Decimal→REAL (lossy) instead of TEXT
        assert_eq!(VoxScalar::Decimal.as_sqlite_affinity(), "TEXT");
    }

    // ── Expr::span ───────────────────────────────────────────────────────────

    #[test]
    fn expr_span_jsx_element_uses_inner_span() {
        // Catches: Expr::Jsx branch returning a default Span instead of el.span
        let s = sp(50, 80);
        let el = JsxElement {
            tag: "div".into(),
            attributes: vec![],
            children: vec![],
            span: s,
        };
        let expr = Expr::Jsx(el);
        assert_eq!(expr.span(), s);
    }

    #[test]
    fn expr_span_jsx_self_closing_uses_inner_span() {
        // Catches: same mistake for JsxSelfClosing variant
        let s = sp(10, 30);
        let el = JsxSelfClosingElement {
            tag: "input".into(),
            attributes: vec![],
            span: s,
        };
        let expr = Expr::JsxSelfClosing(el);
        assert_eq!(expr.span(), s);
    }

    #[test]
    fn expr_span_workflow_version_uses_inner_span() {
        // Catches: WorkflowVersion falling through to a wildcard that ignores c.span
        let s = sp(200, 250);
        let expr = Expr::WorkflowVersion(WorkflowVersionCall {
            change_id: "abc".into(),
            min: 1,
            max: 3,
            span: s,
        });
        assert_eq!(expr.span(), s);
    }

    // ── Pattern::span ────────────────────────────────────────────────────────

    #[test]
    fn pattern_span_all_variants_round_trip() {
        // Catches: missing Wildcard or Literal arm in Pattern::span() returning wrong span
        let s = sp(5, 10);
        let lit_inner = int_lit(0, 5, 10);
        let pats: &[Pattern] = &[
            Pattern::Ident {
                name: "x".into(),
                span: s,
            },
            Pattern::Tuple {
                elements: vec![],
                span: s,
            },
            Pattern::Constructor {
                name: "Ok".into(),
                fields: vec![],
                span: s,
            },
            Pattern::Wildcard { span: s },
            Pattern::Literal {
                value: Box::new(lit_inner),
                span: s,
            },
        ];
        for p in pats {
            assert_eq!(p.span(), s, "Pattern variant span() mismatch for {:?}", p);
        }
    }

    // ── Stmt::span ───────────────────────────────────────────────────────────

    #[test]
    fn stmt_span_break_continue_are_not_phantom() {
        // Catches: Break/Continue arms in Stmt::span() accidentally using a default
        // or compiling with a struct-pattern mismatch
        let s = sp(99, 104);
        assert_eq!(Stmt::Break { span: s }.span(), s);
        assert_eq!(Stmt::Continue { span: s }.span(), s);
    }

    // ── TypeExpr equality ────────────────────────────────────────────────────

    #[test]
    fn type_named_inequality_by_name() {
        // Catches: PartialEq ignoring the name field
        let a = TypeExpr::Named {
            name: "int".into(),
            span: sp(0, 3),
        };
        let b = TypeExpr::Named {
            name: "str".into(),
            span: sp(0, 3),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn type_named_inequality_by_span() {
        // Catches: PartialEq ignoring the span field when names match
        let a = TypeExpr::Named {
            name: "int".into(),
            span: sp(0, 3),
        };
        let b = TypeExpr::Named {
            name: "int".into(),
            span: sp(1, 4),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn type_generic_nested_args_must_all_match() {
        // Catches: shallow PartialEq that only compares the outer name, not args
        let inner_a = TypeExpr::Named {
            name: "int".into(),
            span: sp(5, 8),
        };
        let inner_b = TypeExpr::Named {
            name: "str".into(),
            span: sp(5, 8),
        };
        let outer_a = TypeExpr::Generic {
            name: "list".into(),
            args: vec![inner_a],
            span: sp(0, 9),
        };
        let outer_b = TypeExpr::Generic {
            name: "list".into(),
            args: vec![inner_b],
            span: sp(0, 9),
        };
        assert_ne!(outer_a, outer_b);
    }

    // ── StringPart equality ───────────────────────────────────────────────────

    #[test]
    fn string_part_literal_vs_interpolation_are_distinct() {
        // Catches: PartialEq matching StringPart::Literal("x") == StringPart::Interpolation(ident "x")
        let lit = StringPart::Literal("hello".into());
        let interp = StringPart::Interpolation(Box::new(ident_expr("hello", 0, 5)));
        assert_ne!(lit, interp);
    }

    // ── BinOp / UnOp coverage ─────────────────────────────────────────────────

    #[test]
    fn binop_is_and_isnt_are_distinct() {
        // Catches: BinOp::Is and BinOp::Isnt sharing the same discriminant or PartialEq bug
        assert_ne!(BinOp::Is, BinOp::Isnt);
    }

    #[test]
    fn unop_not_and_neg_are_distinct() {
        // Catches: UnOp variants collapsed into each other
        assert_ne!(UnOp::Not, UnOp::Neg);
    }

    // ── Span new vs fields ────────────────────────────────────────────────────

    #[test]
    fn span_new_stores_start_end_in_correct_fields() {
        // Catches: Span::new swapping start and end arguments
        let s = Span::new(3, 7);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 7);
    }
}
