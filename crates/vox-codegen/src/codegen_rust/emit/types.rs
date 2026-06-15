use vox_compiler::hir::{HirModule, HirType};

/// Rust path for the Vox opaque `Json` value (matches `std.json.parse` / `@json_as`).
pub(crate) const VOX_JSON_RUST_TYPE: &str = "vox_actor_runtime::builtins::VoxJson";

/// Prelude alias emitted in `emit_lib` when the module references Vox `Json`.
pub(crate) fn vox_json_type_alias_prelude() -> String {
    format!("pub use {VOX_JSON_RUST_TYPE};\npub type Json = {VOX_JSON_RUST_TYPE};\n\n")
}

/// Wrap a `serde_json::json!` / `Value` expression as Vox `Json` when the
/// enclosing function returns the opaque JSON type.
pub(crate) fn wrap_vox_json_return_value(expr: &str, return_type: Option<&HirType>) -> String {
    if matches!(return_type, Some(HirType::Function(_, _)))
        && (expr.starts_with("move |") || expr.starts_with('|'))
    {
        return format!("std::rc::Rc::new({expr})");
    }
    let returns_json = match return_type {
        Some(HirType::Named(n)) => n == "Json",
        _ => false,
    };
    if returns_json
        && (expr.contains("serde_json::json!") || expr.contains("serde_json::Value"))
        && !expr.starts_with("VoxJson(")
    {
        format!("VoxJson({expr})")
    } else {
        expr.to_string()
    }
}

/// True when any function surface in the module uses the Vox `Json` type.
pub(crate) fn module_uses_vox_json_type(module: &HirModule) -> bool {
    fn ty_uses_json(ty: &HirType) -> bool {
        match ty {
            HirType::Named(n) => n == "Json",
            HirType::Generic(_, args) => args.iter().any(ty_uses_json),
            _ => false,
        }
    }
    let fn_uses = |f: &vox_compiler::hir::HirFn| {
        f.params
            .iter()
            .any(|p| p.type_ann.as_ref().is_some_and(ty_uses_json))
            || f.return_type.as_ref().is_some_and(ty_uses_json)
    };
    module.functions.iter().any(fn_uses)
        || module.tests.iter().any(fn_uses)
        || module.mcp_tools.iter().any(|t| fn_uses(&t.func))
        || module.mcp_resources.iter().any(|r| fn_uses(&r.func))
        || module.foralls.iter().any(|forall| fn_uses(&forall.func))
}

