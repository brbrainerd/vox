//! Deterministic normalization for spoken code: symbol phrases and casing commands.

/// True if `b` is a byte that can be part of a "word" (letter/digit/underscore).
/// Same definition and purpose as `refine::rules::is_word_byte`: used to reject
/// a substring match that's actually the middle of a longer ordinary word.
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find `phrase` in `haystack` (ASCII case-insensitive) starting at or after
/// byte offset `from`, at a word boundary on both sides. Mirrors
/// `refine::rules::find_boundary_match` — kept as a local copy rather than a
/// shared export since the two callers (code-confusion-map phrases vs.
/// spoken-symbol phrases) live in otherwise-unrelated modules and the
/// function is a handful of lines.
fn find_boundary_match(haystack: &[u8], phrase: &[u8], from: usize) -> Option<usize> {
    if phrase.is_empty() || from > haystack.len() {
        return None;
    }
    let mut start = from;
    while start + phrase.len() <= haystack.len() {
        if haystack[start..start + phrase.len()].eq_ignore_ascii_case(phrase) {
            let before_ok = start == 0 || !is_word_byte(haystack[start - 1]);
            let end = start + phrase.len();
            let after_ok = end == haystack.len() || !is_word_byte(haystack[end]);
            if before_ok && after_ok {
                return Some(start);
            }
        }
        start += 1;
    }
    None
}

/// Replace common spoken symbol phrases with ASCII (conservative list).
///
/// Word-boundary-checked (via [`find_boundary_match`]): a bare substring
/// search here would corrupt ordinary English words that merely contain a
/// phrase as a substring — e.g. "comma" inside "command"/"commander", or
/// "dot" inside "anecdote"/"dotted" — exactly the bug class already fixed
/// for `code_confusion_map` in `refine::rules::apply_phrase_confusions`.
#[must_use]
pub fn expand_spoken_symbols(text: &str) -> String {
    let mut s = text.to_string();
    let pairs: &[(&str, &str)] = &[
        ("open brace", "{"),
        ("close brace", "}"),
        ("open curly", "{"),
        ("close curly", "}"),
        ("open bracket", "["),
        ("close bracket", "]"),
        ("open angle", "<"),
        ("close angle", ">"),
        ("open paren", "("),
        ("close paren", ")"),
        ("fat arrow", "=>"),
        ("arrow", "->"),
        ("semicolon", ";"),
        ("colon colon", "::"),
        ("colon", ":"),
        ("comma", ","),
        ("new line", "\n"),
        ("underscore", "_"),
        ("double equals", "=="),
        ("not equals", "!="),
        ("dot dot dot", "..."),
        ("dot dot", ".."),
        ("dot", "."),
        ("bang", "!"),
        ("ampersand", "&"),
        ("pipe", "|"),
        ("asterisk", "*"),
        ("backslash", "\\"),
    ];
    for (phrase, sym) in pairs {
        loop {
            match find_boundary_match(s.as_bytes(), phrase.as_bytes(), 0) {
                Some(i) => s.replace_range(i..i + phrase.len(), sym),
                None => break,
            }
        }
    }
    s
}

/// If the transcript starts with a casing command, return `(style, remainder)`.
#[must_use]
pub fn strip_casing_command(transcript: &str) -> Option<(CasingStyle, &str)> {
    let t = transcript.trim();
    let lower = t.to_ascii_lowercase();
    for (prefix, style) in [
        ("camel case ", CasingStyle::Camel),
        ("camelcase ", CasingStyle::Camel),
        ("pascal case ", CasingStyle::Pascal),
        ("pascalcase ", CasingStyle::Pascal),
        ("snake case ", CasingStyle::Snake),
        ("snakecase ", CasingStyle::Snake),
        ("constant case ", CasingStyle::Constant),
        ("constantcase ", CasingStyle::Constant),
        ("kebab case ", CasingStyle::Kebab),
        ("kebabcase ", CasingStyle::Kebab),
    ] {
        if lower.starts_with(prefix) {
            let rest = t[prefix.len()..].trim_start();
            return Some((style, rest));
        }
    }
    None
}

