use super::*;
use crate::ast::decl::{Decl, EffectAnnotation, ImportPathKind, RoutesParseSummary};
use crate::ast::expr::{BinOp, Expr};
use crate::ast::stmt::Stmt;
use crate::lexer::cursor::lex;

fn parse_str(source: &str) -> Module {
    let tokens = lex(source);
    parse(tokens).unwrap_or_else(|e| panic!("Parse errors: {e:?}"))
}

fn assert_parse_fails(source: &str) {
    let tokens = lex(source);
    assert!(
        parse(tokens).is_err(),
        "expected parse to fail for source: {source:?}"
    );
}

#[test]
fn test_parse_simple_fn() {
    let m = parse_str("fn add(a, b) to int { return a + b }");
    assert_eq!(m.declarations.len(), 1);
    assert!(matches!(&m.declarations[0], Decl::Function(f) if f.name == "add"));
}

#[test]
fn test_parse_import() {
    let m = parse_str("import react.use_state, network.HTTP");
    assert!(matches!(&m.declarations[0], Decl::Import(i) if i.paths.len() == 2));
}

#[test]
fn test_parse_rust_import() {
    let m = parse_str("import rust:serde_json");
    match &m.declarations[0] {
        Decl::Import(i) => {
            assert_eq!(i.paths.len(), 1);
            assert!(matches!(i.paths[0].kind, ImportPathKind::RustCrate(_)));
        }
        other => panic!("Expected import decl, got {other:?}"),
    }
}

#[test]
fn test_parse_rust_import_with_alias_and_meta() {
    let m = parse_str(
        "import rust:serde_json(version: \"1\", git: \"https://example.invalid/repo\", rev: \"main\") as json",
    );
    match &m.declarations[0] {
        Decl::Import(i) => {
            let p = &i.paths[0];
            assert_eq!(p.alias.as_deref(), Some("json"));
            match &p.kind {
                ImportPathKind::RustCrate(spec) => {
                    assert_eq!(spec.crate_name, "serde_json");
                    assert_eq!(spec.version.as_deref(), Some("1"));
                    assert_eq!(spec.git.as_deref(), Some("https://example.invalid/repo"));
                    assert_eq!(spec.rev.as_deref(), Some("main"));
                }
                _ => panic!("Expected rust crate import"),
            }
        }
        other => panic!("Expected import decl, got {other:?}"),
    }
}

#[test]
fn test_parse_let() {
    let m = parse_str("fn main() { let x = 42\n return x }");
    if let Decl::Function(f) = &m.declarations[0] {
        assert_eq!(f.body.len(), 2);
        assert!(matches!(&f.body[0], Stmt::Let { .. }));
    } else {
        panic!("Expected function");
    }
}

#[test]
fn classic_component_fn_is_parse_error() {
    assert_parse_fails("@component fn Chat() to Element { return 0 }");
}

#[test]
fn test_parse_loading_decl() {
    let m = parse_str("@loading fn RouteSpinner() to Element { return column() }");
    assert!(matches!(
        &m.declarations[0],
        Decl::Loading(l) if l.func.name == "RouteSpinner"
    ));
}

#[test]
fn test_parse_at_component_reactive_path_c_is_tombstoned() {
    assert_parse_fails(
        "@component Widget(x: int) {\n  state n: int = x\n  view: <span>{n}</span>\n}",
    );
}

#[test]
fn test_parse_http_route_is_tombstoned() {
    assert_parse_fails("http post \"/api/chat\" to Result { return 0 }");
}

#[test]
fn test_parse_match() {
    let m = parse_str("fn f() { match x { Ok(r) => r\n Error(e) => e\n } }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Expr {
            expr: Expr::Match { arms, .. },
            ..
        } = &f.body[0]
        {
            assert_eq!(arms.len(), 2);
        } else {
            panic!("Expected match");
        }
    }
}

