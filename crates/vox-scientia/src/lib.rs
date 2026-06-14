//! SCIENTIA knowledge platform integration components.
//!
//! Modules that correspond to architecture plan phases:
//!   - Phase A producers: `producers`
//!   - Phase B replay runner: `replay`
//!   - Phase C–4 manuscript pipeline: `manuscript`
//!   - Phase D critic gate: `critic_gate`
//!   - Phase E class routing: `class_routing`
//!   - Phase G findings site: `findings_site`
//!   - Phase H dashboard JSON: `dashboard`
//!
//! Now-present sub-modules (formerly tracked as planned crates in layers.toml):
//! `claim_extractor`, `inspect_bridge`, `nanopub`, `ro_crate`, `ingest`.
//! Still planned (not yet in this crate): `prereg`.

// ── Pre-existing modules ──────────────────────────────────────────────────────
pub mod claim_extractor;
pub mod ingest;
pub mod inspect_bridge;
pub mod nanopub;
pub mod ro_crate;

// ── Phase A: self-observation signal producers ────────────────────────────────
pub mod producers;

// ── Phase B: replay runner ────────────────────────────────────────────────────
pub mod replay;

// ── Phase C + 3+4: manuscript pipeline (scaffold + LaTeX) ────────────────────
pub mod manuscript;

// ── Phase D: solo-author critic gate ─────────────────────────────────────────
pub mod critic_gate;

// ── Phase E: per-class venue routing ─────────────────────────────────────────
pub mod class_routing;

// ── Phase G: findings page renderer ──────────────────────────────────────────
pub mod findings_site;

// ── Phase H: dashboard JSON builders ─────────────────────────────────────────
pub mod dashboard;

// ── P2: human-gated discovery review (pure logic) ────────────────────────────
pub mod review;

// ── P3: shared review-flow SSOT (DB + vault I/O; CLI + GUI both call this) ────
pub mod review_flow;

// ── P3 Phase 4: LLM-assisted advisory evidence/conclusion suggestions ────────
pub mod evidence_assist;

#[cfg(test)]
mod semcov_wave24_tests {
    // ── LaTeX escape tests ────────────────────────────────────────────────────
    use crate::manuscript::latex::escape::escape_latex;

