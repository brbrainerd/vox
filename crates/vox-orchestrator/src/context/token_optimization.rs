use tiktoken_rs::cl100k_base;

/// Counts tokens using cl100k_base (compatible with GPT-4/o1).
pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().expect("tiktoken cl100k_base data is bundled and must be valid");
    bpe.encode_with_special_tokens(text).len()
}

/// Trims a context string to a maximum number of tokens.
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
        .decode(head_tokens.to_vec())
        .unwrap_or_else(|_| "[DECODE ERROR]".to_string());
    let tail = bpe
        .decode(tail_tokens.to_vec())
        .unwrap_or_else(|_| "[DECODE ERROR]".to_string());

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
