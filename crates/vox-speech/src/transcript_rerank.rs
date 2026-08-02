//! Transcript candidate ranking: synthetic n-best + optional compiler-aware rescoring.

use std::collections::HashSet;

use crate::contextual_bias::bias_hit_score;
use crate::refine::DomainMode;

/// Build alternative transcripts from one ASR pass (raw + refined + spoken-code normalize).
/// Whisper does not expose true n-best here yet; this list improves downstream disambiguation.
#[must_use]
pub fn build_transcript_candidates(raw: &str, refined: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |s: &str| {
        let t = s.trim();
        if t.is_empty() {
            return;
        }
        if seen.insert(t.to_string()) {
            out.push(t.to_string());
        }
    };

    push(refined);
    push(raw);
    let norm = crate::speech_normalize::normalize_spoken_code_phrase(refined);
    push(&norm);
    if norm != refined {
        let norm2 = crate::speech_normalize::normalize_spoken_code_phrase(raw);
        push(&norm2);
    }
    out
}

/// Extra signal: keep n-best strings closer to raw ASR tokens (reduces “fluent but wrong” drift).
#[cfg(feature = "compiler-rerank")]
fn raw_candidate_alignment_bonus(raw: &str, candidate: &str) -> u32 {
    let c = candidate.to_ascii_lowercase();
    let mut bonus = 0u32;
    for tok in raw
        .split(|x: char| !(x.is_alphanumeric() || x == '_'))
        .filter(|t| t.len() >= 3)
    {
        let t = tok.to_ascii_lowercase();
        if c.contains(&t) {
            bonus = bonus.saturating_add(3);
        }
    }
    bonus.min(120)
}

#[cfg(feature = "compiler-rerank")]
fn vox_frontend_penalty(source: &str) -> (u8, u32) {
    use vox_compiler::hir::lower_module;
    use vox_compiler::hir::validate_module;
    use vox_compiler::lexer::lex;
    use vox_compiler::parser::parse;
    use vox_compiler::typeck::diagnostics::TypeckSeverity;
    use vox_compiler::typeck::typecheck_ast_module;

    let tokens = lex(source);
    let Ok(module) = parse(tokens) else {
        return (1, 1);
    };
    let mut penalty: u32 = 0;
    for d in typecheck_ast_module(source, &module) {
        match d.severity {
            TypeckSeverity::Error => penalty = penalty.saturating_add(100),
            TypeckSeverity::Warning => penalty = penalty.saturating_add(1),
        }
    }
    let hir = lower_module(&module);
    penalty = penalty.saturating_add(validate_module(&hir).len() as u32 * 100);
    (0, penalty)
}

/// `scored_pair` gated to code-domain dictation. Non-code domains never pay the compiler-frontend
/// (lex/parse/typecheck/lower/validate) pass — that pass exists to prefer transcripts that read as
/// valid Vox source, which is meaningless (and costly) for plain-English dictation.
#[cfg(feature = "compiler-rerank")]
fn scored_pair(candidate: &str, raw_reference: Option<&str>, domain: DomainMode) -> (u8, u32) {
    if domain != DomainMode::Code {
        return (0, 0);
    }
    let (fail, mut pen) = vox_frontend_penalty(candidate);
    if let Some(r) = raw_reference {
        pen = pen.saturating_sub(raw_candidate_alignment_bonus(r, candidate));
    }
    (fail, pen)
}

/// Pick index of the best candidate using lexicographic order on `(parse_failed, penalty)`.
#[must_use]
pub fn pick_best_transcript_index(candidates: &[String]) -> usize {
    pick_best_transcript_index_with_raw(candidates, None)
}

/// Like [`pick_best_transcript_index`] but prefers candidates that retain tokens from the raw transcript.
///
/// Always scores as code-domain dictation (matches this function's historical, pre-domain-gate
/// behavior); callers that know the dictation domain should use
/// [`pick_best_transcript_index_with_raw_and_domain`] instead.
#[must_use]
pub fn pick_best_transcript_index_with_raw(
    candidates: &[String],
    raw_reference: Option<&str>,
) -> usize {
    pick_best_transcript_index_with_raw_and_domain(candidates, raw_reference, DomainMode::Code)
}

/// Like [`pick_best_transcript_index_with_raw`] but skips the compiler-frontend penalty entirely
/// unless `domain` is [`DomainMode::Code`].
#[must_use]
pub fn pick_best_transcript_index_with_raw_and_domain(
    candidates: &[String],
    raw_reference: Option<&str>,
    domain: DomainMode,
) -> usize {
    if candidates.is_empty() {
        return 0;
    }
    #[cfg(not(feature = "compiler-rerank"))]
    {
        let _ = (raw_reference, domain);
        0
    }
    #[cfg(feature = "compiler-rerank")]
    {
        let mut best_i = 0usize;
        let mut best_score = scored_pair(&candidates[0], raw_reference, domain);
        for (i, c) in candidates.iter().enumerate().skip(1) {
            let s = scored_pair(c, raw_reference, domain);
            if s < best_score {
                best_score = s;
                best_i = i;
            }
        }
        best_i
    }
}

/// Reorder `candidates` so the compiler-preferred hypothesis is first; returns owned list.
#[must_use]
pub fn rerank_candidates_best_first(candidates: Vec<String>) -> Vec<String> {
    rerank_candidates_best_first_with_raw(candidates, None)
}

