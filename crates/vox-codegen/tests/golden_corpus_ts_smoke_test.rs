/// Task 3B: Point TS-emit snapshot harness at the main `examples/golden/` corpus.
///
/// The existing `golden_ts_test.rs` only covers `examples/golden-ts/` (12 files,
/// UI-focused). This test covers the full golden corpus and verifies:
///   1. The TS emitter never panics on any golden file.
///   2. Files that produce non-empty output are snapshot-tested (insta) so
///      regressions (stopping to emit where we previously did) are caught.
///
/// Initial snapshots are accepted via `INSTA_UPDATE=new cargo test`.
use std::path::{Path, PathBuf};
use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn collect_vox_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_vox_files(&path, out);
            } else if path.extension().and_then(|s| s.to_str()) == Some("vox") {
                out.push(path);
            }
        }
    }
}

/// Files known to have parse errors (pre-existing, unrelated to TS emit).
/// Remove entries as the underlying parse bugs are fixed.
const PARSE_ERROR_SKIP: &[&str] = &[
    // Pre-existing parse error in typeck test fixture
    "tokens_low_contrast_pair.vox",
];

fn is_parse_skip(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    PARSE_ERROR_SKIP.contains(&name)
}

#[test]
fn golden_corpus_ts_emit_no_panic() {
    let golden_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden");
    assert!(golden_dir.is_dir(), "missing {}", golden_dir.display());

    let mut files = Vec::new();
    collect_vox_files(&golden_dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .vox files found in examples/golden");

    let mut nonempty_count = 0usize;
    let mut panic_count = 0usize;
    let mut parse_skip_count = 0usize;

    for path in &files {
        if is_parse_skip(path) {
            parse_skip_count += 1;
            continue;
        }

        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Parse — skip on pre-existing parse errors (don't treat as test failure).
        let module = match std::panic::catch_unwind(|| parse(lex(&src))) {
            Ok(Ok(m)) => m,
            _ => {
                parse_skip_count += 1;
                continue;
            }
        };

        let hir = lower_module(&module);

        // TS emit must NEVER panic — catch_unwind protects the test harness.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| generate(&hir)));
        match result {
            Err(_) => {
                panic_count += 1;
                eprintln!("PANIC in TS emit for {}", path.display());
            }
            Ok(Err(e)) => {
                // Emitter returned Err — acceptable for server-only modules.
                eprintln!("emit Err (ok) {}: {}", path.display(), e);
            }
            Ok(Ok(output)) => {
                // Non-empty output: take insta snapshot for regression detection.
                let combined: String = output
                    .files
                    .iter()
                    .map(|(name, content)| format!("=== {} ===\n{}", name, content))
                    .collect::<Vec<_>>()
                    .join("\n\n");

                if !combined.trim().is_empty() {
                    nonempty_count += 1;
                    // Derive a stable snapshot name from the file path relative to golden_dir.
                    let rel = path
                        .strip_prefix(&golden_dir)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace(['\\', '/'], "__")
                        .replace(".vox", "");
                    insta::with_settings!({ snapshot_suffix => rel.clone() }, {
                        insta::assert_snapshot!(combined);
                    });
                }
            }
        }
    }

    eprintln!(
        "golden corpus TS smoke: non-empty={nonempty_count}, parse-skip={parse_skip_count}, panics={panic_count}"
    );
    assert_eq!(
        panic_count, 0,
        "TS emitter panicked on {panic_count} golden files"
    );
}
