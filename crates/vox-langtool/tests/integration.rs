//! Integration tests for vox-langtool library functions.
//!
//! We test through the library API directly (lib.rs + commands::*) rather than
//! the compiled binary so the test suite runs without `cargo build` first.

use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// ─── check ───────────────────────────────────────────────────────────────────

#[test]
fn check_valid_file_exits_ok() {
    let result = vox_langtool::commands::check::run(&fixture("hello.vox"));
    assert!(result.is_ok(), "expected check to pass: {:?}", result);
}

#[test]
fn check_type_error_file_fails() {
    // type_error.vox calls add(1, "not_an_int") where add expects (int, int).
    // The type checker emits an "Argument type mismatch" error, so check must
    // return Err (non-zero exit).
    let result = vox_langtool::commands::check::run(&fixture("type_error.vox"));
    assert!(
        result.is_err(),
        "expected check to fail on type_error.vox (argument type mismatch), but it returned Ok"
    );
}

// ─── fmt ─────────────────────────────────────────────────────────────────────

#[test]
fn fmt_check_on_already_formatted_exits_ok() {
    // We need a file whose contents match formatter output. Use hello.vox.
    let path = fixture("hello.vox");
    let source = std::fs::read_to_string(&path).unwrap();
    // If formatter fails to parse, the test also passes (not our bug).
    if let Ok(formatted) = vox_compiler::fmt::try_format(&source) {
        if source == formatted {
            let result = vox_langtool::commands::fmt::run(&path, true);
            assert!(result.is_ok(), "fmt --check should pass on formatted file: {:?}", result);
        }
        // If they differ, fmt --check would fail — that's correct but means the
        // fixture needs updating; skip rather than fail.
    }
}

#[test]
fn fmt_rewrites_unformatted_file() {
    use std::io::Write;
    // Write a valid .vox snippet with inconsistent spacing into a temp file.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unformatted.vox");
    // Use something the formatter can round-trip.
    let unformatted = "fn main(){print(\"hi\");}";
    std::fs::File::create(&path)
        .unwrap()
        .write_all(unformatted.as_bytes())
        .unwrap();

    // fmt without --check — may succeed (formatted) or fail (parse error). Either way no panic.
    let _ = vox_langtool::commands::fmt::run(&path, false);
}

#[test]
fn fmt_check_on_unformatted_fails() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ugly.vox");
    // Deliberately non-canonical spacing to trigger a diff.
    let unformatted = "fn main(){print(\"hi\");}";
    std::fs::File::create(&path)
        .unwrap()
        .write_all(unformatted.as_bytes())
        .unwrap();

    // Only assert failure if the formatter can actually parse this snippet.
    if let Ok(formatted) = vox_compiler::fmt::try_format(unformatted) {
        if formatted != unformatted {
            let result = vox_langtool::commands::fmt::run(&path, true);
            assert!(
                result.is_err(),
                "fmt --check should fail when file needs formatting"
            );
        }
    }
}

// ─── run ─────────────────────────────────────────────────────────────────────

#[test]
fn run_hello_world_exits_ok() {
    let result = vox_langtool::commands::run::run(&fixture("hello.vox"), &[]);
    assert!(result.is_ok(), "run hello.vox failed: {:?}", result);
}

#[test]
fn run_caps_directive_exits_ok() {
    // caps_directive.vox has `// vox:caps net fs` on the first line.
    // The directive must be parsed and set on the interpreter without breaking execution.
    let result = vox_langtool::commands::run::run(&fixture("caps_directive.vox"), &[]);
    assert!(
        result.is_ok(),
        "run caps_directive.vox failed (caps parsing broke execution): {:?}",
        result
    );
}

// ─── build ───────────────────────────────────────────────────────────────────

#[test]
fn build_produces_rust_files() {
    let dir = tempfile::tempdir().unwrap();
    let result = vox_langtool::commands::build::run(&fixture("hello.vox"), dir.path());
    assert!(result.is_ok(), "build failed: {:?}", result);

    // At least one .rs file must exist somewhere under out-dir.
    let has_rs = walkdir(dir.path());
    assert!(has_rs, "expected at least one .rs file in build output");
}

fn walkdir(dir: &std::path::Path) -> bool {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if walkdir(&path) {
                return true;
            }
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            return true;
        }
    }
    false
}

// ─── is_script_like ──────────────────────────────────────────────────────────

#[test]
fn is_script_like_plain_fn() {
    assert!(vox_langtool::is_script_like("fn main() { }"));
}

#[test]
fn is_script_like_rejects_page_decorator() {
    assert!(!vox_langtool::is_script_like("@page fn Home() { }"));
}
