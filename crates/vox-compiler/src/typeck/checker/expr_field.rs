use crate::ast::span::Span;
use crate::builtin_registry::{std_namespace_method_ty, std_root_field_ty};
use crate::hir::HirExpr;
use crate::rust_interop_support::classify_rust_crate;
use crate::typeck::diagnostics::Diagnostic;
use crate::typeck::env::BindingKind;
use crate::typeck::ty::Ty;

use super::Checker;

impl<'a> Checker<'a> {
    pub(super) fn check_expr_field_access(
        &mut self,
        object: &HirExpr,
        field: &str,
        span: Span,
    ) -> Ty {
        let raw_obj = self.check_expr(object, None);
        let obj_ty = self.uf.resolve(&raw_obj);
        match &obj_ty {
            Ty::Named(n) if n == "JsonBody" => {
                self.check_single_str_field("JsonBody", field, "message", span)
            }
            Ty::Named(n) if n == "KeyboardEvent" => {
                self.check_single_str_field("KeyboardEvent", field, "key", span)
            }
            // Std-namespace dispatch — collapsed from 17 near-duplicate
            // match arms into a table-driven lookup. Each StdXxxNs name
            // maps to a sub-namespace key (e.g. "StdFsNs" → "fs"); we
            // delegate to std_namespace_method_ty and emit a uniform
            // diagnostic on miss. Per CR-A1 plan §5.6 refactoring pass.
            Ty::Named(n) if n == "StdNamespace" => std_root_field_ty(field).unwrap_or_else(|| {
                self.diags.push(Diagnostic::error(
                    format!("Unknown std submodule or field '{field}'"),
                    span,
                    self.source,
                ));
                Ty::Error
            }),
            Ty::Named(n) if std_namespace_short_name(n).is_some() => {
                let ns = std_namespace_short_name(n).expect("guard");
                self.lookup_std_namespace_field(ns, field, span)
            }
            Ty::Named(n) if n.starts_with("RustCrate::") => {
                let crate_name = n.trim_start_matches("RustCrate::");
                let support = classify_rust_crate(crate_name).as_label();
                self.diags.push(Diagnostic::error(
                    format!(
                        "Unknown item '{field}' in rust crate '{crate_name}' (support_class: '{support}'). Add a wrapper/binding or use supported Vox surfaces."
                    ),
                    span,
                    self.source,
                ));
                Ty::Error
            }
            Ty::Record(fields) | Ty::Table(_, fields) | Ty::Collection(_, fields) => fields
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t.clone())
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Field '{field}' not found on {obj_ty:?}"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            // Struct types declared as `type Foo { f: T, ... }` register an AdtDef
            // with non-empty `fields` and empty `variants`. Field access on a value
            // of `Ty::Named(Foo)` resolves to the declared field type.
            Ty::Named(n) if self.env.lookup_adt(n).is_some_and(|a| !a.fields.is_empty()) => {
                let adt = self.env.lookup_adt(n).unwrap();
                if let Some((_, t)) = adt.fields.iter().find(|(fn_, _)| fn_ == field) {
                    t.clone()
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Field '{field}' not found on struct {n}"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }
            }
            // `@table type Foo { f: T }` registers a Table binding. A function
            // parameter `p: Foo` resolves to `Ty::Named("Foo")` during registration
            // (tables are registered after functions in Pass 1). In Pass 2 body
            // checking, the Table binding is live — look up its fields directly.
            Ty::Named(n)
                if self
                    .env
                    .lookup(n)
                    .is_some_and(|b| b.kind == BindingKind::Table) =>
            {
                let table_ty = self.env.lookup(n).unwrap().ty.clone();
                if let Ty::Table(_, fields) | Ty::Collection(_, fields) = table_ty {
                    if let Some((_, ft)) = fields.iter().find(|(fn_, _)| fn_ == field) {
                        ft.clone()
                    } else {
                        self.diags.push(Diagnostic::error(
                            format!("Field '{field}' not found on table type {n}"),
                            span,
                            self.source,
                        ));
                        Ty::Error
                    }
                } else {
                    // Should not happen: Table binding has non-Table type.
                    Ty::Error
                }
            }
            Ty::Database => {
                // CR-A1: collapse two identical error-emit branches into one.
                match self.env.lookup(field) {
                    Some(binding) if binding.kind == BindingKind::Table => binding.ty.clone(),
                    _ => {
                        self.diags.push(Diagnostic::error(
                            format!("Unknown table '{field}' in database"),
                            span,
                            self.source,
                        ));
                        Ty::Error
                    }
                }
            }
            Ty::TypeVar(_) => {
                let ret_var = self.uf.fresh_var();
                self.uf.pending_constraints.push(
                    crate::typeck::unify::PendingConstraint::HasField {
                        target: obj_ty.clone(),
                        field: field.to_string(),
                        result: ret_var.clone(),
                        span,
                    },
                );
                ret_var
            }
            // `any` is an escape hatch — field access always succeeds and
            // produces another `any`. This mirrors TypeScript's semantics.
            Ty::Named(n) if n == "any" => Ty::Named("any".to_string()),
            Ty::Error => Ty::Error,
            _ => {
                self.diags.push(Diagnostic::error(
                    format!("Cannot access field '{field}' on {obj_ty:?}"),
                    span,
                    self.source,
                ));
                Ty::Error
            }
        }
    }

    /// Resolve a `std.<ns>.<field>` lookup. Centralizes the previously-
    /// duplicated diagnostic emission so the 16 std-namespace types share
    /// one well-tested path.
    fn lookup_std_namespace_field(&mut self, ns: &str, field: &str, span: Span) -> Ty {
        match std_namespace_method_ty(ns, field) {
            Some(ty) => ty,
            None => {
                self.diags.push(Diagnostic::error(
                    format!("Unknown std.{ns} method '{field}'"),
                    span,
                    self.source,
                ));
                Ty::Error
            }
        }
    }
    /// Check field access on a named type that exposes exactly one `Str` field.
    ///
    /// CR-A1: extracted from `check_expr_field_access` — each inline
    /// `match field { ... }` contributed 1 DP (the match keyword);
    /// sharing them here removes 2 DPs from the caller.
    fn check_single_str_field(
        &mut self,
        type_name: &str,
        field: &str,
        expected: &str,
        span: Span,
    ) -> Ty {
        if field == expected {
            Ty::Str
        } else {
            self.diags.push(Diagnostic::error(
                format!("Field '{field}' not found on {type_name}"),
                span,
                self.source,
            ));
            Ty::Error
        }
    }
}

/// Map an `StdXxxNs` HIR type name to its `std.xxx` short namespace key.
/// Returns `None` for non-std names. Kept as a free function rather than
/// a method so the table lookup is `pub(super)`-callable without
/// requiring a `&self` borrow.
fn std_namespace_short_name(named: &str) -> Option<&'static str> {
    match named {
        "StdFsNs" => Some("fs"),
        "StdPathNs" => Some("path"),
        "StdEnvNs" => Some("env"),
        "StdProcessNs" => Some("process"),
        "StdJsonNs" => Some("json"),
        "StdAgentosNs" => Some("agentos"),
        "StdCsvNs" => Some("csv"),
        "StdTomlNs" => Some("toml"),
        "StdYamlNs" => Some("yaml"),
        "StdIoNs" => Some("io"),
        "StdHttpNs" => Some("http"),
        "StdCryptoNs" => Some("crypto"),
        "StdTimeNs" => Some("time"),
        "StdLogNs" => Some("log"),
        "StdRegexNs" => Some("regex"),
        _ => None,
    }
}
