/// HIR validation pass (A-079).
///
/// Asserts structural invariants that should hold for all valid HIR:
/// - All function/param/type names are non-empty
/// - All actor/agent/table names are non-empty
use crate::hir::*;
use vox_ast::span::Span;

/// A validation diagnostic.
#[derive(Debug)]
pub struct HirValidationError {
    pub message: String,
    pub span: Span,
}

/// Validate structural invariants of a HirModule.
/// Returns a list of validation errors (empty = valid).
pub fn validate_module(module: &HirModule) -> Vec<HirValidationError> {
    let mut errors = Vec::new();

    for f in &module.functions {
        validate_fn(f, "function", &mut errors);
    }
    for q in &module.queries {
        validate_fn(&q.func, "query", &mut errors);
    }
    for m in &module.mutations {
        validate_fn(&m.func, "mutation", &mut errors);
    }
    for a in &module.actions {
        validate_fn(&a.func, "action", &mut errors);
    }
    for s in &module.scheduled {
        validate_fn(&s.func, "scheduled", &mut errors);
    }
    // Workflows have name/params/body directly (not wrapped in HirFn)
    for w in &module.workflows {
        if w.name.is_empty() {
            errors.push(HirValidationError {
                message: "workflow name is empty".into(),
                span: w.span,
            });
        }
        for p in &w.params {
            if p.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty parameter name in workflow '{}'", w.name),
                    span: p.span,
                });
            }
        }
    }
    // Activities have name/params/body directly (not wrapped in HirFn)
    for a in &module.activities {
        if a.name.is_empty() {
            errors.push(HirValidationError {
                message: "activity name is empty".into(),
                span: a.span,
            });
        }
        for p in &a.params {
            if p.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty parameter name in activity '{}'", a.name),
                    span: p.span,
                });
            }
        }
    }
    for actor in &module.actors {
        if actor.name.is_empty() {
            errors.push(HirValidationError {
                message: "Actor name is empty".into(),
                span: actor.span,
            });
        }
    }
    for table in &module.tables {
        if table.name.is_empty() {
            errors.push(HirValidationError {
                message: "Table name is empty".into(),
                span: table.span,
            });
        }
        for field in &table.fields {
            if field.name.is_empty() {
                errors.push(HirValidationError {
                    message: format!("Empty field name in table '{}'", table.name),
                    span: field.span,
                });
            }
        }
    }
    for t in &module.types {
        if t.name.is_empty() {
            errors.push(HirValidationError {
                message: "Type name is empty".into(),
                span: t.span,
            });
        }
    }

    errors
}

fn validate_fn(f: &HirFn, kind: &str, errors: &mut Vec<HirValidationError>) {
    if f.name.is_empty() {
        errors.push(HirValidationError {
            message: format!("{kind} name is empty"),
            span: f.span,
        });
    }
    for p in &f.params {
        if p.name.is_empty() {
            errors.push(HirValidationError {
                message: format!("Empty parameter name in {kind} '{}'", f.name),
                span: p.span,
            });
        }
    }
}
