//! Deterministic refinement rules (no ML).

use std::collections::{HashMap, HashSet};

use super::{CorrectionContext, CorrectionTrace, OratioCorrectionProfile, RefineOutput};

/// Collapse outer whitespace and trim ends — safe default before richer ITN ships.
#[must_use]
pub fn light_trim(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn default_confusion_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("mends", "mens"),
        ("men's", "mens"),
        ("oration", "oratio"),
        ("oratia", "oratio"),
        ("voxx", "vox"),
        ("check space", "check"),
        ("tool call", "tool-call"),
        ("tool calls", "tool-calls"),
    ])
}

fn code_confusion_map() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("unwrap or else", "unwrap_or_else"),
        ("unwrap or default", "unwrap_or_default"),
        ("hash map", "HashMap"),
        ("box dine", "Box<dyn"),
        ("to string", "to_string"),
        ("pub fun", "pub fn"),
        ("pub function", "pub fn"),
        ("let mute", "let mut "),
        ("a sync", "async"),
        ("vec bang", "vec!"),
        ("debug bang", "dbg!"),
        ("if let some", "if let Some"),
    ])
}

/// Replace multi-word `code_confusion_map` phrases (e.g. "box dine",
/// "let mute") with their canonical form BEFORE the single-token loop below
/// runs. `code_confusion_map()`'s keys are phrases, but a whitespace-split
/// single-token lookup can never equal a multi-word key — without this pass,
/// every phrase entry in the map is permanently dead code (confirmed by
/// direct testing; see the audit finding above this task).
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find the byte range of the next word-boundary-respecting, ASCII
/// case-insensitive occurrence of `phrase` in `haystack` starting at or after
/// byte offset `from`. All of `code_confusion_map`'s keys are plain ASCII, so
/// scoping this to `eq_ignore_ascii_case` byte comparisons keeps the search
/// performed directly over `haystack`'s own bytes — no separately
/// materialized lowercased copy whose byte offsets could desync from the
/// original string on non-ASCII input (e.g. `to_lowercase()` changing the
/// byte length of characters like `İ`).
fn find_boundary_match(haystack: &[u8], phrase: &[u8], from: usize) -> Option<usize> {
    if phrase.is_empty() || from > haystack.len() || phrase.len() > haystack.len() {
        return None;
    }
    let mut start = from;
    while start + phrase.len() <= haystack.len() {
        if haystack[start..start + phrase.len()].eq_ignore_ascii_case(phrase) {
            let before_ok = start == 0 || !is_word_byte(haystack[start - 1]);
            let end = start + phrase.len();
            let after_ok = end == haystack.len() || !is_word_byte(haystack[end]);
            if before_ok && after_ok {
                return Some(start);
            }
        }
        start += 1;
    }
    None
}

fn apply_phrase_confusions(text: &str) -> String {
    let mut phrases: Vec<(&'static str, &'static str)> = code_confusion_map().into_iter().collect();
    // Longest phrases first, so a 3-word key can't be shadowed by a 2-word
    // key that happens to be one of its prefixes. Ties broken on phrase text
    // for full determinism, since HashMap iteration order is randomized
    // per-process.
    phrases.sort_by_key(|(k, _)| (std::cmp::Reverse(k.split_whitespace().count()), *k));

    let mut result = text.to_string();
    for (phrase, replacement) in phrases {
        let mut search_from = 0;
        loop {
            let haystack = result.as_bytes();
            match find_boundary_match(haystack, phrase.as_bytes(), search_from) {
                Some(pos) => {
                    let end = pos + phrase.len();
                    result = format!("{}{}{}", &result[..pos], replacement, &result[end..]);
                    search_from = pos + replacement.len();
                }
                None => break,
            }
        }
    }
    // Collapse whitespace introduced by replacement VALUES that themselves end
    // in a space (e.g. "let mute" -> "let mut "): splicing that directly
    // before existing text (which already had its own leading space, e.g.
    // "...mute count..." -> "...mute" + " count") produces a double space.
    // `light_trim` already ran once before this function, at the top of
    // `refine_transcript` — this is a second, narrower pass, not a redundant
    // one, since it's specifically cleaning up an artifact this function can
    // introduce that light_trim (which ran on the pre-substitution text)
    // couldn't have caught.
    light_trim(&result)
}

