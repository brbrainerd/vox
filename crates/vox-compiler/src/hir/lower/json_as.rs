//! HIR synthesis for `@json_as(...)` decorated type definitions.
//!
//! RFC: json-as-rfc-2026-05-24.md §6 — Compile-time codegen.
//!
//! For each `@json_as`-annotated `type` declaration this pass emits two `HirFn`
//! nodes that are drained into `HirModule::functions` by the main lowering loop:
//!
//! * `<TypeName>_from_json(j: Json) -> Result[<TypeName>]`
//! * `<TypeName>_to_json(v: <TypeName>) -> Json`
//!
//! Both functions are public and are registered in the caller's scope exactly
//! like any user-defined function — no special-casing is needed in typeck, eval,
//! or codegen (RFC §6 step 5: "Registered in the importer's scope").
//!
//! ## Naming
//!
//! The generated functions use `<TypeName>_from_json` / `<TypeName>_to_json` as
//! their Vox identifier, mirroring the `T::from_json(j)` call-site syntax from
//! the RFC which is desugared by the intra-project-imports namespace resolver.

use crate::ast::decl::typedef::{JsonAsAnnotation, TypeDefDecl, VariantField};
use crate::ast::span::Span;
use crate::ast::types::TypeExpr;
use crate::hir::*;

use super::LowerCtx;

// ──────────────────────────────────────────────────────────────────────────────
// Public entry point
// ──────────────────────────────────────────────────────────────────────────────

/// Synthesise the `from_json` / `to_json` pair for a `@json_as`-annotated type.
///
/// Returns an empty vec when the type has no `@json_as` annotation.
pub(crate) fn synthesise_json_as_fns(ctx: &mut LowerCtx, t: &TypeDefDecl) -> Vec<HirFn> {
    let ann = match &t.json_as {
        Some(a) => a,
        None => return vec![],
    };

    let span = t.span;
    vec![
        build_from_json(ctx, t, ann, span),
        build_to_json(ctx, t, ann, span),
    ]
}

// ──────────────────────────────────────────────────────────────────────────────
// from_json  (<TypeName>_from_json(j: Json) -> Result[<TypeName>])
// ──────────────────────────────────────────────────────────────────────────────

fn build_from_json(
    ctx: &mut LowerCtx,
    t: &TypeDefDecl,
    ann: &JsonAsAnnotation,
    span: Span,
) -> HirFn {
    let fn_name = format!("{}_from_json", t.name);
    let fn_id = ctx.def_map.define(fn_name.clone());

    let j_id = ctx.def_map.define("j".to_string());
    let param_j = HirParam {
        id: j_id,
        name: "j".to_string(),
        type_ann: Some(HirType::Named("Json".to_string())),
        default: None,
        span,
    };

    let body = if !t.variants.is_empty() && ann.tag.is_some() {
        // ADT / tagged enum
        build_from_json_adt(ctx, t, ann, span)
    } else if !t.fields.is_empty() {
        // Struct / product type
        build_from_json_struct(ctx, t, ann, span)
    } else {
        // Type alias or otherwise empty — pass through the raw Json unchanged
        vec![ret(expr_ok(expr_ident("j", span), span), span)]
    };

    HirFn {
        id: fn_id,
        name: fn_name,
        generics: vec![],
        params: vec![param_j],
        return_type: Some(HirType::Generic(
            "Result".to_string(),
            vec![HirType::Named(t.name.clone())],
        )),
        body,
        is_pub: t.is_pub,
        is_async: false,
        is_mobile_native: false,
        is_pure: false,
        is_reactive: false,
        capabilities: vec![],
        is_remote: false,
        is_llm: false,
        llm_model: None,
        ai_structured_output: None,
        ai_fixture: None,
        embed: None,
        is_deprecated: false,
        schedule_interval: None,
        durability: None,
        actor_state_fields: vec![],
        postconditions: vec![],
        ts_extern_module: None,
        generated_hash: None,
        span,
        inference_model: None,
        training_step: false,
        distributed_train: None,
    }
}

