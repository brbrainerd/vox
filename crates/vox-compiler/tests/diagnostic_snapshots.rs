//! Golden snapshots for `vox_compiler::pipeline::check_file` payloads (shared fixtures).

#[test]
fn rust_import_dup_diagnostic_payload_snapshot() {
    let src = include_str!("fixtures/diagnostics/rust_import_dup.vox");
    let diags = vox_compiler::pipeline::check_file(src, "fixture.vox");
    insta::assert_json_snapshot!(diags);
}
