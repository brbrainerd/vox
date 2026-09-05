//! Maps completion-time citation declarations into Socrates evidence accounting.
//!
//! Callers should embed retrieval snippet ids in completions using `[[voxcite:YOUR_REF]]` or pass
//! the same strings in [`crate::types::CompletionAttestation::evidence_citations`]. References
//! must appear as substrings (ASCII case-insensitive) in the session [`crate::ContextEnvelope`]
//! JSON used for grounding verification.

use std::collections::HashSet;

use crate::context_envelope::ContextEnvelope;
use crate::socrates::SocratesTaskContext;
use crate::types::CompletionAttestation;

const VOXCITE_OPEN: &str = "[[voxcite:";
const VOXCITE_CLOSE: &str = "]]";

/// Short lines (Unicode scalar count) are treated as procedural in [`classify_line_claim_kind`].
const CLAIM_LINE_MIN_CHARS: usize = 8;
/// Clause segments shorter than this (Unicode scalar count) are ignored in factual-mode citation
/// gap counting.
const FACTUAL_SEGMENT_MIN_CHARS: usize = 14;
/// ASCII inline compound street tokens (`Oststr.`, …) — scalar count, incl. `str` tail.
const COMPOUND_STREET_ASCII_STR_MIN_CHARS: usize = 6;
/// ASCII `*strasse` compounds (`Hauptstrasse.`, …) — min total scalar count.
const COMPOUND_STREET_ASCII_STRASSE_MIN_CHARS: usize = 10;
/// Unicode German street token (`Müllerstraße.`, …) — min scalar count.
const COMPOUND_STREET_UNICODE_MIN_CHARS: usize = 8;

/// Coarse claim taxonomy for factual-mode completion checks (W2 ledger alignment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    /// Asserts properties of the world / codebase (needs evidence when `factual_mode` is on).
    Factual,
    /// Instructions, steps, or process language (citations usually optional).
    Procedural,
    /// Hedged or opinion-like language.
    Speculative,
}

