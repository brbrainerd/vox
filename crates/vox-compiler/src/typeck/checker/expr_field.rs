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
            Ty::Named(n) if n == "JsonBody" => match field {
                "message" => Ty::Str,
                _ => {
                    self.diags.push(Diagnostic::error(
                        format!("Field '{field}' not found on JsonBody"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }
            },
            Ty::Named(n) if n == "KeyboardEvent" => match field {
                "key" => Ty::Str,
                _ => {
                    self.diags.push(Diagnostic::error(
                        format!("Field '{field}' not found on KeyboardEvent"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }
            },
            Ty::Named(n) if n == "StdNamespace" => std_root_field_ty(field).unwrap_or_else(|| {
                self.diags.push(Diagnostic::error(
                    format!("Unknown std submodule or field '{field}'"),
                    span,
                    self.source,
                ));
                Ty::Error
            }),
            Ty::Named(n) if n == "StdFsNs" => {
                std_namespace_method_ty("fs", field).unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.fs method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                })
            }
            Ty::Named(n) if n == "StdPathNs" => std_namespace_method_ty("path", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.path method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdEnvNs" => std_namespace_method_ty("env", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.env method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdProcessNs" => std_namespace_method_ty("process", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.process method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdJsonNs" => std_namespace_method_ty("json", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.json method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdAgentosNs" => std_namespace_method_ty("agentos", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.agentos method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdCsvNs" => std_namespace_method_ty("csv", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.csv method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdTomlNs" => std_namespace_method_ty("toml", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.toml method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdYamlNs" => std_namespace_method_ty("yaml", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.yaml method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdIoNs" => {
                std_namespace_method_ty("io", field).unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.io method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                })
            }
            Ty::Named(n) if n == "StdHttpNs" => std_namespace_method_ty("http", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.http method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdCryptoNs" => std_namespace_method_ty("crypto", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.crypto method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdTimeNs" => std_namespace_method_ty("time", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.time method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdLogNs" => std_namespace_method_ty("log", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.log method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
            Ty::Named(n) if n == "StdRegexNs" => std_namespace_method_ty("regex", field)
                .unwrap_or_else(|| {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown std.regex method '{field}'"),
                        span,
                        self.source,
                    ));
                    Ty::Error
                }),
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
                if let Some(binding) = self.env.lookup(field) {
                    if binding.kind == BindingKind::Table {
                        binding.ty.clone()
                    } else {
                        self.diags.push(Diagnostic::error(
                            format!("Unknown table '{field}' in database"),
                            span,
                            self.source,
                        ));
                        Ty::Error
                    }
                } else {
                    self.diags.push(Diagnostic::error(
                        format!("Unknown table '{field}' in database"),
                        span,
                        self.source,
                    ));
                    Ty::Error
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
}
