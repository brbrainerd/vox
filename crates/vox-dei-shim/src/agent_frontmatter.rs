//! Parse YAML frontmatter in `.vox/agents/*.md` agent definitions.
//!
//! Used for `scope:` and `model:` so runtime routing matches documented agent files.

use std::path::Path;

/// Parse `key: value` or `key : value` at line start (trimmed), with optional quotes on the value.
fn line_value_after_key(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    let colon = format!("{key}:");
    let spaced = format!("{key} :");
    let rest = t
        .strip_prefix(colon.as_str())
        .or_else(|| t.strip_prefix(spaced.as_str()))?;
    let val = rest.trim();
    if val.is_empty() {
        return None;
    }
    Some(
        val.trim_matches('"')
            .trim_matches('\'')
            .to_string(),
    )
}

/// Parsed agent frontmatter fields used by Dei and MCP tools.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentFrontmatter {
    /// Glob patterns from `scope:` (if any).
    pub scope: Option<Vec<String>>,
    /// Model id from `model:` (if any), e.g. `google/gemini-2.5-flash-preview`.
    pub model: Option<String>,
}

/// Parse `scope:` and `model:` from a file if it exists and has YAML frontmatter.
pub fn parse_agent_file_path(path: &Path) -> std::io::Result<AgentFrontmatter> {
    if !path.exists() {
        return Ok(AgentFrontmatter::default());
    }
    let content = std::fs::read_to_string(path)?;
    let trimmed = content.trim_start();
    if trimmed.starts_with("---") && parse_agent_frontmatter(&content).is_none() {
        tracing::warn!(
            path = %path.display(),
            "agent markdown begins with '---' but frontmatter could not be parsed (e.g. missing closing '---'); scope/model ignored"
        );
    }
    Ok(parse_agent_frontmatter(&content).unwrap_or_default())
}

/// Parse frontmatter from raw file contents.
///
/// Returns `None` if there is no `---` block; otherwise returns parsed fields (possibly empty).
pub fn parse_agent_frontmatter(content: &str) -> Option<AgentFrontmatter> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = trimmed
        .trim_start_matches('-')
        .trim_start_matches('\n')
        .trim_start_matches('\r');
    let end = after_first
        .find("\n---")
        .or_else(|| after_first.find("\r\n---"))?;
    let frontmatter = &after_first[..end];

    let mut scope: Option<Vec<String>> = None;
    let mut model: Option<String> = None;
    let mut in_scope_block = false;

    for line in frontmatter.lines() {
        if in_scope_block {
            if let Some(stripped) = line.trim().strip_prefix('-') {
                let pat = stripped.trim().trim_matches('"').trim_matches('\'');
                if !pat.is_empty() {
                    scope
                        .get_or_insert_with(Vec::new)
                        .push(pat.to_string());
                }
                continue;
            }
            in_scope_block = false;
        }

        let scope_line = line.trim_start();
        if let Some(val) = scope_line
            .strip_prefix("scope:")
            .or_else(|| scope_line.strip_prefix("scope :"))
        {
            let val = val.trim();
            let mut patterns: Vec<String> = Vec::new();
            if val.is_empty() {
                in_scope_block = true;
                scope = Some(patterns);
                continue;
            }
            if val.starts_with('[') {
                let inner = val.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let pat = item.trim().trim_matches('"').trim_matches('\'');
                    if !pat.is_empty() {
                        patterns.push(pat.to_string());
                    }
                }
            } else {
                let pat = val.trim_matches('"').trim_matches('\'');
                if !pat.is_empty() {
                    patterns.push(pat.to_string());
                }
            }
            if !patterns.is_empty() {
                scope = Some(patterns);
            }
            continue;
        }

        if let Some(val) = line.strip_prefix("model:") {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                model = Some(val.to_string());
            }
        }
    }

    Some(AgentFrontmatter { scope, model })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_model_and_scope() {
        let raw = "---\nscope: [\"src/**\"]\nmodel: google/gemini-flash\n---\n# Agent\n";
        let fm = parse_agent_frontmatter(raw).expect("frontmatter");
        assert_eq!(fm.model.as_deref(), Some("google/gemini-flash"));
        assert_eq!(fm.scope, Some(vec!["src/**".to_string()]));
    }

    #[test]
    fn parses_model_only() {
        let raw = "---\nmodel: openrouter/auto\n---\n";
        let fm = parse_agent_frontmatter(raw).expect("frontmatter");
        assert_eq!(fm.model.as_deref(), Some("openrouter/auto"));
        assert!(fm.scope.is_none());
    }

    #[test]
    fn parses_model_with_space_after_key() {
        let raw = "---\nmodel : google/gemini-flash\n---\n";
        let fm = parse_agent_frontmatter(raw).expect("frontmatter");
        assert_eq!(fm.model.as_deref(), Some("google/gemini-flash"));
    }

    #[test]
    fn malformed_frontmatter_opening_without_close_returns_none() {
        let raw = "---\nmodel: x\nno closing fence\n";
        assert!(parse_agent_frontmatter(raw).is_none());
    }
}
