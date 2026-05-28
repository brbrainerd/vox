//! Form validation: cross-checks `@form` declarations against the `@endpoint` they reference.

use crate::hir::HirModule;
use crate::typeck::diagnostics::{Diagnostic, DiagnosticCategory, TypeckSeverity};

/// Render a [`HirType`] as a human-readable string for diagnostic messages.
fn display_hir_ty(ty: &crate::hir::HirType) -> String {
    use crate::hir::HirType;
    match ty {
        HirType::Named(n) => n.clone(),
        HirType::Generic(name, args) => {
            format!(
                "{}[{}]",
                name,
                args.iter()
                    .map(display_hir_ty)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        HirType::Unit => "Unit".into(),
        HirType::Decimal => "decimal".into(),
        HirType::Function(params, ret) => {
            format!(
                "fn({}) -> {}",
                params
                    .iter()
                    .map(display_hir_ty)
                    .collect::<Vec<_>>()
                    .join(", "),
                display_hir_ty(ret)
            )
        }
        HirType::Tuple(ts) => {
            format!(
                "({})",
                ts.iter().map(display_hir_ty).collect::<Vec<_>>().join(", ")
            )
        }
    }
}

/// Check all `@form` declarations in the HIR module.
///
/// Emits diagnostics for:
/// - `vox/form/missing-label` (P0, GA-03) — a non-hidden form field has no
///   label; accessibility-critical, blocks screen-reader and label-association.
/// - `lint.form.unknown_endpoint` — `on_submit` names a function that is not an `@endpoint`.
/// - `lint.form.field_unmatched` — a form field has no matching parameter in the endpoint.
/// - `lint.form.field_type_mismatch` — a form field's type differs from the endpoint parameter type.
pub fn check_forms(hir: &HirModule, _source: &str) -> Vec<Diagnostic> {
    let mut diags = vec![];
    // GA-03 P0: every visible form field must declare a label. No exceptions.
    // Hidden fields with auto-supplied defaults are exempt.
    for form in &hir.forms {
        for field in &form.fields {
            if field.hidden {
                continue;
            }
            if field.label.is_none() {
                diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "@form `{}` field `{}` has no label. Labels are required on every visible field for accessibility.",
                        form.name, field.name
                    ),
                    span: field.span,
                    code: Some("vox/form/missing-label".into()),
                    category: DiagnosticCategory::Typecheck,
                    suggestions: vec![format!(
                        "Add `label: \"…\"` to field `{}`, or mark it `hidden: true` if it must not be user-visible.",
                        field.name
                    )],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: Some("Some(label)".into()),
                    found_type: Some("None".into()),
                    context: Some(format!("@form `{}`", form.name)),
                    ast_node_kind: None,
                });
            }
        }
    }
    for form in &hir.forms {
        if let Some(submit) = &form.on_submit {
            let endpoint = hir.endpoint_fns.iter().find(|e| &e.name == submit);
            if endpoint.is_none() {
                diags.push(Diagnostic {
                    severity: TypeckSeverity::Error,
                    message: format!(
                        "@form `{}` references on_submit `{}` but no @endpoint with that name exists.",
                        form.name, submit
                    ),
                    span: form.span,
                    code: Some("lint.form.unknown_endpoint".into()),
                    category: DiagnosticCategory::Lint,
                    suggestions: vec![],
                    fixes: vec![],
                    line_col: None,
                    missing_cases: vec![],
                    expected_type: None,
                    found_type: None,
                    context: None,
                    ast_node_kind: None,
                });
                continue;
            }
            let ep = endpoint.unwrap();
            // Skip hidden fields that supply their own default — they don't map to endpoint params.
            for vf in form
                .fields
                .iter()
                .filter(|f| !(f.hidden && f.default.is_some()))
            {
                let param = ep.params.iter().find(|p| p.name == vf.name);
                match param {
                    None => diags.push(Diagnostic {
                        severity: TypeckSeverity::Error,
                        message: format!(
                            "@form `{}` field `{}` has no matching parameter in @endpoint `{}`.",
                            form.name, vf.name, submit
                        ),
                        span: vf.span,
                        code: Some("lint.form.field_unmatched".into()),
                        category: DiagnosticCategory::Lint,
                        suggestions: vec![format!(
                            "Add `{}: {}` to @endpoint `{}` or remove the field.",
                            vf.name,
                            display_hir_ty(&vf.ty),
                            submit
                        )],
                        fixes: vec![],
                        line_col: None,
                        missing_cases: vec![],
                        expected_type: None,
                        found_type: None,
                        context: None,
                        ast_node_kind: None,
                    }),
                    Some(p) => {
                        // Compare field type against endpoint param type annotation.
                        // If the param has no type annotation we cannot verify — skip.
                        if let Some(param_ty) = &p.type_ann {
                            if param_ty != &vf.ty {
                                diags.push(Diagnostic {
                                    severity: TypeckSeverity::Error,
                                    message: format!(
                                        "@form `{}` field `{}` has type `{}` but @endpoint `{}` expects `{}`.",
                                        form.name,
                                        vf.name,
                                        display_hir_ty(&vf.ty),
                                        submit,
                                        display_hir_ty(param_ty)
                                    ),
                                    span: vf.span,
                                    code: Some("lint.form.field_type_mismatch".into()),
                                    category: DiagnosticCategory::Lint,
                                    suggestions: vec![],
                                    fixes: vec![],
                                    line_col: None,
                                    missing_cases: vec![],
                                    expected_type: Some(display_hir_ty(param_ty)),
                                    found_type: Some(display_hir_ty(&vf.ty)),
                                    context: None,
                                    ast_node_kind: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    diags
}