#[test]
fn test_parse_match_arm_print_string_with_backticks() {
    let m = parse_str(
        r#"fn f() {
            match x {
                Error(e) => print("  [err] `cargo clean` failed to spawn: " + e),
            }
        }"#,
    );
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    let Stmt::Expr {
        expr: Expr::Match { arms, .. },
        ..
    } = &f.body[0]
    else {
        panic!("expected match expr");
    };
    assert_eq!(arms.len(), 1);
    let body = arms[0].body.as_ref();
    let Expr::Call { callee, args, .. } = body else {
        panic!("expected call, got {body:?}");
    };
    assert!(matches!(callee.as_ref(), Expr::Ident { name, .. } if name == "print"));
    assert_eq!(args.len(), 1);
    let Expr::Binary {
        op: BinOp::Add,
        left,
        right,
        ..
    } = &args[0].value
    else {
        panic!("expected string + ident, got {:?}", args[0].value);
    };
    assert!(matches!(
        left.as_ref(),
        Expr::StringLit { value, .. }
            if value == "  [err] `cargo clean` failed to spawn: "
    ));
    assert!(matches!(
        right.as_ref(),
        Expr::Ident { name, .. } if name == "e"
    ));
}

#[test]
fn test_parse_match_trailing_comma_after_arm() {
    let m = parse_str("fn f() { match x { Ok(r) => r,\n Error(e) => e,\n } }");
    let Decl::Function(f) = &m.declarations[0] else {
        panic!("expected function");
    };
    let Stmt::Expr {
        expr: Expr::Match { arms, .. },
        ..
    } = &f.body[0]
    else {
        panic!("expected match");
    };
    assert_eq!(arms.len(), 2);
}

#[test]
fn test_parse_type_def() {
    let m = parse_str("type Shape =\n    | Circle(r: float)\n    | Point");
    if let Decl::TypeDef(t) = &m.declarations[0] {
        assert_eq!(t.name, "Shape");
        assert_eq!(t.variants.len(), 2);
    } else {
        panic!("Expected type def");
    }
}

#[test]
fn test_parse_operator_precedence() {
    let m = parse_str("fn f() { return 1 + 2 * 3 }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Return {
            value:
                Some(Expr::Binary {
                    op: BinOp::Add,
                    right,
                    ..
                }),
            ..
        } = &f.body[0]
        {
            assert!(matches!(
                right.as_ref(),
                Expr::Binary { op: BinOp::Mul, .. }
            ));
        } else {
            panic!("Expected add(1, mul(2,3))");
        }
    }
}

#[test]
fn test_parse_pipe() {
    let m = parse_str("fn f() { return x |> transform |> render }");
    assert!(matches!(&m.declarations[0], Decl::Function(_)));
}

#[test]
fn test_parse_actor() {
    let m = parse_str("actor Worker { on receive(msg: str) to str { return msg } }");
    assert!(
        matches!(&m.declarations[0], Decl::Actor(a) if a.name == "Worker"),
        "Expected Actor declaration"
    );
    if let Decl::Actor(a) = &m.declarations[0] {
        assert_eq!(a.handlers.len(), 1);
        assert_eq!(a.handlers[0].event_name, "receive");
    }
}

#[test]
fn test_parse_workflow() {
    let m = parse_str("workflow process(file: str) to str { return file }");
    assert!(
        matches!(&m.declarations[0], Decl::Workflow(w) if w.name == "process"),
        "Expected Workflow declaration"
    );
}

#[test]
fn test_parse_lambda() {
    let m = parse_str("fn f() { let add = fn(a, b) a + b\n return add(1, 2) }");
    if let Decl::Function(f) = &m.declarations[0] {
        assert_eq!(f.body.len(), 2);
        if let Stmt::Let {
            value: Expr::Lambda { params, .. },
            ..
        } = &f.body[0]
        {
            assert_eq!(params.len(), 2);
        } else {
            panic!("Expected lambda let");
        }
    } else {
        panic!("Expected function");
    }
}

#[test]
fn test_parse_if_else() {
    let m = parse_str("fn f(x) { if x { return 1\n} else { return 0\n} }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Expr {
            expr:
                Expr::If {
                    then_body,
                    else_body,
                    ..
                },
            ..
        } = &f.body[0]
        {
            assert_eq!(then_body.len(), 1);
            assert!(else_body.is_some());
        } else {
            panic!("Expected if/else");
        }
    }
}

