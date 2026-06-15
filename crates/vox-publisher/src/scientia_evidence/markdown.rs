use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DocSectionHint {
    pub heading_level: u8,
    pub slug: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

/// Best-effort markdown title inference: frontmatter `title:` first, then first `# Heading`, else `Untitled`.
#[must_use]
pub fn infer_markdown_title(content: &str) -> String {
    let content = content.trim();
    if let Some(rest) = content.strip_prefix("---") {
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(idx) = rest.find("\n---") {
            let fm = &rest[..idx];
            for line in fm.lines() {
                if let Some(val) = line.trim().strip_prefix("title:") {
                    let inferred = val.trim().trim_matches('"').trim_matches('\'').trim();
                    if !inferred.is_empty() {
                        return inferred.to_string();
                    }
                }
            }
        }
    }
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let inferred = heading.trim();
            if !inferred.is_empty() {
                return inferred.to_string();
            }
        }
    }
    "Untitled".to_string()
}

fn slugify_heading(title: &str) -> String {
    let mut out = String::new();
    let mut last_sep = true;
    for c in title.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_sep = false;
        } else if !last_sep {
            out.push('-');
            last_sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "section".into()
    } else {
        out
    }
}

/// Collect up to 64 markdown headings (`#` … `######`) for doc navigation hints.
#[must_use]
pub fn infer_doc_sections_from_markdown(content: &str) -> Vec<DocSectionHint> {
    let lines: Vec<&str> = content.lines().collect();
    let mut start = 0usize;
    if content.trim_start().starts_with("---") && lines.len() > 1 {
        start = 1;
        while start < lines.len() {
            if lines[start].trim() == "---" {
                start += 1;
                break;
            }
            start += 1;
        }
    }
    let mut hints = Vec::new();
    let mut line_no = start + 1;
    for line in lines.iter().skip(start) {
        let t = line.trim_start();
        if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count();
            if level == 0 || level > 6 {
                line_no += 1;
                continue;
            }
            let after = t[level..].trim();
            if after.is_empty() {
                line_no += 1;
                continue;
            }
            let title = after.to_string();
            hints.push(DocSectionHint {
                heading_level: u8::try_from(level).unwrap_or(6),
                slug: slugify_heading(&title),
                title,
                line: Some(line_no),
            });
            if hints.len() >= 64 {
                break;
            }
        }
        line_no += 1;
    }
    hints
}

#[cfg(test)]
mod semcov_behavior_tests {
    use super::*;

    #[test]
    fn slugify_heading_lowercases_collapses_separators_and_falls_back() {
        // Catches: trailing/duplicate hyphens leaking, non-ascii not stripped, or the
        // empty-result branch not yielding the "section" fallback.
        assert_eq!(slugify_heading("Hello, World!"), "hello-world");
        assert_eq!(
            slugify_heading("  Multiple   Spaces--Here  "),
            "multiple-spaces-here"
        );
        assert_eq!(slugify_heading("***"), "section");
        assert_eq!(slugify_heading("Already-slug"), "already-slug");
    }
}
