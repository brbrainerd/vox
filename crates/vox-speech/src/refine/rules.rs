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
        ("box dine", "Box<dyn "),
        ("to string", "to_string"),
        ("pub fun", "pub fn"),
        ("pub function", "pub fn"),
        ("let mute", "let mut "),
        ("a sync", "async"),
        ("vec bang", "vec!"),
        ("debug bang", "dbg!"),
        ("print len", "println!"),
        ("print el in", "println!"),
        ("if let some", "if let Some"),
        ("impl for", "impl for "),
        ("mut self", "mut self"),
    ])
}

fn default_domain_lexicon() -> HashSet<String> {
    [
        "vox",
        "mens",
        "oratio",
        "schola",
        "candle",
        "whisper",
        "transcribe",
        "orchestrator",
        "tool-call",
        "workflow",
        "status",
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
}
