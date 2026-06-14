use crate::ast::span::Span;
use crate::hir::{HirMatchArm, HirPattern};
use crate::typeck::diagnostics::Diagnostic;
use crate::typeck::env::TypeEnv;
use crate::typeck::ty::Ty;

/// ADT / option-style exhaustiveness (mirrors `check::patterns::check_match_exhaustiveness` for HIR).
///
/// CR-A1: the Bool arm was extracted to `check_bool_exhaustiveness`
/// (12 decision points removed, function now ~8 CC).
pub(crate) fn check_hir_match_exhaustiveness(
    env: &TypeEnv,
    diags: &mut Vec<Diagnostic>,
    subject_ty: &Ty,
    arms: &[HirMatchArm],
    span: Span,
    source: &str,
) {
    let type_name = match subject_ty {
        Ty::Bool => {
            check_bool_exhaustiveness(arms, span, diags, source);
            return;
        }
        Ty::Named(name) => name.as_str(),
        // Built-in sum types: Option (Some/None) and Result (Ok/Err|Error).
        // These are Ty::Option/Ty::Result, never Ty::Named, so they previously
        // fell through unchecked — a non-exhaustive match on the two most-matched
        // types in the language compiled clean. (C2.)
        Ty::Option(_) => {
            check_builtin_match_exhaustiveness(
                arms,
                span,
                diags,
                source,
                "Option",
                &[&["Some"], &["None"]],
            );
            return;
        }
        Ty::Result(_, _) => {
            check_builtin_match_exhaustiveness(
                arms,
                span,
                diags,
                source,
                "Result",
                &[&["Ok"], &["Err", "Error"]],
            );
            return;
        }
        _ => return,
    };

    let adt = match env.lookup_adt(type_name) {
        Some(adt) => adt,
        None => return,
    };

    let mut covered_variants: Vec<String> = Vec::new();
    let mut wildcard_span = None;

    for arm in arms {
        match &arm.pattern {
            HirPattern::Wildcard(s) => {
                if wildcard_span.is_none() {
                    wildcard_span = Some(*s);
                }
            }
            HirPattern::Ident(name, s) => {
                if adt.variants.iter().any(|v| v.name == *name) {
                    covered_variants.push(name.clone());
                } else if wildcard_span.is_none() {
                    wildcard_span = Some(*s);
                }
            }
            HirPattern::Constructor(name, _, _) => {
                covered_variants.push(name.clone());
            }
            HirPattern::Tuple(_, _) | HirPattern::Literal(_, _) => {}
        }
    }

    let missing: Vec<&str> = adt
        .variants
        .iter()
        .filter(|v| !covered_variants.contains(&v.name))
        .map(|v| v.name.as_str())
        .collect();

    if let Some(w_span) = wildcard_span {
        if missing.is_empty() {
            diags.push(Diagnostic::warning(
                format!(
                    "Divergent wildcard: all variants of '{}' are already covered",
                    type_name
                ),
                w_span,
                source,
            ));
        }
        return;
    }

    if !missing.is_empty() {
        let mut d = Diagnostic::error(
            format!(
                "Non-exhaustive match on type '{}'. Missing variant(s): {}",
                type_name,
                missing.join(", ")
            ),
            span,
            source,
        );
        d.missing_cases = missing.iter().map(|s| s.to_string()).collect();
        d.ast_node_kind = Some("MatchExpr".to_string());
        d.code = Some("E0301".into());
        diags.push(d);
    }
}

/// Boolean exhaustiveness helper — extracted from `check_hir_match_exhaustiveness`
/// (CR-A1: the for-loop + pattern match + `!has_wildcard && (!has_true || !has_false)`
/// chain contributed ~12 decision points inline).
fn check_bool_exhaustiveness(
    arms: &[HirMatchArm],
    span: Span,
    diags: &mut Vec<Diagnostic>,
    source: &str,
) {
    let mut has_true = false;
    let mut has_false = false;
    let mut has_wildcard = false;
    for arm in arms {
        match &arm.pattern {
            HirPattern::Literal(lit, _) => {
                if let crate::hir::HirExpr::BoolLit(b, _) = lit.as_ref() {
                    if *b {
                        has_true = true;
                    } else {
                        has_false = true;
                    }
                }
            }
            HirPattern::Wildcard(_) | HirPattern::Ident(_, _) => has_wildcard = true,
            _ => {}
        }
    }
    if !has_wildcard && (!has_true || !has_false) {
        let mut missing = Vec::new();
        if !has_true {
            missing.push("true".to_string());
        }
        if !has_false {
            missing.push("false".to_string());
        }
        let mut d = Diagnostic::error(
            format!(
                "Non-exhaustive match on bool. Missing: {}",
                missing.join(", ")
            ),
            span,
            source,
        );
        d.missing_cases = missing;
        d.code = Some("E0301".into());
        diags.push(d);
    }
}