/// Build a `from_json` body for a struct (product type).
fn build_from_json_struct(
    ctx: &mut LowerCtx,
    t: &TypeDefDecl,
    ann: &JsonAsAnnotation,
    span: Span,
) -> Vec<HirStmt> {
    let mut stmts: Vec<HirStmt> = Vec::new();
    let mut field_names: Vec<String> = Vec::new();

    for field in &t.fields {
        let json_key = resolve_key(field, &ann.naming);
        field_names.push(field.name.clone());
        emit_field_stmts(ctx, &mut stmts, field, &json_key, &t.name, ann, span);
    }

    // Return Ok(TypeName { field1: field1, ... })
    let obj = HirExpr::ObjectLit(
        field_names
            .iter()
            .map(|n| (n.clone(), expr_ident(n, span)))
            .collect(),
        span,
    );
    stmts.push(ret(expr_ok(obj, span), span));
    stmts
}

/// Build a `from_json` body for a tagged-enum ADT (RFC §4.4).
fn build_from_json_adt(
    ctx: &mut LowerCtx,
    t: &TypeDefDecl,
    ann: &JsonAsAnnotation,
    span: Span,
) -> Vec<HirStmt> {
    let tag_field = ann.tag.as_deref().unwrap_or("kind");
    let mut stmts: Vec<HirStmt> = Vec::new();

    // let _tag = j.get("<tag_field>").and_then(fn(_jv) { _jv.as_str() })
    stmts.push(let_stmt(
        "_tag",
        expr_get_and_then(ctx, "j", tag_field, "as_str", span),
        span,
    ));
    // if _tag.is_none() { return Err("...") }
    stmts.push(guard_none(
        "_tag",
        &format!(
            "json_as {}: missing tag field '{}' at path \"/\"",
            t.name, tag_field
        ),
        span,
    ));
    // let __tag = _tag.unwrap()
    stmts.push(let_stmt(
        "__tag",
        expr_method0("_tag", "unwrap", span),
        span,
    ));

    // For each variant emit: if __tag is "VariantName" { ... return Ok(...) }
    let variant_names: Vec<String> = t.variants.iter().map(|v| v.name.clone()).collect();
    for variant in &t.variants {
        let cond = HirExpr::Binary(
            HirBinOp::Is,
            Box::new(expr_ident("__tag", span)),
            Box::new(HirExpr::StringLit(variant.name.clone(), span)),
            span,
        );

        let mut branch: Vec<HirStmt> = Vec::new();
        let mut vfield_names: Vec<String> = Vec::new();

        for field in &variant.fields {
            let json_key = resolve_key(field, &ann.naming);
            vfield_names.push(field.name.clone());
            emit_field_stmts(ctx, &mut branch, field, &json_key, &t.name, ann, span);
        }

        // Return Ok(TypeName::VariantName { f1: f1, ... }) or Ok(TypeName::VariantName) for unit
        let variant_value = if vfield_names.is_empty() {
            // Unit variant: just the constructor identifier
            expr_ident(&format!("{}::{}", t.name, variant.name), span)
        } else {
            HirExpr::ObjectLit(
                vfield_names
                    .iter()
                    .map(|n| (n.clone(), expr_ident(n, span)))
                    .collect(),
                span,
            )
        };
        branch.push(ret(expr_ok(variant_value, span), span));

        stmts.push(HirStmt::Expr {
            expr: HirExpr::If(Box::new(cond), branch, None, span),
            span,
        });
    }

    // Fallback: unknown tag value
    stmts.push(ret(
        expr_err(
            &format!(
                "json_as {}: unknown tag value at \"/{}\"; expected one of: {}",
                t.name,
                tag_field,
                variant_names.join(", ")
            ),
            span,
        ),
        span,
    ));
    stmts
}

