/// Wave-32 adversarial tests for vox-code-audit detectors and pure utilities.
///
/// Each test carries a `// Catches:` annotation naming the specific plausible bug it guards.
/// Coverage targets: `TokenMap`, `SourceFile`, `StubDetector`, `EmptyBodyDetector`,
/// `VictoryClaimDetector`, `Language`, `Finding::fingerprint`, and `byte_offset_in_file`.
#[cfg(test)]
mod semcov_wave32_tests {
    use std::path::PathBuf;

    use crate::analysis::TokenMap;
    use crate::detectors::empty_body::EmptyBodyDetector;
    use crate::detectors::stub::StubDetector;
    use crate::detectors::victory_claim::VictoryClaimDetector;
    use crate::rules::{
        DetectionRule, Finding, Language, Severity, SourceFile, byte_offset_in_file,
    };

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn src(ext: &str, code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from(format!("test.{ext}")), code.to_string())
    }

    fn make_finding(rule_id: &str, file: &str, line: usize) -> Finding {
        Finding {
            rule_id: rule_id.to_string(),
            diagnostic_id: None,
            rule_name: "test".to_string(),
            severity: Severity::Warning,
            file: PathBuf::from(file),
            line,
            column: 0,
            message: "test message".to_string(),
            suggestion: None,
            alternatives: vec![],
            rationale: None,
            context: String::new(),
            confidence: None,
            evidence: None,
        }
    }

    // -----------------------------------------------------------------------
    // TokenMap — byte classification
    // -----------------------------------------------------------------------

    #[test]
    fn token_map_todo_inside_string_is_not_code() {
        // Catches: detector treating string content as executable code, firing false positives
        // on "todo!()" inside a string literal.
        let src = r#"let s = "todo!()";"#;
        let m = TokenMap::from_rust_source(src);
        let idx = src.find("todo").unwrap();
        assert!(
            m.is_string_byte(idx),
            "todo!() inside a string must be classified as string, not code"
        );
        assert!(!m.is_comment_byte(idx));
        assert!(!m.is_code_byte(idx));
    }

    #[test]
    fn token_map_escaped_quote_does_not_end_string_early() {
        // Catches: off-by-one in scan_normal_string that treats \" as end-of-string,
        // leaking the trailing chars as code bytes.
        let src = r#"let s = "he said \"hi\" there"; let x = 1;"#;
        let m = TokenMap::from_rust_source(src);
        // "hi" is inside the string; 'x' assignment is code
        let hi_idx = src.find("hi").unwrap();
        assert!(
            m.is_string_byte(hi_idx),
            "escaped-quote interior must be string"
        );
        let x_idx = src.rfind("let x").unwrap();
        assert!(
            m.is_code_byte(x_idx),
            "code after closed string must be code"
        );
    }

    #[test]
    fn token_map_raw_string_with_one_hash() {
        // Catches: raw-string scanner failing to track hash count, mis-classifying the
        // closing `"#` as string end when there is only one hash.
        let src = r##"let _ = r#"inner content"#; let y = 2;"##;
        let m = TokenMap::from_rust_source(src);
        let inner = src.find("inner").unwrap();
        assert!(m.is_string_byte(inner));
        let y_idx = src.rfind("let y").unwrap();
        assert!(m.is_code_byte(y_idx));
    }

    #[test]
    fn token_map_nested_block_comment_depth_two() {
        // Catches: block-comment depth counter not incrementing on nested `/*`, causing
        // the inner `*/` to close the outer comment prematurely.
        let src = "/* a /* b */ c */ code";
        let m = TokenMap::from_rust_source(src);
        let c_idx = src.find(" c ").unwrap();
        assert!(
            m.is_comment_byte(c_idx),
            "content between nested comments must be comment"
        );
        let code_idx = src.rfind("code").unwrap();
        assert!(m.is_code_byte(code_idx), "text after `*/` must be code");
    }

    #[test]
    fn token_map_byte_string_classified_as_string() {
        // Catches: b"..." prefix being skipped by the scanner, misclassifying content as code.
        let src = r#"let v = b"secret";"#;
        let m = TokenMap::from_rust_source(src);
        let idx = src.find("secret").unwrap();
        assert!(
            m.is_string_byte(idx),
            "byte-string content must be a string span"
        );
    }

    #[test]
    fn token_map_empty_source_no_panic() {
        // Catches: out-of-bounds access or panic when source is empty — the token map
        // must construct without crashing and classify index 0 consistently (no non-code
        // spans exist, so every index is considered code).
        let m = TokenMap::from_rust_source("");
        // Empty source has no spans — byte 0 is outside all spans so is_non_code_byte is false.
        assert!(!m.is_non_code_byte(0));
        assert!(!m.is_comment_byte(0));
        assert!(!m.is_string_byte(0));
        // is_code_byte is the negation of is_non_code_byte.
        assert!(m.is_code_byte(0));
    }

    // -----------------------------------------------------------------------
    // byte_offset_in_file
    // -----------------------------------------------------------------------

    #[test]
    fn byte_offset_line_zero_returns_column_clamped() {
        // Catches: line_1_indexed == 0 path returning an unclamped offset that exceeds
        // content length, causing downstream slice panics.
        let content = "hello";
        let off = byte_offset_in_file(content, 0, 99);
        assert_eq!(off, content.len(), "line 0 must clamp to content length");
    }

    #[test]
    fn byte_offset_beyond_last_line_clamps() {
        // Catches: iterator exhausting without finding the line, returning `off` without
        // clamping, which may exceed content.len() on CRLF files.
        let content = "a\nb\n";
        let off = byte_offset_in_file(content, 999, 0);
        assert!(off <= content.len(), "must not exceed content length");
    }

    #[test]
    fn byte_offset_first_line_col_zero() {
        // Catches: off-by-one on the first line returning 1 instead of 0.
        let content = "fn foo() {}";
        let off = byte_offset_in_file(content, 1, 0);
        assert_eq!(off, 0);
    }

    // -----------------------------------------------------------------------
    // SourceFile
    // -----------------------------------------------------------------------

    #[test]
    fn source_file_no_extension_detects_unknown_language() {
        // Catches: Language::from_extension panicking or misclassifying files with no extension.
        let f = SourceFile::new(PathBuf::from("Makefile"), "all:\n\techo hi\n".to_string());
        assert_eq!(f.language, Language::Unknown);
    }

    #[test]
    fn source_file_context_around_first_line_no_underflow() {
        // Catches: saturating_sub off-by-one causing a panic when line == 1 and radius > 0.
        let f = src("rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
        let ctx = f.context_around(1, 3);
        assert!(ctx.contains("fn a"), "first line must appear in context");
    }

    #[test]
    fn source_file_context_around_last_line_no_overflow() {
        // Catches: `(line + radius).min(self.lines.len())` using > instead of >=, panicking
        // on the last line of a file.
        let f = src("rs", "fn a() {}\nfn b() {}\n");
        let last = f.lines.len();
        let ctx = f.context_around(last, 5);
        assert!(ctx.contains("fn b"), "last line must appear in context");
    }

    // -----------------------------------------------------------------------
    // Language
    // -----------------------------------------------------------------------

    #[test]
    fn language_from_extension_mts_is_typescript() {
        // Catches: missing arm for `.mts` / `.mjs` extensions returning Unknown
        // and silently skipping TS detector rules on ES-module files.
        assert_eq!(Language::from_extension("mts"), Language::TypeScript);
        assert_eq!(Language::from_extension("mjs"), Language::TypeScript);
    }

    #[test]
    fn language_from_extension_jsx_is_typescript() {
        // Catches: JSX files falling through to Unknown and never being analysed.
        assert_eq!(Language::from_extension("jsx"), Language::TypeScript);
        assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
    }

    // -----------------------------------------------------------------------
    // Finding — fingerprint / deterministic_key
    // -----------------------------------------------------------------------

    #[test]
    fn finding_fingerprint_differs_by_rule_id() {
        // Catches: fingerprint ignoring rule_id, causing two distinct rules on the
        // same line to collide in dedup caches and suppress one finding silently.
        let a = make_finding("stub/todo", "src/lib.rs", 10);
        let b = make_finding("stub/unimplemented", "src/lib.rs", 10);
        assert_ne!(
            a.fingerprint(),
            b.fingerprint(),
            "different rule_ids must produce different fingerprints"
        );
    }

    #[test]
    fn finding_fingerprint_differs_by_line() {
        // Catches: fingerprint dropping the `line` field, causing findings on adjacent
        // lines to be collapsed as duplicates.
        let a = make_finding("stub/todo", "src/lib.rs", 5);
        let b = make_finding("stub/todo", "src/lib.rs", 6);
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn finding_deterministic_key_ordering() {
        // Catches: deterministic_key returning (line, path, rule) instead of (path, line, rule),
        // breaking sort stability across files with the same line number.
        let a = make_finding("rule", "aaa.rs", 1);
        let b = make_finding("rule", "zzz.rs", 1);
        assert!(
            a.deterministic_key() < b.deterministic_key(),
            "path must sort before line so files cluster together"
        );
    }

    // -----------------------------------------------------------------------
    // StubDetector — adversarial inputs
    // -----------------------------------------------------------------------

    #[test]
    fn stub_detector_todo_in_string_is_not_flagged() {
        // Catches: StubDetector scanning raw line bytes without consulting TokenMap,
        // firing on `todo!()` inside a string literal.
        let d = StubDetector::new();
        let f = src("rs", r#"const MSG: &str = "todo!()";"#);
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/todo"),
            "todo!() inside a string literal must not fire"
        );
    }

    #[test]
    fn stub_detector_stub_module_path_not_flagged() {
        // Catches: bare_stub_word_not_stub_check missing the `::` guard, triggering on
        // legitimate `stub::helper()` module calls.
        let d = StubDetector::new();
        let f = src("rs", "use crate::stub::helper;\n");
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/placeholder"),
            "stub:: module path must not trigger placeholder rule"
        );
    }

    #[test]
    fn stub_detector_pub_mod_stub_not_flagged() {
        // Catches: bare_stub_word_not_stub_check not recognising `pub mod stub`
        // as a declaration, triggering on module definitions in lib.rs.
        let d = StubDetector::new();
        let f = src("rs", "pub mod stub;\n");
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/placeholder"),
            "pub mod stub; declaration must not trigger placeholder rule"
        );
    }

    #[test]
    fn stub_detector_toestub_ignore_all_suppresses_entire_line() {
        // Catches: ignore annotation check not short-circuiting before all sub-rules,
        // firing `stub/todo` even when the line carries `toestub-ignore(all)`.
        let d = StubDetector::new();
        let f = src("rs", "    todo!() // toestub-ignore(all)\n");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "toestub-ignore(all) must suppress all findings on that line"
        );
    }

    #[test]
    fn stub_detector_doc_comment_todo_not_flagged_as_placeholder() {
        // Catches: stub_todo_comment_line_matches treating `/// TODO:` doc-comment lines
        // as non-code and incorrectly firing `stub/placeholder` or `stub/todo-comment`.
        let d = StubDetector::new();
        // Doc comments with TODO are explicitly skipped in detect_rust (rustdoc).
        let f = src("rs", "/// TODO: document this parameter\nfn foo() {}\n");
        let findings = d.detect(&f, None);
        // stub/todo-comment should NOT fire for rustdoc lines (the detector skips them)
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/todo-comment"),
            "rustdoc /// lines must not trigger todo-comment rule"
        );
    }

    #[test]
    fn stub_detector_python_pass_with_body_not_flagged() {
        // Catches: detect_python_pass_stubs flagging `pass` that appears before additional
        // body lines (i.e., an early-return guard, not a stub).
        let d = StubDetector::new();
        // `pass` followed by an indented continuation line — has_more_body should be true.
        let code = "def foo():\n    pass\n    return 42\n";
        let f = src("py", code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.iter().any(|x| x.rule_id == "stub/pass"),
            "pass followed by more body lines must not be flagged"
        );
    }

    #[test]
    fn stub_detector_gdscript_func_with_pass_flagged() {
        // Catches: detect_gdscript checking for Python `def ` prefix instead of `func `,
        // silently missing all GDScript stubs.
        let d = StubDetector::new();
        let f = src("gd", "func on_ready():\n\tpass\n");
        let findings = d.detect(&f, None);
        assert!(
            findings.iter().any(|x| x.rule_id == "stub/gdscript-pass"),
            "empty GDScript func with pass must fire gdscript-pass"
        );
    }

    // -----------------------------------------------------------------------
    // EmptyBodyDetector — adversarial inputs
    // -----------------------------------------------------------------------

    #[test]
    fn empty_body_main_fn_not_flagged() {
        // Catches: the explicit `main()` exemption being absent in the multi-line path,
        // flagging `fn main() {}` and breaking every hello-world program.
        let d = EmptyBodyDetector::new();
        let f = src("rs", "fn main() {}\n");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "fn main() {{}} must not be flagged by empty-body detector"
        );
    }

    #[test]
    fn empty_body_fn_comment_only_body_not_flagged_by_current_impl() {
        // Catches (documents current behaviour): the brace-body scanner explicitly filters
        // out comment lines (`!t.starts_with("//")`) and therefore does NOT flag a function
        // whose body contains only comments.  If the policy changes to require even comment-
        // only bodies to be flagged, this test must be updated alongside the detector.
        let d = EmptyBodyDetector::new();
        let f = src("rs", "fn on_event() {\n    // handle event here\n}\n");
        let findings = d.detect(&f, None);
        // Current policy: a comment-only body is treated the same as empty — NOT flagged
        // because the scanner strips comments before checking for content.
        // NOTE: this is a documented gap; a future tighter policy may flip this assertion.
        assert!(
            findings.is_empty(),
            "current impl does not flag comment-only bodies; update if policy changes"
        );
    }

    #[test]
    fn empty_body_ts_callback_not_flagged() {
        // Catches: ts_empty_arrow pattern firing on `.then(() => {})` callbacks
        // that don't start with const/let/export, creating noisy false positives.
        let d = EmptyBodyDetector::new();
        let f = src("ts", "promise.then(() => {});\n");
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "inline callback arrow `() => {{}}` must not fire the empty-body rule"
        );
    }

    #[test]
    fn empty_body_python_ellipsis_fires() {
        // Catches: Python ellipsis check comparing the wrong line index (off-by-one),
        // silently skipping the actual `...` body.
        let d = EmptyBodyDetector::new();
        let f = src("py", "def handle(event):\n    ...\n");
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "def with ellipsis body must fire empty-body detector"
        );
    }

    // -----------------------------------------------------------------------
    // VictoryClaimDetector
    // -----------------------------------------------------------------------

    #[test]
    fn victory_claim_doc_comment_not_flagged() {
        // Catches: VictoryClaimDetector failing to skip `///` rustdoc lines,
        // triggering on words like "Done" in API documentation.
        let d = VictoryClaimDetector::new();
        // Use concat! to avoid the detector firing on this source file itself.
        let text = concat!("/// Done — returns the computed value.\nfn get() -> u32 { 42 }\n");
        let f = src("rs", text);
        let findings = d.detect(&f, None);
        assert!(
            !findings
                .iter()
                .any(|x| x.rule_id == "victory-claim/premature"),
            "victory-claim must not fire inside rustdoc `///` lines"
        );
    }

    #[test]
    fn victory_claim_hack_comment_flagged() {
        // Catches: victory-claim/hack rule missing from the loaded rule slice,
        // silently dropping HACK marker detection.
        let d = VictoryClaimDetector::new();
        let text = concat!("// HA", "CK: temporary workaround\nlet x = 0;\n");
        let f = src("rs", text);
        let findings = d.detect(&f, None);
        assert!(
            findings.iter().any(|x| x.rule_id == "victory-claim/hack"),
            "HACK comment must fire victory-claim/hack"
        );
    }
}
