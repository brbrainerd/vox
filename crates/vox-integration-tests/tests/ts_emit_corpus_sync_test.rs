//! Fails if a promoted `examples/golden-ts/*.vox` fixture has drifted from its
//! `examples/golden/*.vox` source of the same name. Promotion (see
//! ts_emit_corpus_triage_test.rs) is a byte-copy; this test is what keeps that
//! copy honest instead of silently rotting when the source is edited.
//!
//! A golden-ts fixture with NO same-named file in golden/ is fine (it may have
//! been authored directly for TS-specific coverage) — only same-named pairs are compared.

#![allow(missing_docs)]

use std::path::{Path, PathBuf};

fn examples_dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples").join(sub)
}

fn collect_vox_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "vox") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn promoted_fixtures_match_their_golden_source() {
    let golden_ts_dir = examples_dir("golden-ts");
    let golden_dir = examples_dir("golden");

    let mut drifted = Vec::new();

    for ts_path in collect_vox_files(&golden_ts_dir) {
        let name = ts_path.file_name().unwrap();
        let source_path = golden_dir.join(name);
        if !source_path.exists() {
            // Not a promoted fixture (authored directly for golden-ts) — skip.
            continue;
        }

        let ts_content = std::fs::read_to_string(&ts_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", ts_path.display()));
        let source_content = std::fs::read_to_string(&source_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", source_path.display()));

        if ts_content != source_content {
            drifted.push(name.to_string_lossy().to_string());
        }
    }

    assert!(
        drifted.is_empty(),
        "These examples/golden-ts/ fixtures have drifted from their examples/golden/ source:\n  {}\n\
         Re-sync with: cp examples/golden/<name>.vox examples/golden-ts/<name>.vox",
        drifted.join("\n  ")
    );
}
