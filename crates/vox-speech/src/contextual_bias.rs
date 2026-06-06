//! Soft contextual biasing without retraining ASR: rerank transcript candidates using
//! project/session phrase lists (see [arXiv:2410.18363](https://arxiv.org/abs/2410.18363)-style vocabulary steering at the **hypothesis** level).
//!
//! Candle Whisper decoding does not yet expose logit biasing here; we approximate by scoring
//! n-best strings for occurrences of important phrases (identifiers, aliases).

/// Count overlapping case-insensitive substring hits for `phrases` in `text`.
#[must_use]
pub fn bias_hit_score(text: &str, phrases: &[String]) -> u32 {
    if phrases.is_empty() || text.is_empty() {
        return 0;
    }
    let lower = text.to_ascii_lowercase();
    let mut score = 0u32;
    for p in phrases {
        let needle = p.trim();
        if needle.is_empty() {
            continue;
        }
        let n = needle.to_ascii_lowercase();
        let mut start = 0usize;
        while let Some(pos) = lower[start..].find(&n) {
            score = score.saturating_add(1);
            start = start.saturating_add(pos).saturating_add(n.len().max(1));
            if start >= lower.len() {
                break;
            }
        }
    }
    score
}

/// Merge lexicon-derived bias strings with `extra` (e.g. session hotwords), dedupe, longest-first, cap `max_phrases`.
#[must_use]
pub fn merge_bias_phrases(
    mut lexicon_phrases: Vec<String>,
    extra: &[String],
    max_phrases: usize,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for s in lexicon_phrases
        .drain(..)
        .chain(extra.iter().cloned())
        .map(|s| s.trim().to_string())
    {
        if s.is_empty() || !seen.insert(s.clone()) {
            continue;
        }
        out.push(s);
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.len()));
    out.truncate(max_phrases.max(1));
    out
}

/// Parse comma-separated session hotwords from env-style strings.
#[must_use]
pub fn parse_hotword_csv(raw: &str) -> Vec<String> {
    raw.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_score_counts_substrings() {
        let phrases = vec!["getUser".to_string(), "MENS".to_string()];
        assert_eq!(bias_hit_score("call getUser for mens", &phrases), 2);
    }

    #[test]
    fn merge_dedupes() {
        let v = merge_bias_phrases(
            vec!["a".into(), "b".into()],
            &["a".into(), "longphrase".into()],
            10,
        );
        assert!(v.contains(&"longphrase".to_string()));
        assert!(v.iter().filter(|x| *x == "a").count() == 1);
    }
}
