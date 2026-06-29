//! Core-surface taxonomy — soft-keyword parse + retired-decorator rejection.
//!
//! Spec: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md (rev2).
//! Post hard-error flip: the Tier-1/2 `@`-decorator heads are a parse ERROR carrying
//! a machine-readable Replacement payload (from→to→code); only the soft-keyword form
//! parses. (Pre-flip this harness proved `@` ≡ keyword AST-equivalence; that property
//! is now enforced by rejecting `@` outright and steering authors to the keyword.)

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

/// The soft-keyword form must parse cleanly.
fn keyword_parses(src: &str) {
    parse(lex(src)).unwrap_or_else(|e| panic!("keyword form must parse: {src}\n{e:?}"));
}

/// The retired `@` form must be rejected with the replacement payload from→to→code.
fn decorator_rejected(src: &str, from: &str, to: &str) {
    let code = format!("vox/decorator/{to}-retired");
    let errs = parse(lex(src)).expect_err(&format!("retired decorator must error: {src}"));
    assert!(
        errs.iter().any(|e| {
            e.replacement
                .as_ref()
                .is_some_and(|r| r.from == from && r.to == to && r.code == code)
        }),
        "expected retired-decorator payload {from}->{to} ({code}) for `{src}`, got: {errs:?}"
    );
}

// ── Tier-1/2: keyword form parses; the `@` form is rejected with its payload ──

#[test]
fn table() {
    keyword_parses("table User { name: str }");
    decorator_rejected("@table type User { name: str }", "@table", "table");
}

#[test]
fn table_pk() {
    keyword_parses("table(pk: uid) User { uid: int }");
    decorator_rejected("@table(pk: uid) type User { uid: int }", "@table", "table");
}

#[test]
fn index() {
    keyword_parses("index User.by_name on (name)");
    decorator_rejected("@index User.by_name on (name)", "@index", "index");
}

#[test]
fn query() {
    keyword_parses("query c() to int { return 0 }");
    decorator_rejected("@query fn c() to int { return 0 }", "@query", "query");
}

#[test]
fn mutation() {
    keyword_parses("mutation add(b: str) to int { return 0 }");
    decorator_rejected("@mutation fn add(b: str) to int { return 0 }", "@mutation", "mutation");
}

#[test]
fn server() {
    keyword_parses("server handler() to int { return 0 }");
    decorator_rejected("@server fn handler() to int { return 0 }", "@server", "server");
}

#[test]
fn tool_empty_description() {
    keyword_parses("tool search(q: str) to str { return q }");
    decorator_rejected("@tool fn search(q: str) to str { return q }", "@tool", "tool");
}

#[test]
fn tool_with_description() {
    keyword_parses("tool \"web\" search(q: str) to str { return q }");
    decorator_rejected(
        "@tool \"web\" fn search(q: str) to str { return q }",
        "@tool",
        "tool",
    );
}

#[test]
fn resource() {
    keyword_parses("resource \"u\" \"d\" load() to str { return \"\" }");
    decorator_rejected(
        "@resource(\"u\", \"d\") fn load() to str { return \"\" }",
        "@resource",
        "resource",
    );
}

#[test]
fn form() {
    keyword_parses("form Signup { field email: str\n on_submit: register }");
    decorator_rejected(
        "@form Signup { field email: str\n on_submit: register }",
        "@form",
        "form",
    );
}

#[test]
fn tier1_decorators_now_rejected() {
    // The hard-error flip: every retired `@` head errors (was a warning during the
    // warning-first rollout). Each carries the machine-readable replacement payload.
    decorator_rejected("@table type User { name: str }", "@table", "table");
    decorator_rejected("@index User.by_name on (name)", "@index", "index");
    decorator_rejected("@query fn c() to int { return 0 }", "@query", "query");
    decorator_rejected("@mutation fn m(b: str) to int { return 0 }", "@mutation", "mutation");
    decorator_rejected("@server fn h() to int { return 0 }", "@server", "server");
    decorator_rejected("@tool fn t(q: str) to str { return q }", "@tool", "tool");
    decorator_rejected("@resource \"u\" \"d\" fn r() to str { return \"\" }", "@resource", "resource");
    decorator_rejected("@form Signup { field a: str\n on_submit: x }", "@form", "form");
}

#[test]
fn soft_keyword_recognized_in_script_mode() {
    use vox_compiler::parser::parse_script;
    // Soft keywords must be decl-position in SCRIPT mode too (parse_module_script has
    // its own is_decl_position gate); the retired `@` form must error there as well.
    parse_script(lex("query c() to int { return 0 }")).expect("keyword parses (script)");
    assert!(
        parse_script(lex("@query fn c() to int { return 0 }")).is_err(),
        "retired @query must error in script mode too"
    );
}

// ── Identifier preservation: the words stay Token::Ident — usable as field/param/
//    local names everywhere but a declaration head. ──

#[test]
fn ident_uses_preserved() {
    for src in [
        "type Search { query: str }",
        "fn f(query: str, resource: str, table: str) to int { return 0 }",
        "query g() to int { return len(db.query()) }",
        "fn h() to int { let table = 1\n return table }",
    ] {
        parse(lex(src)).unwrap_or_else(|e| panic!("must still parse: {src}\n{e:?}"));
    }
}

#[test]
fn form_as_identifier_preserved() {
    for src in [
        "fn f(form: str) to int { return 0 }",
        "type T { form: str }",
    ] {
        parse(lex(src)).unwrap_or_else(|e| panic!("must still parse: {src}\n{e:?}"));
    }
}

// ── Invariant-2 guards: the optional-`fn` relaxation is keyword-path-ONLY. ──

#[test]
fn headless_query_parses() {
    parse(lex("query f() to int { return 1 }")).expect("headless query parses standalone");
}

#[test]
fn keyword_form_shrinks_tokens_and_bytes() {
    // The program's whole point: the soft-keyword form costs fewer lexer tokens AND
    // fewer source bytes than the decorator form it replaced. (Lexing the `@` form
    // still works — the flip is a *parse* error, not a lex error.)
    let cases = [
        ("@table type User { name: str }", "table User { name: str }"),
        ("@query fn c() to int { return 0 }", "query c() to int { return 0 }"),
        ("@mutation fn m() to int { return 0 }", "mutation m() to int { return 0 }"),
        ("@server fn s() to int { return 0 }", "server s() to int { return 0 }"),
        ("@tool fn t() to int { return 0 }", "tool t() to int { return 0 }"),
    ];
    for (decorated, keyword) in cases {
        assert!(
            lex(keyword).len() < lex(decorated).len(),
            "keyword fewer tokens: {keyword} !< {decorated}"
        );
        assert!(
            keyword.len() < decorated.len(),
            "keyword fewer bytes: {keyword} !< {decorated}"
        );
    }
}

#[test]
fn mcp_resource_still_parses_after_arm_split() {
    // @mcp.resource is a SEPARATE legacy token (not a flipped Tier-1 head) — it still
    // routes to the non-headless parse_mcp_resource and parses.
    parse(lex(
        "@mcp.resource \"vox://x\" \"d\" fn load() to str { return \"\" }",
    ))
    .unwrap_or_else(|e| panic!("@mcp.resource must still parse: {e:?}"));
}

#[test]
fn bare_call_still_rejected_at_toplevel() {
    assert!(
        parse(lex("foo() { }")).is_err(),
        "bare top-level call must remain a parse error"
    );
}

#[test]
fn decorator_without_fn_still_errors() {
    assert!(
        parse(lex("@pure foo() to int { return 0 }")).is_err(),
        "decorator without `fn` must remain a parse error"
    );
}