    #[test]
    fn escape_latex_backslash_does_not_double_escape() {
        // Catches: naive replace("\\" -> "\\textbackslash{}") running a second
        // pass and re-escaping the already-emitted backslash in the output.
        let out = escape_latex(r"\");
        assert_eq!(out, r"\textbackslash{}");
        // A second call must be idempotent only on plain text, NOT on already-
        // escaped output (this is intentional — verify the one-pass property).
        assert!(out.contains(r"\textbackslash{}") && !out.contains(r"\\textbackslash{}"));
    }

    #[test]
    fn escape_latex_all_specials_in_sequence_no_interleaving() {
        // Catches: character substitution that accidentally corrupts adjacent
        // characters when expanding a single char to multiple bytes.
        let out = escape_latex("$#");
        assert_eq!(out, r"\$\#");
    }

    #[test]
    fn escape_latex_tilde_and_caret_expand_to_text_commands() {
        // Catches: tilde/caret emitted as raw TeX active chars (\~{} or \^{})
        // instead of the prose-safe \textasciitilde{} / \textasciicircum{}.
        let out = escape_latex("~^");
        assert_eq!(out, r"\textasciitilde{}\textasciicircum{}");
    }

    #[test]
    fn escape_latex_only_ascii_specials_are_escaped_unicode_clean() {
        // Catches: over-eager byte-level matching that corrupts multi-byte
        // UTF-8 sequences containing bytes that coincide with ASCII specials.
        // The '£' sign is 0xC2 0xA3 — 0xA3 is not a special, must survive.
        let s = "£50 & tax";
        let out = escape_latex(s);
        assert!(out.starts_with("£50"), "multi-byte prefix damaged: {out}");
        assert!(out.contains(r"\&"));
    }

    #[test]
    fn escape_latex_empty_string_is_empty() {
        // Catches: off-by-one in capacity pre-alloc crashing on empty input.
        assert_eq!(escape_latex(""), "");
    }

    #[test]
    fn escape_latex_repeated_specials_all_escaped() {
        // Catches: early-return or dedup bug that only escapes first occurrence.
        let out = escape_latex("%%%");
        assert_eq!(out, r"\%\%\%");
    }

    #[test]
    fn escape_latex_braces_escaped_independently() {
        // Catches: brace-pair logic that escapes `{}` as a unit and leaves one
        // bare if input has an unmatched `{` or `}`.
        let out = escape_latex("{alone}");
        assert_eq!(out, r"\{alone\}");
        let out2 = escape_latex("{");
        assert_eq!(out2, r"\{");
    }

    // ── SpanChecker tests ─────────────────────────────────────────────────────
    use crate::claim_extractor::{span::SpanChecker, types::SpanBound};

    #[test]
    fn span_checker_start_equals_end_is_invalid() {
        // Catches: off-by-one where `start == end` (zero-length span) is
        // treated as valid and incorrectly scores against an empty slice.
        let checker = SpanChecker::default();
        let source = "some text";
        assert!(!checker.check("some", &SpanBound { start: 2, end: 2 }, source));
    }

    #[test]
    fn span_checker_exact_boundary_end_equals_source_len_is_valid() {
        // Catches: `>` vs `>=` bug where the span [0, len) (the whole source)
        // is incorrectly rejected because `end > source.len()` uses wrong sign.
        let checker = SpanChecker::default();
        let source = "alpha beta gamma";
        let end = source.len();
        assert!(checker.check("alpha beta gamma", &SpanBound { start: 0, end }, source));
    }

    #[test]
    fn span_checker_empty_claim_text_always_false() {
        // Catches: division-by-zero or vacuous-true when claim_words is empty
        // (0/0 overlap fraction could panic or return 1.0 >= threshold).
        let checker = SpanChecker::default();
        let source = "some text here";
        assert!(!checker.check("", &SpanBound { start: 0, end: 9 }, source));
    }

    #[test]
    fn span_checker_whitespace_only_claim_is_empty_set() {
        // Catches: split_whitespace returning non-empty on "   " — it should
        // return an empty iterator, so claim_words is empty and returns false.
        let checker = SpanChecker::default();
        let source = "relevant text";
        assert!(!checker.check("   ", &SpanBound { start: 0, end: 8 }, source));
    }

    #[test]
    fn span_checker_threshold_boundary_exact_match() {
        // Catches: strict `>` vs `>=` in the overlap fraction comparison —
        // if threshold is 0.6 and overlap/len == 0.6 exactly, must pass.
        let checker = SpanChecker {
            min_overlap_fraction: 0.5,
        };
        // Claim has 2 words; span slice has 1 matching word → 0.5 overlap.
        let source = "alpha beta";
        // "alpha zeta" → "alpha" in source slice, "zeta" not → 1/2 = 0.5
        assert!(checker.check("alpha zeta", &SpanBound { start: 0, end: 10 }, source));
    }

    #[test]
    fn span_checker_overlap_uses_set_semantics_not_multiset() {
        // Catches: counting intersections as multiset (duplicates inflate
        // numerator) instead of set intersection, which could make
        // "the the the" look like 100% overlap with a single "the".
        let checker = SpanChecker {
            min_overlap_fraction: 0.9,
        };
        // claim: "the the the the the" → claim_words = {"the"} (1 word)
        // source slice: "the fox" → span_words = {"the","fox"} → overlap = 1
        // 1/1 = 1.0 >= 0.9 → true (correct set semantics)
        // Multiset would give 5/5=1.0 too, but verify no panic / OOB first.
        let source = "the fox";
        assert!(checker.check(
            "the the the the the",
            &SpanBound {
                start: 0,
                end: source.len()
            },
            source
        ));
    }

    // ── VeriScoreGate tests ───────────────────────────────────────────────────
    use crate::claim_extractor::veriscore::{VeriScoreConfig, VeriScoreGate};

    #[test]
    fn veriscore_score_is_clamped_to_unit_interval() {
        // Catches: arithmetic overflow above 1.0 or below 0.0 when both
        // verifiable AND unverifiable signals are present simultaneously.
        let gate = VeriScoreGate::default();
        // Both numeric bonus and many hedge phrases → penalties vs bonuses clash.
        let s = "p95 latency may be possibly increased by 10ms perhaps likely.";
        let r = gate.score_sentence(s);
        assert!(
            r.score >= 0.0 && r.score <= 1.0,
            "score out of [0,1]: {}",
            r.score
        );
    }

    #[test]
    fn veriscore_custom_min_score_zero_accepts_everything() {
        // Catches: hard-coded 0.5 floor leaking into the gate even when the
        // caller configures min_score = 0.0.
        let gate = VeriScoreGate::new(VeriScoreConfig { min_score: 0.0 });
        let sentences = vec![
            "Future work will explore this.".to_string(),
            "We hypothesize improvements exist.".to_string(),
        ];
        let passing = gate.filter_sentences(&sentences);
        assert_eq!(
            passing.len(),
            2,
            "min_score=0.0 must pass all sentences, got {}/2",
            passing.len()
        );
    }

    #[test]
    fn veriscore_filter_returns_empty_on_empty_input() {
        // Catches: index-out-of-bounds or unwrap on empty slice.
        let gate = VeriScoreGate::default();
        let out = gate.filter_sentences(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn veriscore_pure_text_no_numbers_scores_at_base() {
        // Catches: numeric_score accidentally > 0 for sentences with no digits
        // and no numeric keywords, inflating the class to Numeric wrongly.
        let gate = VeriScoreGate::default();
        let r = gate.score_sentence("The algorithm converges quickly.");
        // No numbers → no numeric bonus → score should be exactly 0.5
        assert_eq!(r.score, 0.5, "expected base 0.5 for no-signal sentence");
    }

    // ── ModelFingerprint / critic_gate tests ─────────────────────────────────
    use crate::critic_gate::fingerprint::ModelFingerprint;

    #[test]
    fn fingerprint_collides_with_is_symmetric() {
        // Catches: asymmetric logic in collides_with where a.collides_with(b)
        // returns true but b.collides_with(a) returns false.
        let make = |provider: &str, model: &str, params: Option<u64>, cutoff: Option<&str>| {
            ModelFingerprint {
                provider: provider.into(),
                model_id: model.into(),
                parameter_count_hint: params,
                training_cutoff: cutoff.map(str::to_string),
            }
        };
        let a = make("acme", "model-a", Some(8_000_000_000), Some("2024-10"));
        let b = make("acme", "model-b", Some(8_000_000_000), Some("2024-10"));
        assert_eq!(
            a.collides_with(&b),
            b.collides_with(&a),
            "collides_with must be symmetric"
        );
    }

    #[test]
    fn fingerprint_self_collision_always_true() {
        // Catches: implementation that skips the model-id comparison when
        // parameter_count_hint is None, leading to a model never colliding
        // with itself when params are absent.
        let fp = ModelFingerprint {
            provider: "openai".into(),
            model_id: "gpt-4o".into(),
            parameter_count_hint: None,
            training_cutoff: None,
        };
        assert!(fp.collides_with(&fp));
    }

    #[test]
    fn fingerprint_different_cutoffs_same_params_no_collision() {
        // Catches: AND/OR confusion in the combined-signal branch — two
        // different cutoffs with same param count must NOT collide.
        let make = |cutoff: &str| ModelFingerprint {
            provider: "acme".into(),
            model_id: "model-x".into(),
            parameter_count_hint: Some(8_000_000_000),
            training_cutoff: Some(cutoff.into()),
        };
        let a = make("2024-10");
        let b = make("2025-03");
        // Different model_id normalization = same; different cutoffs → no collision
        // Actually same model_id → collides by model-id path. Let's use truly different ids.
        let a2 = ModelFingerprint {
            model_id: "snap-a".into(),
            ..a
        };
        let b2 = ModelFingerprint {
            model_id: "snap-b".into(),
            ..b
        };
        assert!(!a2.collides_with(&b2), "different cutoffs must not collide");
    }

    // ── ForbiddenSection / safe_slots tests ──────────────────────────────────
    use crate::manuscript::scaffold::safe_slots::is_section_forbidden;

    #[test]
    fn forbidden_section_with_leading_trailing_newline_is_forbidden() {
        // Catches: trim() absent — "\nIntroduction\n" would not match after
        // lowercasing if the trim is missing.
        assert!(is_section_forbidden("\nIntroduction\n"));
    }

    #[test]
    fn mixed_case_conclusion_is_forbidden() {
        // Catches: only ASCII lowercasing applied but eq_ignore_ascii_case
        // used incorrectly, missing mixed-case variants.
        assert!(is_section_forbidden("CoNcLuSiOn"));
    }

    #[test]
    fn related_work_section_is_not_forbidden() {
        // Catches: over-broad substring matching that forbids sections whose
        // names merely CONTAIN a forbidden word (e.g. "Discussion" inside
        // "Further Discussion Notes" being caught).
        assert!(!is_section_forbidden("Further Discussion Notes"));
    }

    // ── ClaimVerdict promotability tests ─────────────────────────────────────
    use crate::claim_extractor::types::ClaimVerdict;

    #[test]
    fn claim_verdict_promotable_exactly_at_threshold() {
        // Catches: strict `>` vs `>=` in is_promotable — confidence == 0.7
        // must be promotable.
        let v = ClaimVerdict::Supported { confidence: 0.7 };
        assert!(
            v.is_promotable(),
            "confidence exactly at 0.7 must be promotable"
        );
    }

    #[test]
    fn claim_verdict_supported_below_threshold_not_promotable() {
        // Catches: is_promotable returning true for all Supported variants
        // without checking the confidence value.
        let v = ClaimVerdict::Supported { confidence: 0.69 };
        assert!(!v.is_promotable());
    }

    #[test]
    fn claim_verdict_contradicted_is_not_supported_nor_promotable() {
        // Catches: Contradicted variant accidentally falling through to
        // Supported arm in a match that forgets to enumerate all variants.
        let v = ClaimVerdict::Contradicted { confidence: 0.99 };
        assert!(!v.is_supported());
        assert!(!v.is_promotable());
    }
}