/// Extract a PascalCase struct payload from `Result[T, E]` (Vox → std `Result`).
pub(crate) fn result_ok_struct_name(ty: &HirType) -> Option<String> {
    match ty {
        HirType::Generic(name, args) if name == "Result" => match args.first()? {
            HirType::Named(n) if n.chars().next().is_some_and(|c| c.is_ascii_uppercase()) => {
                Some(n.clone())
            }
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn emit_type(ty: &HirType) -> String {
    match ty {
        HirType::Named(n) => match n.as_str() {
            "int" => "i64".into(),
            "float" => "f64".into(),
            "bool" => "bool".into(),
            "str" => "String".into(),
            "Json" => "Json".into(),
            // Typeck back-fills `never` for fns whose trailing statement
            // diverges (e.g. `if cond { process.exit(..) }`), but such fns do
            // not diverge on every path, so `-> !` would not compile; unit is
            // the only always-valid Rust signature (diverging exprs coerce).
            "never" => "()".into(),
            "Element" | "Result" | "Any" => "serde_json::Value".into(),
            other => other.to_string(),
        },
        HirType::Generic(n, args) => {
            let args_str: Vec<_> = args.iter().map(emit_type).collect();
            match n.as_str() {
                // Id[Task] → i64 (SQLite rowid)
                "Id" => "i64".into(),
                "List" | "list" => format!(
                    "Vec<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                "Option" => format!(
                    "Option<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                // Vox `Result[T]` / `Result[T, E]` → std `Result` (Ok/Err), default err `String`.
                "Result" => {
                    let ok = args_str
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "serde_json::Value".to_string());
                    let err = args_str
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "String".to_string());
                    format!("Result<{ok}, {err}>")
                }
                // Deprecated aliases — emit DurablePromise during the v0.6→v0.7 migration window.
                "Future" | "Promise" => format!(
                    "DurablePromise<{}>",
                    args_str.first().unwrap_or(&"serde_json::Value".to_string())
                ),
                _ => format!("{}<{}>", n, args_str.join(", ")),
            }
        }
        HirType::Unit => "()".into(),
        HirType::Decimal => "rust_decimal::Decimal".into(),
        HirType::Function(params, ret) => {
            let params_str: Vec<String> = params.iter().map(emit_type).collect();
            format!(
                "std::rc::Rc<dyn Fn({}) -> {} + 'static>",
                params_str.join(", "),
                emit_type(ret)
            )
        }
        _ => "serde_json::Value".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_compiler::ast::span::Span;
    use vox_compiler::hir::{DefId, HirFn, HirModule, HirParam};

    #[test]
    fn emit_type_maps_json_to_alias_name() {
        assert_eq!(emit_type(&HirType::Named("Json".into())), "Json");
    }

    #[test]
    fn emit_type_function_is_boxed_fn_trait() {
        assert_eq!(
            emit_type(&HirType::Function(
                vec![HirType::Named("int".into())],
                Box::new(HirType::Named("int".into()))
            )),
            "std::rc::Rc<dyn Fn(i64) -> i64 + 'static>"
        );
    }

    #[test]
    fn result_ok_struct_name_extracts_payload() {
        let ty = HirType::Generic("Result".into(), vec![HirType::Named("Widget".into())]);
        assert_eq!(result_ok_struct_name(&ty), Some("Widget".into()));
        assert_eq!(result_ok_struct_name(&HirType::Named("str".into())), None);
    }

    // -----------------------------------------------------------------------
    // semcov_wave20_tests — adversarial emit_type / wrap_vox_json / result_ok
    // -----------------------------------------------------------------------

    mod semcov_wave20_tests {
        use super::super::{emit_type, result_ok_struct_name, wrap_vox_json_return_value};
        use vox_compiler::hir::HirType;

        #[test]
        fn emit_type_int_maps_to_i64() {
            // Catches: primitive mapping table off-by-one or wrong literal string.
            assert_eq!(emit_type(&HirType::Named("int".into())), "i64");
        }

        #[test]
        fn emit_type_float_maps_to_f64() {
            // Catches: float accidentally mapped to f32 or left as "float".
            assert_eq!(emit_type(&HirType::Named("float".into())), "f64");
        }

        #[test]
        fn emit_type_str_maps_to_string_owned() {
            // Catches: str → &str (wrong; should be owned String in codegen output).
            assert_eq!(emit_type(&HirType::Named("str".into())), "String");
        }

        #[test]
        fn emit_type_never_maps_to_unit_not_bang() {
            // Catches: "never" emitted as "!" which fails to compile when the fn
            // does not diverge on every path.
            assert_eq!(emit_type(&HirType::Named("never".into())), "()");
        }

        #[test]
        fn emit_type_list_with_no_type_args_falls_back_to_json_value() {
            // Catches: unwrap() panic on empty args instead of graceful fallback.
            let ty = HirType::Generic("List".into(), vec![]);
            assert_eq!(emit_type(&ty), "Vec<serde_json::Value>");
        }

        #[test]
        fn emit_type_option_with_no_type_args_falls_back_to_json_value() {
            // Catches: same unwrap hazard for Option<>.
            let ty = HirType::Generic("Option".into(), vec![]);
            assert_eq!(emit_type(&ty), "Option<serde_json::Value>");
        }

        #[test]
        fn emit_type_result_with_no_args_defaults_ok_and_err() {
            // Catches: Result<> panicking or producing wrong defaults.
            let ty = HirType::Generic("Result".into(), vec![]);
            assert_eq!(emit_type(&ty), "Result<serde_json::Value, String>");
        }

        #[test]
        fn emit_type_result_with_one_arg_defaults_err_to_string() {
            // Catches: Result[T] emitting Result<T> without an Error parameter.
            let ty = HirType::Generic("Result".into(), vec![HirType::Named("int".into())]);
            assert_eq!(emit_type(&ty), "Result<i64, String>");
        }

        #[test]
        fn emit_type_id_generic_collapses_to_i64() {
            // Catches: Id[Task] → "Id<Task>" instead of "i64" (SQLite rowid).
            let ty = HirType::Generic("Id".into(), vec![HirType::Named("Task".into())]);
            assert_eq!(emit_type(&ty), "i64");
        }

        #[test]
        fn emit_type_future_alias_emits_durable_promise() {
            // Catches: deprecated Future alias emitting "Future<…>" which breaks
            // the v0.6→v0.7 migration window expectation.
            let ty = HirType::Generic("Future".into(), vec![HirType::Named("int".into())]);
            assert_eq!(emit_type(&ty), "DurablePromise<i64>");
        }

        #[test]
        fn emit_type_function_zero_params_emits_correct_rc_wrapper() {
            // Catches: zero-param function type emitting extra commas or omitting
            // the Rc wrapper entirely.
            let ty = HirType::Function(vec![], Box::new(HirType::Unit));
            assert_eq!(emit_type(&ty), "std::rc::Rc<dyn Fn() -> () + 'static>");
        }

        #[test]
        fn emit_type_unknown_named_type_passes_through_unchanged() {
            // Catches: catch-all arm mangling casing or prepending a path prefix.
            assert_eq!(emit_type(&HirType::Named("MyStruct".into())), "MyStruct");
        }

        #[test]
        fn wrap_vox_json_skips_wrapping_when_return_type_is_not_json() {
            // Catches: wrapping triggered for non-Json return types, corrupting
            // serde_json expressions in regular functions.
            let out = wrap_vox_json_return_value(
                "serde_json::json!({\"k\": 1})",
                Some(&HirType::Named("str".into())),
            );
            assert_eq!(out, "serde_json::json!({\"k\": 1})");
        }

        #[test]
        fn wrap_vox_json_skips_already_wrapped_value() {
            // Catches: double-wrapping VoxJson(VoxJson(…)) on successive lowering.
            let out = wrap_vox_json_return_value(
                "VoxJson(serde_json::json!({}))",
                Some(&HirType::Named("Json".into())),
            );
            assert_eq!(out, "VoxJson(serde_json::json!({}))");
        }

        #[test]
        fn wrap_vox_json_wraps_bare_serde_json_expression() {
            // Catches: failing to inject VoxJson wrapper for Json return type.
            let out = wrap_vox_json_return_value(
                "serde_json::json!({\"x\": 1})",
                Some(&HirType::Named("Json".into())),
            );
            assert_eq!(out, "VoxJson(serde_json::json!({\"x\": 1}))");
        }

        #[test]
        fn wrap_vox_json_emits_rc_for_lambda_with_fn_return_type() {
            // Catches: lambda expressions not wrapped in Rc when return type is
            // Function — bare closure would not satisfy Rc<dyn Fn>.
            let fn_ty = HirType::Function(vec![], Box::new(HirType::Unit));
            let out = wrap_vox_json_return_value("move |x| x", Some(&fn_ty));
            assert_eq!(out, "std::rc::Rc::new(move |x| x)");
        }

        #[test]
        fn wrap_vox_json_none_return_type_passes_through() {
            // Catches: None return type triggering an unwrap/panic.
            let out = wrap_vox_json_return_value("serde_json::json!({})", None);
            assert_eq!(out, "serde_json::json!({})");
        }

        #[test]
        fn result_ok_struct_name_rejects_lowercase_type_arg() {
            // Catches: primitive "str" returned as a struct name — only PascalCase.
            let ty = HirType::Generic("Result".into(), vec![HirType::Named("str".into())]);
            assert_eq!(result_ok_struct_name(&ty), None);
        }

        #[test]
        fn result_ok_struct_name_rejects_non_result_generic() {
            // Catches: Option[Widget] yielding "Widget" as result payload.
            let ty = HirType::Generic("Option".into(), vec![HirType::Named("Widget".into())]);
            assert_eq!(result_ok_struct_name(&ty), None);
        }

        #[test]
        fn result_ok_struct_name_rejects_empty_args() {
            // Catches: unwrap on empty args vec causing a panic.
            let ty = HirType::Generic("Result".into(), vec![]);
            assert_eq!(result_ok_struct_name(&ty), None);
        }

        #[test]
        fn result_ok_struct_name_rejects_named_result_not_generic() {
            // Catches: Named("Result") (non-generic) triggering the generic match.
            assert_eq!(
                result_ok_struct_name(&HirType::Named("Result".into())),
                None
            );
        }
    }

    #[test]
    fn module_uses_vox_json_type_detects_json_as_fns() {
        let mut module = HirModule::default();
        module.functions.push(HirFn {
            id: DefId(1),
            name: "Widget_from_json".into(),
            generics: vec![],
            params: vec![HirParam {
                id: DefId(2),
                name: "j".into(),
                type_ann: Some(HirType::Named("Json".into())),
                default: None,
                span: Span::new(0, 0),
            }],
            return_type: Some(HirType::Generic(
                "Result".into(),
                vec![HirType::Named("Widget".into())],
            )),
            body: vec![],
            is_pub: true,
            is_async: false,
            is_mobile_native: false,
            is_pure: false,
            is_reactive: false,
            is_versioned: false,
            capabilities: vec![],
            is_remote: false,
            is_llm: false,
            llm_model: None,
            ai_structured_output: None,
            ai_fixture: None,
            embed: None,
            is_deprecated: false,
            is_traced: false,
            schedule_interval: None,
            durability: None,
            actor_state_fields: vec![],
            postconditions: vec![],
            ts_extern_module: None,
            generated_hash: None,
            span: Span::new(0, 0),
            inference_model: None,
            training_step: false,
            distributed_train: None,
        });
        assert!(module_uses_vox_json_type(&module));
    }
}
