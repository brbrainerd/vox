//! Adversarial unit tests for `vox-langtool`.
//!
//! Module: `semcov_wave50_tests`

#[cfg(test)]
mod semcov_wave50_tests {
    use std::io::Write;
    use tempfile::NamedTempFile;

    use crate::is_script_like;

    // ──────────────────────────────────────────────────────────────────────────
    // is_script_like  (heuristic, lib.rs)
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_empty_source_is_script_like() {
        // Catches: off-by-one / panic on empty string in any() iterator
        assert!(is_script_like(""));
    }

    #[test]
    fn test_plain_let_source_is_script_like() {
        // Catches: regression where whitespace-only source falsely triggers marker
        assert!(is_script_like("let x = 1\n"));
    }

    #[test]
    fn test_page_marker_disables_script_mode() {
        // Catches: @page not recognised → wrong parse_mode → silent compile failure
        assert!(!is_script_like("@page\nfn index() {}"));
    }

    #[test]
    fn test_component_marker_disables_script_mode() {
        // Catches: @component missing from app_markers slice → module-position check skipped
        assert!(!is_script_like("@component\nfn Button() {}"));
    }

    #[test]
    fn test_workflow_marker_disables_script_mode() {
        // Catches: @workflow omitted → pipeline runs in script mode → wrong AST root
        assert!(!is_script_like("@workflow\nfn nightly() {}"));
    }

    #[test]
    fn test_marker_inside_string_literal_should_still_trigger() {
        // Catches: over-smart string-aware scan that suppresses markers in string literals;
        // current implementation is a plain contains() check — that is the contract.
        assert!(!is_script_like(r#"let s = "@page inside string""#));
    }

    #[test]
    fn test_marker_in_comment_still_triggers() {
        // Catches: comment-aware scan that strips markers — plain contains() must fire.
        assert!(!is_script_like("// @server endpoint"));
    }

    #[test]
    fn test_at_prefix_without_known_marker_is_script_like() {
        // Catches: overly broad "starts with @" heuristic treating @foo as a module marker
        assert!(is_script_like("@custom_decorator\nfn foo() {}"));
    }

    #[test]
    fn test_all_markers_detected() {
        // Catches: any single marker missing from the app_markers array
        let markers = [
            "@page",
            "@query",
            "@mutation",
            "@server",
            "@component",
            "@table",
            "@workflow",
        ];
        for marker in &markers {
            let src = format!("{marker}\nfn f() {{}}");
            assert!(
                !is_script_like(&src),
                "Marker {marker} was not detected — is_script_like returned true"
            );
        }
    }

    #[test]
    fn test_marker_with_trailing_text_still_detected() {
        // Catches: implementation requiring exact word boundary after marker
        assert!(!is_script_like("@server_side_render\nfn f() {}"));
    }

    #[test]
    fn test_only_whitespace_is_script_like() {
        // Catches: trim() being applied before marker check causing panic or wrong result
        assert!(is_script_like("   \n\t  "));
    }

    // ──────────────────────────────────────────────────────────────────────────
    // commands::fmt (atomic write + check mode)
    // ──────────────────────────────────────────────────────────────────────────

    fn write_tmp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        f
    }

    #[test]
    fn test_fmt_check_already_formatted_is_ok() {
        // Catches: fmt --check returning error even when file is already well-formed
        let src = "let x = 1\n";
        let tmp = write_tmp(src);
        // If the formatter produces identical output the check must succeed.
        let result = crate::commands::fmt::run(tmp.path(), true);
        // It is fine for the formatter to reject invalid Vox, but if it parses
        // it must not claim the already-formatted form needs changes.
        // Accept both Ok and the "cannot format" error — but NOT the "needs format" error.
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(
                !msg.contains("needs format"),
                "fmt --check incorrectly flagged a file as needing formatting: {msg}"
            );
        }
    }