/// Emit the HIR statements required to extract one field from the JSON object `j`.
///
/// For required scalars:
/// ```text
/// let _f_name = j.get("key").and_then(fn(_jv) { _jv.as_TYPE() })
/// if _f_name.is_none() { return Err("...") }
/// let name = _f_name.unwrap()
/// ```
///
/// For `Option[T]`:
/// ```text
/// let name = j.get("key").and_then(fn(_jv) { _jv.as_TYPE() })
/// ```
///
/// For `Json`:
/// ```text
/// let _f_name = j.get("key")
/// if _f_name.is_none() { return Err("...") }
/// let name = _f_name.unwrap()
/// ```
fn emit_field_stmts(
    ctx: &mut LowerCtx,
    stmts: &mut Vec<HirStmt>,
    field: &VariantField,
    json_key: &str,
    type_name: &str,
    ann: &JsonAsAnnotation,
    span: Span,
) {
    let missing_err = format!(
        "json_as {}: missing required field '{}' at path \"/\"",
        type_name, json_key
    );

    if is_option_type(&field.type_ann) {
        // Option[T] — absent or null → None; no error required
        let inner_method = inner_option_as_method(&field.type_ann);
        stmts.push(let_stmt(
            &field.name,
            expr_get_and_then(ctx, "j", json_key, &inner_method, span),
            span,
        ));
        return;
    }

    if is_json_type(&field.type_ann) {
        // Json field — required; no type conversion needed
        let tmp = format!("_f_{}", field.name);
        stmts.push(let_stmt(&tmp, expr_get("j", json_key, span), span));
        stmts.push(guard_none(&tmp, &missing_err, span));
        stmts.push(let_stmt(
            &field.name,
            expr_method0(&tmp, "unwrap", span),
            span,
        ));
        return;
    }

    if let Some(as_method) = as_method_for_type(&field.type_ann) {
        let tmp = format!("_f_{}", field.name);
        stmts.push(let_stmt(
            &tmp,
            expr_get_and_then(ctx, "j", json_key, &as_method, span),
            span,
        ));

        if let Some(ref default_src) = field.json_as_attr.default_expr {
            // @default(expr) — emit unwrap_or with the source expression as a string literal
            // (v1 simplification: only literal string defaults are faithfully round-tripped)
            stmts.push(let_stmt(
                &field.name,
                expr_method1(
                    &tmp,
                    "unwrap_or",
                    HirExpr::StringLit(default_src.clone(), span),
                    span,
                ),
                span,
            ));
        } else if ann.defaults {
            // defaults: true — use the type's zero value when the field is absent
            stmts.push(let_stmt(
                &field.name,
                expr_method1(&tmp, "unwrap_or", scalar_zero(&field.type_ann, span), span),
                span,
            ));
        } else {
            // Required field — guard on None then unwrap
            stmts.push(guard_none(&tmp, &missing_err, span));
            stmts.push(let_stmt(
                &field.name,
                expr_method0(&tmp, "unwrap", span),
                span,
            ));
        }
        return;
    }

    // Unknown / list / nested generic — fall back: get raw Json value, require it present
    let tmp = format!("_f_{}", field.name);
    stmts.push(let_stmt(&tmp, expr_get("j", json_key, span), span));
    stmts.push(guard_none(&tmp, &missing_err, span));
    stmts.push(let_stmt(
        &field.name,
        expr_method0(&tmp, "unwrap", span),
        span,
    ));
}

// ──────────────────────────────────────────────────────────────────────────────
// to_json  (<TypeName>_to_json(v: <TypeName>) -> Json)
// ──────────────────────────────────────────────────────────────────────────────

