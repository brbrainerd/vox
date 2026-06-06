//! Token- and character-level error rates for ASR evaluation (WER / CER).

/// Normalize `s` into lowercase word tokens (whitespace split).
#[must_use]
pub fn tokenize_words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// **Word error rate** over whitespace-delimited tokens (case-insensitive).
///
/// Uses Levenshtein distance at the token level. Returns `0.0` when both strings are
/// empty after tokenization; if the reference is empty but the hypothesis is not,
/// returns `1.0`.
#[must_use]
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let r = tokenize_words(reference);
    let h = tokenize_words(hypothesis);
    if r.is_empty() && h.is_empty() {
        return 0.0;
    }
    if r.is_empty() {
        return 1.0;
    }
    let d = levenshtein(r.as_slice(), h.as_slice());
    d as f64 / r.len() as f64
}

/// **Character error rate** over non-whitespace Unicode scalar values.
///
/// If both sides are empty after stripping whitespace, returns `0.0`. If reference is
/// empty and hypothesis is not, returns `1.0`.
#[must_use]
pub fn char_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let r: Vec<char> = reference.chars().filter(|c| !c.is_whitespace()).collect();
    let h: Vec<char> = hypothesis.chars().filter(|c| !c.is_whitespace()).collect();
    if r.is_empty() && h.is_empty() {
        return 0.0;
    }
    if r.is_empty() {
        return 1.0;
    }
    let d = levenshtein(r.as_slice(), h.as_slice());
    d as f64 / r.len() as f64
}

/// **Symbol error rate** for code-domain evaluation.
/// Only considers tokens that look like identifiers `[a-zA-Z_][a-zA-Z0-9_]*`.
#[must_use]
pub fn symbol_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let is_ident = |s: &str| -> bool {
        let mut chars = s.chars();
        if let Some(c) = chars.next()
            && (c.is_ascii_alphabetic() || c == '_')
        {
            return chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        }
        false
    };

    let r: Vec<String> = tokenize_words(reference)
        .into_iter()
        .filter(|s| is_ident(s))
        .collect();
    let h: Vec<String> = tokenize_words(hypothesis)
        .into_iter()
        .filter(|s| is_ident(s))
        .collect();

    if r.is_empty() && h.is_empty() {
        return 0.0;
    }
    if r.is_empty() {
        return 1.0;
    }
    let d = levenshtein(r.as_slice(), h.as_slice());
    d as f64 / r.len() as f64
}

fn levenshtein<T: Eq>(a: &[T], b: &[T]) -> usize {
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wer_perfect() {
        assert!((word_error_rate("hello world", "hello world") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wer_one_substitution() {
        let w = word_error_rate("hello world", "hello there");
        assert!((w - 0.5).abs() < 1e-9);
    }

    #[test]
    fn wer_empty_both() {
        assert!((word_error_rate("   ", "") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cer_ignores_space() {
        let c = char_error_rate("a b", "ab");
        assert!((c - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cer_substitution() {
        let c = char_error_rate("ab", "ac");
        assert!((c - 0.5).abs() < 1e-9);
    }
}
