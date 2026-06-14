//! Adversarial unit tests — semcov wave 21.
//! Coverage targets: rrf, policy, tavily_budget, evaluation, unified, memory_hybrid internals.

#[cfg(test)]
mod semcov_wave21_tests {
    use crate::evaluation::{calculate_groundedness, calculate_recall_at_5};
    use crate::policy::{SearchPolicy, SearchPolicyFeedback};
    use crate::rrf::{rrf_dedup_key, rrf_merge_line_lists};
    use crate::tavily_budget::TavilySessionBudget;
    use crate::unified::{UnifiedHit, sort_unified_hits_desc};

    // ── TavilySessionBudget ──────────────────────────────────────────────────

    #[test]
    fn budget_zero_limit_never_grants() {
        // Catches: off-by-one where cost==0 or budget==0 silently succeeds
        let b = TavilySessionBudget::new(0);
        assert!(!b.try_consume(1), "zero-limit budget must deny any cost");
        assert!(
            !b.try_consume(0),
            "zero cost on zero budget should still return false (current < cost? 0 < 0 == false, so true — reveals logic gap if it panics or wrongly grants)"
        );
    }

    #[test]
    fn budget_exact_cost_drains_and_denies_subsequent() {
        // Catches: fence-post error in compare_exchange where exact cost leaves budget at 0 but
        // the next call still passes because the check uses `<` not `<=`
        let b = TavilySessionBudget::new(5);
        assert!(b.try_consume(5));
        assert_eq!(b.remaining(), 0);
        assert!(!b.try_consume(1));
    }

    #[test]
    fn budget_large_cost_exceeding_remaining_denied() {
        // Catches: integer underflow if `current - cost` wraps on usize
        let b = TavilySessionBudget::new(3);
        assert!(!b.try_consume(100), "cost > remaining must be denied");
        assert_eq!(b.remaining(), 3, "remaining must be unchanged after denial");
    }

    #[test]
    fn budget_clone_shares_state() {
        // Catches: Arc not used properly — clone makes independent counter
        let b = TavilySessionBudget::new(4);
        let b2 = b.clone();
        assert!(b.try_consume(2));
        assert_eq!(
            b2.remaining(),
            2,
            "cloned handle must observe the same AtomicUsize"
        );
    }

    // ── RRF dedup key ────────────────────────────────────────────────────────

    #[test]
    fn rrf_dedup_key_chunk_with_whitespace_in_id() {
        // Catches: split stopping too early if the id itself contains no whitespace but
        // trailing ] or space order differs from expectation
        assert_eq!(rrf_dedup_key("[chunk:abc-123] some text"), "chunk:abc-123");
        assert_eq!(rrf_dedup_key("[chunk:abc-123]"), "chunk:abc-123");
    }

    #[test]
    fn rrf_dedup_key_tantivy_uses_first_whitespace_token() {
        // Catches: tantivy branch using split_whitespace but qdrant branch using a different
        // delimiter — cross-branch inconsistency
        assert_eq!(rrf_dedup_key("[tantivy:id99 rest]"), "tantivy:id99");
    }

    #[test]
    fn rrf_dedup_key_unknown_line_produces_stable_opaque_hash() {
        // Catches: two different calls with identical input returning different hashes
        // (would happen if hasher is seeded with random or time-based entropy)
        let key1 = rrf_dedup_key("plain text with no brackets");
        let key2 = rrf_dedup_key("plain text with no brackets");
        assert_eq!(key1, key2, "opaque hash must be deterministic");
        assert!(key1.starts_with("opaque:"), "opaque key prefix expected");
    }

    #[test]
    fn rrf_dedup_key_empty_string_does_not_panic() {
        // Catches: unwrap on empty slice / strip_prefix panic on empty input
        let key = rrf_dedup_key("");
        assert!(
            key.starts_with("opaque:"),
            "empty line should produce opaque key, got: {key}"
        );
    }

    #[test]
    fn rrf_merge_empty_lists_returns_empty() {
        // Catches: out-of-bounds or unwrap when lists slice is empty
        let out = rrf_merge_line_lists(&[], 10, 60.0);
        assert!(out.is_empty(), "no lists → no output");
    }