/// True when `needle` appears in `haystack_lower` (already ASCII-lowercased) with non-alphanumeric
/// boundaries (ASCII + `_`), so `merge` does not match inside `emerge` and `read` not inside `spread`.
#[must_use]
fn lower_contains_ascii_whole_word(haystack_lower: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    for (idx, _) in haystack_lower.match_indices(needle) {
        let before_ok = haystack_lower[..idx]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
        let after_idx = idx + needle.len();
        let after_ok = haystack_lower[after_idx..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Lightweight heuristic: single-line / clause classification (no LLM).
#[must_use]
pub fn classify_line_claim_kind(line: &str) -> ClaimKind {
    let t = line.trim();
    if t.chars().count() < CLAIM_LINE_MIN_CHARS {
        return ClaimKind::Procedural;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with('#')
        || lower.starts_with("//")
        || lower.starts_with("```")
        || lower.starts_with('|')
    {
        return ClaimKind::Procedural;
    }
    const SPEC_PHRASE: &[&str] = &[
        "i think",
        "appears to",
        "seem to",
        "seems to",
        "suggests that",
        "not sure",
        "i'm not sure",
        "in my opinion",
    ];
    if SPEC_PHRASE.iter().any(|m| lower.contains(m)) {
        return ClaimKind::Speculative;
    }
    const SPEC_WORD: &[&str] = &[
        "maybe",
        "perhaps",
        "might",
        "could",
        "possibly",
        "likely",
        "probably",
        "unclear",
        "uncertain",
        "imo",
    ];
    if SPEC_WORD
        .iter()
        .any(|w| lower_contains_ascii_whole_word(&lower, w))
    {
        return ClaimKind::Speculative;
    }
    const PROC_PHRASE: &[&str] = &["add file", "create a pr", "use command", "cherry-pick"];
    if PROC_PHRASE.iter().any(|m| lower.contains(m)) {
        return ClaimKind::Procedural;
    }
    const PROC_WORD: &[&str] = &[
        "run", "execute", "navigate", "open", "install", "click", "then", "first", "next",
        "submit", "refactor", "rename", "delete", "remove", "rebuild", "restart", "rebase",
        "squash", "bump",
    ];
    if PROC_WORD
        .iter()
        .any(|w| lower_contains_ascii_whole_word(&lower, w))
    {
        return ClaimKind::Procedural;
    }
    ClaimKind::Factual
}

/// Result of the opt-in, non-blocking post-reply grounding/hallucination
/// check (see [`crate::events::AgentEventKind::GroundingCheckCompleted`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundingCheckResult {
    /// Fraction (0.0-1.0) of judged clauses that were NOT classified
    /// [`ClaimKind::Speculative`]. 1.0 when there was nothing to judge.
    pub confidence: f64,
    /// True when `confidence` fell below [`GROUNDING_FLAG_THRESHOLD`].
    pub flagged: bool,
}

/// Confidence threshold below which [`assess_reply_confidence`] flags a
/// reply. Chosen so a reply needs a clear majority of its judged clauses to
/// read as hedged/uncertain before the GUI surfaces a badge — a single
/// hedge word in an otherwise confident multi-clause reply should not trip
/// it.
pub const GROUNDING_FLAG_THRESHOLD: f64 = 0.6;

/// Cheap, deterministic post-reply confidence check: reuses the same
/// per-clause claim classifier ([`classify_line_claim_kind`]) that factual-mode
/// citation gating already relies on. A reply whose clauses are mostly
/// hedged ("might", "unclear", "I think", ...) scores low confidence and is
/// flagged. This is intentionally NOT a second LLM call — the doc comment on
/// `GroundingCheckCompleted` promises the check "never delays the reply", so
/// it must be near-instant and run after the reply has already streamed.
/// Procedural clauses (steps/instructions) carry no confidence signal and
/// are excluded from the ratio so a numbered how-to reply doesn't get
/// penalized just for using imperative verbs.
#[must_use]
pub fn assess_reply_confidence(reply: &str) -> GroundingCheckResult {
    let judged: Vec<ClaimKind> = split_summary_into_claim_segments(reply)
        .into_iter()
        .map(classify_line_claim_kind)
        .filter(|k| !matches!(k, ClaimKind::Procedural))
        .collect();
    if judged.is_empty() {
        // Nothing to judge (empty, or entirely procedural/too-short clauses)
        // — neutral confidence, never flagged.
        return GroundingCheckResult {
            confidence: 1.0,
            flagged: false,
        };
    }
    let speculative = judged
        .iter()
        .filter(|k| matches!(k, ClaimKind::Speculative))
        .count();
    let confidence = 1.0 - (speculative as f64 / judged.len() as f64);
    GroundingCheckResult {
        confidence,
        flagged: confidence < GROUNDING_FLAG_THRESHOLD,
    }
}

/// Task M2 (`completeness_ok`): a reply that opens a code fence and never closes it reads as
/// truncated mid-answer. ` ``` ` markers must appear an even number of times.
#[must_use]
pub fn has_unbalanced_fence(text: &str) -> bool {
    !text.matches("```").count().is_multiple_of(2)
}

/// Task M2: phrases a user's follow-up commonly uses when the prior answer didn't land.
/// Deliberately small and literal (no fuzzy matching) — broadening this list later is easy;
/// narrowing a false-positive-prone one after it has already shipped and started driving
/// `completeness_ok` is not.
const REASK_DISSATISFACTION_PHRASES: &[&str] = &[
    "that's wrong",
    "that is wrong",
    "try again",
    "that didn't work",
    "that did not work",
    "no, actually",
    "not what i asked",
    "that's not right",
    "that is not right",
    "doesn't work",
    "does not work",
    "still not working",
    "still doesn't work",
];

fn reask_phrase_heuristic(followup_prompt: &str) -> bool {
    let lower = followup_prompt.to_lowercase();
    REASK_DISSATISFACTION_PHRASES
        .iter()
        .any(|phrase| lower.contains(phrase))
}

/// Jaccard-similarity threshold used by [`reask_similarity_heuristic`].
const REASK_SIMILARITY_THRESHOLD: f64 = 0.5;

/// Token-overlap (Jaccard) similarity over lowercased whitespace-split tokens, ignoring
/// tokens under 3 chars (articles/prepositions dominate short prompts and would inflate
/// overlap between genuinely unrelated questions).
fn reask_similarity_heuristic(original_prompt: &str, followup_prompt: &str) -> bool {
    fn tokenize(s: &str) -> std::collections::HashSet<String> {
        s.to_lowercase()
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|t| t.chars().count() >= 3)
            .collect()
    }
    let a = tokenize(original_prompt);
    let b = tokenize(followup_prompt);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let union = a.union(&b).count();
    if union == 0 {
        return false;
    }
    (a.intersection(&b).count() as f64 / union as f64) >= REASK_SIMILARITY_THRESHOLD
}

/// Task M2 (`completeness_ok`'s "user re-asked within 2 turns" clause): flags a follow-up
/// prompt as a re-ask of `original_prompt` when EITHER signal fires (design decision
/// confirmed with the user) — a literal dissatisfaction phrase, or high token-overlap
/// similarity to the original prompt. Combining the two trades precision for recall: a
/// missed re-ask silently keeps crediting a bad answer as a success, which is the worse
/// failure mode for a metric meant to catch bad answers.
#[must_use]
pub fn looks_like_reask(original_prompt: &str, followup_prompt: &str) -> bool {
    reask_phrase_heuristic(followup_prompt)
        || reask_similarity_heuristic(original_prompt, followup_prompt)
}

/// Split completion summaries into clauses for factual-mode scans.
///
/// Uses newlines, `;`, `!`, `?`, and `.` only when the period ends a sentence (next char is
/// whitespace or EOF) so version strings like `v1.2.3` stay intact. Periods after common
/// abbreviations (`Mr.`, `Mme.`, `e.g.`, …) do not start a new clause. German-style inline
/// compound street tokens (`Hauptstr.`, `Oststr.`, `Hauptstrasse.`, `Müllerstraße.`, …): ASCII
/// branch — capitalized token ≥ [`COMPOUND_STREET_ASCII_STR_MIN_CHARS`] ending in `str` (with
/// `istr`/`ustr`/`estr` exclusions) or ending in `strasse` with ≥
/// [`COMPOUND_STREET_ASCII_STRASSE_MIN_CHARS`] chars. Unicode German —
/// ≥ [`COMPOUND_STREET_UNICODE_MIN_CHARS`] chars ending in `straße` or `strasse`.
#[must_use]
pub(crate) fn split_summary_into_claim_segments(summary: &str) -> Vec<&str> {
    fn last_token_looks_like_compound_street_line(head_trimmed: &str) -> bool {
        let Some(tok) = head_trimmed.split_whitespace().next_back() else {
            return false;
        };
        let base = tok.trim_end_matches('.');
        if base.is_empty() {
            return false;
        }
        let Some(c0) = base.chars().next() else {
            return false;
        };
        let all_ascii = base.is_ascii();
        if all_ascii {
            if base.chars().count() < COMPOUND_STREET_ASCII_STR_MIN_CHARS
                || !c0.is_ascii_uppercase()
            {
                return false;
            }
            let lower = base.to_ascii_lowercase();
            let str_tail = lower.ends_with("str")
                && !lower.ends_with("istr")
                && !lower.ends_with("ustr")
                && !lower.ends_with("estr");
            let strasse_tail = lower.ends_with("strasse")
                && lower.chars().count() >= COMPOUND_STREET_ASCII_STRASSE_MIN_CHARS;
            str_tail || strasse_tail
        } else {
            if !c0.is_uppercase() {
                return false;
            }
            if base.chars().count() < COMPOUND_STREET_UNICODE_MIN_CHARS {
                return false;
            }
            if !base.chars().all(|c| {
                c.is_ascii_alphabetic() || matches!(c, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' | 'ß')
            }) {
                return false;
            }
            let lower = base.to_lowercase();
            lower.ends_with("straße") || lower.ends_with("strasse")
        }
    }

    fn last_token_is_sentence_abbrev(head_trimmed: &str) -> bool {
        let Some(tok) = head_trimmed.split_whitespace().next_back() else {
            return false;
        };
        let base = tok.trim_end_matches('.');
        let t = base.to_ascii_lowercase();
        // Titles and postal/corporate suffixes: require leading ASCII uppercase so lowercase typos
        // do not suppress real sentence breaks (`st.` / `inc.` / …).
        if matches!(
            t.as_str(),
            "st" | "jr" | "sr" | "ave" | "blvd" | "ltd" | "inc" | "mme" | "mlle" | "nr" | "tel"
        ) && !base.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            return false;
        }
        matches!(
            t.as_str(),
            "mr" | "mrs"
                | "ms"
                | "dr"
                | "prof"
                | "vs"
                | "etc"
                | "eg"
                | "ie"
                | "fig"
                | "vol"
                | "al"
                | "approx"
                | "cf"
                | "st"
                | "ave"
                | "blvd"
                | "ltd"
                | "inc"
                | "jr"
                | "sr"
                | "mme"
                | "mlle"
                | "nr"
                | "tel"
        )
    }

    let summary = summary.trim();
    if summary.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < summary.len() {
        let c = summary[i..].chars().next().unwrap_or_else(|| {
            panic!(
                "BUG: byte index {i} is not on a UTF-8 char boundary \
                 in summary of {} bytes — this indicates a logic error in \
                 the summarization loop",
                summary.len()
            )
        });
        let clen = c.len_utf8();
        let split_here = match c {
            '\n' | ';' | '!' | '?' => true,
            '.' => {
                let rest = &summary[i + clen..];
                let followed_by_break = if rest.is_empty() {
                    true
                } else {
                    rest.chars().next().is_some_and(|next| next.is_whitespace())
                };
                if !followed_by_break {
                    false
                } else {
                    let head = summary[start..i].trim_end();
                    !last_token_is_sentence_abbrev(head)
                        && !last_token_looks_like_compound_street_line(head)
                }
            }
            _ => false,
        };
        if split_here {
            let seg = summary[start..i].trim();
            if !seg.is_empty() {
                out.push(seg);
            }
            start = i + clen;
        }
        i += clen;
    }
    let tail = summary[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// When Socrates is in factual mode with required citations, reject completions that read as
/// factual assertions but declare no `evidence_citations` / `[[voxcite:…]]` markers.
#[must_use]
pub fn grounding_violation_factual_mode_without_declarations(
    attestation: Option<&CompletionAttestation>,
    socrates: &SocratesTaskContext,
) -> Option<String> {
    if !socrates.factual_mode || socrates.required_citations == 0 {
        return None;
    }
    let att = attestation?;
    let declared = declared_evidence_citations(Some(att));
    if !declared.is_empty() {
        return None;
    }
    let summary = att.completion_summary.as_deref()?.trim();
    if summary.is_empty() {
        return None;
    }
    let mut factual_segments = 0usize;
    for chunk in split_summary_into_claim_segments(summary) {
        if chunk.chars().count() < FACTUAL_SEGMENT_MIN_CHARS {
            continue;
        }
        if matches!(classify_line_claim_kind(chunk), ClaimKind::Factual) {
            factual_segments += 1;
        }
    }
    if factual_segments == 0 {
        return None;
    }
    Some(format!(
        "factual_mode requires explicit evidence: summary contains ~{factual_segments} factual-looking segment(s) but no evidence_citations or [[voxcite:…]] markers (required_citations={})",
        socrates.required_citations
    ))
}

/// Extract `[[voxcite:...]]` markers from free text (marker body is trimmed).
#[must_use]
pub fn parse_voxcite_markers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = text;
    let mut search_from = 0usize;
    while let Some(i) = lower[search_from..].find(VOXCITE_OPEN) {
        let abs = search_from + i + VOXCITE_OPEN.len();
        if let Some(end_rel) = lower[abs..].find(VOXCITE_CLOSE) {
            let body = lower[abs..abs + end_rel].trim();
            if !body.is_empty() {
                out.push(body.to_string());
            }
            search_from = abs + end_rel + VOXCITE_CLOSE.len();
        } else {
            break;
        }
    }
    out
}

/// Lowercased searchable projection of envelope text for substring citation checks.
#[must_use]
pub fn envelope_grounding_blob_lower(raw_json: &str) -> Option<String> {
    let env: ContextEnvelope = serde_json::from_str(raw_json).ok()?;
    let mut parts: Vec<String> = Vec::new();
    parts.push(env.content.summary_text.clone());
    for f in &env.content.facts {
        parts.push(f.fact_id.clone());
        parts.push(f.text.clone());
        for r in &f.evidence_refs {
            parts.push(r.clone());
        }
    }
    for p in &env.content.repo_paths {
        parts.push(p.clone());
    }
    for r in &env.content.artifact_refs {
        parts.push(r.clone());
    }
    for c in &env.content.citations {
        parts.push(c.clone());
    }
    if let Some(pl) = &env.content.structured_payload {
        parts.push(pl.to_string());
    }
    parts.push(format!("{:?}", env.envelope_type).to_ascii_lowercase());
    parts.push(env.envelope_id.clone());
    Some(parts.join("\n").to_ascii_lowercase())
}

/// Deduplicated declaration list from attestation fields and summary markers.
#[must_use]
pub fn declared_evidence_citations(attestation: Option<&CompletionAttestation>) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut ordered: Vec<String> = Vec::new();
    let push = |s: String, ordered: &mut Vec<String>, seen: &mut HashSet<String>| {
        let t = s.trim().to_string();
        if t.is_empty() {
            return;
        }
        if seen.insert(t.clone()) {
            ordered.push(t);
        }
    };
    if let Some(a) = attestation {
        for c in &a.evidence_citations {
            push(c.clone(), &mut ordered, &mut seen);
        }
        if let Some(ref sum) = a.completion_summary {
            for m in parse_voxcite_markers(sum) {
                push(m, &mut ordered, &mut seen);
            }
        }
    }
    ordered
}

/// Count declarations that appear in the envelope blob (case-insensitive substring).
#[must_use]
pub fn match_citations_in_blob(declared: &[String], blob_lower: &str) -> usize {
    declared
        .iter()
        .filter(|id| {
            let t = id.trim();
            !t.is_empty() && blob_lower.contains(&t.to_ascii_lowercase())
        })
        .count()
}

/// Merge declared grounded citations into task context for Socrates gate evaluation.
#[must_use]
pub fn merge_attestation_into_socrates_context(
    mut base: SocratesTaskContext,
    attestation: Option<&CompletionAttestation>,
    envelope_json: Option<&str>,
) -> SocratesTaskContext {
    let declared = declared_evidence_citations(attestation);
    if declared.is_empty() {
        return base;
    }
    let matched = envelope_json
        .and_then(envelope_grounding_blob_lower)
        .map(|blob| match_citations_in_blob(&declared, &blob))
        .unwrap_or(0);
    let n = matched.min(u8::MAX as usize) as u8;
    base.evidence_count = base.evidence_count.max(n);
    base
}

/// When the caller declared citations, verify they appear in the session envelope.
/// Returns `Some(reason)` when declarations cannot be verified.
#[must_use]
pub fn grounding_violation_declared_not_in_envelope(
    attestation: Option<&CompletionAttestation>,
    envelope_json: Option<&str>,
) -> Option<String> {
    let declared = declared_evidence_citations(attestation);
    if declared.is_empty() {
        return None;
    }
    let Some(raw) = envelope_json.map(str::trim).filter(|s| !s.is_empty()) else {
        return Some(format!(
            "declared {} evidence citation(s) but no session context envelope is stored for this task",
            declared.len()
        ));
    };
    let Some(blob) = envelope_grounding_blob_lower(raw) else {
        return Some(
            "declared evidence citations but the session context envelope could not be parsed as JSON"
                .to_string(),
        );
    };
    let matched = match_citations_in_blob(&declared, &blob);
    if matched < declared.len() {
        Some(format!(
            "only {matched} of {} declared evidence citation(s) appear in the session envelope (use [[voxcite:ID]] or evidence_citations[] matching retrieval text)",
            declared.len()
        ))
    } else {
        None
    }
}

/// Bounded MCP tool name hints from natural-language intent (AgentOS planner stub).
#[must_use]
pub fn agentos_suggested_tools_from_intent(intent: &str, max_steps: usize) -> Vec<String> {
    crate::agentos::intent_planner::plan_intent(intent, max_steps)
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assess_reply_confidence_flags_heavily_hedged_reply() {
        let reply = "Perhaps this is the cause. It might be a race condition. \
                      I think the lock is unclear here. This is probably the bug.";
        let result = assess_reply_confidence(reply);
        assert!(
            result.flagged,
            "mostly-hedged reply should be flagged: {result:?}"
        );
        assert!(result.confidence < GROUNDING_FLAG_THRESHOLD);
    }

    #[test]
    fn assess_reply_confidence_does_not_flag_confident_reply() {
        let reply = "The function returns the sum of the two arguments. \
                      It validates that both inputs are non-negative first.";
        let result = assess_reply_confidence(reply);
        assert!(
            !result.flagged,
            "confident factual reply should not be flagged: {result:?}"
        );
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn assess_reply_confidence_ignores_purely_procedural_reply() {
        let reply = "Run cargo test. Then open the PR. Rebase onto main first.";
        let result = assess_reply_confidence(reply);
        assert!(
            !result.flagged,
            "purely procedural reply has no confidence signal: {result:?}"
        );
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn assess_reply_confidence_empty_reply_is_neutral() {
        let result = assess_reply_confidence("");
        assert!(!result.flagged);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn has_unbalanced_fence_detects_odd_marker_count() {
        assert!(has_unbalanced_fence("```rust\nfn main() {}\n"));
        assert!(!has_unbalanced_fence("```rust\nfn main() {}\n```"));
        assert!(!has_unbalanced_fence("no fences here at all"));
    }

    #[test]
    fn looks_like_reask_flags_dissatisfaction_phrase_even_for_dissimilar_topic() {
        assert!(looks_like_reask(
            "What's the capital of France?",
            "That's wrong, please look it up again."
        ));
    }

    #[test]
    fn looks_like_reask_flags_high_token_overlap_without_a_phrase_match() {
        assert!(looks_like_reask(
            "How do I fix the null pointer exception in the parser?",
            "How do I actually fix the null pointer exception in the parser module?"
        ));
    }

    #[test]
    fn looks_like_reask_does_not_flag_an_unrelated_new_question() {
        assert!(!looks_like_reask(
            "How do I fix the null pointer exception in the parser?",
            "What's a good recipe for banana bread?"
        ));
    }

    #[test]
    fn assess_reply_confidence_mixed_reply_scores_between() {
        let reply = "The config file lives at src/config.rs. \
                      Perhaps it also needs a migration, unclear without checking.";
        let result = assess_reply_confidence(reply);
        assert!(
            result.confidence > 0.0 && result.confidence < 1.0,
            "{result:?}"
        );
    }

    #[test]
    fn agentos_intent_maps_tests_to_run_tool() {
        let v = agentos_suggested_tools_from_intent("please run cargo tests", 4);
        assert!(v.iter().any(|t| t == "vox_run_tests"));
    }

    #[test]
    fn voxcite_parses_ids() {
        let t = "done [[voxcite:chunk-1]] and [[voxcite:  mem:x  ]] tail";
        let v = parse_voxcite_markers(t);
        assert_eq!(v, vec!["chunk-1", "mem:x"]);
    }

    #[test]
    fn classify_short_line_uses_scalar_count_not_byte_len() {
        assert_eq!(classify_line_claim_kind("😀😀😀"), ClaimKind::Procedural);
    }

    #[test]
    fn classify_marks_speculative_and_factual() {
        assert_eq!(
            classify_line_claim_kind("Perhaps the server is down."),
            ClaimKind::Speculative
        );
        assert_eq!(
            classify_line_claim_kind("The outage was likely caused by DNS."),
            ClaimKind::Speculative
        );
        assert_eq!(
            classify_line_claim_kind("Run cargo test and report results."),
            ClaimKind::Procedural
        );
        assert_eq!(
            classify_line_claim_kind("Run cargo test; then open the PR."),
            ClaimKind::Procedural
        );
        assert_eq!(
            classify_line_claim_kind("The handler returns HTTP 403 for anonymous users."),
            ClaimKind::Factual
        );
    }

    #[test]
    fn split_keeps_dotted_versions_intact() {
        let s = "Target release v1.2.3; the API returns 404 for missing keys.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(
            p,
            vec![
                "Target release v1.2.3",
                "the API returns 404 for missing keys"
            ]
        );
    }

    #[test]
    fn split_respects_common_abbreviations_before_period() {
        let s = "Contact Mr. Smith; the API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Contact Mr. Smith", "the API returns 404"]);
    }

    #[test]
    fn split_respects_street_suffix_abbrev_before_period() {
        let s = "Office at Main St.; the API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Office at Main St.", "the API returns 404"]);
    }

    #[test]
    fn lowercase_st_period_still_splits_clause() {
        let s = "bad st. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["bad st", "The API returns 404"]);
    }

    #[test]
    fn lowercase_inc_period_still_splits_clause() {
        let s = "bad inc. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["bad inc", "The API returns 404"]);
    }

    #[test]
    fn split_respects_french_title_abbrev_before_period() {
        let s = "Contact Mme. Dupont; the API returns 200.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Contact Mme. Dupont", "the API returns 200"]);
    }

    #[test]
    fn lowercase_mme_period_still_splits_clause() {
        let s = "bad mme. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["bad mme", "The API returns 404"]);
    }

    #[test]
    fn split_respects_nr_and_tel_abbrev_before_period() {
        let s = "Box Nr. 7; Tel. +1-800; the API returns 200.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Box Nr. 7", "Tel. +1-800", "the API returns 200"]);
    }

    #[test]
    fn lowercase_tel_period_still_splits_clause() {
        let s = "bad tel. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["bad tel", "The API returns 404"]);
    }

    #[test]
    fn split_keeps_german_compound_street_before_period() {
        let s = "Office at Hauptstr. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Office at Hauptstr. The API returns 404"]);
    }

    #[test]
    fn short_str_street_oststr_kept_when_not_false_positive_suffix() {
        let s = "Near Oststr. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Near Oststr. The API returns 404"]);
    }

    #[test]
    fn istr_and_estr_truncations_still_split_sentence() {
        assert_eq!(
            split_summary_into_claim_segments("See Ministr. Then call."),
            vec!["See Ministr", "Then call"]
        );
        assert_eq!(
            split_summary_into_claim_segments("Use Illustr. in the deck."),
            vec!["Use Illustr", "in the deck"]
        );
        assert_eq!(
            split_summary_into_claim_segments("The Orchestr. recorded live."),
            vec!["The Orchestr", "recorded live"]
        );
    }

    #[test]
    fn split_keeps_ascii_strasse_compound_before_period() {
        let s = "Office at Hauptstrasse. The API returns 404.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Office at Hauptstrasse. The API returns 404"]);
    }

    #[test]
    fn split_keeps_unicode_strasse_suffix_before_period() {
        let s = "Büro Müllerstraße. Der Port ist 443.";
        let p = split_summary_into_claim_segments(s);
        assert_eq!(p, vec!["Büro Müllerstraße. Der Port ist 443"]);
    }

    #[test]
    fn split_summary_handles_multibyte_unicode() {
        // Exercises the production splitter end-to-end with multibyte (CJK)
        // characters, catching any char-boundary panic in the real code path.
        let s = "Hello 中文 world. Next clause.";
        let parts = split_summary_into_claim_segments(s);
        assert_eq!(parts, vec!["Hello 中文 world", "Next clause"]);
    }

    #[test]
    fn procedural_detection_uses_whole_words() {
        assert_eq!(
            classify_line_claim_kind("The emerge path triggers on deploy."),
            ClaimKind::Factual
        );
        assert_eq!(
            classify_line_claim_kind("The spread of the outage was regional."),
            ClaimKind::Factual
        );
    }

    #[test]
    fn factual_mode_flags_missing_citations() {
        let soc = SocratesTaskContext {
            risk_budget: String::new(),
            factual_mode: true,
            required_citations: 1,
            ..Default::default()
        };
        let att = CompletionAttestation {
            completion_summary: Some(
                "The handler returns HTTP 403 for anonymous users when auth is enabled.".into(),
            ),
            ..Default::default()
        };
        let v = grounding_violation_factual_mode_without_declarations(Some(&att), &soc);
        assert!(v.is_some(), "{v:?}");
    }

    #[test]
    fn factual_mode_flags_semicolon_separated_facts() {
        let soc = SocratesTaskContext {
            risk_budget: String::new(),
            factual_mode: true,
            required_citations: 1,
            ..Default::default()
        };
        let att = CompletionAttestation {
            completion_summary: Some(
                "The handler returns HTTP 403 for anonymous users; the gateway removes cookies on that path."
                    .into(),
            ),
            ..Default::default()
        };
        let v = grounding_violation_factual_mode_without_declarations(Some(&att), &soc);
        assert!(v.is_some(), "{v:?}");
    }

    #[test]
    fn procedural_semicolon_summary_does_not_trigger_factual_gate() {
        let soc = SocratesTaskContext {
            risk_budget: String::new(),
            factual_mode: true,
            required_citations: 1,
            ..Default::default()
        };
        let att = CompletionAttestation {
            completion_summary: Some("Run cargo test; then submit the patch for review.".into()),
            ..Default::default()
        };
        assert!(grounding_violation_factual_mode_without_declarations(Some(&att), &soc).is_none());
    }

    #[test]
    fn merge_boosts_evidence_when_substring_hits() {
        let env = ContextEnvelope::from_session_retrieval(
            "r1",
            "s1",
            &crate::socrates::SessionRetrievalEnvelope {
                retrieval_tier: "hybrid".into(),
                memory_hit_count: 0,
                knowledge_hit_count: 0,
                chunk_hit_count: 1,
                repo_hit_count: 0,
                rrf_fused_hit_count: 0,
                used_vector: false,
                used_bm25: true,
                used_lexical_fallback: false,
                contradiction_count: 0,
                source_diversity: 1,
                evidence_quality: 0.5,
                citation_coverage: 0.5,
                verification_performed: false,
                verification_reason: None,
                recommended_next_action: None,
            },
        );
        let raw = serde_json::to_string(&env).unwrap();
        let att = CompletionAttestation {
            completion_summary: Some("see [[voxcite:chunk]] ref".into()),
            evidence_citations: vec![],
            ..Default::default()
        };
        let mut ctx = SocratesTaskContext {
            factual_mode: true,
            required_citations: 2,
            evidence_count: 0,
            ..Default::default()
        };
        ctx = merge_attestation_into_socrates_context(ctx, Some(&att), Some(&raw));
        assert!(ctx.evidence_count >= 1);
    }
}

