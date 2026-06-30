//! Compose a spec-valid `SKILL.md` from primitive inputs (SP-4 skill authoring).

/// Lowercase, collapse non-`[a-z0-9]` runs to single hyphens, trim/dedupe hyphens.
/// Guarantees the result passes `validate_skill_name` for any input (falls back
/// to `"skill"` if nothing survives).
fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_hyphen = true; // suppress leading hyphen
    for ch in name.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_hyphen = false;
        } else if !prev_hyphen {
            out.push('-');
            prev_hyphen = true;
        }
    }
    let trimmed: String = out.trim_matches('-').chars().take(64).collect();
    let trimmed = trimmed.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "skill".to_string()
    } else {
        trimmed
    }
}

/// Build a TOML-frontmatter `SKILL.md`. `steps` render as a numbered list of
/// inline-code tokens; an empty list yields a valid file with no steps.
pub fn author_skill_md(name: &str, description: &str, steps: &[String]) -> String {
    let name = kebab(name);
    let desc = description.replace('"', "'"); // keep the TOML string valid + single-line
    let steps_md = if steps.is_empty() {
        "_No individual steps were captured._".to_string()
    } else {
        steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. `{}`", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "---\n\
name = \"{name}\"\n\
description = \"{desc}\"\n\
\n\
[metadata]\n\
\"vox-author\" = \"vox-skill-discovery\"\n\
\"vox-category\" = \"workflow\"\n\
\"vox-tags\" = [\"auto-discovered\", \"operations\"]\n\
---\n\
\n\
# {name}\n\
\n\
{desc}\n\
\n\
## Steps\n\
\n\
{steps_md}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_parser::parse_skill_md;
    use crate::user_install::validate_skill_name;

    #[test]
    fn authored_skill_round_trips_and_is_valid() {
        let md = author_skill_md(
            "Read Edit Run!!",
            "Recurring procedure: read → edit → run (seen 4× across 2 sessions)",
            &["read".into(), "edit".into(), "run".into()],
        );
        let parsed = parse_skill_md(&md).expect("authored SKILL.md must parse");
        assert_eq!(parsed.manifest.name, "read-edit-run");
        assert!(validate_skill_name(&parsed.manifest.name).is_ok());
        assert!(md.contains("1. `read`"));
        assert!(md.contains("3. `run`"));
    }

    #[test]
    fn empty_steps_still_valid() {
        let md = author_skill_md("proc", "desc", &[]);
        let parsed = parse_skill_md(&md).expect("must parse");
        assert_eq!(parsed.manifest.name, "proc");
    }
}
