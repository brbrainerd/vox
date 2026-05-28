//! LSP diagnostic summaries for shared compiler fixtures (`crates/vox-compiler/tests/fixtures/`).

use tower_lsp_server::ls_types::{Diagnostic, NumberOrString};

fn diagnostic_summaries(diags: &[Diagnostic]) -> Vec<serde_json::Value> {
    diags
        .iter()
        .map(|d| {
            let code = d.code.as_ref().map(|c| match c {
                NumberOrString::String(s) => serde_json::json!(s),
                NumberOrString::Number(n) => serde_json::json!(n),
            });
            serde_json::json!({
                "range": {
                    "start": {"line": d.range.start.line, "character": d.range.start.character},
                    "end": {"line": d.range.end.line, "character": d.range.end.character},
                },
                "severity": d.severity.map(|s| format!("{s:?}")),
                "code": code,
                "message": d.message,
                "source": d.source,
                "data": d.data,
            })
        })
        .collect()
}

#[test]
fn rust_import_dup_lsp_diagnostic_snapshot_with_hir() {
    let src = include_str!("../../vox-compiler/tests/fixtures/diagnostics/rust_import_dup.vox");
    let diags = vox_lsp::validate_document_with_hir(src);
    insta::assert_json_snapshot!(diagnostic_summaries(&diags));
}
