//! P0 core-surface taxonomy — soft-keyword ⇄ decorator AST-equivalence harness.
//!
//! Spec: docs/superpowers/specs/2026-06-29-core-surface-taxonomy-design.md (rev2).
//! The safety property: a Tier-1 soft keyword parses to the SAME AST node (modulo
//! spans) the decorator did, so HIR/codegen are unchanged. Equality is serde-based
//! (mirrors the `strip_spans` pattern at hir/lower/mod.rs:733), immune to Debug drift.

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;

/// Null every `{start,end}` span object so structural shape alone is compared.
fn strip_spans(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            if map.len() == 2 && map.contains_key("start") && map.contains_key("end") {
                *v = serde_json::Value::Null;
                return;
            }
            for val in map.values_mut() {
                strip_spans(val);
            }
        }
        serde_json::Value::Array(arr) => {
            for val in arr.iter_mut() {
                strip_spans(val);
            }
        }
        _ => {}
    }
}

/// Assert the decorator form and the soft-keyword form parse to identical ASTs.
fn ast_eq(old_src: &str, new_src: &str) {
    let a = parse(lex(old_src)).expect("decorator form must parse");
    let b = parse(lex(new_src)).expect("keyword form must parse");
    let mut va = serde_json::to_value(&a).expect("serialize old AST");
    let mut vb = serde_json::to_value(&b).expect("serialize new AST");
    strip_spans(&mut va);
    strip_spans(&mut vb);
    assert_eq!(va, vb, "keyword form must be AST-equivalent to decorator form");
}

// ── Tier-1 equivalence (these FAIL until the soft-keyword heads land) ──

#[test]
fn table() {
    ast_eq("@table type User { name: str }", "table User { name: str }");
}

#[test]
fn table_pk() {
    ast_eq(
        "@table(pk: uid) type User { uid: int }",
        "table(pk: uid) User { uid: int }",
    );
}

#[test]
fn index() {
    ast_eq("@index User.by_name on (name)", "index User.by_name on (name)");
}

#[test]
fn query() {
    ast_eq(
        "@query fn c() to int { return 0 }",
        "query c() to int { return 0 }",
    );
}

#[test]
fn mutation() {
    ast_eq(
        "@mutation fn add(b: str) to int { return 0 }",
        "mutation add(b: str) to int { return 0 }",
    );
}

#[test]
fn server() {
    ast_eq(
        "@server fn handler() to int { return 0 }",
        "server handler() to int { return 0 }",
    );
}

#[test]
fn tool_empty_description() {
    // No string on either side → both description="".
    ast_eq(
        "@tool fn search(q: str) to str { return q }",
        "tool search(q: str) to str { return q }",
    );
}

#[test]
fn tool_with_description() {
    // The @tool decorator takes a BARE string (no parens) per parse_mcp_tool
    // (head.rs:50) — the LSP snippet's `@tool("…")` form does not actually parse.
    ast_eq(
        "@tool \"web\" fn search(q: str) to str { return q }",
        "tool \"web\" search(q: str) to str { return q }",
    );
}

#[test]
fn resource() {
    ast_eq(
        "@resource(\"u\", \"d\") fn load() to str { return \"\" }",
        "resource \"u\" \"d\" load() to str { return \"\" }",
    );
}

#[test]
fn soft_keyword_recognized_in_script_mode() {
    use vox_compiler::parser::parse_script;
    // Soft keywords must be decl-position in SCRIPT mode too (not just module mode) —
    // parse_module_script has its own is_decl_position gate. Assert the keyword form
    // parses identically to the decorator form there, not as a statement.
    let mut old = serde_json::to_value(
        parse_script(lex("@query fn c() to int { return 0 }")).expect("decorator parses (script)"),
    )
    .unwrap();
    let mut new = serde_json::to_value(
        parse_script(lex("query c() to int { return 0 }")).expect("keyword parses (script)"),
    )
    .unwrap();
    strip_spans(&mut old);
    strip_spans(&mut new);
    assert_eq!(old, new, "soft keyword must parse like the decorator in script mode");
}

// ── Identifier preservation (these must PASS even before the heads land:
//    the words stay Token::Ident; this is the proof soft keywords don't steal
//    the value namespace). Task 0 confirmed zero decl-head collisions. ──