#[test]
fn test_parse_else_if_chain() {
    // Verify `else if` chains parse as nested Expr::If rather than requiring
    // the workaround `else { if … }` form.
    let src = r#"fn classify(x) {
  if x > 10 { return "big"
  } else if x > 5 { return "medium"
  } else if x > 0 { return "small"
  } else { return "none"
  }
}"#;
    let m = parse_str(src);
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Expr {
            expr:
                Expr::If {
                    else_body: Some(else_body),
                    ..
                },
            ..
        } = &f.body[0]
        {
            // The else branch should be a single `Stmt::Expr` wrapping another `Expr::If`
            assert_eq!(
                else_body.len(),
                1,
                "else body should have exactly one statement"
            );
            if let Stmt::Expr {
                expr:
                    Expr::If {
                        else_body: Some(inner_else),
                        ..
                    },
                ..
            } = &else_body[0]
            {
                // Inner else is another if (or the final else block)
                assert_eq!(inner_else.len(), 1);
            } else {
                panic!("Expected nested Expr::If in else branch");
            }
        } else {
            panic!("Expected if/else-if chain");
        }
    } else {
        panic!("Expected function declaration");
    }
}

#[test]
fn test_parse_mutable_let() {
    let m = parse_str("fn f() { let mut x = 0\n x = 1\n return x }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Let { mutable, .. } = &f.body[0] {
            assert!(mutable, "Should be mutable");
        } else {
            panic!("Expected mutable let");
        }
    }
}

#[test]
fn test_parse_method_chain() {
    let m = parse_str("fn f() { return list.map(fn(x) x).filter(fn(x) x) }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Return {
            value: Some(Expr::MethodCall { method, .. }),
            ..
        } = &f.body[0]
        {
            assert_eq!(method, "filter");
        } else {
            panic!("Expected method chain");
        }
    }
}

// JSX angle-bracket parser path retired (VUV). View calls now use the trailing-block form
// (`Ident(kwargs) { children }` and `Capitalized()` / `primitive_name()`). See the
// `test_parse_view_call_*` tests below for the canonical coverage.

#[test]
fn test_parse_view_call_form_lowers_to_jsx() {
    // VUV: view-call form `Ident(kwargs) { children }` parses as Expr::Jsx so HIR / web_ir / codegen
    // are untouched. This test asserts the parser sugars the new shape into the existing JSX AST.
    let m = parse_str(
        r#"component A() {
            view: row(gap=2) {
                text(size="xs") { "hello" }
            }
        }"#,
    );
    let Decl::ReactiveComponent(r) = &m.declarations[0] else {
        panic!("Expected reactive component, got {:?}", m.declarations[0]);
    };
    let Some(Expr::Jsx(outer)) = &r.view else {
        panic!(
            "Expected outer view-call to lower to Expr::Jsx, got {:?}",
            r.view
        );
    };
    assert_eq!(outer.tag, "row");
    assert_eq!(outer.attributes.len(), 1);
    assert_eq!(outer.attributes[0].name, "gap");
    assert_eq!(outer.children.len(), 1);
    let Expr::Jsx(inner) = &outer.children[0] else {
        panic!(
            "Expected inner child to be Expr::Jsx, got {:?}",
            outer.children[0]
        );
    };
    assert_eq!(inner.tag, "text");
    assert_eq!(inner.attributes.len(), 1);
    assert_eq!(inner.attributes[0].name, "size");
    assert_eq!(inner.children.len(), 1);
    assert!(matches!(inner.children[0], Expr::StringLit { .. }));
}

