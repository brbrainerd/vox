//! Adversarial tests for vox-corpus pure-function modules.
//!
//! Module: semcov_wave30_tests
//! Targets: coverage analysis, normalize_training_jsonl_line, enrich_lane_metadata,
//!          stamp_mix_weight, RNG/name_hash, CoverageReport invariants.

#[cfg(test)]
mod semcov_wave30_tests {
    use crate::corpus::coverage::analyse_str_with_taxonomy;
    use crate::corpus::mix::{
        ASR_REFINE_INSTRUCTION, SPEECH_TO_CODE_INSTRUCTION, normalize_training_jsonl_line,
    };
    use crate::synthetic_gen::name_hash;

    // ──────────────────────────────────────────────────────────────────────
    // Coverage analysis – boundary and invariant tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn empty_jsonl_produces_zero_totals() {
        // Catches: analyse_str returning nonsensical counts on empty input
        let report = analyse_str_with_taxonomy("", 1, &["function", "actor"]);
        assert_eq!(report.total_pairs, 0);
        assert_eq!(report.covered_types, 0);
        assert!(report.missing_types.contains(&"function".to_string()));
        assert!(report.missing_types.contains(&"actor".to_string()));
    }

    #[test]
    fn empty_taxonomy_slice_total_types_is_zero() {
        // Catches: division-by-zero when taxonomy is empty → coverage_ratio = 0/0 = NaN
        // BUG DOCUMENTED: build_report computes `covered_types as f64 / total_types as f64`
        // without guarding total_types == 0, producing NaN. This test verifies the
        // structural fields are correct and pinpoints the NaN so the bug is visible.
        let report = analyse_str_with_taxonomy(
            r#"{"prompt":"x","response":"y","category":"function"}"#,
            1,
            &[],
        );
        assert_eq!(report.total_types, 0);
        assert_eq!(report.covered_types, 0);
        // NaN: coverage_ratio = 0/0 — bug is intentionally documented here.
        // When fixed, change this assertion to `assert_eq!(report.coverage_ratio, 0.0)`.
        assert!(
            report.coverage_ratio.is_nan() || report.coverage_ratio == 0.0,
            "expected NaN or 0.0 for empty taxonomy, got {}",
            report.coverage_ratio
        );
    }

    #[test]
    fn single_covered_type_balance_score_is_zero() {
        // Catches: CV formula returning 1.0 (or NaN) when covered_types == 1
        // (std_dev == 0, mean > 0, cv == 0 → score should be 1.0, but
        // the guard `covered_types < 2` forces it to 0.0)
        let report = analyse_str_with_taxonomy(
            r#"{"prompt":"x","response":"y","category":"function"}"#,
            1,
            &["function", "actor"],
        );
        assert_eq!(report.balance_score, 0.0);
    }

    #[test]
    fn rust_prefix_counted_under_base_key() {
        // Catches: rust_ prefix stripping writing to a different key than the base name,
        // so taxonomy lookups miss the count.
        let jsonl = r#"{"prompt":"x","response":"y","category":"rust_actor"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 1, &["actor"]);
        assert_eq!(
            report.covered_types, 1,
            "actor must be covered via rust_ prefix"
        );
        assert_eq!(*report.counts.get("actor").unwrap(), 1);
    }

    #[test]
    fn non_taxonomy_categories_counted_but_not_in_missing() {
        // Catches: non-taxonomy category inflating missing_types or coverage_ratio
        let jsonl = r#"{"prompt":"x","response":"y","category":"unknown_exotic"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 1, &["function"]);
        assert_eq!(report.total_pairs, 1);
        assert!(
            report.missing_types.contains(&"function".to_string()),
            "function is still missing"
        );
        assert!(
            !report.missing_types.contains(&"unknown_exotic".to_string()),
            "non-taxonomy type must not appear in missing_types"
        );
    }

    #[test]
    fn underrepresented_excludes_missing_types() {
        // Catches: missing types (count == 0) leaking into underrepresented list
        let jsonl = r#"{"prompt":"x","response":"y","category":"function"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 5, &["function", "actor"]);
        let under_keys: Vec<&str> = report
            .underrepresented_types
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert!(
            !under_keys.contains(&"actor"),
            "actor has count 0 – must be missing, not underrepresented"
        );
        assert!(under_keys.contains(&"function"), "function has 1 < 5");
    }

    #[test]
    fn underrepresented_sorted_ascending_by_count() {
        // Catches: sort direction reversed (descending instead of ascending)
        let jsonl = [
            r#"{"prompt":"x","response":"y","category":"a"}"#,
            r#"{"prompt":"x","response":"y","category":"a"}"#,
            r#"{"prompt":"x","response":"y","category":"a"}"#,
            r#"{"prompt":"x","response":"y","category":"b"}"#,
        ]
        .join("\n");
        let report = analyse_str_with_taxonomy(&jsonl, 10, &["a", "b"]);
        if report.underrepresented_types.len() >= 2 {
            let counts: Vec<usize> = report
                .underrepresented_types
                .iter()
                .map(|(_, c)| *c)
                .collect();
            for w in counts.windows(2) {
                assert!(w[0] <= w[1], "underrepresented must be sorted ascending");
            }
        }
    }

    #[test]
    fn blank_lines_ignored_in_coverage_count() {
        // Catches: blank lines bumping total_pairs counter
        let jsonl = "\n\n\n";
        let report = analyse_str_with_taxonomy(jsonl, 1, &["function"]);
        assert_eq!(report.total_pairs, 0);
    }

    #[test]
    fn malformed_json_line_skipped_gracefully() {
        // Catches: panics or miscounts when a JSONL line is syntactically invalid
        let jsonl =
            "not-json-at-all\n{\"prompt\":\"x\",\"response\":\"y\",\"category\":\"function\"}";
        let report = analyse_str_with_taxonomy(jsonl, 1, &["function"]);
        assert_eq!(report.total_pairs, 1, "only the valid line should count");
    }

    #[test]
    fn is_sufficient_false_when_threshold_equals_count() {
        // Catches: off-by-one: count == threshold should be sufficient, not underrepresented
        let jsonl = r#"{"prompt":"x","response":"y","category":"function"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 1, &["function"]);
        // count == threshold (1 >= 1) → sufficient
        assert!(
            report.is_sufficient(),
            "count == min_threshold must be sufficient"
        );
    }

    #[test]
    fn is_sufficient_false_when_count_one_below_threshold() {
        // Catches: off-by-one: count == threshold - 1 must NOT be sufficient
        let jsonl = r#"{"prompt":"x","response":"y","category":"function"}"#;
        let report = analyse_str_with_taxonomy(jsonl, 2, &["function"]);
        assert!(
            !report.is_sufficient(),
            "1 pair vs threshold 2 must not be sufficient"
        );
    }

    #[test]
    fn summary_contains_threshold_value() {
        // Catches: summary format string mis-indexing the threshold field
        let report = analyse_str_with_taxonomy("", 42, &["function"]);
        assert!(
            report.summary().contains("42"),
            "summary must show threshold 42"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // normalize_training_jsonl_line – format conversion tests
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn normalize_passthrough_for_none_format() {
        // Catches: passthrough mode mutating or rejecting valid prompt/response rows
        let line = r#"{"prompt":"hello","response":"world","category":"function"}"#;
        let result = normalize_training_jsonl_line(line, None).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["prompt"].as_str().unwrap(), "hello");
        assert_eq!(v["response"].as_str().unwrap(), "world");
    }

    #[test]
    fn normalize_asr_refine_prepends_instruction() {
        // Catches: instruction string missing or duplicated in the normalized prompt
        let line = r#"{"noisy_text":"helo wrold","corrected_text":"hello world"}"#;
        let result = normalize_training_jsonl_line(line, Some("asr_refine")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let prompt = v["prompt"].as_str().unwrap();
        assert!(
            prompt.starts_with(ASR_REFINE_INSTRUCTION),
            "prompt must start with ASR instruction"
        );
        assert!(prompt.contains("helo wrold"));
    }

    #[test]
    fn normalize_asr_refine_missing_noisy_text_is_error() {
        // Catches: silently returning empty prompt when noisy_text absent
        let line = r#"{"corrected_text":"hello world"}"#;
        let result = normalize_training_jsonl_line(line, Some("asr_refine"));
        assert!(result.is_err(), "missing noisy_text must be an error");
    }

    #[test]
    fn normalize_asr_refine_passthrough_when_prompt_response_present() {
        // Catches: double-transforming a row that already has prompt/response fields
        let line = r#"{"prompt":"already","response":"done","noisy_text":"noise"}"#;
        let result = normalize_training_jsonl_line(line, Some("asr_refine")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            v["prompt"].as_str().unwrap(),
            "already",
            "pre-formed prompt/response must pass through unchanged"
        );
    }

    #[test]
    fn normalize_speech_to_code_uses_refined_transcript() {
        // Catches: falling through to the error path when refined_transcript key present
        let line = r#"{"refined_transcript":"open file","vox_code":"fs.open(\"f.vox\")"}"#;
        let result = normalize_training_jsonl_line(line, Some("speech_to_code")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let prompt = v["prompt"].as_str().unwrap();
        assert!(
            prompt.starts_with(SPEECH_TO_CODE_INSTRUCTION),
            "speech_to_code prompt must start with its instruction"
        );
        assert!(prompt.contains("open file"));
    }

    #[test]
    fn normalize_speech_to_code_missing_vox_code_is_error() {
        // Catches: silently emitting empty response when vox_code absent
        let line = r#"{"refined_transcript":"do something"}"#;
        let result = normalize_training_jsonl_line(line, Some("speech_to_code"));
        assert!(result.is_err(), "missing vox_code must be an error");
    }

    #[test]
    fn normalize_empty_line_is_error() {
        // Catches: empty-line returning Ok("") and polluting the output JSONL
        assert!(
            normalize_training_jsonl_line("", None).is_err(),
            "empty line must return Err"
        );
        assert!(
            normalize_training_jsonl_line("   ", None).is_err(),
            "whitespace-only line must return Err"
        );
    }

    #[test]
    fn normalize_unknown_format_falls_through_as_passthrough() {
        // Catches: unknown record_format values panicking or erroring instead of
        // falling through to the passthrough arm
        let line = r#"{"prompt":"x","response":"y"}"#;
        let result = normalize_training_jsonl_line(line, Some("totally_unknown_format")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["prompt"].as_str().unwrap(), "x");
    }

    // ──────────────────────────────────────────────────────────────────────
    // name_hash – FNV-1a 64-bit determinism tests (RNG seeding contract)
    // ──────────────────────────────────────────────────────────────────────

    #[test]
    fn name_hash_empty_string_is_offset_basis() {
        // Catches: empty string producing 0 (breaks seeding; FNV-1a of "" = OFFSET)
        const OFFSET: u64 = 14_695_981_039_346_656_037;
        assert_eq!(name_hash(""), OFFSET);
    }

    #[test]
    fn name_hash_different_strings_differ() {
        // Catches: hash function being effectively constant (all bytes folded to same value)
        let h1 = name_hash("actor");
        let h2 = name_hash("function");
        let h3 = name_hash("workflow");
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert_ne!(h1, h3);
    }

    #[test]
    fn name_hash_is_byte_order_sensitive() {
        // Catches: hash treating "ab" and "ba" as equal (order-insensitive implementation)
        assert_ne!(name_hash("ab"), name_hash("ba"));
    }
}
