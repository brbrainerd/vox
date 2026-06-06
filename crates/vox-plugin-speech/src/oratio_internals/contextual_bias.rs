//! Soft contextual biasing without retraining ASR: rerank transcript candidates using
//! project/session phrase lists (see [arXiv:2410.18363](https://arxiv.org/abs/2410.18363)-style vocabulary steering at the **hypothesis** level).
//!
//! Candle Whisper decoding does not yet expose logit biasing here; we approximate by scoring
//! n-best strings for occurrences of important phrases (identifiers, aliases).

/// Parse comma-separated session hotwords from env-style strings.
#[must_use]
pub fn parse_hotword_csv(raw: &str) -> Vec<String> {
    raw.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
