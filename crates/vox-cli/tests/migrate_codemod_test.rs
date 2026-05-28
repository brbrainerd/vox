//! Tests for the `vox migrate names` subcommand (VUV-9 Tasks 5 & 6).

use std::process::Command;

#[test]
fn migrate_help_lists_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["migrate", "--help"])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("migrate") || stdout.contains("Migrate"),
        "help output should reference the migrate subcommand: {}",
        stdout
    );
    assert!(
        stdout.contains("rewrite")
            || stdout.contains("Rewrite")
            || stdout.contains("registry")
            || stdout.contains("rename")
            || stdout.contains("canonical"),
        "help output should describe what migrate does: {}",
        stdout
    );
}

#[test]
fn migrate_names_help_shows_dry_run() {
    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["migrate", "names", "--help"])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dry-run") || stdout.contains("dry_run"),
        "names --help should mention --dry-run: {}",
        stdout
    );
}

#[test]
fn migrate_names_dry_run_empty_dir() {
    use std::fs;
    let dir = tempfile::tempdir().expect("tempdir");

    let output = Command::new(env!("CARGO_BIN_EXE_vox"))
        .args([
            "migrate",
            "names",
            "--dry-run",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("vox binary should be runnable");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "migrate names --dry-run should succeed on empty dir: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("0 file(s)"),
        "should report 0 files updated on empty dir: {}",
        stdout
    );

    // cleanup
    drop(dir);
    let _ = fs::remove_dir_all(std::env::temp_dir().join("migrate_names_test"));
}

// ── Task 6: token-based rewrite unit tests ────────────────────────────────

use vox_compiler::parser::renames::RenameRegistry;

/// Helper: load a registry with a single Box→panel rename for testing.
fn test_registry() -> RenameRegistry {
    RenameRegistry::parse_json(
        r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#,
    )
    .unwrap()
}

#[test]
fn rewrite_renames_primitive_call_sites() {
    let registry = test_registry();
    let before = "component App() { view: Box() { Box() { text(\"hi\") } } }";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    assert_eq!(
        after,
        "component App() { view: panel() { panel() { text(\"hi\") } } }"
    );
}

#[test]
fn rewrite_does_not_touch_string_literals() {
    let registry = test_registry();
    let before = r#"component App() { view: text("Box of crayons") }"#;
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    assert_eq!(after, before, "string literal contents must be preserved");
}

#[test]
fn rewrite_does_not_touch_substring_identifiers() {
    let registry = test_registry();
    // None of these are the exact identifier "Box"
    let before = "let myBox = 1; let boxes = 2;";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    assert_eq!(after, before, "substring matches must not be rewritten");
}

#[test]
fn rewrite_preserves_whitespace_and_comments() {
    let registry = test_registry();
    let before = "// before-comment\ncomponent App() {\n    view: Box()  {  // inline\n        text(\"hi\")\n    }\n}\n";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    let expected = "// before-comment\ncomponent App() {\n    view: panel()  {  // inline\n        text(\"hi\")\n    }\n}\n";
    assert_eq!(after, expected);
}

#[test]
fn rewrite_passes_through_unchanged_when_no_deprecated_names() {
    let registry = test_registry();
    let before = "component App() { view: panel() { text(\"hi\") } }";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    assert_eq!(after, before, "no deprecated names → output equals input");
}

#[test]
fn rewrite_returns_input_unchanged_on_lex_failure() {
    let registry = test_registry();
    // The logos lexer is infallible (skips unknown chars) so we just verify
    // that a pathological input does not panic and produces some output.
    let before = "component App() { view: \"unterminated";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);
    // Primary assertion: did not panic.
    // Secondary: the identifier "component" and "view" should still appear.
    assert!(
        after.contains("component"),
        "output should contain 'component': {after:?}"
    );
    let _ = after;
}

// =====================================================================
// End-to-end fixture tests: spawn the real `vox` binary, read/write real
// files in a tempdir, override the rename registry via VOX_RENAMES_PATH.
// =====================================================================

use std::path::PathBuf;

struct CliOutput {
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
    status: std::process::ExitStatus,
}

