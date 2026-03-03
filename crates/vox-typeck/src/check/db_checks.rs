use crate::diagnostics::{Diagnostic, Severity};
use crate::env::{BindingKind, TypeEnv};
use crate::ty::Ty;
use vox_ast::decl::TableDecl;
use super::resolve::resolve_type;

pub fn check_table(env: &TypeEnv, diags: &mut Vec<Diagnostic>, t: &TableDecl, source: &str) {
    for field in &t.fields {
        let ty = resolve_type(&field.type_ann, env);
        if !is_db_storable(&ty) {
            diags.push(Diagnostic {
                message: format!("Type '{:?}' for field '{}' cannot be stored in a database table", ty, field.name),
                span: field.span,
                severity: Severity::Error,
                expected_type: None,
                found_type: Some(format!("{:?}", ty)),
                context: Some(Diagnostic::capture_context(source, field.span)),
                suggestions: vec![],
            });
        }
    }
}

pub fn check_collection(env: &TypeEnv, diags: &mut Vec<Diagnostic>, c: &vox_ast::decl::CollectionDecl, source: &str) {
    for field in &c.fields {
        let ty = resolve_type(&field.type_ann, env);
        if !is_db_storable(&ty) {
            diags.push(Diagnostic {
                message: format!("Type '{:?}' for field '{}' cannot be stored in a database collection", ty, field.name),
                span: field.span,
                severity: Severity::Error,
                expected_type: None,
                found_type: Some(format!("{:?}", ty)),
                context: Some(Diagnostic::capture_context(source, field.span)),
                suggestions: vec![],
            });
        }
    }
}

pub fn check_index(env: &TypeEnv, diags: &mut Vec<Diagnostic>, idx: &vox_ast::decl::IndexDecl, source: &str) {
    let binding = match env.lookup(&idx.table_name) {
        Some(b) => b,
        None => {
            diags.push(Diagnostic {
                message: format!("@index references unknown table '{}'", idx.table_name),
                span: idx.span,
                severity: Severity::Error,
                expected_type: None,
                found_type: None,
                context: Some(Diagnostic::capture_context(source, idx.span)),
                suggestions: vec![],
            });
            return;
        }
    };

    let fields = match &binding.ty {
        Ty::Table(_, fields) | Ty::Collection(_, fields) => fields,
        _ => {
            diags.push(Diagnostic {
                message: format!("@index must reference a @table or @collection, but '{}' is not one", idx.table_name),
                span: idx.span,
                severity: Severity::Error,
                expected_type: None,
                found_type: None,
                context: Some(Diagnostic::capture_context(source, idx.span)),
                suggestions: vec![],
            });
            return;
        }
    };

    for col in &idx.columns {
        if !fields.iter().any(|(name, _)| name == col) {
            diags.push(Diagnostic {
                message: format!("Index column '{}' does not exist on table '{}'", col, idx.table_name),
                span: idx.span,
                severity: Severity::Error,
                expected_type: None,
                found_type: None,
                context: Some(Diagnostic::capture_context(source, idx.span)),
                suggestions: fields.iter().map(|(n, _)| n.clone()).collect(),
            });
        }
    }
}

pub fn check_vector_index(env: &TypeEnv, diags: &mut Vec<Diagnostic>, idx: &vox_ast::decl::VectorIndexDecl, source: &str) {
    let binding = match env.lookup(&idx.table_name) {
        Some(b) => b,
        None => {
            diags.push(Diagnostic::error(
                format!("@vector_index references unknown table '{}'", idx.table_name),
                idx.span,
                source,
            ));
            return;
        }
    };

    let fields = match &binding.ty {
        Ty::Table(_, fields) | Ty::Collection(_, fields) => fields,
        _ => {
            diags.push(Diagnostic::error(
                format!("@vector_index must reference a @table or @collection"),
                idx.span,
                source,
            ));
            return;
        }
    };

    if !fields.iter().any(|(name, _)| name == &idx.column) {
        diags.push(Diagnostic::error(
            format!("Vector column '{}' does not exist on table '{}'", idx.column, idx.table_name),
            idx.span,
            source,
        ));
    }
}

pub fn check_search_index(env: &TypeEnv, diags: &mut Vec<Diagnostic>, idx: &vox_ast::decl::SearchIndexDecl, source: &str) {
    let binding = match env.lookup(&idx.table_name) {
        Some(b) => b,
        None => {
            diags.push(Diagnostic::error(
                format!("@search_index references unknown table '{}'", idx.table_name),
                idx.span,
                source,
            ));
            return;
        }
    };

    let fields = match &binding.ty {
        Ty::Table(_, fields) | Ty::Collection(_, fields) => fields,
        _ => {
            diags.push(Diagnostic::error(
                format!("@search_index must reference a @table or @collection"),
                idx.span,
                source,
            ));
            return;
        }
    };

    let mut found_str = false;
    for (name, ty) in fields {
        if name == &idx.search_field {
            let is_str = match ty {
                Ty::Str => true,
                Ty::Named(n) if n == "String" => true,
                _ => false,
            };
            if !is_str {
                diags.push(Diagnostic::error(
                    format!("Search field '{}' must be type 'str' for indexing", idx.search_field),
                    idx.span,
                    source,
                ));
            } else {
                found_str = true;
            }
        }
    }

    if !found_str && !diags.iter().any(|d| d.message.contains("must be type 'str'")) {
        diags.push(Diagnostic::error(
            format!("Search field '{}' does not exist on table '{}'", idx.search_field, idx.table_name),
            idx.span,
            source,
        ));
    }

    for col in &idx.filter_fields {
        if !fields.iter().any(|(name, _)| name == col) {
            diags.push(Diagnostic::error(
                format!("Search filter field '{}' does not exist on table '{}'", col, idx.table_name),
                idx.span,
                source,
            ));
        }
    }
}

pub fn is_db_storable(ty: &Ty) -> bool {
    match ty {
        Ty::Int
        | Ty::Float
        | Ty::Str
        | Ty::Bool
        | Ty::Char
        | Ty::Bytes
        | Ty::Unit
        | Ty::Id(_)
        | Ty::Named(_) => true,
        Ty::Option(inner) | Ty::List(inner) | Ty::Set(inner) | Ty::Stream(inner) => {
            is_db_storable(inner)
        }
        Ty::Map(k, v) => is_db_storable(k) && is_db_storable(v),
        Ty::Tuple(els) | Ty::Union(els) => els.iter().all(is_db_storable),
        Ty::Record(fields) => fields.iter().all(|(_, t)| is_db_storable(t)),
        _ => false,
    }
}
