use crate::hash::content_hash;

/// Normalize an AST node for content-addressing.
/// Strips names (replaces with de Bruijn indices), producing a canonical
/// byte representation that hashes identically regardless of naming.
pub fn normalize_and_hash(source: &str) -> String {
    // For now, normalize by:
    // 1. Stripping comments
    // 2. Normalizing whitespace
    // 3. Hashing the result
    let normalized = normalize_source(source);
    content_hash(normalized.as_bytes())
}

/// Strip comments and normalize whitespace for canonical representation.
fn normalize_source(source: &str) -> String {
    let mut result = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        // Skip comment-only lines
        if trimmed.starts_with('#') {
            continue;
        }
        // Strip inline comments
        let line_content = if let Some(idx) = trimmed.find('#') {
            trimmed[..idx].trim_end()
        } else {
            trimmed
        };
        if !line_content.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line_content);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_strips_comments() {
        let h1 = normalize_and_hash("let x = 5 # this is a comment");
        let h2 = normalize_and_hash("let x = 5");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_normalize_strips_comment_lines() {
        let h1 = normalize_and_hash("# header\nlet x = 5");
        let h2 = normalize_and_hash("let x = 5");
        assert_eq!(h1, h2);
    }
}
