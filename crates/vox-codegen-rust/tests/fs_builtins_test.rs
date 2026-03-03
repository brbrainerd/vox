use vox_hir::{HirExpr, HirArg};
use vox_ast::span::Span;
use vox_codegen_rust::emit_expr::emit_expr;

fn dummy_span() -> Span {
    Span { start: 0, end: 0 }
}

fn str_arg(s: &str) -> HirArg {
    HirArg {
        name: None,
        value: HirExpr::StringLit(s.to_string(), dummy_span()),
    }
}

fn ident(name: &str) -> Box<HirExpr> {
    Box::new(HirExpr::Ident(name.to_string(), dummy_span()))
}

fn field(obj: Box<HirExpr>, field: &str) -> Box<HirExpr> {
    Box::new(HirExpr::FieldAccess(obj, field.to_string(), dummy_span()))
}

fn call(callee: Box<HirExpr>, args: Vec<HirArg>) -> HirExpr {
    HirExpr::Call(callee, args, false, dummy_span())
}

#[test]
fn fs_builtins_emit_correctly() {
    // std.fs.read(path) → std::fs::read_to_string(path)?
    let expr = call(
        field(field(ident("std"), "fs"), "read"),
        vec![str_arg("/tmp/file.txt")],
    );
    let out = emit_expr(&expr);
    assert!(
        out.contains("std::fs::read_to_string"),
        "Expected std::fs::read_to_string, got: {out}"
    );

    // std.fs.write(path, content)
    let expr2 = call(
        field(field(ident("std"), "fs"), "write"),
        vec![str_arg("/tmp/out.txt"), str_arg("hello")],
    );
    let out2 = emit_expr(&expr2);
    assert!(
        out2.contains("std::fs::write"),
        "Expected std::fs::write, got: {out2}"
    );

    // std.fs.exists(path)
    let expr3 = call(
        field(field(ident("std"), "fs"), "exists"),
        vec![str_arg("/tmp/x")],
    );
    let out3 = emit_expr(&expr3);
    assert!(
        out3.contains("std::path::Path::new"),
        "Expected std::path::Path::new, got: {out3}"
    );

    // std.path.join(a, b)
    let expr4 = call(
        field(field(ident("std"), "path"), "join"),
        vec![str_arg("/tmp"), str_arg("file.txt")],
    );
    let out4 = emit_expr(&expr4);
    assert!(
        out4.contains("join"),
        "Expected join, got: {out4}"
    );

    // std.path.basename(path)
    let expr5 = call(
        field(field(ident("std"), "path"), "basename"),
        vec![str_arg("/tmp/file.txt")],
    );
    let out5 = emit_expr(&expr5);
    assert!(
        out5.contains("file_name"),
        "Expected file_name, got: {out5}"
    );
}

#[test]
fn fs_remove_emits_remove_file() {
    let expr = call(
        field(field(ident("std"), "fs"), "remove"),
        vec![str_arg("/tmp/old.txt")],
    );
    let out = emit_expr(&expr);
    assert!(out.contains("std::fs::remove_file"), "Expected remove_file, got: {out}");
}

#[test]
fn fs_mkdir_emits_create_dir_all() {
    let expr = call(
        field(field(ident("std"), "fs"), "mkdir"),
        vec![str_arg("/tmp/newdir")],
    );
    let out = emit_expr(&expr);
    assert!(out.contains("create_dir_all"), "Expected create_dir_all, got: {out}");
}