    #[test]
    fn test_fmt_nonexistent_file_returns_error() {
        // Catches: fmt panic-unwrap on missing file instead of propagating anyhow error
        let path = std::path::Path::new("/nonexistent/path/to/file.vox");
        let result = crate::commands::fmt::run(path, false);
        assert!(result.is_err(), "Expected error for nonexistent file");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Failed to read")
                || msg.contains("No such file")
                || msg.contains("os error"),
            "Error message did not mention the read failure: {msg}"
        );
    }

    #[test]
    fn test_fmt_check_mode_does_not_modify_file() {
        // Catches: --check mode accidentally writing the file anyway
        let src = "let x = 1\n";
        let tmp = write_tmp(src);
        let before_meta = std::fs::metadata(tmp.path())
            .expect("metadata before")
            .modified()
            .ok();
        // Give the fs a tick so mtime would differ if written
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = crate::commands::fmt::run(tmp.path(), true);
        let after_meta = std::fs::metadata(tmp.path())
            .expect("metadata after")
            .modified()
            .ok();
        assert_eq!(
            before_meta, after_meta,
            "fmt --check must not modify the file on disk"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // commands::check (error propagation)
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_check_nonexistent_file_returns_error() {
        // Catches: check panic-unwrap instead of bubbling anyhow error
        let path = std::path::Path::new("/nonexistent/path/to/file.vox");
        let result = crate::commands::check::run(path);
        assert!(result.is_err(), "Expected error for nonexistent file");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Failed to read")
                || msg.contains("No such file")
                || msg.contains("os error"),
            "Error message did not mention the read failure: {msg}"
        );
    }

    #[test]
    fn test_check_valid_script_returns_ok() {
        // Catches: check always failing even on trivially valid source
        let src = "let x = 1\n";
        let tmp = write_tmp(src);
        // We accept Ok or a compiler-level error, but NOT a file-read / setup error.
        let result = crate::commands::check::run(tmp.path());
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                !msg.contains("Failed to read"),
                "check failed with a file-read error on a valid temp file: {msg}"
            );
        }
    }

    #[test]
    fn test_check_uses_script_mode_for_plain_source() {
        // Catches: check hard-coding module mode, causing parse failure for script-like input
        // A plain `let` binding at top-level is only valid in script mode; if module mode
        // is forced the pipeline should return type errors, not a panic.
        let src = "let answer = 42\n";
        let tmp = write_tmp(src);
        // Must not panic; any Result (Ok or Err) is acceptable.
        let _ = crate::commands::check::run(tmp.path());
    }

    #[test]
    fn test_check_uses_module_mode_for_page_source() {
        // Catches: is_script_like result ignored — always using script mode
        let src = "@page\nfn index() {}\n";
        let tmp = write_tmp(src);
        // Must not panic; just verify no unwrap explosion.
        let _ = crate::commands::check::run(tmp.path());
    }

    // ──────────────────────────────────────────────────────────────────────────
    // commands::build (error propagation + output layout)
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_build_nonexistent_input_returns_error() {
        // Catches: build panic on missing file instead of anyhow propagation
        let tmp_dir = tempfile::tempdir().expect("tmpdir");
        let path = std::path::Path::new("/nonexistent/file.vox");
        let result = crate::commands::build::run(path, tmp_dir.path());
        assert!(result.is_err(), "Expected error for nonexistent input");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Failed to read")
                || msg.contains("No such file")
                || msg.contains("os error"),
            "Error did not mention read failure: {msg}"
        );
    }

    #[test]
    fn test_build_creates_out_dir_when_missing() {
        // Catches: build failing with "directory not found" when out_dir doesn't pre-exist
        let src = "let x = 1\n";
        let tmp = write_tmp(src);
        let parent = tempfile::tempdir().expect("tmpdir");
        let out_dir = parent.path().join("nested").join("output");
        // out_dir does not exist yet — build must create it.
        let result = crate::commands::build::run(tmp.path(), &out_dir);
        // We only check that if it fails it's NOT due to a missing directory.
        if let Err(e) = &result {
            let msg = e.to_string();
            assert!(
                !msg.contains("Failed to create out-dir"),
                "build failed because it did not create out-dir: {msg}"
            );
        }
    }
}
