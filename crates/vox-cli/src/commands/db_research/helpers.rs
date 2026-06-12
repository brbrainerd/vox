pub(super) fn split_csv(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub(super) fn summarize_text(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let mut summary = trimmed.chars().take(max_chars).collect::<String>();
        summary.push_str("...");
        summary
    }
}

pub(super) fn html_to_text_lossy(input: &str) -> String {
    let without_scripts = regex::Regex::new(r"(?is)<script.*?</script>|<style.*?</style>")
        .ok()
        .map(|re| re.replace_all(input, " ").into_owned())
        .unwrap_or_else(|| input.to_string());
    let without_tags = regex::Regex::new(r"(?is)<[^>]+>")
        .ok()
        .map(|re| re.replace_all(&without_scripts, " ").into_owned())
        .unwrap_or(without_scripts);
    let decoded = without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract title from markdown: frontmatter `title:` or first `# Heading`.
pub(super) fn extract_md_title(content: &str) -> String {
    let content = content.trim();
    if let Some(rest) = content.strip_prefix("---") {
        let rest = rest.strip_prefix('\n').unwrap_or(rest);
        if let Some(idx) = rest.find("\n---") {
            let fm = &rest[..idx];
            for line in fm.lines() {
                if let Some(val) = line.trim().strip_prefix("title:") {
                    return val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            return heading.trim().to_string();
        }
    }
    "Untitled".to_string()
}
