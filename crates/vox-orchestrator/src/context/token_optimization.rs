#[cfg(feature = "token-counting")]
use tiktoken_rs::cl100k_base;

/// Counts tokens using cl100k_base (compatible with GPT-4/o1).
#[cfg(feature = "token-counting")]
pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("tiktoken cl100k_base data is bundled and must be valid");
    bpe.encode_with_special_tokens(text).len()
}

#[cfg(not(feature = "token-counting"))]
pub fn count_tokens(text: &str) -> usize {
    // Cheap word-count approximation when tiktoken is disabled.
    // Accuracy is sufficient for routing decisions but not billing.
    text.split_whitespace().count() * 4 / 3
}

/// Trims a context string to a maximum number of tokens.
#[cfg(feature = "token-counting")]
pub fn trim_context(text: &str, max_tokens: usize) -> String {
    let bpe = cl100k_base().expect("tiktoken cl100k_base data is bundled and must be valid");
    let tokens = bpe.encode_with_special_tokens(text);
    if tokens.len() <= max_tokens {
        return text.to_string();
    }

    // Trim from the middle to keep head and tail
    let head_len = max_tokens / 2;
    let tail_len = max_tokens - head_len;

    let head_tokens = &tokens[..head_len];
    let tail_tokens = &tokens[tokens.len() - tail_len..];

    let head = bpe
        .decode(head_tokens)
        .unwrap_or_else(|_| "[DECODE ERROR]".to_string());
    let tail = bpe
        .decode(tail_tokens)
        .unwrap_or_else(|_| "[DECODE ERROR]".to_string());

    format!("{}\n... [TRUNCATED] ...\n{}", head, tail)
}

#[cfg(not(feature = "token-counting"))]
pub fn trim_context(text: &str, max_tokens: usize) -> String {
    // Cheap word-based truncation when tiktoken is disabled.
    let words: Vec<&str> = text.split_whitespace().collect();
    // Assuming roughly 4/3 tokens per word, so target words = max_tokens * 3 / 4
    let target_words = max_tokens * 3 / 4;
    if words.len() <= target_words {
        return text.to_string();
    }
    let head_len = target_words / 2;
    let tail_len = target_words - head_len;
    let head = words[..head_len].join(" ");
    let tail = words[words.len() - tail_len..].join(" ");
    format!("{}\n... [TRUNCATED] ...\n{}", head, tail)
}

/// Optimizes a list of context snippets by prioritizing higher priority ones.
pub fn prioritize_context(
    snippets: Vec<(String, crate::context_envelope::ContextPriority)>,
    max_total_tokens: usize,
) -> String {
    let mut sorted = snippets.clone();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.1)); // Higher priority first

    let mut total_tokens = 0;
    let mut result = String::new();

    for (text, _) in sorted {
        let tokens = count_tokens(&text);
        if total_tokens + tokens > max_total_tokens {
            if total_tokens < max_total_tokens - 10 {
                let remaining = max_total_tokens - total_tokens;
                result.push_str(&trim_context(&text, remaining));
                result.push('\n');
            }
            break;
        }
        result.push_str(&text);
        result.push('\n');
        total_tokens += tokens + 1; // +1 for the newline
    }

    result
}

#[cfg(all(test, feature = "token-counting"))]
mod tests {
    use super::count_tokens;

    /// T4.2 follow-up (Gap 2): when `token-counting` is compiled in, `count_tokens`
    /// must use real cl100k_base BPE tokenization, not the
    /// `split_whitespace().count() * 4 / 3` heuristic fallback. The two diverge
    /// on inputs with no whitespace (a single long word tokenizes into several
    /// BPE tokens; the whitespace heuristic reports exactly 1 word -> 1 token).
    /// This is a build-time proof that the daemon binary (and any other
    /// consumer that enables this feature) actually exercises the tiktoken-rs
    /// path described in `crates/vox-orchestrator/Cargo.toml`, not the
    /// fallback.
    #[test]
    fn count_tokens_uses_real_bpe_not_whitespace_heuristic() {
        let no_whitespace = "supercalifragilisticexpialidocious".repeat(10);
        let whitespace_heuristic = no_whitespace.split_whitespace().count() * 4 / 3;
        let real = count_tokens(&no_whitespace);
        assert_ne!(
            real, whitespace_heuristic,
            "count_tokens must diverge from the whitespace-count fallback when \
             token-counting is enabled — got the same value ({real}), which means \
             the heuristic fallback is running instead of tiktoken-rs"
        );
        // A single "word" with no whitespace: the heuristic degenerates to 1
        // (word count 1 * 4 / 3 == 1), while real BPE tokenization of ~350
        // characters of gibberish must produce far more tokens than that.
        assert!(
            real > 10,
            "real tiktoken tokenization of a long no-whitespace string should \
             produce far more than 10 tokens; got {real}"
        );
    }
}