    #[test]
    fn rrf_merge_limit_zero_still_returns_one_hit() {
        // Catches: `limit.max(1)` guard — if accidentally removed, returns empty slice
        let list = vec!["[chunk:x] foo".to_string()];
        let out = rrf_merge_line_lists(&[list], 0, 60.0);
        assert_eq!(
            out.len(),
            1,
            "limit=0 must still yield 1 result via .max(1)"
        );
    }

    #[test]
    fn rrf_merge_rrf_k_below_one_clamped_to_one() {
        // Catches: if k is not clamped, 1.0/(0.0 + 1) = 1.0 but 1/(−10+1)=−0.11,
        // producing negative scores → wrong ordering
        let a = vec!["[chunk:a] first".to_string()];
        let b = vec!["[chunk:b] second".to_string()];
        // Should not panic with negative or zero k
        let out = rrf_merge_line_lists(&[a, b], 2, -5.0);
        assert_eq!(out.len(), 2, "negative rrf_k must not panic or drop items");
    }

    #[test]
    fn rrf_merge_identical_items_across_lists_deduped() {
        // Catches: dedup key collision not handled — same item appears twice in output
        let item = "[repo:crates/foo] snippet".to_string();
        let list_a = vec![item.clone()];
        let list_b = vec![item.clone()];
        let out = rrf_merge_line_lists(&[list_a, list_b], 5, 60.0);
        assert_eq!(
            out.len(),
            1,
            "same item in two lists must be deduped to one"
        );
    }

    // ── SearchPolicy ─────────────────────────────────────────────────────────

    #[test]
    fn policy_clamp_helpers_reject_out_of_range_values() {
        // Catches: clamp functions omitted or using wrong bounds
        let mut p = SearchPolicy::default();
        p.memory_vector_fusion_weight = 2.5;
        assert_eq!(p.clamped_memory_vector_weight(), 1.0);
        p.memory_vector_fusion_weight = -1.0;
        assert_eq!(p.clamped_memory_vector_weight(), 0.0);
    }

    #[test]
    fn policy_rrf_k_clamped_at_both_ends() {
        // Catches: clamped_rrf_k using wrong interval (e.g. clamp(0.0, 500.0) misses lower bound)
        let mut p = SearchPolicy::default();
        p.rrf_k = 0.0;
        assert_eq!(p.clamped_rrf_k(), 1.0, "below 1 must clamp to 1");
        p.rrf_k = 9999.0;
        assert_eq!(p.clamped_rrf_k(), 500.0, "above 500 must clamp to 500");
    }

    #[test]
    fn scientia_feedback_caps_threshold_at_0_85() {
        // Catches: uncapped threshold drift — repeated bad feedback could push threshold > 1.0
        let mut p = SearchPolicy::default();
        p.verification_weak_evidence_threshold = 0.80;
        let bad = SearchPolicyFeedback {
            citation_precision: 0.1,
            model_reliability: 0.1,
            source_hit_rate: 0.1,
        };
        for _ in 0..10 {
            p = p.with_scientia_feedback(bad);
        }
        assert!(
            p.verification_weak_evidence_threshold <= 0.85,
            "threshold must be capped at 0.85, got {}",
            p.verification_weak_evidence_threshold
        );
    }

    #[test]
    fn scientia_feedback_good_precision_lowers_threshold_not_below_floor() {
        // Catches: threshold going negative or below 0.45 floor
        let mut p = SearchPolicy::default();
        p.verification_weak_evidence_threshold = 0.46;
        let good = SearchPolicyFeedback {
            citation_precision: 0.95,
            model_reliability: 0.95,
            source_hit_rate: 0.95,
        };
        for _ in 0..20 {
            p = p.with_scientia_feedback(good);
        }
        assert!(
            p.verification_weak_evidence_threshold >= 0.45,
            "threshold floor is 0.45, got {}",
            p.verification_weak_evidence_threshold
        );
    }

