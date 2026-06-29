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
    ] {
        parse(lex(src)).unwrap_or_else(|e| panic!("must still parse: {src}\n{e:?}"));
    }
}
