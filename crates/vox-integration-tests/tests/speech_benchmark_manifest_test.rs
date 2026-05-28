#![allow(missing_docs)]

//! Ensures frozen paths listed in `contracts/speech-to-code/benchmark-fixtures.manifest.txt` exist.
//!
//! The speech audit corpus uses manifest triples:
//! `audio<TAB>transcript<TAB>expected_vox_or_dash<TAB>domain<TAB>sample_rate_hz`.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn speech_benchmark_manifest_paths_exist() {
    let root = workspace_root();
    let manifest = root.join("contracts/speech-to-code/benchmark-fixtures.manifest.txt");
    let raw = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));

    for (line_no, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.contains('\t') {
            continue;
        }
        let rel = Path::new(line);
        let abs = root.join(rel);
        assert!(
            abs.exists(),
            "benchmark manifest line {}: missing path {} (resolved: {})",
            line_no + 1,
            line,
            abs.display()
        );
    }
}

#[test]
fn speech_benchmark_manifest_has_audit_corpus_triples() {
    let root = workspace_root();
    let manifest = root.join("contracts/speech-to-code/benchmark-fixtures.manifest.txt");
    let raw = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));

    let mut corpus_rows = 0usize;
    let mut code_dictation_rows = 0usize;
    for (line_no, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('\t') {
            continue;
        }

        let parts: Vec<_> = line.split('\t').collect();
        assert_eq!(
            parts.len(),
            5,
            "benchmark manifest line {} must be audio<TAB>transcript<TAB>expected_vox_or_dash<TAB>domain<TAB>sample_rate_hz",
            line_no + 1
        );

        let audio = root.join(parts[0]);
        let transcript = root.join(parts[1]);
        assert!(
            audio.exists(),
            "benchmark manifest line {} missing audio {}",
            line_no + 1,
            audio.display()
        );
        assert!(
            transcript.exists(),
            "benchmark manifest line {} missing transcript {}",
            line_no + 1,
            transcript.display()
        );
        assert!(
            parts[0].ends_with(".wav"),
            "benchmark manifest line {} audio must be .wav: {}",
            line_no + 1,
            parts[0]
        );
        assert!(
            matches!(
                parts[3],
                "code-dictation" | "command-phrasing" | "identifier-heavy" | "mixed-natural" | "noisy"
            ),
            "benchmark manifest line {} has unknown domain {}",
            line_no + 1,
            parts[3]
        );
        assert_eq!(
            parts[4],
            "16000",
            "benchmark manifest line {} expected 16 kHz corpus audio",
            line_no + 1
        );

        if parts[2] != "-" {
            let expected = root.join(parts[2]);
            assert!(
                expected.exists(),
                "benchmark manifest line {} missing expected Vox file {}",
                line_no + 1,
                expected.display()
            );
        }
        if parts[3] == "code-dictation" {
            code_dictation_rows += 1;
            assert_ne!(
                parts[2],
                "-",
                "code-dictation row {} must include expected Vox output",
                line_no + 1
            );
        }
        corpus_rows += 1;
    }

    assert!(
        corpus_rows >= 30,
        "speech audit corpus must contain at least 30 manifest triples, found {corpus_rows}"
    );
    assert!(
        code_dictation_rows >= 10,
        "speech audit corpus must contain at least 10 code-dictation rows, found {code_dictation_rows}"
    );
}