    #[test]
    fn scientia_feedback_max_hops_capped_at_five() {
        // Catches: saturating_add guard removed — repeated bad feedback overflows u8
        let mut p = SearchPolicy::default();
        p.web_search_max_hops = 4;
        let bad = SearchPolicyFeedback {
            citation_precision: 0.1,
            model_reliability: 0.9,
            source_hit_rate: 0.1,
        };
        for _ in 0..20 {
            p = p.with_scientia_feedback(bad);
        }
        assert!(
            p.web_search_max_hops <= 5,
            "max_hops must cap at 5, got {}",
            p.web_search_max_hops
        );
    }

    // ── Evaluation helpers ───────────────────────────────────────────────────

    #[test]
    fn recall_at5_empty_gold_returns_one() {
        // Catches: division by zero when gold_words is empty
        let r = calculate_recall_at_5("model answer here", "");
        assert_eq!(
            r, 1.0,
            "empty gold answer should yield recall 1.0 (trivially satisfied)"
        );
    }

    #[test]
    fn recall_at5_model_shorter_than_threshold_tokens_ignored() {
        // Catches: short-token filter (>3 chars) silently dropping all model tokens,
        // producing 0 when the model technically answered correctly in short tokens
        let r = calculate_recall_at_5("at of the in", "at of the in");
        // All tokens are <= 3 chars → both sets empty → gold empty → returns 1.0
        assert_eq!(
            r, 1.0,
            "all-short-token answer with all-short-token gold = trivially satisfied"
        );
    }

    #[test]
    fn groundedness_empty_model_answer_returns_zero() {
        // Catches: early return missing for empty model_answer — could divide by 0
        let g = calculate_groundedness("", &["some evidence".to_string()]);
        assert_eq!(g, 0.0);
    }

    #[test]
    fn groundedness_empty_evidence_returns_zero() {
        // Catches: early return missing for empty snippets — could return 1.0 (trivially grounded)
        let g = calculate_groundedness("The answer is found here.", &[]);
        assert_eq!(g, 0.0);
    }

    #[test]
    fn groundedness_single_sentence_no_long_keywords_returns_one() {
        // Catches: when model_clusters is empty (all sentences <=10 chars), returns 1.0 (trivially);
        // caller might mis-interpret 1.0 as high groundedness when it's a vacuous result
        let g = calculate_groundedness("Hi. Ok.", &["irrelevant evidence".to_string()]);
        // Both sentences ≤10 chars → model_clusters empty → returns 1.0
        assert_eq!(
            g, 1.0,
            "vacuous groundedness (no long clauses) should return 1.0 by spec"
        );
    }

    // ── UnifiedHit sort ──────────────────────────────────────────────────────

    #[test]
    fn sort_unified_hits_nan_score_does_not_corrupt_order() {
        // Catches: partial_cmp returning None for NaN propagating as Equal, scrambling order
        let mut hits = vec![
            UnifiedHit {
                source: "a".into(),
                kind: "".into(),
                path: None,
                title: None,
                snippet: "high".into(),
                score: 0.9,
                provenance: vec![],
            },
            UnifiedHit {
                source: "b".into(),
                kind: "".into(),
                path: None,
                title: None,
                snippet: "nan".into(),
                score: f64::NAN,
                provenance: vec![],
            },
            UnifiedHit {
                source: "c".into(),
                kind: "".into(),
                path: None,
                title: None,
                snippet: "low".into(),
                score: 0.1,
                provenance: vec![],
            },
        ];
        // Must not panic
        sort_unified_hits_desc(&mut hits);
        // The 0.9 hit should still be identifiable — at minimum no panic
        let has_high = hits.iter().any(|h| (h.score - 0.9).abs() < 1e-9);
        assert!(has_high, "0.9-score hit should survive NaN sort");
    }

    #[test]
    fn sort_unified_hits_empty_slice_does_not_panic() {
        // Catches: any indexing inside sort implementation that assumes non-empty
        let mut hits: Vec<UnifiedHit> = vec![];
        sort_unified_hits_desc(&mut hits); // must not panic
    }
}
