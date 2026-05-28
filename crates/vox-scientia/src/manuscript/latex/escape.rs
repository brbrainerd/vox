//! Canonical LaTeX escape for arbitrary text.
//!
//! The TeX special characters that need escaping in text mode are:
//!   `\`, `{`, `}`, `$`, `&`, `#`, `_`, `%`, `^`, `~`.
//!
//! This module implements the canonical escape recommended by the LaTeX
//! Companion (2nd ed.) for general text content. Math mode is NOT covered —
//! callers must wrap math expressions in `$...$` themselves before
//! escaping (or use the markdown-aware `render_latex` which respects
//! pulldown-cmark's code-block and inline-code spans).

/// Escape `s` for safe inclusion in LaTeX text mode.
pub fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '\\' => out.push_str(r"\textbackslash{}"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
            '$' => out.push_str(r"\$"),
            '&' => out.push_str(r"\&"),
            '#' => out.push_str(r"\#"),
            '_' => out.push_str(r"\_"),
            '%' => out.push_str(r"\%"),
            '^' => out.push_str(r"\textasciicircum{}"),
            '~' => out.push_str(r"\textasciitilde{}"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_ordinary_text_unchanged() {
        assert_eq!(escape_latex("Hello world."), "Hello world.");
    }

    #[test]
    fn escapes_all_ten_special_characters() {
        let input = r"\ { } $ & # _ % ^ ~";
        let out = escape_latex(input);
        // Each special should produce its escape form.
        assert!(out.contains(r"\textbackslash{}"));
        assert!(out.contains(r"\{"));
        assert!(out.contains(r"\}"));
        assert!(out.contains(r"\$"));
        assert!(out.contains(r"\&"));
        assert!(out.contains(r"\#"));
        assert!(out.contains(r"\_"));
        assert!(out.contains(r"\%"));
        assert!(out.contains(r"\textasciicircum{}"));
        assert!(out.contains(r"\textasciitilde{}"));
    }

    #[test]
    fn unicode_passes_through() {
        let s = "Café — naïve résumé";
        assert_eq!(escape_latex(s), s);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(escape_latex(""), "");
    }

    #[test]
    fn double_underscore_in_variable_names_escapes_twice() {
        assert_eq!(escape_latex("foo_bar_baz"), r"foo\_bar\_baz");
    }

    #[test]
    fn percent_sign_in_numbers_is_escaped() {
        assert_eq!(escape_latex("23%"), r"23\%");
    }
}
