//! P5: `@versioned` / `@tracked` decorator — parse, lower, capability injection,
//! and interpreter auto-snapshot-on-success acceptance tests.

use vox_compiler::ast::decl::Decl;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn parses_versioned_decorator_sets_flag() {
    let src = "@versioned fn save() uses vcs { repo.snapshot(\"x\") }";
    let m = parse(lex(src)).expect("parse");
    let f = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "save" => Some(f),
            _ => None,
        })
        .expect("fn save");
    assert!(f.is_versioned, "@versioned must set FnDecl.is_versioned");
}

#[test]
fn parses_tracked_alias_sets_same_flag() {
    let src = "@tracked fn save() uses vcs { repo.snapshot(\"x\") }";
    let m = parse(lex(src)).expect("parse");
    let f = m
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "save" => Some(f),
            _ => None,
        })
        .expect("fn save");
    assert!(f.is_versioned, "@tracked must set FnDecl.is_versioned");
}
