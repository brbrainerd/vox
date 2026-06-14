/// Task 4B: @auth decorator args parsing.
///
/// `@auth(provider: "clerk", roles: ["admin"])` must populate
/// `FnDecl::auth_provider` and `FnDecl::roles`.
///
/// Note: @server/@query/@mutation are consumed by the top-level dispatcher in
/// descent/mod.rs before parse_fn_decl is called, so @auth must come AFTER the
/// endpoint-kind decorator in source (e.g. `@server @auth(...) fn ...`).
use vox_compiler::{ast::decl::Decl, lexer::lex, parser::descent::parse};

fn first_endpoint_fn(src: &str) -> vox_compiler::ast::decl::FnDecl {
    let toks = lex(src);
    let module = parse(toks).expect("parse");
    for d in &module.declarations {
        if let Decl::Endpoint(e) = d {
            return e.func.clone();
        }
    }
    panic!("no endpoint decl found in: {src}");
}

fn first_fn(src: &str) -> vox_compiler::ast::decl::FnDecl {
    let toks = lex(src);
    let module = parse(toks).expect("parse");
    for d in &module.declarations {
        if let Decl::Function(f) = d {
            return f.clone();
        }
    }
    panic!("no fn decl found in: {src}");
}

#[test]
fn auth_provider_is_parsed() {
    // @server is consumed first by dispatch; @auth is handled inside parse_fn_decl
    let f = first_endpoint_fn("@server @auth(provider: \"clerk\")\nfn admin() -> Str { \"ok\" }\n");
    assert_eq!(
        f.auth_provider.as_deref(),
        Some("clerk"),
        "auth_provider must be set from @auth(provider: ...)"
    );
}

#[test]
fn auth_roles_are_parsed() {
    let f = first_endpoint_fn(
        "@server @auth(provider: \"clerk\", roles: [\"admin\", \"editor\"])\nfn admin_only() -> Str { \"ok\" }\n",
    );
    assert_eq!(
        f.roles,
        vec!["admin".to_string(), "editor".to_string()],
        "roles must be populated from @auth(roles: [...])"
    );
}

#[test]
fn auth_without_args_sets_presence_marker() {
    let f = first_fn("@auth fn protected() -> Str { \"ok\" }\n");
    assert!(
        f.auth_provider.is_some(),
        "@auth with no args must still set auth_provider (presence marker)"
    );
}

#[test]
fn auth_provider_only_leaves_roles_empty() {
    let f =
        first_endpoint_fn("@server @auth(provider: \"supabase\")\nfn api() -> Str { \"ok\" }\n");
    assert!(
        f.roles.is_empty(),
        "roles must be empty when not specified in @auth"
    );
}