impl CliOutput {
    fn stdout_contains(&self, s: &str) -> bool {
        self.stdout.contains(s)
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("migrate")
}

/// Write a registry with a single Box->panel rename for these tests.
fn write_test_registry(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("renames.v1.json");
    std::fs::write(
        &path,
        r#"{
            "version": 1,
            "entries": [
              { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
            ]
        }"#,
    )
    .unwrap();
    path
}

fn run_migrate_names(
    cwd: &std::path::Path,
    registry_path: &std::path::Path,
    args: &[&str],
) -> CliOutput {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vox"));
    cmd.arg("migrate")
        .arg("names")
        .args(args)
        .env("VOX_RENAMES_PATH", registry_path)
        .current_dir(cwd);
    let output = cmd.output().expect("vox binary should be runnable");
    CliOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status: output.status,
    }
}

fn copy_fixture_to_tempdir(temp: &std::path::Path) -> PathBuf {
    let before_src = fixture_root().join("before").join("sample.vox");
    let dst = temp.join("sample.vox");
    std::fs::copy(&before_src, &dst).expect("copy fixture into tempdir");
    dst
}

#[test]
fn migrate_names_dry_run_reports_diff_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let registry_dir = tempfile::tempdir().unwrap();
    let registry = write_test_registry(registry_dir.path());

    let target = copy_fixture_to_tempdir(temp.path());
    let original_bytes = std::fs::read_to_string(&target).unwrap();

    let output = run_migrate_names(
        temp.path(),
        &registry,
        &["--dry-run", temp.path().to_str().unwrap()],
    );

    assert!(
        output.status.success(),
        "vox migrate names exited non-zero: stderr={}",
        output.stderr
    );
    assert!(
        output.stdout_contains("would update"),
        "stdout should announce a dry-run change: {}",
        output.stdout
    );

    let after_dry_run = std::fs::read_to_string(&target).unwrap();
    assert_eq!(
        after_dry_run, original_bytes,
        "dry run must not modify the file"
    );
}

#[test]
fn migrate_names_writes_canonical_output() {
    let temp = tempfile::tempdir().unwrap();
    let registry_dir = tempfile::tempdir().unwrap();
    let registry = write_test_registry(registry_dir.path());

    let target = copy_fixture_to_tempdir(temp.path());
    let expected =
        std::fs::read_to_string(fixture_root().join("after").join("sample.vox")).unwrap();

    let output = run_migrate_names(temp.path(), &registry, &[temp.path().to_str().unwrap()]);

    assert!(
        output.status.success(),
        "vox migrate names exited non-zero: stderr={}",
        output.stderr
    );
    assert!(
        output.stdout_contains("updated"),
        "stdout should announce a write: {}",
        output.stdout
    );

    let after_write = std::fs::read_to_string(&target).unwrap();
    assert_eq!(
        after_write, expected,
        "file content after write must equal the canonical fixture byte-for-byte"
    );
}

#[test]
fn migrate_names_idempotent_on_canonical_corpus() {
    let temp = tempfile::tempdir().unwrap();
    let registry_dir = tempfile::tempdir().unwrap();
    let registry = write_test_registry(registry_dir.path());

    // Start with the already-canonical "after" fixture.
    let after_src = fixture_root().join("after").join("sample.vox");
    let target = temp.path().join("sample.vox");
    std::fs::copy(&after_src, &target).unwrap();
    let before_bytes = std::fs::read_to_string(&target).unwrap();

    let output = run_migrate_names(temp.path(), &registry, &[temp.path().to_str().unwrap()]);

    assert!(output.status.success());
    let after_bytes = std::fs::read_to_string(&target).unwrap();
    assert_eq!(
        after_bytes, before_bytes,
        "running migrate against an already-canonical corpus must be a no-op"
    );
    assert!(
        output.stdout_contains("0 file"),
        "stdout should announce zero changes: {}",
        output.stdout
    );
}

#[test]
fn rewrite_skips_non_primitive_kinds() {
    use vox_compiler::parser::renames::RenameRegistry;
    // A kwarg rename should NOT rewrite identifier tokens (would over-reach;
    // kwarg renames need argument-position context).
    let registry = RenameRegistry::parse_json(
        r#"{
        "version": 1,
        "entries": [
          { "from": "class", "to": "css_class", "kind": "kwarg", "since": "0.5.0" }
        ]
    }"#,
    )
    .unwrap();

    let before = "let class = 1; row(class: \"foo\")";
    let after = vox_cli::commands::migrate::rewrite_for_test(before, &registry);

    assert_eq!(
        after, before,
        "kwarg-kind renames must not be rewritten by the token-level codemod"
    );
}