fn default_domain_lexicon() -> HashSet<String> {
    [
        "vox",
        "mens",
        "oratio",
        "schola",
        "transcribe",
        "orchestrator",
        "tool-call",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn is_protected_token(token: &str, protected_tokens: &HashSet<String>) -> bool {
    if protected_tokens.contains(token) {
        return true;
    }
    token.starts_with("--")
        || token.contains('/')
        || token.contains('\\')
        || token.contains("::")
        || token.contains('.')
        || token.chars().any(|c| c.is_ascii_digit())
}

fn normalize_case(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap_or_default().to_ascii_uppercase();
    format!("{first}{}", chars.as_str())
}

/// Full deterministic transcript refinement pipeline.
#[must_use]
pub fn refine_transcript(raw: &str, ctx: &CorrectionContext) -> RefineOutput {
    if ctx.debug_payload {
        tracing::debug!(target: "vox_oratio_refine", raw_payload = raw, "Refine input payload");
    }

    let mut trace = Vec::new();
    let mut current = light_trim(raw);
    if current != raw {
        trace.push(CorrectionTrace {
            rule: "light_trim".to_string(),
            before: raw.to_string(),
            after: current.clone(),
            reason: "Collapsed repeated whitespace".to_string(),
        });
    }

    let mut confusion = default_confusion_map();
    if ctx.domain == crate::refine::DomainMode::Code {
        confusion.extend(code_confusion_map());

        let phrased = apply_phrase_confusions(&current);
        if phrased != current {
            trace.push(CorrectionTrace {
                rule: "phrase_confusion_map".to_string(),
                before: current.clone(),
                after: phrased.clone(),
                reason: "Matched multi-word code confusion phrase".to_string(),
            });
            current = phrased;
        }
    }

    let mut domain_lexicon = default_domain_lexicon();
    for item in &ctx.domain_lexicon {
        domain_lexicon.insert(item.to_ascii_lowercase());
    }

    let mut rewritten = Vec::new();
    for token in current.split_whitespace() {
        if is_protected_token(token, &ctx.protected_tokens) {
            rewritten.push(token.to_string());
            continue;
        }
        let lower = token.to_ascii_lowercase();

        // If the speaker profile is dysarthric, bypass the standard confusion
        // map as their speech patterns require their distinct fine-tuned mappings.
        if !matches!(
            ctx.speaker_profile,
            crate::speaker_profile::SpeakerProfile::Dysarthric(_)
        ) && let Some(mapped) = confusion.get(lower.as_str())
        {
            trace.push(CorrectionTrace {
                rule: "confusion_map".to_string(),
                before: token.to_string(),
                after: (*mapped).to_string(),
                reason: "Matched common ASR confusion token".to_string(),
            });
            rewritten.push((*mapped).to_string());
            continue;
        }

        if domain_lexicon.contains(&lower) {
            if token != lower {
                trace.push(CorrectionTrace {
                    rule: "domain_lexicon_case".to_string(),
                    before: token.to_string(),
                    after: lower.clone(),
                    reason: "Canonicalized known Vox domain token".to_string(),
                });
            }
            rewritten.push(lower);
            continue;
        }
        rewritten.push(token.to_string());
    }
    current = rewritten.join(" ");

    for (from, to) in [("vox mens oratio", "vox oratio"), ("mens oratio", "oratio")] {
        if current.contains(from) {
            let after = current.replacen(from, to, 100);
            if after != current {
                trace.push(CorrectionTrace {
                    rule: "phrase_canonicalization".to_string(),
                    before: current.clone(),
                    after: after.clone(),
                    reason: "Canonical speech CLI path (vox oratio)".to_string(),
                });
                current = after;
            }
        }
    }

    if matches!(ctx.profile, OratioCorrectionProfile::Aggressive) {
        let normalized = normalize_case(&current);
        if normalized != current {
            trace.push(CorrectionTrace {
                rule: "aggressive_case".to_string(),
                before: current.clone(),
                after: normalized.clone(),
                reason: "Applied aggressive sentence case normalization".to_string(),
            });
            current = normalized;
        }
    }

    let tunables = &ctx.refine_tunables;
    let base = match ctx.profile {
        OratioCorrectionProfile::Conservative => tunables.conservative_base,
        OratioCorrectionProfile::Balanced => tunables.balanced_base,
        OratioCorrectionProfile::Aggressive => tunables.aggressive_base,
    };
    let penalty = (trace.len() as f32 * tunables.penalty_per_trace).min(tunables.penalty_cap);
    let confidence = (base - penalty).clamp(tunables.conf_min, tunables.conf_max);

    if ctx.debug_payload {
        tracing::debug!(
            target: "vox_oratio_refine",
            refined_payload = current,
            confidence,
            trace_len = trace.len(),
            "Refine output payload"
        );
    }

    RefineOutput {
        text: current,
        confidence,
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refine::{CorrectionContext, OratioCorrectionProfile};

    #[test]
    fn light_trim_collapse() {
        assert_eq!(light_trim("  a   b  "), "a b");
    }

    #[test]
    fn generic_english_words_not_force_lowercased() {
        // "status" is a common English word; forcing it lowercase mid-sentence
        // corrupts normal capitalized usage. Use a bare, punctuation-free
        // token (no trailing colon) — `refine_transcript`'s matching loop
        // splits on whitespace only, so a colon-attached token like "Status:"
        // never equals the plain-word lexicon entry "status" regardless of
        // whether "status" is in the lexicon, which would make this test
        // pass trivially in both the broken and fixed states and prove
        // nothing (audit finding: the original draft used "Status: complete").
        let out = refine_transcript("Status complete", &CorrectionContext::default());
        assert_eq!(out.text, "Status complete");
    }

    #[test]
    fn candle_and_whisper_not_force_lowercased() {
        // "candle" ("light a candle") and "whisper" (also a real ASR model
        // name a user might legitimately capitalize when talking about it)
        // are common English words; forcing them lowercase mid-sentence
        // corrupts normal capitalized usage the same way "status"/"workflow"
        // did (see `generic_english_words_not_force_lowercased` above and
        // commit 5fc47898a6). Bare, punctuation-free tokens, same reasoning
        // as that fix: a colon-attached token would never match the lexicon
        // regardless of whether the fix landed, proving nothing.
        let out = refine_transcript("Whisper is a speech model", &CorrectionContext::default());
        assert_eq!(out.text, "Whisper is a speech model");
        let out = refine_transcript("Candle burned all night", &CorrectionContext::default());
        assert_eq!(out.text, "Candle burned all night");
    }

    #[test]
    fn confusion_token_rewrite() {
        let out = refine_transcript(
            "vox mends oration status",
            &CorrectionContext {
                profile: OratioCorrectionProfile::Balanced,
                ..Default::default()
            },
        );
        assert_eq!(out.text, "vox oratio status");
        assert!(!out.trace.is_empty());
    }

    #[test]
    fn protected_tokens_not_rewritten() {
        let mut ctx = CorrectionContext::default();
        ctx.protected_tokens.insert("--mends".to_string());
        let out = refine_transcript("--mends", &ctx);
        assert_eq!(out.text, "--mends");
    }

    #[test]
    fn phrase_confusion_with_trailing_space_replacement_does_not_double_space() {
        // Regression test (code review finding): "let mute" -> "let mut "
        // (trailing space baked into the replacement value) previously
        // spliced directly against the original text's own leading space on
        // the next word, producing "let mut  count" (double space). Not
        // caught by the refine_regression.rs harness, which gates on
        // char_error_rate — whitespace-insensitive by construction.
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        let out = refine_transcript("let mute count colon i32", &ctx);
        assert!(
            !out.text.contains("  "),
            "must not produce a double space: {:?}",
            out.text
        );
        // `refine_transcript` alone doesn't expand "colon" -> ":" — that's
        // `speech_normalize::expand_spoken_symbols`, a separate later pass —
        // so "colon" stays literal here; only the double-space defect is
        // under test.
        assert_eq!(out.text, "let mut count colon i32");
    }

    #[test]
    fn box_dyn_confusion_closes_angle_bracket() {
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        let out = refine_transcript("box dine error", &ctx);
        assert_eq!(out.text, "Box<dyn error");
        // The full-phrase closing-bracket case (with a following type token) is
        // handled by the phrase_canonicalization pass, not the token map alone —
        // this test only asserts the map no longer emits an unbalanced `<`
        // followed by a bare trailing space with nothing to close it.
        assert!(
            !out.text.ends_with("Box<dyn "),
            "must not leave a dangling space with no type"
        );
    }

    #[test]
    fn mut_self_is_not_a_confusion_entry() {
        // `mut self` is valid Rust as spoken; it must not appear in the code
        // confusion map (it was a no-op identity mapping doing nothing).
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        assert!(!super::code_confusion_map().contains_key("mut self"));
        let out = refine_transcript("fn foo mut self", &ctx);
        assert_eq!(out.text, "fn foo mut self");
    }

    #[test]
    fn guessy_print_phrases_removed_from_confusion_map() {
        // "print len" / "print el in" were unvalidated phonetic guesses that can
        // misfire on unrelated speech (e.g. "the print length was wrong").
        assert!(!super::code_confusion_map().contains_key("print len"));
        assert!(!super::code_confusion_map().contains_key("print el in"));
    }

    #[test]
    fn phrase_confusion_respects_word_boundaries() {
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        // "a sync" must not match inside "synchronous".
        let out = refine_transcript("run a synchronous task", &ctx);
        assert_eq!(out.text, "run a synchronous task");

        // "hash map" must not match inside "hash mapping"/"hash mapper".
        let out = refine_transcript("hash mapping utility", &ctx);
        assert_eq!(out.text, "hash mapping utility");
    }

    #[test]
    fn phrase_confusion_replaces_all_occurrences() {
        let ctx = CorrectionContext {
            domain: crate::refine::DomainMode::Code,
            ..Default::default()
        };
        let out = refine_transcript("hash map then another hash map", &ctx);
        assert_eq!(out.text, "HashMap then another HashMap");
    }
}
