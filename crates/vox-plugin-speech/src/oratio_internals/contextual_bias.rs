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

#[cfg(test)]
mod semcov_wave9_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: parse_hotword_csv including empty strings when the input is purely
    // whitespace or delimiter-only, poisoning downstream bias tables with blank tokens.
    #[test]
    fn parse_hotword_csv_empty_string_returns_empty_vec() {
        let result = parse_hotword_csv("");
        assert!(
            result.is_empty(),
            "empty string must yield zero hotwords, got: {result:?}"
        );
    }

    // Catches: parse_hotword_csv failing to trim leading/trailing whitespace from
    // individual tokens, causing exact-match lookups to miss " vox " != "vox".
    #[test]
    fn parse_hotword_csv_trims_whitespace_around_tokens() {
        let result = parse_hotword_csv("  hello , world  ,  foo  ");
        assert_eq!(
            result,
            vec!["hello", "world", "foo"],
            "tokens must be trimmed"
        );
    }

    // Catches: parse_hotword_csv treating semicolons and newlines as separators
    // inconsistently — some paths might only split on commas, leaving ";bar" as a token.
    #[test]
    fn parse_hotword_csv_semicolon_and_newline_delimiters() {
        let result = parse_hotword_csv("alpha;beta\ngamma,delta");
        assert_eq!(
            result.len(),
            4,
            "all 4 tokens across mixed delimiters, got: {result:?}"
        );
        assert!(result.contains(&"alpha".to_string()));
        assert!(result.contains(&"beta".to_string()));
        assert!(result.contains(&"gamma".to_string()));
        assert!(result.contains(&"delta".to_string()));
    }

    // Catches: parse_hotword_csv producing duplicate empty entries when input has
    // consecutive delimiters like ",," or ";\n;".
    #[test]
    fn parse_hotword_csv_consecutive_delimiters_produce_no_empty_entries() {
        let result = parse_hotword_csv("a,,b;;c\n\nd");
        assert!(
            result.iter().all(|s| !s.is_empty()),
            "no empty hotword entries allowed, got: {result:?}"
        );
        assert_eq!(
            result.len(),
            4,
            "exactly 4 non-empty tokens, got: {result:?}"
        );
    }
}