#[test]
fn test_attr_prefix_strips_to_reserved_keyword_attribute_name() {
    // VUV: `attr_type="checkbox"` parses and lowers to JsxAttribute name "type" so HTML
    // attributes whose names are Vox keywords can still be expressed.
    let m = parse_str(r#"component A() { view: input(attr_type="checkbox") }"#);
    let Decl::ReactiveComponent(r) = &m.declarations[0] else {
        panic!();
    };
    let Some(Expr::JsxSelfClosing(el)) = &r.view else {
        panic!("expected self-closing JSX, got {:?}", r.view);
    };
    assert_eq!(el.tag, "input");
    assert_eq!(el.attributes.len(), 1);
    assert_eq!(
        el.attributes[0].name, "type",
        "attr_ prefix should be stripped"
    );
}

#[test]
fn test_capitalized_call_no_block_lowers_to_self_closing_jsx() {
    // VUV: `ComposerPanel()` (no trailing block, capitalized callee, no args or all-named)
    // sugars to Expr::JsxSelfClosing. Lowercase callees and positional-arg calls do not.
    let m = parse_str("component A() { view: ComposerPanel() }");
    let Decl::ReactiveComponent(r) = &m.declarations[0] else {
        panic!("expected reactive component, got {:?}", m.declarations[0]);
    };
    let Some(Expr::JsxSelfClosing(el)) = &r.view else {
        panic!("expected self-closing JSX, got {:?}", r.view);
    };
    assert_eq!(el.tag, "ComposerPanel");
    assert!(el.attributes.is_empty());
}

#[test]
fn test_capitalized_call_with_named_args_lowers_to_self_closing() {
    let m = parse_str(r#"component A() { view: PipelineStage(name="Lexer", desc="tok") }"#);
    let Decl::ReactiveComponent(r) = &m.declarations[0] else {
        panic!();
    };
    let Some(Expr::JsxSelfClosing(el)) = &r.view else {
        panic!("expected self-closing JSX, got {:?}", r.view);
    };
    assert_eq!(el.tag, "PipelineStage");
    assert_eq!(el.attributes.len(), 2);
}

#[test]
fn test_capitalized_call_with_positional_arg_stays_call() {
    // Enum constructors (Some, Ok, Err) use positional args — must NOT be sugared to JSX.
    let m = parse_str("fn f() -> int { let x = Some(42); return 1 }");
    let Decl::Function(func) = &m.declarations[0] else {
        panic!();
    };
    if let Stmt::Let {
        value: Expr::Call { callee, .. },
        ..
    } = &func.body[0]
        && let Expr::Ident { name, .. } = callee.as_ref()
    {
        assert_eq!(name, "Some");
        return;
    }
    panic!(
        "expected Some(42) to remain Expr::Call, got {:?}",
        func.body[0]
    );
}

#[test]
fn test_view_call_positional_arg_does_not_sugar_to_jsx() {
    // VUV view calls are keyword-only. A positional arg disqualifies the call from view-call
    // sugar — it stays a regular `Expr::Call` so it can be evaluated as an ordinary function.
    // Critically, this means `row(2)` (a hypothetical row constructor) does NOT silently
    // become `<row 2 />`. The parser's view-call path must require all-named args.
    let m = parse_str("fn build() -> int { return row(2) }");
    let Decl::Function(func) = &m.declarations[0] else {
        panic!("expected fn");
    };
    let Stmt::Return {
        value: Some(Expr::Call { callee, args, .. }),
        ..
    } = &func.body[0]
    else {
        panic!("expected return Expr::Call, got {:?}", func.body[0]);
    };
    let Expr::Ident { name, .. } = callee.as_ref() else {
        panic!("callee not Ident");
    };
    assert_eq!(name, "row");
    assert_eq!(args.len(), 1);
    assert!(args[0].name.is_none(), "positional arg name should be None");
}

#[test]
fn test_parse_spawn() {
    let m = parse_str("fn f() { return spawn(Worker) }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Return {
            value: Some(Expr::Spawn { .. }),
            ..
        } = &f.body[0]
        {
            // ok
        } else {
            panic!("Expected spawn");
        }
    }
}

#[test]
fn test_parse_for_loop() {
    let m = parse_str("fn f() { for x in items { x } }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Expr {
            expr: Expr::For { binding, .. },
            ..
        } = &f.body[0]
        {
            assert_eq!(binding, "x");
        } else {
            panic!("Expected for loop");
        }
    }
}

#[test]
fn test_parse_pub_fn() {
    let m = parse_str("pub fn helper() to int { return 42 }");
    if let Decl::Function(f) = &m.declarations[0] {
        assert!(f.is_pub);
        assert_eq!(f.name, "helper");
    } else {
        panic!("Expected pub fn");
    }
}

#[test]
fn test_parse_multiple_decls() {
    let src = "import std\n\nfn a() { return 1 }\n\nfn b() { return 2 }";
    let m = parse_str(src);
    assert_eq!(m.declarations.len(), 3, "import + 2 functions");
}

#[test]
fn test_parse_activity() {
    let m = parse_str("activity send_email(recipient: str) to str { return recipient }");
    assert!(
        matches!(&m.declarations[0], Decl::Activity(a) if a.name == "send_email"),
        "Expected Activity declaration"
    );
}