/// Identifier casing style from voice commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CasingStyle {
    /// `camelCase` (first word lower, rest capitalized).
    Camel,
    /// `PascalCase`.
    Pascal,
    /// `snake_case`.
    Snake,
    /// `SCREAMING_SNAKE_CASE`.
    Constant,
    /// `kebab-case`.
    Kebab,
}

impl CasingStyle {
    /// Apply casing to space-separated words (e.g. "get user name" → `getUserName` for Camel).
    #[must_use]
    pub fn apply_words(self, words: &str) -> String {
        let parts: Vec<&str> = words.split_whitespace().filter(|w| !w.is_empty()).collect();
        if parts.is_empty() {
            return String::new();
        }
        match self {
            Self::Kebab => parts
                .iter()
                .map(|w| w.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join("-"),
            Self::Snake => parts
                .iter()
                .map(|w| w.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
            Self::Constant => parts
                .iter()
                .map(|w| w.to_ascii_uppercase())
                .collect::<Vec<_>>()
                .join("_"),
            Self::Pascal => parts
                .iter()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => {
                            let mut s = String::new();
                            s.push(f.to_ascii_uppercase());
                            s.push_str(c.as_str());
                            s
                        }
                    }
                })
                .collect(),
            Self::Camel => {
                let mut it = parts.iter();
                let first = it.next().unwrap().to_ascii_lowercase();
                let rest: String = it
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => {
                                let mut s = String::new();
                                s.push(f.to_ascii_uppercase());
                                s.push_str(c.as_str());
                                s
                            }
                        }
                    })
                    .collect();
                first + &rest
            }
        }
    }
}

/// Full deterministic pass: spoken symbols, then optional casing prefix.
#[must_use]
pub fn normalize_spoken_code_phrase(transcript: &str) -> String {
    let sym = expand_spoken_symbols(transcript);
    if let Some((style, rest)) = strip_casing_command(&sym) {
        let body = style.apply_words(rest);
        return body;
    }
    sym
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_spoken_symbols_respects_word_boundaries() {
        // Regression test (code review finding): "comma" and "dot" are
        // ordinary English word fragments — a bare substring match would
        // corrupt them mid-word, the same bug class already fixed for
        // code_confusion_map's phrase matching (refine::rules).
        assert_eq!(
            expand_spoken_symbols("run the command now"),
            "run the command now",
            "must not match \"comma\" inside \"command\""
        );
        assert_eq!(
            expand_spoken_symbols("an anecdote about dogs"),
            "an anecdote about dogs",
            "must not match \"dot\" inside \"anecdote\""
        );
        // The real, boundary-respecting cases must still work.
        assert_eq!(expand_spoken_symbols("a comma b"), "a , b");
        assert_eq!(expand_spoken_symbols("self dot user"), "self . user");
    }

    #[test]
    fn camel_case_command() {
        assert_eq!(
            normalize_spoken_code_phrase("camel case get user name"),
            "getUserName"
        );
    }

    #[test]
    fn fat_arrow() {
        assert!(normalize_spoken_code_phrase("x fat arrow y").contains("=>"));
    }

    #[test]
    fn curly_comma_angle_and_bare_dot_expand() {
        assert_eq!(expand_spoken_symbols("open curly close curly"), "{ }");
        assert_eq!(expand_spoken_symbols("a comma b"), "a , b");
        assert_eq!(expand_spoken_symbols("open angle close angle"), "< >");
        assert_eq!(expand_spoken_symbols("self dot user"), "self . user");
        assert_eq!(
            expand_spoken_symbols("user state colon colon active"),
            "user state :: active"
        );
        // Existing ellipsis mappings must still win over the new bare "dot".
        assert_eq!(
            expand_spoken_symbols("wait dot dot dot done"),
            "wait ... done"
        );
    }
}
