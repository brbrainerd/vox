//! `vox ci agentskills-compliance`
//!
//! Walks `crates/` for any `*.skill.md` file inside `vox-plugin-*` directories
//! and verifies the AgentSkills open standard frontmatter contract:
//!
//! 1. Frontmatter block (`---` … `---`) exists at the start of the file.
//! 2. `name` field is present.
//! 3. `name` matches `^[a-z0-9][a-z0-9-]{0,63}$`.
//! 4. `name` matches the crate directory's short-name (directory name with
//!    the leading `vox-plugin-` prefix stripped).
//! 5. `description` field is present.
//! 6. `description` length is ≤ 1024 chars.
//!
//! Reference: <https://agentskills.io/specification>

use anyhow::{Context, Result};
use std::path::Path;

/// Whether a `*.skill.md` path is a test fixture and must be skipped by the
/// gate. Fixtures live under `tests/` (e.g. `tests/fixtures/noop-skill/`) and
/// are intentionally minimal — they cannot satisfy the crate-name==skill-name
/// rule (their crate dir is the host crate, not a `vox-plugin-<skill>` crate).
fn is_fixture_path(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::Normal(p) if p == "tests" || p == "fixtures"))
}

/// Regex-free name validation: `^[a-z0-9][a-z0-9-]{0,63}$`
fn is_valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Extract a top-level scalar from a frontmatter block, accepting both the
/// TOML form (`key = "value"`) and the spec-mandated YAML form (`key: value`,
/// quoted or bare). Ignores `[section]` headers. Returns the value with any
/// surrounding single/double quotes stripped.
fn extract_scalar<'a>(frontmatter: &'a str, key: &str) -> Option<&'a str> {
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        // Skip section headers like `[metadata]`
        if trimmed.starts_with('[') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest = rest.trim_start();
            // TOML `key = value` or YAML `key: value`.
            let val = match rest.strip_prefix('=').or_else(|| rest.strip_prefix(':')) {
                Some(v) => v.trim(),
                None => continue,
            };
            // Strip surrounding quotes (single or double); YAML allows bare.
            if val.len() >= 2
                && ((val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\'')))
            {
                return Some(&val[1..val.len() - 1]);
            }
            return Some(val);
        }
    }
    None
}

/// Parse the frontmatter block from the start of `content`.
/// Returns `Some(frontmatter_body)` if a `---\n…\n---` block is present.
fn parse_frontmatter(content: &str) -> Option<&str> {
    let content = content.trim_start_matches('\u{feff}'); // strip BOM
    let rest = content.strip_prefix("---")?;
    // Allow `---` immediately followed by newline or windows `---\r\n`
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    // Find the closing `---`
    let close = rest.find("\n---")?;
    Some(&rest[..close])
}