#[test]
fn test_parse_with_expression() {
    let m = parse_str("fn f() { return call() with { timeout: 5 } }");
    if let Decl::Function(f) = &m.declarations[0] {
        if let Stmt::Return {
            value: Some(Expr::With {
                operand, options, ..
            }),
            ..
        } = &f.body[0]
        {
            assert!(matches!(operand.as_ref(), Expr::Call { .. }));
            assert!(matches!(options.as_ref(), Expr::ObjectLit { .. }));
        } else {
            panic!("Expected With expression in return");
        }
    } else {
        panic!("Expected function");
    }
}

#[test]
fn test_parse_table() {
    let m = parse_str("@table type Task { title: str\n done: bool\n priority: int }");
    if let Decl::Table(t) = &m.declarations[0] {
        assert_eq!(t.name, "Task");
        assert_eq!(t.fields.len(), 3);
        assert_eq!(t.fields[0].name, "title");
        assert_eq!(t.fields[1].name, "done");
        assert_eq!(t.fields[2].name, "priority");
    } else {
        panic!("Expected table declaration, got {:?}", m.declarations[0]);
    }
}

#[test]
fn test_parse_index() {
    let m = parse_str("@index Task.by_done on (done, priority)");
    if let Decl::Index(idx) = &m.declarations[0] {
        assert_eq!(idx.table_name, "Task");
        assert_eq!(idx.index_name, "by_done");
        assert_eq!(idx.columns, vec!["done", "priority"]);
    } else {
        panic!("Expected index declaration, got {:?}", m.declarations[0]);
    }
}
#[test]
fn test_parse_v0_component() {
    let m = parse_str("@v0 \"yM1xXq6\" Dashboard {}");
    if let Decl::V0Component(v) = &m.declarations[0] {
        assert_eq!(v.name, "Dashboard");
        assert_eq!(v.v0_id, "yM1xXq6");
        assert!(v.image_path.is_none());
    } else {
        panic!("Expected V0Component, got {:?}", m.declarations[0]);
    }
}

#[test]
fn test_parse_v0_component_from_image() {
    let m = parse_str(r#"@v0 from "mock.png" Landing {}"#);
    if let Decl::V0Component(v) = &m.declarations[0] {
        assert_eq!(v.name, "Landing");
        assert_eq!(v.v0_id, "");
        assert_eq!(v.image_path.as_deref(), Some("mock.png"));
    } else {
        panic!("Expected V0Component, got {:?}", m.declarations[0]);
    }
}

// WebIR blueprint G1: parser-truth coverage for server fns, routes, reactive surface.

/// `@server fn ...` is the canonical replacement for the v0.5-era
/// `@endpoint(kind: server) fn ...` form (retired in v0.6.0 per
/// `vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`).  Both
/// produced the same `Decl::Endpoint(EndpointKind::Server)` AST.
#[test]
fn test_parse_server_decorator_fn_brace_shape() {
    let m = parse_str("@server fn echo(x: str) to str {\n    return x\n}");
    if let Decl::Endpoint(e) = &m.declarations[0] {
        assert_eq!(e.func.name, "echo");
        assert_eq!(e.func.params.len(), 1);
        assert_eq!(e.func.params[0].name, "x");
        assert!(matches!(e.kind, crate::ast::decl::EndpointKind::Server));
    } else {
        panic!("Expected Decl::Endpoint, got {:?}", m.declarations[0]);
    }
}

#[test]
fn test_parse_routes_multiple_entries() {
    let m = parse_str("routes { \"/\" to Home \"/about\" to About }");
    if let Decl::Routes(r) = &m.declarations[0] {
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].path, "/");
        assert_eq!(r.entries[0].component_name, "Home");
        assert_eq!(r.entries[1].path, "/about");
        assert_eq!(r.entries[1].component_name, "About");
    } else {
        panic!("Expected Decl::Routes, got {:?}", m.declarations[0]);
    }
}

/// OP-0022: malformed routes entry (`path` then component ident without `to`) rejects gracefully.
#[test]
fn test_parse_rejects_invalid_route_entry_missing_to() {
    assert_parse_fails(r#"routes { "/" Home }"#);
}

/// OP-0026: root path and multi-segment path literals in one `routes` block.
#[test]
fn test_parse_routes_root_and_nested_path_literals() {
    let m = parse_str(r#"routes { "/" to Home "/blog/post" to Article }"#);
    if let Decl::Routes(r) = &m.declarations[0] {
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].path, "/");
        assert_eq!(r.entries[0].component_name, "Home");
        assert_eq!(r.entries[1].path, "/blog/post");
        assert_eq!(r.entries[1].component_name, "Article");
    } else {
        panic!("Expected Decl::Routes, got {:?}", m.declarations[0]);
    }
}

