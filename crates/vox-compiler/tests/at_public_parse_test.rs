/// Task 8J: @public decorator parsing.
///
/// `@public` opts a `@server`/`@query`/`@mutation` fn out of the `@auth`
/// requirement. It is a prefix modifier that sets `FnDecl::is_pub = true`.
use vox_compiler::{lexer::lex, parser::descent::parse};

fn parse_ok(src: &str) {
    let toks = lex(src);
    let result = parse(toks);
    assert!(result.is_ok(), "parse failed for: {src}\nerr: {:?}", result.err());
}

#[test]
fn at_public_server_fn_parses() {
    parse_ok("@public @server\nfn get_health() -> Str { \"ok\" }\n");
}

#[test]
fn at_public_query_fn_parses() {
    parse_ok("@public @query\nfn list_products(limit: Int) -> Str { \"[]\" }\n");
}

#[test]
fn at_public_mutation_fn_parses() {
    parse_ok("@public @mutation\nfn create_session(token: Str) -> Str { \"ok\" }\n");
}

#[test]
fn at_public_plain_fn_parses() {
    // @public on a plain fn is also valid (marks it exported)
    parse_ok("@public fn helper(x: Int) -> Int { x }\n");
}

#[test]
fn at_public_sets_is_pub_true() {
    use vox_compiler::ast::decl::Decl;
    let toks = lex("@public @server\nfn get_health() -> Str { \"ok\" }\n");
    let module = parse(toks).expect("parse");
    let decl = module.declarations.iter().find(|d| matches!(d, Decl::Endpoint(_)));
    let Decl::Endpoint(ep) = decl.expect("endpoint decl") else {
        panic!("expected Endpoint decl");
    };
    assert!(ep.func.is_pub, "@public must set FnDecl::is_pub = true");
}