pub fn run() -> Result<()> {
    let mut errors: Vec<String> = Vec::new();
    let mut checked = 0usize;

    let crates_root = Path::new("crates");
    if !crates_root.is_dir() {
        println!("✓ no crates/ dir; nothing to check");
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(crates_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_str()
                    .map(|n| n.ends_with(".skill.md"))
                    .unwrap_or(false)
        })
    {
        let path = entry.path();

        // Skip test fixtures — deliberately minimal, not real plugin skills.
        if is_fixture_path(path) {
            continue;
        }

        // Only check skill.md files that live inside a `vox-plugin-*` crate.
        // Walk up to find the crate dir (the one whose parent is `crates/`).
        let crate_dir = {
            let mut ancestor = path.parent();
            let mut found = None;
            while let Some(p) = ancestor {
                if p.parent().map(|pp| pp == crates_root).unwrap_or(false) {
                    found = Some(p);
                    break;
                }
                ancestor = p.parent();
            }
            match found {
                Some(d) => d,
                None => continue, // not under crates/ directly
            }
        };

        let crate_name = crate_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Only enforce for `vox-plugin-*` crates.
        if !crate_name.starts_with("vox-plugin-") {
            continue;
        }

        // The expected `name` value: strip the `vox-plugin-` prefix.
        let expected_name = crate_name.strip_prefix("vox-plugin-").unwrap_or(crate_name);

        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        // 1. Frontmatter must exist.
        let frontmatter = match parse_frontmatter(&content) {
            Some(fm) => fm,
            None => {
                errors.push(format!(
                    "{}:no frontmatter block (--- ... ---) found",
                    path.display()
                ));
                continue;
            }
        };

        // 2. `name` must be present.
        let name = match extract_scalar(frontmatter, "name") {
            Some(n) => n,
            None => {
                errors.push(format!("{}:missing `name` field", path.display()));
                checked += 1;
                continue;
            }
        };

        // 3. `name` format validation.
        if !is_valid_name(name) {
            errors.push(format!(
                "{}:`name` = {:?} does not match ^[a-z0-9][a-z0-9-]{{0,63}}$",
                path.display(),
                name
            ));
        }

        // 4. `name` must match expected directory short-name.
        if name != expected_name {
            errors.push(format!(
                "{}:`name` = {:?} does not match crate directory short-name {:?}",
                path.display(),
                name,
                expected_name
            ));
        }

        // 5. `description` must be present.
        let description = match extract_scalar(frontmatter, "description") {
            Some(d) => d,
            None => {
                errors.push(format!("{}:missing `description` field", path.display()));
                checked += 1;
                continue;
            }
        };

        // 6. `description` length ≤ 1024 chars (the spec limit is characters,
        //    not bytes — count code points so multibyte text isn't over-counted).
        let desc_chars = description.chars().count();
        if desc_chars > 1024 {
            errors.push(format!(
                "{}:`description` is {} chars (max 1024)",
                path.display(),
                desc_chars
            ));
        }

        checked += 1;
    }

    if errors.is_empty() {
        println!(
            "✓ agentskills-compliance ok ({} skill files checked)",
            checked
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("✗ {e}");
        }
        anyhow::bail!(
            "agentskills-compliance failed with {} error(s)",
            errors.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names() {
        assert!(is_valid_name("skill-compiler"));
        assert!(is_valid_name("noop-skill"));
        assert!(is_valid_name("a"));
        assert!(is_valid_name("skill-testing-validate"));
    }

    #[test]
    fn invalid_names() {
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("Skill-Compiler")); // uppercase
        assert!(!is_valid_name("-skill")); // leading hyphen
        assert!(!is_valid_name("skill_compiler")); // underscore
        assert!(!is_valid_name(&"a".repeat(65))); // too long
    }

    #[test]
    fn parse_frontmatter_basic() {
        let content = "---\nname = \"foo\"\n---\n# rest";
        let fm = parse_frontmatter(content).expect("frontmatter");
        assert_eq!(extract_scalar(fm, "name"), Some("foo"));
    }

    #[test]
    fn parse_frontmatter_missing() {
        let content = "# no frontmatter\n";
        assert!(parse_frontmatter(content).is_none());
    }

    #[test]
    fn extract_scalar_ignores_sections() {
        let fm = "name = \"x\"\n[metadata]\nname = \"y\"";
        // First `name` before any section
        assert_eq!(extract_scalar(fm, "name"), Some("x"));
    }

    #[test]
    fn extract_scalar_handles_yaml_form() {
        // agentskills.io mandates YAML frontmatter; the gate must read it.
        let fm = "name: brainstorming\ndescription: Socratic design refinement";
        assert_eq!(extract_scalar(fm, "name"), Some("brainstorming"));
        assert_eq!(
            extract_scalar(fm, "description"),
            Some("Socratic design refinement")
        );
    }

    #[test]
    fn extract_scalar_handles_quoted_yaml() {
        let fm = "name: \"skill-x\"\ndescription: 'has: a colon'";
        assert_eq!(extract_scalar(fm, "name"), Some("skill-x"));
        assert_eq!(extract_scalar(fm, "description"), Some("has: a colon"));
    }

    #[test]
    fn fixture_paths_are_skipped() {
        assert!(is_fixture_path(Path::new(
            "crates/vox-plugin-host/tests/fixtures/noop-skill/noop.skill.md"
        )));
        assert!(is_fixture_path(Path::new("crates/foo/tests/bar.skill.md")));
        assert!(!is_fixture_path(Path::new(
            "crates/vox-plugin-skill-compiler/compiler.skill.md"
        )));
    }

    #[test]
    fn yaml_skill_passes_name_and_description_checks() {
        let content = "---\nname: brainstorming\ndescription: Explore intent before building\n---\n\n# Body\n";
        let fm = parse_frontmatter(content).expect("frontmatter");
        let name = extract_scalar(fm, "name").expect("name");
        assert!(is_valid_name(name));
        assert!(extract_scalar(fm, "description").is_some());
    }
}
