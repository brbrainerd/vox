//! Per-file Rust analysis: token map + optional `syn` AST.

use super::token_map::TokenMap;

/// The byte offset where line `line_1_indexed` (1-based) starts, and the line itself
/// without its terminator.
///
/// Deliberately not `content.lines()` + `len() + 1`: `lines()` strips `\r`, so on a
/// CRLF file that accumulator loses one byte per line and every later offset is
/// consulted against the wrong position in the [`TokenMap`] — which silently blinds
/// the detectors rather than failing. Three tracked `.rs` files in this workspace are
/// CRLF in the working tree, so this is a live path, not a hypothetical.
///
/// Line numbering matches [`str::lines`], and therefore
/// [`crate::rules::SourceFile::lines`], for every input.
fn line_span(content: &str, line_1_indexed: usize) -> Option<(usize, &str)> {
    if line_1_indexed == 0 {
        return None;
    }
    let target = line_1_indexed - 1;
    let mut start = 0usize;
    for (i, segment) in content.split_inclusive('\n').enumerate() {
        if i == target {
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            return Some((start, line.strip_suffix('\r').unwrap_or(line)));
        }
        start = start.saturating_add(segment.len());
    }
    None
}

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

    /// Returns line `line_1_indexed` (1-based) with every comment and string-literal
    /// byte replaced by a space, so a pattern can match code without matching text
    /// that merely *mentions* it.
    ///
    /// `use ring::digest;` is an import; `let code = "use ring::digest;";` is a test
    /// fixture, and a detector that cannot tell them apart reports its own test data.
    /// Because this projects the file-wide [`TokenMap`], it handles what a per-line
    /// scan cannot: literals spanning several physical lines, `b"..."`, raw
    /// `r#"..."#`, and nested block comments.
    ///
    /// Byte length is preserved — a blanked multi-byte character emits that many
    /// spaces — so a byte offset taken from the raw line indexes the same character
    /// here. [`Self::is_code_at`] depends on this exactly.
    ///
    /// Not for detectors whose target legitimately lives inside a literal (hardcoded
    /// secrets, provider hostnames) — blanking would erase exactly what they hunt.
    pub fn code_only_line(&self, content: &str, line_1_indexed: usize) -> String {
        let Some((start, line)) = line_span(content, line_1_indexed) else {
            return String::new();
        };
        let mut out = String::with_capacity(line.len());
        for (off, ch) in line.char_indices() {
            if self.token_map.is_code_byte(start.saturating_add(off)) {
                out.push(ch);
            } else {
                // One space per BYTE, not per char: a `☕` inside a blanked literal
                // would otherwise shrink the line by 2 bytes and shift every later
                // column left, silently moving a caller's anchor off its token.
                for _ in 0..ch.len_utf8() {
                    out.push(' ');
                }
            }
        }
        out
    }

    /// True if the byte at `col_0_indexed` on line `line_1_indexed` (1-based) is source
    /// code rather than comment or string-literal text.
    ///
    /// The complement of [`Self::code_only_line`], for detectors whose *target*
    /// legitimately lives in a literal but whose *anchor* is code: an env-var name
    /// (`env::var("OPENAI_API_KEY")`) or a provider hostname
    /// (`client.post("https://api.openai.com/…")`). Match the pattern on the raw line,
    /// then ask whether the call expression that matched is real code — which it is not
    /// when the whole thing sits inside a test fixture or a doc string.
    ///
    /// Callers must anchor on a non-space character: blanking is done with spaces, so a
    /// column that holds a genuine space in code also reports `false`.
    pub fn is_code_at(&self, content: &str, line_1_indexed: usize, col_0_indexed: usize) -> bool {
        self.code_only_line(content, line_1_indexed)
            .as_bytes()
            .get(col_0_indexed)
            .is_some_and(|b| *b != b' ')
    }

    /// True if every byte on line `line_1_indexed` (1-based) that overlaps `content` is in a code span.
    /// Lines are split the same way as [`crate::rules::SourceFile::lines`].
    pub fn line_is_prose_safe(&self, content: &str, line_1_indexed: usize) -> bool {
        let Some((start, line)) = line_span(content, line_1_indexed) else {
            return false;
        };
        let end = start.saturating_add(line.len());
        !(start..end).any(|b| self.token_map.is_code_byte(b))
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

    #[test]
    fn code_only_line_blanks_strings_and_comments_but_keeps_code() {
        let content = "let code = \"use ring::digest;\";\nuse ring::digest; // use md5;\n";
        let ctx = RustFileContext::parse(content);

        let l1 = ctx.code_only_line(content, 1);
        assert_eq!(l1.len(), "let code = \"use ring::digest;\";".len());
        assert!(!l1.contains("use ring"), "string content blanked: {l1}");
        assert!(l1.contains("let code"), "code kept: {l1}");

        let l2 = ctx.code_only_line(content, 2);
        assert!(l2.contains("use ring::digest;"), "real import kept: {l2}");
        assert!(!l2.contains("use md5"), "trailing comment blanked: {l2}");
    }

    #[test]
    fn is_code_at_separates_real_call_from_fixture_and_doc_string() {
        let content = concat!(
            "let fixture = \"std::env::var(\\\"OPENAI_API_KEY\\\")\";\n",
            "let key = std::env::var(\"OPENAI_API_KEY\").unwrap();\n",
        );
        let ctx = RustFileContext::parse(content);

        let in_literal = content.lines().next().unwrap().find("std::env").unwrap();
        assert!(
            !ctx.is_code_at(content, 1, in_literal),
            "env call inside a string literal is fixture text"
        );

        let real = content.lines().nth(1).unwrap().find("std::env").unwrap();
        assert!(
            ctx.is_code_at(content, 2, real),
            "real env call must still read as code"
        );
    }

    #[test]
    fn code_only_line_handles_raw_and_multiline_literals() {
        // A raw string, and a normal literal spanning two physical lines — the case a
        // per-line scanner cannot see.
        let content = "let r = r#\"use aegis::x;\"#;\nlet s = \"use openssl;\nuse md5;\";\n";
        let ctx = RustFileContext::parse(content);
        assert!(!ctx.code_only_line(content, 1).contains("use aegis"));
        assert!(!ctx.code_only_line(content, 2).contains("use openssl"));
        assert!(
            !ctx.code_only_line(content, 3).contains("use md5"),
            "continuation line of a multi-line literal must stay blanked"
        );
    }

    /// A blanked multi-byte char must emit one space PER BYTE, or every column after
    /// it shifts left and `is_code_at` silently checks the wrong position. Proven to
    /// hide real violations before this fix.
    #[test]
    fn code_only_line_preserves_byte_width_across_non_ascii() {
        let content = "let doc = \"☕☕☕ use ring\"; use md5::Md5;\n";
        let ctx = RustFileContext::parse(content);
        let line = ctx.code_only_line(content, 1);
        assert_eq!(
            line.len(),
            content.trim_end_matches('\n').len(),
            "byte length must be preserved: {line:?}"
        );
        // The real import survives at its true byte column.
        let col = content.find("use md5").expect("anchor present");
        assert!(ctx.is_code_at(content, 1, col), "anchor must read as code");
        // The mention inside the literal does not.
        let lit = content.find("use ring").expect("literal present");
        assert!(
            !ctx.is_code_at(content, 1, lit),
            "literal must read as non-code"
        );
    }

    /// `str::lines()` strips `\r`, so a naive `len() + 1` accumulator drifts one byte
    /// per line on CRLF input and consults the TokenMap at the wrong offsets.
    #[test]
    fn line_offsets_are_correct_under_crlf() {
        let lf = "// c\r\nlet s = \"use ring\";\r\nuse md5::Md5;\r\n";
        let ctx = RustFileContext::parse(lf);
        assert!(
            ctx.code_only_line(lf, 3).contains("use md5"),
            "line 3 code must survive CRLF: {:?}",
            ctx.code_only_line(lf, 3)
        );
        assert!(
            !ctx.code_only_line(lf, 2).contains("use ring"),
            "line 2 literal must still be blanked under CRLF"
        );
    }
}
