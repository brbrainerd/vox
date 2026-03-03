use vox_ast::decl::{Decl, FnDecl, Module};
use vox_ast::span::Span;
use vox_ast::stmt::Stmt;
use vox_typeck::diagnostics::Severity;
use vox_typeck::Diagnostic;

pub fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

pub fn module_with_fn(name: &str, body: Vec<Stmt>) -> Module {
    Module {
        declarations: vec![Decl::Function(FnDecl {
            name: name.to_string(),
            generics: vec![],
            params: vec![],
            return_type: None,
            body,
            is_async: false,
            is_deprecated: false,
            is_traced: false,
            is_llm: false,
            llm_model: None,
            is_layout: false,
            is_pure: false,
            is_pub: false,
            is_metric: false,
            metric_name: None,
            is_health: false,
            auth_provider: None,
            roles: vec![],
            cors: None,
            preconditions: vec![],
            span: dummy_span(),
        })],
        span: dummy_span(),
    }
}

pub fn has_error(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

pub fn error_messages(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect()
}