/// Exhaustiveness for the built-in sum types Option and Result. `required` lists
/// the variant slots; each inner slice holds the acceptable aliases for that slot
/// (Result's error slot is satisfied by `Err` *or* `Error`). A wildcard, or a
/// bare binding identifier that names no known variant (e.g. `other`), covers all
/// remaining cases. Mirrors the ADT path's `E0301` diagnostic.
fn check_builtin_match_exhaustiveness(
    arms: &[HirMatchArm],
    span: Span,
    diags: &mut Vec<Diagnostic>,
    source: &str,
    type_name: &str,
    required: &[&[&str]],
) {
    let mut has_wildcard = false;
    let mut covered: Vec<String> = Vec::new();
    for arm in arms {
        match &arm.pattern {
            HirPattern::Wildcard(_) => has_wildcard = true,
            HirPattern::Ident(name, _) => {
                if required.iter().any(|grp| grp.contains(&name.as_str())) {
                    covered.push(name.clone());
                } else {
                    // A binding identifier matches anything (acts as a wildcard).
                    has_wildcard = true;
                }
            }
            HirPattern::Constructor(name, _, _) => covered.push(name.clone()),
            HirPattern::Tuple(_, _) | HirPattern::Literal(_, _) => {}
        }
    }

    if has_wildcard {
        return;
    }

    let missing: Vec<String> = required
        .iter()
        .filter(|grp| !grp.iter().any(|alias| covered.iter().any(|c| c == alias)))
        .map(|grp| grp[0].to_string())
        .collect();

    if !missing.is_empty() {
        let mut d = Diagnostic::error(
            format!(
                "Non-exhaustive match on type '{}'. Missing variant(s): {}",
                type_name,
                missing.join(", ")
            ),
            span,
            source,
        );
        d.missing_cases = missing;
        d.ast_node_kind = Some("MatchExpr".to_string());
        d.code = Some("E0301".into());
        diags.push(d);
    }
}

#[cfg(test)]
mod semcov_wave1c_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::hir::HirExpr;

    #[test]
    fn builtin_exhaustiveness_reports_missing_option_variant() {
        let sp = Span::new(0, 1);
        // match opt { Some(_) => 0 }  -- missing `None`
        let arm = HirMatchArm {
            pattern: HirPattern::Constructor("Some".to_string(), vec![], sp),
            guard: None,
            body: Box::new(HirExpr::IntLit(0, sp)),
            span: sp,
        };
        let mut diags: Vec<Diagnostic> = Vec::new();
        check_builtin_match_exhaustiveness(
            &[arm],
            sp,
            &mut diags,
            "match opt {}",
            "Option",
            &[&["Some"], &["None"]],
        );
        assert_eq!(diags.len(), 1, "expected one non-exhaustive diagnostic");
        assert_eq!(diags[0].code.as_deref(), Some("E0301"));
        assert_eq!(diags[0].missing_cases, vec!["None".to_string()]);
        assert!(diags[0].message.contains("Option"));
    }

    #[test]
    fn builtin_exhaustiveness_wildcard_binding_suppresses_diagnostic() {
        let sp = Span::new(0, 1);
        // A bare binding identifier (not a known variant) acts as a wildcard.
        let arm = HirMatchArm {
            pattern: HirPattern::Ident("other".to_string(), sp),
            guard: None,
            body: Box::new(HirExpr::IntLit(0, sp)),
            span: sp,
        };
        let mut diags: Vec<Diagnostic> = Vec::new();
        check_builtin_match_exhaustiveness(
            &[arm],
            sp,
            &mut diags,
            "match res {}",
            "Result",
            &[&["Ok"], &["Err", "Error"]],
        );
        assert!(
            diags.is_empty(),
            "binding ident should cover all remaining cases"
        );
    }
}
