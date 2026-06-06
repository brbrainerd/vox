//! Deterministic normalization for spoken code: symbol phrases and casing commands.

/// Replace common spoken symbol phrases with ASCII (conservative list).
#[must_use]
pub fn expand_spoken_symbols(text: &str) -> String {
    let mut s = text.to_string();
    let pairs: &[(&str, &str)] = &[
        ("open brace", "{"),
        ("close brace", "}"),
        ("open bracket", "["),
        ("close bracket", "]"),
        ("open paren", "("),
        ("close paren", ")"),
        ("fat arrow", "=>"),
        ("arrow", "->"),
        ("semicolon", ";"),
        ("colon", ":"),
        ("new line", "\n"),
        ("underscore", "_"),
        ("double equals", "=="),
        ("not equals", "!="),
        ("dot dot dot", "..."),
        ("dot dot", ".."),
        ("bang", "!"),
        ("ampersand", "&"),
        ("pipe", "|"),
        ("asterisk", "*"),
        ("backslash", "\\"),
    ];
    for (phrase, sym) in pairs {
        let plen = phrase.len();
        loop {
            let lower = s.to_ascii_lowercase();
            if let Some(i) = lower.find(phrase) {
                s.replace_range(i..i + plen, sym);
            } else {
                break;
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
}