/// Like [`rerank_candidates_best_first`] but uses `raw_reference` to prefer hypotheses that retain ASR tokens.
#[must_use]
pub fn rerank_candidates_best_first_with_raw(
    candidates: Vec<String>,
    raw_reference: Option<&str>,
) -> Vec<String> {
    rerank_candidates_best_first_with_raw_and_domain(candidates, raw_reference, DomainMode::Code)
}

/// Like [`rerank_candidates_best_first_with_raw`] but gates the compiler-frontend penalty on `domain`.
#[must_use]
pub fn rerank_candidates_best_first_with_raw_and_domain(
    mut candidates: Vec<String>,
    raw_reference: Option<&str>,
    domain: DomainMode,
) -> Vec<String> {
    if candidates.len() < 2 {
        return candidates;
    }
    let idx = pick_best_transcript_index_with_raw_and_domain(&candidates, raw_reference, domain);
    if idx != 0 && idx < candidates.len() {
        candidates.swap(0, idx);
    }
    candidates
}

/// Compiler-first rerank, then order remaining hypotheses by contextual phrase hits (n-best quality).
///
/// The compiler-frontend penalty (which requires source to lex/parse/typecheck as Vox) only applies
/// when `domain` is [`DomainMode::Code`]; other domains (e.g. plain-English dictation) skip it and
/// fall back to the contextual bias-phrase heuristic below.
#[must_use]
pub fn rerank_candidates_best_first_with_context(
    mut candidates: Vec<String>,
    bias_phrases: &[String],
    raw_reference: Option<&str>,
    domain: DomainMode,
) -> Vec<String> {
    candidates = rerank_candidates_best_first_with_raw_and_domain(candidates, raw_reference, domain);
    if bias_phrases.is_empty() || candidates.len() < 2 {
        return candidates;
    }
    #[cfg(feature = "compiler-rerank")]
    {
        let mut tail = candidates.split_off(1);
        tail.sort_by(|a, b| {
            let sa = bias_hit_score(a, bias_phrases);
            let sb = bias_hit_score(b, bias_phrases);
            sb.cmp(&sa)
        });
        candidates.extend(tail);
        candidates
    }
    #[cfg(not(feature = "compiler-rerank"))]
    {
        let mut indexed: Vec<(usize, String)> = candidates.into_iter().enumerate().collect();
        indexed.sort_by(|(ia, a), (ib, b)| {
            let sa = bias_hit_score(a, bias_phrases);
            let sb = bias_hit_score(b, bias_phrases);
            sb.cmp(&sa).then_with(|| ia.cmp(ib))
        });
        indexed.into_iter().map(|(_, s)| s).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dedupes() {
        let v = build_transcript_candidates("hello", "hello");
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn rerank_context_prefers_hotwords_without_compiler() {
        let cands = vec![
            "do something vague".to_string(),
            "workflow uses MENS adapter".to_string(),
        ];
        let bias = vec!["MENS".to_string()];
        let out =
            rerank_candidates_best_first_with_context(cands, &bias, None, DomainMode::Code);
        #[cfg(not(feature = "compiler-rerank"))]
        assert!(
            out[0].contains("MENS"),
            "expected bias to prefer MENS hypothesis: {:?}",
            out
        );
        #[cfg(feature = "compiler-rerank")]
        {
            // Compiler-first ordering may keep a parse-friendlier head; tail is bias-sorted.
            assert!(
                out.iter().any(|s| s.contains("MENS")),
                "expected a MENS hypothesis retained: {:?}",
                out
            );
        }
    }

    #[test]
    fn rerank_prefers_parseable_vox_when_compiler_rerank_enabled() {
        let cands = vec![
            "this is not vox [[[ broken".to_string(),
            "workflow w() { }".to_string(),
        ];
        let idx = pick_best_transcript_index(&cands);
        #[cfg(feature = "compiler-rerank")]
        assert_eq!(idx, 1, "parseable Vox should beat junk");
        #[cfg(not(feature = "compiler-rerank"))]
        assert_eq!(idx, 0);
        #[cfg(feature = "compiler-rerank")]
        {
            let cands = rerank_candidates_best_first(cands);
            assert!(
                cands[0].contains("workflow"),
                "first after rerank: {:?}",
                cands
            );
        }
    }

    #[cfg(feature = "compiler-rerank")]
    #[test]
    fn compiler_penalty_not_applied_outside_code_domain() {
        // "workflow w() { }" is Vox-shaped and would win under the compiler-frontend penalty;
        // "this is not vox [[[ broken" is not. For General dictation, the compiler penalty must
        // be skipped, so the chosen index falls back to the raw n-best ordering (index 0), not
        // the compiler-preferred one (index 1).
        let cands = vec![
            "this is not vox [[[ broken".to_string(),
            "workflow w() { }".to_string(),
        ];
        let idx_general = pick_best_transcript_index_with_raw_and_domain(
            &cands,
            None,
            DomainMode::General,
        );
        assert_eq!(
            idx_general, 0,
            "compiler-frontend penalty must not influence General-domain dictation: {:?}",
            cands
        );

        let idx_code =
            pick_best_transcript_index_with_raw_and_domain(&cands, None, DomainMode::Code);
        assert_eq!(
            idx_code, 1,
            "compiler-frontend penalty should still apply for Code-domain dictation: {:?}",
            cands
        );
    }
}
