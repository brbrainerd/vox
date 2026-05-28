#![allow(missing_docs)]

//! Validates frozen speech-to-code `.vox` fixtures against HIR parity (compile gate for benchmarks).

use std::fs;
use std::path::PathBuf;

use tower_lsp_server::ls_types::DiagnosticSeverity;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn speech_fixture_expected_vox_passes_hir_gate() {
    let root = workspace_root();
    let path = root.join("tests/speech-to-code/fixtures/p001_expected.vox");
    let src = fs::read_to_string(&path).expect("read fixture vox");
    let diags = vox_lsp::validate_document_with_hir(&src);
    let errs: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        errs.is_empty(),
        "HIR validation failed for {}: {:?}",
        path.display(),
        errs
    );
}

#[test]
fn speech_audit_manifest_expected_vox_passes_hir_gate() {
    let root = workspace_root();
    let manifest = root.join("contracts/speech-to-code/benchmark-fixtures.manifest.txt");
    let raw = fs::read_to_string(&manifest).expect("read speech benchmark manifest");
    let mut checked = 0usize;

    for (line_no, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('\t') {
            continue;
        }
        let parts: Vec<_> = line.split('\t').collect();
        assert_eq!(
            parts.len(),
            5,
            "benchmark manifest line {} must be an audit triple",
            line_no + 1
        );
        let expected = parts[2];
        if expected == "-" {
            continue;
        }
        checked += 1;
        let path = root.join(expected);
        let src = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read expected Vox {}: {e}", path.display()));
        let diags = vox_lsp::validate_document_with_hir(&src);
        let errs: Vec<_> = diags
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .collect();
        assert!(
            errs.is_empty(),
            "HIR validation failed for manifest line {} {}: {:?}",
            line_no + 1,
            path.display(),
            errs
        );
    }

    assert!(
        checked >= 10,
        "expected at least 10 code-dictation Vox fixtures, checked {checked}"
    );
}