#[test]
fn test_parse_reactive_effect_mount_cleanup_view_is_tombstoned() {
    assert_parse_fails(
        "@component Demo(x: int) {\n  state n: int = x\n  effect: { }\n  on mount: { }\n  on cleanup: { }\n  view: <span>{n}</span>\n}",
    );
}

#[test]
fn test_parse_std_http_dotted_path_and_import() {
    let m = parse_str(
        "import std.http\n\nfn main() {\n  let _ = std.http.get_text(\"https://example.com\")\n}\n",
    );
    assert!(
        m.declarations.iter().any(|d| matches!(d, Decl::Import(_))),
        "expected import decl"
    );
    assert!(
        m.declarations
            .iter()
            .any(|d| matches!(d, Decl::Function(_))),
        "expected fn decl"
    );
}

#[test]
fn test_parse_reactive_rejects_misplaced_view_without_colon() {
    assert_parse_fails("@component Bad() {\n  view <div />\n}");
}

/// OP-0028: [`RoutesDecl::parse_summary`] is stable for multi-entry blocks.
#[test]
fn test_routes_parse_summary_matches_paths() {
    let m = parse_str(r#"routes { "/" to Home "/blog/post" to Article }"#);
    if let Decl::Routes(r) = &m.declarations[0] {
        assert_eq!(
            r.parse_summary(),
            RoutesParseSummary {
                entry_count: 2,
                paths: vec!["/".to_string(), "/blog/post".to_string()],
            }
        );
    } else {
        panic!("Expected Decl::Routes, got {:?}", m.declarations[0]);
    }
}

/// OP-0015: syntax inventory strings remain wired for tooling/docs extraction.
#[test]
fn test_web_surface_syntax_inventory_non_empty() {
    use crate::parser::WEB_SURFACE_SYNTAX_INVENTORY;
    let joined = WEB_SURFACE_SYNTAX_INVENTORY.join("\n");
    assert!(joined.contains("routes {"), "{joined}");
}

#[test]
#[ignore = "owner: compiler — sunset: 2026-08-01 — agent/environment decl parse not in active grammar path"]
fn test_parse_agent_and_environment() {
    let m = parse_str(
        r#"
agent Assistant {
    version "1.0"
}
environment staging {
    base "node"
}
"#,
    );
    assert_eq!(2, m.declarations.len());
    assert!(matches!(m.declarations[0], Decl::Agent(_)));
    assert!(matches!(m.declarations[1], Decl::Environment(_)));
}

// ── parse_script tests (audit item A.1) ──────────────────────────────────────

fn parse_script_str(source: &str) -> Module {
    let tokens = lex(source);
    parse_script(tokens).unwrap_or_else(|e| panic!("Script parse errors: {e:?}"))
}

/// Top-level `let` statement is wrapped in a synthetic `fn main()`.
#[test]
fn test_parse_script_top_level_let_becomes_main() {
    let m = parse_script_str("let x = 42");
    // The module must contain exactly one declaration: synthetic fn main.
    assert_eq!(
        m.declarations.len(),
        1,
        "expected exactly synthetic fn main"
    );
    if let Decl::Function(f) = &m.declarations[0] {
        assert_eq!(f.name, "main");
        assert_eq!(f.params.len(), 0);
        assert!(!f.is_pub);
        assert_eq!(f.body.len(), 1);
        assert!(matches!(&f.body[0], Stmt::Let { .. }));
    } else {
        panic!("Expected Decl::Function(main), got {:?}", m.declarations[0]);
    }
}

/// A plain expression at the top level becomes a Stmt::Expr inside main.
#[test]
fn test_parse_script_top_level_expr_becomes_main_body() {
    let m = parse_script_str("print(42)");
    if let Decl::Function(f) = &m.declarations[0] {
        assert_eq!(f.name, "main");
        assert_eq!(f.body.len(), 1);
        assert!(matches!(&f.body[0], Stmt::Expr { .. }));
    } else {
        panic!("expected fn main");
    }
}

/// Mixed file: a declaration + top-level statements. Declarations stay as-is;
/// statements are collected into synthetic main appended after them.
#[test]
fn test_parse_script_mixed_decl_and_stmts() {
    let src = "fn helper() to int { return 1 }\nlet result = helper()";
    let m = parse_script_str(src);
    // Expect: fn helper, then synthetic fn main.
    assert_eq!(m.declarations.len(), 2, "expected helper + synthetic main");
    assert!(matches!(&m.declarations[0], Decl::Function(f) if f.name == "helper"));
    assert!(matches!(&m.declarations[1], Decl::Function(f) if f.name == "main"));
    if let Decl::Function(main) = &m.declarations[1] {
        assert_eq!(main.body.len(), 1);
        assert!(matches!(&main.body[0], Stmt::Let { .. }));
    }
}

/// A pure-declaration file (no top-level statements) produces no synthetic main.
#[test]
fn test_parse_script_pure_decl_file_no_synthetic_main() {
    let src = "fn add(a, b) to int { return a + b }";
    let m = parse_script_str(src);
    assert_eq!(m.declarations.len(), 1);
    assert!(matches!(&m.declarations[0], Decl::Function(f) if f.name == "add"));
}

/// `url Name { Variant }` parses to `Decl::Url`.
#[test]
fn test_parse_url_decl_simple() {
    let m = parse_str("url Path {\nHome\n}");
    assert_eq!(m.declarations.len(), 1);
    match &m.declarations[0] {
        Decl::Url(u) => {
            assert_eq!(u.name, "Path");
            assert_eq!(u.variants.len(), 1);
            assert_eq!(u.variants[0].name, "Home");
            assert!(u.variants[0].args.is_empty());
            assert!(!u.is_pub);
        }
        other => panic!("Expected Decl::Url, got {other:?}"),
    }
}

/// `url` block with parameterized variants.
#[test]
fn test_parse_url_decl_with_args() {
    let m = parse_str("url Path {\nHome\nTask(id: str)\n}");
    match &m.declarations[0] {
        Decl::Url(u) => {
            assert_eq!(u.variants.len(), 2);
            assert_eq!(u.variants[1].name, "Task");
            assert_eq!(u.variants[1].args.len(), 1);
            assert_eq!(u.variants[1].args[0].name, "id");
            assert!(!u.variants[1].args[0].optional);
        }
        other => panic!("Expected Decl::Url, got {other:?}"),
    }
}

/// `url` block with optional argument (`?` prefix).
#[test]
fn test_parse_url_decl_optional_arg() {
    let m = parse_str("url Path {\nLogin(?return_to: str)\n}");
    match &m.declarations[0] {
        Decl::Url(u) => {
            assert!(u.variants[0].args[0].optional);
            assert_eq!(u.variants[0].args[0].name, "return_to");
        }
        other => panic!("Expected Decl::Url, got {other:?}"),
    }
}

/// `pub url` parses as `is_pub = true`.
#[test]
fn test_parse_url_decl_pub() {
    let m = parse_str("pub url Path {\nHome\n}");
    match &m.declarations[0] {
        Decl::Url(u) => assert!(u.is_pub),
        other => panic!("Expected Decl::Url, got {other:?}"),
    }
}

/// Multiple top-level statements all end up in one synthetic main body.
#[test]
fn test_parse_script_multiple_stmts_single_main() {
    let src = "let x = 1\nlet y = 2\nlet z = x";
    let m = parse_script_str(src);
    assert_eq!(m.declarations.len(), 1);
    if let Decl::Function(f) = &m.declarations[0] {
        assert_eq!(f.name, "main");
        assert_eq!(f.body.len(), 3);
    } else {
        panic!("expected fn main");
    }
}

// ── uses clause (TASK-4.2) ─────────────────────────────────────────────────

/// `uses nothing` parses as `[EffectAnnotation::Nothing]`.
#[test]
fn test_parse_uses_nothing() {
    let m = parse_str("fn add(a: int, b: int) uses nothing to int { a + b }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert_eq!(f.effects, vec![EffectAnnotation::Nothing]);
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

/// `uses net` parses correctly.
#[test]
fn test_parse_uses_single_effect() {
    let m = parse_str("fn fetch() uses net to str { \"ok\" }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert_eq!(f.effects, vec![EffectAnnotation::Net]);
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

/// `uses net, db` parses as two effects.
#[test]
fn test_parse_uses_multiple_effects() {
    let m = parse_str("fn save() uses net, db to bool { true }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert_eq!(f.effects, vec![EffectAnnotation::Net, EffectAnnotation::Db]);
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

/// `@uses(net, env)` — `env` is a dedicated lexer keyword (`Token::Env`), not `Ident`.
#[test]
fn test_parse_at_uses_decorator_accepts_env_keyword_token() {
    let m = parse_str("@uses(net, env)\nfn x() to str { \"\" }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert_eq!(
                f.effects,
                vec![EffectAnnotation::Net, EffectAnnotation::Env]
            );
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

/// `uses mcp(vox_notify)` parses as `Mcp("vox_notify")`.
#[test]
fn test_parse_uses_mcp_parameterized() {
    let m = parse_str("fn notify() uses mcp(vox_notify) to bool { true }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert_eq!(f.effects, vec![EffectAnnotation::Mcp("vox_notify".into())]);
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

/// Missing `uses` clause leaves `effects` empty.
#[test]
fn test_parse_no_uses_clause_is_empty() {
    let m = parse_str("fn pure_fn(x: int) to int { x + 1 }");
    match &m.declarations[0] {
        Decl::Function(f) => {
            assert!(
                f.effects.is_empty(),
                "expected empty effects for unannotated fn"
            );
        }
        other => panic!("Expected Decl::Function, got {other:?}"),
    }
}

// ── ADR-032: .vox.ui module-scope reactive members ──────────────────────────

/// Source classification: regular `.vox` file rejects module-scope `state`.
#[test]
fn module_scope_state_in_source_file_is_a_parse_error() {
    let tokens = lex("state count: int = 0");
    assert!(
        super::parse_with_kind(tokens, crate::module::FileKind::Source).is_err(),
        "module-scope `state` must not parse in a regular .vox file"
    );
}

/// `.vox.ui` classification: module-scope `state` parses cleanly into a
/// ReactiveModuleDecl.
#[test]
fn module_scope_state_in_reactive_module_parses_into_reactive_module_decl() {
    let tokens = lex("state count: int = 0\nstate flag: bool = true");
    let module = super::parse_with_kind(tokens, crate::module::FileKind::ReactiveModule)
        .expect("`.vox.ui` source should parse");
    assert_eq!(module.declarations.len(), 1);
    match &module.declarations[0] {
        Decl::ReactiveModule(r) => {
            assert_eq!(r.members.len(), 2);
            assert!(matches!(
                r.members[0],
                crate::ast::decl::ReactiveMemberDecl::State(_)
            ));
        }
        other => panic!("expected ReactiveModule, got {other:?}"),
    }
}

/// `.vox.ui` files allow `derived` and `effect` at module scope alongside `state`.
#[test]
fn module_scope_derived_and_effect_in_reactive_module_parse() {
    let tokens = lex("state count: int = 0\nderived doubled = count * 2\neffect: { print(count) }");
    let module = super::parse_with_kind(tokens, crate::module::FileKind::ReactiveModule)
        .expect("`.vox.ui` derived+effect should parse");
    match &module.declarations[0] {
        Decl::ReactiveModule(r) => {
            assert_eq!(r.members.len(), 3);
        }
        other => panic!("expected ReactiveModule, got {other:?}"),
    }
}

// ── ADR-033: typed parametric fragment primitive ────────────────────────────

/// Fragment with no parameters — body is parsed as a Phase-6 primitive markup
/// expression (raw JSX is removed from the parser as of TASK-6.1).
#[test]
fn fragment_decl_with_no_params_parses() {
    let m = parse_str("fragment Greeting() { text() { \"hi\" } }");
    assert_eq!(m.declarations.len(), 1);
    match &m.declarations[0] {
        Decl::Fragment(f) => {
            assert_eq!(f.name, "Greeting");
            assert!(f.params.is_empty());
        }
        other => panic!("expected Fragment, got {other:?}"),
    }
}

/// Fragment with typed parameters parses; param names and types preserved.
#[test]
fn fragment_decl_with_params_parses() {
    let m = parse_str("fragment Row(item: str, idx: int) { text() { \"row\" } }");
    match &m.declarations[0] {
        Decl::Fragment(f) => {
            assert_eq!(f.name, "Row");
            assert_eq!(f.params.len(), 2);
            assert_eq!(f.params[0].name, "item");
            assert_eq!(f.params[1].name, "idx");
        }
        other => panic!("expected Fragment, got {other:?}"),
    }
}