fn build_to_json(ctx: &mut LowerCtx, t: &TypeDefDecl, ann: &JsonAsAnnotation, span: Span) -> HirFn {
    let fn_name = format!("{}_to_json", t.name);
    let fn_id = ctx.def_map.define(fn_name.clone());

    let v_id = ctx.def_map.define("v".to_string());
    let param_v = HirParam {
        id: v_id,
        name: "v".to_string(),
        type_ann: Some(HirType::Named(t.name.clone())),
        default: None,
        span,
    };

    let body = if !t.fields.is_empty() {
        // Struct: emit { "key": v.field, ... }
        let pairs: Vec<(String, HirExpr)> = t
            .fields
            .iter()
            .map(|f| {
                let key = resolve_key(f, &ann.naming);
                let val =
                    HirExpr::FieldAccess(Box::new(expr_ident("v", span)), f.name.clone(), span);
                (key, val)
            })
            .collect();
        vec![ret(HirExpr::ObjectLit(pairs, span), span)]
    } else {
        // ADT full serialisation is deferred to v2 (RFC §8).
        // For now emit a minimal { "_type": "<TypeName>" } sentinel so callers
        // get a valid Json back rather than crashing.
        vec![ret(
            HirExpr::ObjectLit(
                vec![(
                    "_type".to_string(),
                    HirExpr::StringLit(t.name.clone(), span),
                )],
                span,
            ),
            span,
        )]
    };

    HirFn {
        id: fn_id,
        name: fn_name,
        generics: vec![],
        params: vec![param_v],
        return_type: Some(HirType::Named("Json".to_string())),
        body,
        is_pub: t.is_pub,
        is_async: false,
        is_mobile_native: false,
        is_pure: false,
        is_reactive: false,
        capabilities: vec![],
        is_remote: false,
        is_llm: false,
        llm_model: None,
        ai_structured_output: None,
        ai_fixture: None,
        embed: None,
        is_deprecated: false,
        schedule_interval: None,
        durability: None,
        actor_state_fields: vec![],
        postconditions: vec![],
        ts_extern_module: None,
        generated_hash: None,
        span,
        inference_model: None,
        training_step: false,
        distributed_train: None,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Type inspection helpers
// ──────────────────────────────────────────────────────────────────────────────

fn is_option_type(t: &TypeExpr) -> bool {
    matches!(t, TypeExpr::Generic { name, .. } if name == "Option")
}

fn is_json_type(t: &TypeExpr) -> bool {
    matches!(t, TypeExpr::Named { name, .. } if name == "Json")
}

/// The `.as_X()` method name for a known scalar type; `None` for unknowns.
fn as_method_for_type(t: &TypeExpr) -> Option<String> {
    if let TypeExpr::Named { name, .. } = t {
        return match name.as_str() {
            "str" => Some("as_str".to_string()),
            "int" => Some("as_int".to_string()),
            "float" => Some("as_float".to_string()),
            "bool" => Some("as_bool".to_string()),
            _ => None,
        };
    }
    None
}

/// For `Option[T]`, the `.as_X()` method of the inner type `T`.
fn inner_option_as_method(t: &TypeExpr) -> String {
    if let TypeExpr::Generic { name, args, .. } = t
        && name == "Option"
            && let Some(inner) = args.first() {
                return as_method_for_type(inner).unwrap_or_else(|| "as_str".to_string());
            }
    "as_str".to_string()
}

/// Zero-value default for a scalar type (used when `defaults: true`).
fn scalar_zero(t: &TypeExpr, span: Span) -> HirExpr {
    if let TypeExpr::Named { name, .. } = t {
        return match name.as_str() {
            "int" => HirExpr::IntLit(0, span),
            "float" => HirExpr::FloatLit(0.0, span),
            "bool" => HirExpr::BoolLit(false, span),
            _ => HirExpr::StringLit(String::new(), span),
        };
    }
    HirExpr::StringLit(String::new(), span)
}

/// Resolve the JSON key for a field: `@field_name("…")` wins, then the
/// type-wide naming convention, then identity (snake_case default).
fn resolve_key(field: &VariantField, naming: &str) -> String {
    if let Some(ref override_name) = field.json_as_attr.field_name {
        return override_name.clone();
    }
    apply_naming(&field.name, naming)
}

fn apply_naming(s: &str, naming: &str) -> String {
    match naming {
        "camelCase" => snake_to_camel(s),
        "PascalCase" => snake_to_pascal(s),
        "kebab-case" => s.replace('_', "-"),
        _ => s.to_string(), // "snake_case" (default) — identity
    }
}

fn snake_to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut cap_next = false;
    for ch in s.chars() {
        if ch == '_' {
            cap_next = true;
        } else if cap_next {
            out.extend(ch.to_uppercase());
            cap_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn snake_to_pascal(s: &str) -> String {
    let mut out = String::new();
    let mut cap_next = true;
    for ch in s.chars() {
        if ch == '_' {
            cap_next = true;
        } else if cap_next {
            out.extend(ch.to_uppercase());
            cap_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────────
// HIR builder helpers
// ──────────────────────────────────────────────────────────────────────────────

fn expr_ident(name: &str, span: Span) -> HirExpr {
    HirExpr::Ident(name.to_string(), span)
}

/// `receiver.method()` — no-argument method call.
fn expr_method0(receiver: &str, method: &str, span: Span) -> HirExpr {
    HirExpr::MethodCall(
        Box::new(expr_ident(receiver, span)),
        method.to_string(),
        vec![],
        None,
        span,
    )
}

/// `receiver.method(arg)` — single-argument method call.
fn expr_method1(receiver: &str, method: &str, arg: HirExpr, span: Span) -> HirExpr {
    HirExpr::MethodCall(
        Box::new(expr_ident(receiver, span)),
        method.to_string(),
        vec![HirArg {
            name: None,
            value: arg,
        }],
        None,
        span,
    )
}

/// `j.get("key")` — returns `Option[Json]`.
fn expr_get(receiver: &str, key: &str, span: Span) -> HirExpr {
    expr_method1(
        receiver,
        "get",
        HirExpr::StringLit(key.to_string(), span),
        span,
    )
}

/// `j.get("key").and_then(fn(_jv: Json) { _jv.<as_method>() })` — returns `Option[T]`.
///
/// Allocates a fresh `DefId` for the `_jv` lambda parameter via `ctx.def_map`.
fn expr_get_and_then(
    ctx: &mut LowerCtx,
    receiver: &str,
    key: &str,
    as_method: &str,
    span: Span,
) -> HirExpr {
    let jv_id = ctx.def_map.define("_jv".to_string());
    let jv_param = HirParam {
        id: jv_id,
        name: "_jv".to_string(),
        type_ann: Some(HirType::Named("Json".to_string())),
        default: None,
        span,
    };
    let lambda_body = Box::new(expr_method0("_jv", as_method, span));
    let lambda = HirExpr::Lambda(vec![jv_param], None, lambda_body, false, span);

    HirExpr::MethodCall(
        Box::new(expr_get(receiver, key, span)),
        "and_then".to_string(),
        vec![HirArg {
            name: None,
            value: lambda,
        }],
        None,
        span,
    )
}

/// `Ok(val)` — wraps a value in the `Ok` result constructor.
fn expr_ok(val: HirExpr, span: Span) -> HirExpr {
    HirExpr::Call(
        Box::new(expr_ident("Ok", span)),
        vec![HirArg {
            name: None,
            value: val,
        }],
        false,
        span,
    )
}

/// `Error("msg")` — wraps a string message in the `Error` result constructor.
///
/// The Vox builtin is `Error(message: str) → Result[T]`, **not** `Err`.
fn expr_err(msg: &str, span: Span) -> HirExpr {
    HirExpr::Call(
        Box::new(expr_ident("Error", span)),
        vec![HirArg {
            name: None,
            value: HirExpr::StringLit(msg.to_string(), span),
        }],
        false,
        span,
    )
}

/// `let <name> = <value>` — immutable let binding.
fn let_stmt(name: &str, value: HirExpr, span: Span) -> HirStmt {
    HirStmt::Let {
        pattern: HirPattern::Ident(name.to_string(), span),
        type_ann: None,
        value,
        mutable: false,
        span,
    }
}

/// `return <value>` statement.
fn ret(value: HirExpr, span: Span) -> HirStmt {
    HirStmt::Return {
        value: Some(value),
        span,
    }
}

/// `if <tmp>.is_none() { return Err("<msg>") }` — required-field presence guard.
fn guard_none(tmp: &str, err_msg: &str, span: Span) -> HirStmt {
    HirStmt::Expr {
        expr: HirExpr::If(
            Box::new(expr_method0(tmp, "is_none", span)),
            vec![ret(expr_err(err_msg, span), span)],
            None,
            span,
        ),
        span,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::decl::typedef::{JsonAsAnnotation, JsonAsFieldAttr, TypeDefDecl, VariantField};
    use crate::ast::span::Span;
    use crate::ast::types::TypeExpr;
    // LowerCtx / LowerConfig are private in the parent module; child modules can see them.
    use super::super::{LowerConfig, LowerCtx};

    fn zero_span() -> Span {
        Span::new(0, 0)
    }

    fn make_ctx() -> LowerCtx {
        LowerCtx::new(LowerConfig::default())
    }

    fn str_field(name: &str) -> VariantField {
        VariantField {
            name: name.to_string(),
            type_ann: TypeExpr::Named {
                name: "str".to_string(),
                span: zero_span(),
            },
            json_as_attr: JsonAsFieldAttr::default(),
            span: zero_span(),
        }
    }

    fn opt_str_field(name: &str) -> VariantField {
        VariantField {
            name: name.to_string(),
            type_ann: TypeExpr::Generic {
                name: "Option".to_string(),
                args: vec![TypeExpr::Named {
                    name: "str".to_string(),
                    span: zero_span(),
                }],
                span: zero_span(),
            },
            json_as_attr: JsonAsFieldAttr::default(),
            span: zero_span(),
        }
    }

    fn make_ann(naming: &str) -> JsonAsAnnotation {
        JsonAsAnnotation {
            type_name: "T".to_string(),
            naming: naming.to_string(),
            strict: false,
            defaults: false,
            tag: None,
            span: zero_span(),
        }
    }

    // ── naming helpers ────────────────────────────────────────────────────────

    #[test]
    fn snake_case_is_identity() {
        assert_eq!(apply_naming("foo_bar", "snake_case"), "foo_bar");
    }

    #[test]
    fn camel_case_conversion() {
        assert_eq!(apply_naming("foo_bar_baz", "camelCase"), "fooBarBaz");
    }

    #[test]
    fn pascal_case_conversion() {
        assert_eq!(apply_naming("foo_bar", "PascalCase"), "FooBar");
    }

    #[test]
    fn kebab_case_conversion() {
        assert_eq!(apply_naming("foo_bar", "kebab-case"), "foo-bar");
    }

    #[test]
    fn field_name_override_wins() {
        let mut f = str_field("internal_name");
        f.json_as_attr.field_name = Some("jsonName".to_string());
        assert_eq!(resolve_key(&f, "snake_case"), "jsonName");
    }

    // ── no annotation → empty synthesis ──────────────────────────────────────

    #[test]
    fn no_annotation_yields_no_fns() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "NoAnn".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("x")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: None,
            span: zero_span(),
        };
        assert!(synthesise_json_as_fns(&mut ctx, &t).is_empty());
    }

    // ── struct synthesis ──────────────────────────────────────────────────────

    #[test]
    fn struct_emits_two_fns() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Product".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("name"), str_field("sku")],
            type_alias: None,
            json_layout: None,
            is_pub: true,
            is_deprecated: false,
            json_as: Some(make_ann("snake_case")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        assert_eq!(fns.len(), 2);
        assert_eq!(fns[0].name, "Product_from_json");
        assert_eq!(fns[1].name, "Product_to_json");
    }

    #[test]
    fn from_json_return_type_is_result() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Widget".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("id")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: Some(make_ann("snake_case")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        let from_fn = &fns[0];
        assert_eq!(
            from_fn.return_type,
            Some(HirType::Generic(
                "Result".to_string(),
                vec![HirType::Named("Widget".to_string())]
            ))
        );
    }

    #[test]
    fn to_json_return_type_is_json() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Widget".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("id")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: Some(make_ann("snake_case")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        let to_fn = &fns[1];
        assert_eq!(to_fn.return_type, Some(HirType::Named("Json".to_string())));
    }

    #[test]
    fn option_field_does_not_add_guard() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Item".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![opt_str_field("description")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: Some(make_ann("snake_case")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        let from_fn = &fns[0];
        // Option field → exactly 1 let stmt + 1 return = 2 stmts total
        assert_eq!(
            from_fn.body.len(),
            2,
            "expected let + return for option field"
        );
    }

    #[test]
    fn required_field_adds_guard() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Item".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("name")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: Some(make_ann("snake_case")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        let from_fn = &fns[0];
        // Required field → let _f_name + if guard + let name = unwrap + return = 4 stmts
        assert_eq!(
            from_fn.body.len(),
            4,
            "expected _f let + guard + unwrap + return"
        );
    }

    #[test]
    fn camel_case_naming_applied_to_to_json() {
        let mut ctx = make_ctx();
        let t = TypeDefDecl {
            name: "Event".to_string(),
            generics: vec![],
            variants: vec![],
            fields: vec![str_field("event_type"), str_field("created_at")],
            type_alias: None,
            json_layout: None,
            is_pub: false,
            is_deprecated: false,
            json_as: Some(make_ann("camelCase")),
            span: zero_span(),
        };
        let fns = synthesise_json_as_fns(&mut ctx, &t);
        let to_fn = &fns[1];
        // to_json body should be a single return of an ObjectLit
        if let HirStmt::Return {
            value: Some(HirExpr::ObjectLit(pairs, _)),
            ..
        } = &to_fn.body[0]
        {
            assert_eq!(pairs[0].0, "eventType");
            assert_eq!(pairs[1].0, "createdAt");
        } else {
            panic!("expected ObjectLit return in to_json body");
        }
    }
}
