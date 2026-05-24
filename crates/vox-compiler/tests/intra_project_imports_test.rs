//! Integration test for intra-project Vox-file imports
//! (`import "./helpers/foo.vox"` + `pub fn`).
//!
//! Covers the four invariants that the RFC §3 + audit §11.7 promise:
//!
//! 1. **Bare import** brings `pub fn`s into the importer's scope; non-`pub`
//!    fns stay file-private.
//! 2. **Alias import** (`import "./foo.vox" as alias`) exposes pubs under
//!    `alias.fn_name(...)` at run time.
//! 3. **Cycle detection**: `a.vox` importing `b.vox` importing `a.vox` does
//!    not loop; both files' pubs are visible to a `fn main()` that calls
//!    across the cycle.
//! 4. **Typecheck visibility**: `vox check` succeeds on bare-form imports
//!    (pub fns flow into TypeEnv) and rejects calls to non-pub fns.
//!
//! Without this test the surface depends on the corpus baseline to catch
//! regressions, which is too coarse — a change to e.g. the FileResolver
//! could break privacy without flipping any committed script.

use std::fs;
use std::path::Path;

use tempfile::TempDir;
use vox_compiler::eval::Interpreter;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::{typecheck_hir_module_with_path, TypeckSeverity};

/// Lower the file at `path` and run it; returns Ok on success, Err with the
/// EvalError debug-formatted on failure. Sets the interpreter's source path
/// so relative imports resolve against `path.parent()`.
fn run_file(path: &Path) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let tokens = lex(&source);
    let module = parse_script(tokens).map_err(|errs| format!("parse failed: {} error(s)", errs.len()))?;
    let lowered = lower_module(&module);

    let mut interp = Interpreter::new(10_000_000);
    interp.set_source_path(path.to_path_buf());
    interp.run_module(&lowered).map_err(|e| format!("run_module: {e:?}"))?;
    interp
        .call("main", Vec::new())
        .map(|_| ())
        .map_err(|e| format!("call main: {e:?}"))
}

/// Typecheck the file at `path`, returning the list of error-severity
/// diagnostic messages (empty on clean check).
fn check_file_errors(path: &Path) -> Vec<String> {
    let source = fs::read_to_string(path).expect("read source");
    let tokens = lex(&source);
    let module = parse_script(tokens).expect("parse");
    let mut hir = lower_module(&module);
    let diags = typecheck_hir_module_with_path(&source, &mut hir, Some(path));
    diags
        .into_iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .map(|d| d.message)
        .collect()
}

#[test]
fn bare_import_pub_fn_resolves_and_private_is_hidden() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("helpers")).unwrap();
    fs::write(
        dir.path().join("helpers/greet.vox"),
        "pub fn shout(msg: str) to str { return msg + \"!\" }\n\
         fn private_helper() to str { return \"hidden\" }\n",
    )
    .unwrap();

    let main_pub = dir.path().join("main_pub.vox");
    fs::write(
        &main_pub,
        "import \"./helpers/greet.vox\"\n\
         fn main() { print(shout(\"hi\")) }\n",
    )
    .unwrap();
    run_file(&main_pub).expect("bare pub-fn call should succeed at runtime");

    let main_private = dir.path().join("main_private.vox");
    fs::write(
        &main_private,
        "import \"./helpers/greet.vox\"\n\
         fn main() { print(private_helper()) }\n",
    )
    .unwrap();
    let err = run_file(&main_private).expect_err("private fn should not be visible at runtime");
    assert!(
        err.contains("UndefinedVariable") && err.contains("private_helper"),
        "expected UndefinedVariable(\"private_helper\"), got: {err}",
    );
}

#[test]
fn typecheck_resolves_pubs_and_rejects_private() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("helpers")).unwrap();
    fs::write(
        dir.path().join("helpers/greet.vox"),
        "pub fn shout(msg: str) to str { return msg + \"!\" }\n\
         fn private_helper() to str { return \"hidden\" }\n",
    )
    .unwrap();

    let main_pub = dir.path().join("main_pub.vox");
    fs::write(
        &main_pub,
        "import \"./helpers/greet.vox\"\n\
         fn main() { print(shout(\"hi\")) }\n",
    )
    .unwrap();
    let errs = check_file_errors(&main_pub);
    assert!(errs.is_empty(), "expected clean check, got errors: {errs:?}");

    let main_private = dir.path().join("main_private.vox");
    fs::write(
        &main_private,
        "import \"./helpers/greet.vox\"\n\
         fn main() { print(private_helper()) }\n",
    )
    .unwrap();
    let errs = check_file_errors(&main_private);
    assert!(
        errs.iter().any(|m| m.contains("private_helper")),
        "expected an Undefined-variable error mentioning `private_helper`, got: {errs:?}",
    );
}

#[test]
fn alias_form_typecheck_resolves_namespace_method() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("helpers")).unwrap();
    fs::write(
        dir.path().join("helpers/greet.vox"),
        "pub fn shout(msg: str) to str { return msg + \"!\" }\n",
    )
    .unwrap();
    let main = dir.path().join("main.vox");
    fs::write(
        &main,
        "import \"./helpers/greet.vox\" as g\n\
         fn main() { print(g.shout(\"aliased\")) }\n",
    )
    .unwrap();
    let errs = check_file_errors(&main);
    assert!(
        errs.is_empty(),
        "expected clean check for aliased namespace method, got: {errs:?}",
    );
}

#[test]
fn alias_form_typecheck_rejects_unknown_method() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("helpers")).unwrap();
    fs::write(
        dir.path().join("helpers/greet.vox"),
        "pub fn shout(msg: str) to str { return msg + \"!\" }\n",
    )
    .unwrap();
    let main = dir.path().join("main.vox");
    fs::write(
        &main,
        "import \"./helpers/greet.vox\" as g\n\
         fn main() { print(g.whisper(\"x\")) }\n",
    )
    .unwrap();
    let errs = check_file_errors(&main);
    assert!(
        !errs.is_empty(),
        "expected at least one error for unknown alias method",
    );
}

#[test]
fn alias_form_namespace_dispatch_runtime() {
    let dir = TempDir::new().unwrap();
    fs::create_dir(dir.path().join("helpers")).unwrap();
    fs::write(
        dir.path().join("helpers/greet.vox"),
        "pub fn shout(msg: str) to str { return msg + \"!\" }\n",
    )
    .unwrap();
    let main = dir.path().join("main.vox");
    fs::write(
        &main,
        "import \"./helpers/greet.vox\" as g\n\
         fn main() { print(g.shout(\"aliased\")) }\n",
    )
    .unwrap();
    run_file(&main).expect("aliased pub-fn call should succeed at runtime");
}

#[test]
fn cycle_a_imports_b_imports_a_resolves_without_infinite_loop() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("a.vox"),
        "import \"./b.vox\"\n\
         pub fn from_a() to str { return \"A\" }\n\
         fn main() { print(from_b()) }\n",
    )
    .unwrap();
    fs::write(
        dir.path().join("b.vox"),
        "import \"./a.vox\"\n\
         pub fn from_b() to str { return \"B-\" + from_a() }\n",
    )
    .unwrap();
    run_file(&dir.path().join("a.vox")).expect("a↔b cycle should resolve cleanly");
}
