//! Forbidden-section policy.
//!
//! The worthiness rubric in
//! `docs/src/reference/scientia-publication-worthiness-rules.md` lists
//! categories that MUST NOT be auto-generated without explicit human
//! authorship. The scaffolder enforces this at emission time: any section
//! named here renders as a `<!-- TODO(narrative): -->` placeholder block,
//! never as prose lifted from a model output.

use serde::{Deserialize, Serialize};

/// Sections whose body the scaffolder will never auto-generate. Names are
/// matched case-insensitively against the section title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForbiddenSection {
    Introduction,
    Discussion,
    Significance,
    /// "Conclusion" sections in many CS venues are interpretation, not
    /// summary — kept forbidden by default.
    Conclusion,
}

impl ForbiddenSection {
    pub const ALL: &'static [Self] = &[
        Self::Introduction,
        Self::Discussion,
        Self::Significance,
        Self::Conclusion,
    ];

    pub fn as_title(&self) -> &'static str {
        match self {
            Self::Introduction => "Introduction",
            Self::Discussion => "Discussion",
            Self::Significance => "Significance",
            Self::Conclusion => "Conclusion",
        }
    }
}

/// Is `title` the title of a forbidden section? Case-insensitive.
pub fn is_section_forbidden(title: &str) -> bool {
    let normalized = title.trim().to_ascii_lowercase();
    ForbiddenSection::ALL
        .iter()
        .any(|s| s.as_title().eq_ignore_ascii_case(&normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_forbidden_sections_match_case_insensitively() {
        for s in ForbiddenSection::ALL {
            assert!(is_section_forbidden(s.as_title()));
            assert!(is_section_forbidden(&s.as_title().to_uppercase()));
            assert!(is_section_forbidden(&s.as_title().to_lowercase()));
            assert!(is_section_forbidden(&format!("  {}  ", s.as_title())));
        }
    }

    #[test]
    fn safe_sections_are_not_forbidden() {
        for t in ["Methods", "Results", "Limitations", "References", "Author Block"] {
            assert!(!is_section_forbidden(t), "{t} should be safe");
        }
    }

    #[test]
    fn empty_or_whitespace_title_is_not_forbidden() {
        assert!(!is_section_forbidden(""));
        assert!(!is_section_forbidden("   "));
    }
}