#[cfg(test)]
mod semcov_wave1_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn declared_evidence_citations_none_is_empty() {
        assert!(declared_evidence_citations(None).is_empty());
    }

    #[test]
    fn declared_evidence_citations_trims_dedups_and_merges_summary_markers() {
        let att = CompletionAttestation {
            // "a" appears twice (once padded) -> single trimmed entry; "" / "   " dropped
            evidence_citations: vec![
                "a".to_string(),
                " a ".to_string(),
                String::new(),
                "   ".to_string(),
                "b".to_string(),
            ],
            // summary adds "c" (new) and "a" (dup of explicit -> skipped)
            completion_summary: Some("see [[voxcite:c]] and [[voxcite:a]] done".to_string()),
            ..Default::default()
        };
        // Explicit citations come first (order-preserving), then new markers.
        assert_eq!(
            declared_evidence_citations(Some(&att)),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn whole_word_rejects_substring_and_empty_needle() {
        // empty needle is always false
        assert!(!lower_contains_ascii_whole_word("anything here", ""));
        // "merge" must NOT match inside "emerge"
        assert!(!lower_contains_ascii_whole_word(
            "they emerge slowly",
            "merge"
        ));
        // "read" must NOT match inside "spread"
        assert!(!lower_contains_ascii_whole_word("we spread thin", "read"));
    }

    #[test]
    fn whole_word_matches_on_non_alnum_boundaries() {
        // standalone word delimited by spaces
        assert!(lower_contains_ascii_whole_word("we merge it", "merge"));
        // underscore counts as a word char -> "merge" inside "merge_now" is rejected
        assert!(!lower_contains_ascii_whole_word(
            "call merge_now please",
            "merge"
        ));
        // match at very start and end of haystack
        assert!(lower_contains_ascii_whole_word("merge", "merge"));
    }
}
