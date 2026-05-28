use crate::features::ExtractedFeatures;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub trait LanguageExtractor: Send + Sync {
    fn extract(&self, path: &Path, content: &str) -> Result<ExtractedFeatures>;
}

/// Extract crate name from path like `crates/vox-foo/src/lib.rs` → `"vox-foo"`.
pub fn crate_name_from_path(path: &Path) -> Option<String> {
    let mut components = path.components().peekable();
    while let Some(part) = components.next() {
        let part_str = part.as_os_str().to_string_lossy();
        if part_str == "crates"
            && let Some(name_part) = components.next()
        {
            let name = name_part.as_os_str().to_string_lossy().into_owned();
            return Some(name);
        }
    }
    None
}

/// Scan source text for `// drift-allow(rule-id[,rule-id…])` and the
/// `//! drift-allow(...)` module-attribute variant. Returns a map keyed by
/// rule-id whose values are the 1-indexed line numbers covered.
///
/// Each annotation covers the line it sits on AND the following line. That
/// supports both trailing-comment style:
///
/// ```ignore
/// Duration::from_secs(5);          // drift-allow(timeout-literal): rationale
/// ```
///
/// and the comment-above style:
///
/// ```ignore
/// // drift-allow(timeout-literal): rationale
/// Duration::from_secs(5);
/// ```
///
/// Rule IDs may be slash-segmented (`drift/timeout-literal`) or short-form
/// (`timeout-literal`); both forms get stored verbatim. Rules accept either
/// `self.id()` or its short suffix via [`ExtractedFeatures::is_allowed`] +
/// [`short_rule_id`].
pub fn parse_drift_allow_comments(content: &str) -> HashMap<String, HashSet<usize>> {
    let mut out: HashMap<String, HashSet<usize>> = HashMap::new();
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1; // 1-indexed
        let Some(start) = line.find("drift-allow(") else {
            continue;
        };
        // Require the marker to follow a comment opener (`//`, `///`, `//!`, `/*`),
        // not appear inside e.g. a string literal called `drift-allow(...)`.
        let prefix = &line[..start];
        let prefix_trimmed = prefix.trim_end();
        let looks_like_comment = prefix_trimmed.ends_with("//")
            || prefix_trimmed.ends_with("///")
            || prefix_trimmed.ends_with("//!")
            || prefix_trimmed.ends_with("/*")
            || prefix_trimmed.ends_with("/**")
            || prefix_trimmed.ends_with('#');
        if !looks_like_comment {
            continue;
        }
        let rest = &line[start + "drift-allow(".len()..];
        let Some(end) = rest.find(')') else {
            continue;
        };
        let ids_segment = &rest[..end];
        for raw in ids_segment.split(',') {
            let id = raw.trim();
            if id.is_empty() {
                continue;
            }
            let bucket = out.entry(id.to_string()).or_default();
            bucket.insert(line_no);
            bucket.insert(line_no + 1);
        }
    }
    out
}

/// Strip the `drift/` (or `sweep/`) prefix from a rule id. Per-line annotations
/// accept either form, so rules whose `id()` returns `"drift/timeout-literal"`
/// should also check `"timeout-literal"`.
pub fn short_rule_id(id: &str) -> &str {
    id.split_once('/').map(|(_, s)| s).unwrap_or(id)
}

/// Convenience: true if either the full rule id or its short form is allowed at `line`.
pub fn is_allowed_at(features: &ExtractedFeatures, rule_id: &str, line: usize) -> bool {
    features.is_allowed(rule_id, line) || features.is_allowed(short_rule_id(rule_id), line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn crate_name_from_path_under_crates() {
        let p = Path::new("crates/vox-config/src/lib.rs");
        assert_eq!(crate_name_from_path(p), Some("vox-config".to_string()));
    }

    #[test]
    fn crate_name_from_path_unknown() {
        let p = Path::new("apps/my-app/index.ts");
        assert_eq!(crate_name_from_path(p), None);
    }

    #[test]
    fn parse_drift_allow_trailing_comment_covers_same_line() {
        let src = "let opts_max = Duration::from_secs(5); // drift-allow(timeout-literal): test\n";
        let allowed = parse_drift_allow_comments(src);
        assert!(allowed.get("timeout-literal").unwrap().contains(&1));
    }

    #[test]
    fn parse_drift_allow_above_covers_next_line() {
        let src = "// drift-allow(timeout-literal): test\nlet x = Duration::from_secs(5);\n";
        let allowed = parse_drift_allow_comments(src);
        let set = allowed.get("timeout-literal").unwrap();
        assert!(set.contains(&1));
        assert!(set.contains(&2));
    }

    #[test]
    fn parse_drift_allow_multiple_ids() {
        let src = "// drift-allow(timeout-literal, bearer-header-inline): both\nx\n";
        let allowed = parse_drift_allow_comments(src);
        assert!(allowed.get("timeout-literal").unwrap().contains(&1));
        assert!(allowed.get("bearer-header-inline").unwrap().contains(&1));
    }

    #[test]
    fn parse_drift_allow_accepts_full_rule_id() {
        let src = "// drift-allow(drift/timeout-literal): explicit form\nx\n";
        let allowed = parse_drift_allow_comments(src);
        assert!(allowed.get("drift/timeout-literal").unwrap().contains(&1));
    }

    #[test]
    fn parse_drift_allow_ignores_marker_in_string_literal() {
        // The marker MUST follow a comment opener; raw occurrences in a string don't count.
        let src = "let s = \"drift-allow(timeout-literal)\";\n";
        let allowed = parse_drift_allow_comments(src);
        assert!(allowed.is_empty());
    }

    #[test]
    fn short_rule_id_strips_prefix() {
        assert_eq!(short_rule_id("drift/timeout-literal"), "timeout-literal");
        assert_eq!(short_rule_id("sweep/duplicate-body"), "duplicate-body");
        assert_eq!(short_rule_id("plain-id"), "plain-id");
    }

    #[test]
    fn is_allowed_at_accepts_either_form() {
        let mut f =
            ExtractedFeatures::new(std::path::PathBuf::from("t.rs"), vox_code_audit::rules::Language::Rust);
        let mut set = HashSet::new();
        set.insert(42);
        f.allowed_lines.insert("timeout-literal".to_string(), set);
        assert!(is_allowed_at(&f, "drift/timeout-literal", 42));
        assert!(is_allowed_at(&f, "timeout-literal", 42));
        assert!(!is_allowed_at(&f, "timeout-literal", 43));
    }
}
