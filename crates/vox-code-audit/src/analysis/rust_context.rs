//! Per-file Rust analysis: token map + optional `syn` AST.

use super::token_map::TokenMap;

/// Shared context for Rust detectors: lexical non-code spans + parsed AST when possible.
#[derive(Debug)]
pub struct RustFileContext {
    pub token_map: TokenMap,
    pub ast: Result<syn::File, syn::Error>,
}

impl RustFileContext {
    /// Parse `content` as Rust source (UTF-8).
    pub fn parse(content: &str) -> Self {
        let token_map = TokenMap::from_rust_source(content);
        let ast = syn::parse_file(content);
        Self { token_map, ast }
    }

    /// True if every byte on line `line_1_indexed` (1-based) that overlaps `content` is in a code span.
    /// Lines are split the same way as [`crate::rules::SourceFile::lines`].
    pub fn line_is_prose_safe(&self, content: &str, line_1_indexed: usize) -> bool {
        if line_1_indexed == 0 {
            return false;
        }
        let line_idx = line_1_indexed.saturating_sub(1);
        let mut start = 0usize;
        for (i, line) in content.lines().enumerate() {
            if i == line_idx {
                let end = start.saturating_add(line.len());
                if (start..end).any(|b| self.token_map.is_code_byte(b)) {
                    return false;
                }
                return true;
            }
            start = start.saturating_add(line.len()).saturating_add(1);
        }
        false
    }
}

#[cfg(test)]
mod semcov_wave1e_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn comment_only_line_is_prose_safe() {
        let content = "// this is a comment\nlet x = 1;";
        let ctx = RustFileContext::parse(content);
        // Line 1 is entirely a comment — no code bytes
        assert!(ctx.line_is_prose_safe(content, 1));
    }

    #[test]
    fn code_line_is_not_prose_safe() {
        let content = "let x = 1;\n// comment";
        let ctx = RustFileContext::parse(content);
        // Line 1 has code bytes
        assert!(!ctx.line_is_prose_safe(content, 1));
    }

    #[test]
    fn second_line_code_detected() {
        let content = "// comment\nlet y = 2;";
        let ctx = RustFileContext::parse(content);
        assert!(ctx.line_is_prose_safe(content, 1));
        assert!(!ctx.line_is_prose_safe(content, 2));
    }

    #[test]
    fn zero_line_returns_false() {
        let content = "let x = 1;";
        let ctx = RustFileContext::parse(content);
        assert!(!ctx.line_is_prose_safe(content, 0));
    }

    #[test]
    fn out_of_range_line_returns_false() {
        let content = "let x = 1;";
        let ctx = RustFileContext::parse(content);
        assert!(!ctx.line_is_prose_safe(content, 99));
    }

    #[test]
    fn string_literal_line_is_not_prose_safe() {
        // A string literal is non-code but NOT a comment; the function checks is_code_byte.
        // String bytes are non-code, so a line with only a string is prose-safe.
        let content = r#"let _ = "hello world";"#;
        let ctx = RustFileContext::parse(content);
        // The line has code bytes (let, =, ;) so it's NOT prose safe
        assert!(!ctx.line_is_prose_safe(content, 1));
    }
}