#[test]
fn ident_uses_preserved() {
    for src in [
        // field name
        "type Search { query: str }",
        // param names (query, resource, table)
        "fn f(query: str, resource: str, table: str) to int { return 0 }",
        // method call on the db surface
        "@query fn g() to int { return len(db.query()) }",
        // local `let` binding named with a soft keyword (must not be stolen in body)
        "fn h() to int { let table = 1\n return table }",
    ] {
        parse(lex(src)).unwrap_or_else(|e| panic!("must still parse: {src}\n{e:?}"));
    }
}

// ── Invariant-2 guards: the optional-`fn` relaxation is keyword-path-ONLY and did
//    NOT widen the grammar. (Flagged by code review — these were specified in the
//    plan but missing from the harness.) ──

#[test]
fn headless_query_parses() {
    parse(lex("query f() to int { return 1 }")).expect("headless query parses standalone");
}

#[test]
fn keyword_form_shrinks_tokens_and_bytes() {
    // The program's whole point: the soft-keyword form costs fewer lexer tokens AND
    // fewer source bytes than the decorator form it replaces (the keyword subsumes
    // the `@` + `fn`/`type` token pair). This is what the `vox ci source-token-budget`
    // gate witnesses once the corpus migrates.
    let cases = [
        ("@table type User { name: str }", "table User { name: str }"),
        ("@query fn c() to int { return 0 }", "query c() to int { return 0 }"),
        ("@mutation fn m() to int { return 0 }", "mutation m() to int { return 0 }"),
        ("@server fn s() to int { return 0 }", "server s() to int { return 0 }"),
        ("@tool fn t() to int { return 0 }", "tool t() to int { return 0 }"),
    ];
    for (decorated, keyword) in cases {
        let d_tokens = lex(decorated).len();
        let k_tokens = lex(keyword).len();
        assert!(
            k_tokens < d_tokens,
            "keyword fewer tokens: {keyword} ({k_tokens}) !< {decorated} ({d_tokens})"
        );
        assert!(
            keyword.len() < decorated.len(),
            "keyword fewer bytes: {keyword} ({}) !< {decorated} ({})",
            keyword.len(),
            decorated.len()
        );
    }
}

#[test]
fn mcp_resource_still_parses_after_arm_split() {
    // The dispatch split of `AtResource | AtMcpResource` (so only @resource warns) must
    // not change @mcp.resource — it still routes to the non-headless parse_mcp_resource.
    parse(lex("@mcp.resource \"vox://x\" \"d\" fn load() to str { return \"\" }"))
        .unwrap_or_else(|e| panic!("@mcp.resource must still parse: {e:?}"));
}

#[test]
fn tier1_decorators_still_parse_during_warning_first() {
    // Warning-first rollout: the retired `@` forms emit a deprecation warning but
    // STILL parse, so the suite stays green while the codemod migrates the corpus.
    // (The machine-readable replacement payload is asserted at the hard-error flip.)
    for src in [
        "@table type User { name: str }",
        "@index User.by_name on (name)",
        "@query fn c() to int { return 0 }",
        "@mutation fn m(b: str) to int { return 0 }",
        "@server fn h() to int { return 0 }",
        "@tool fn t(q: str) to str { return q }",
        "@resource \"u\" \"d\" fn r() to str { return \"\" }",
    ] {
        parse(lex(src)).unwrap_or_else(|e| panic!("@ form must still parse: {src}\n{e:?}"));
    }
}

#[test]
fn bare_call_still_rejected_at_toplevel() {
    // A bare `foo() {}` (no keyword, no `fn`) must STILL error — the optional-`fn`
    // change must not legalize top-level calls. `foo` is an Ident matching no soft
    // keyword, so it never reaches the headless path.
    assert!(
        parse(lex("foo() { }")).is_err(),
        "bare top-level call must remain a parse error"
    );
}

#[test]
fn decorator_without_fn_still_errors() {
    // `@pure foo()` (decorator, missing `fn`) routes through the MANDATORY-fn path
    // and must error — proving expect→eat was not weakened globally.
    assert!(
        parse(lex("@pure foo() to int { return 0 }")).is_err(),
        "decorator without `fn` must remain a parse error"
    );
}
