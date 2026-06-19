//! Deterministic (offline) review checks over a parsed skill manifest + body.

use vox_plugin_types::skill_manifest::SkillManifest;
use vox_skill_discovery::{dedup_skills, validate_ssot, DiscoverOptions};

use crate::model::{ReviewItem, Severity};

/// Frontmatter completeness: name + description required and non-trivial.
pub fn check_frontmatter(m: &SkillManifest, out: &mut Vec<ReviewItem>) {
    if m.name.trim().is_empty() {
        out.push(ReviewItem {
            severity: Severity::Error,
            rule: "frontmatter/missing-name".into(),
            message: "skill has no name".into(),
        });
    }
    if m.description.trim().len() < 16 {
        out.push(ReviewItem {
            severity: Severity::Error,
            rule: "frontmatter/weak-description".into(),
            message: "description is missing or too short (< 16 chars) to be discoverable".into(),
        });
    }
}

/// Stub / placeholder detection over the skill body text.
pub fn check_stub(body: &str, out: &mut Vec<ReviewItem>) {
    const MARKERS: &[&str] = &[
        "TODO",
        "FIXME",
        "PLACEHOLDER",
        "coming soon",
        "fill in",
        "lorem ipsum",
        "<your ",
    ];
    let lower = body.to_lowercase();
    for marker in MARKERS {
        if lower.contains(&marker.to_lowercase()) {
            out.push(ReviewItem {
                severity: Severity::Error,
                rule: "stub/placeholder".into(),
                message: format!("body contains placeholder marker `{marker}` — finish the skill before publishing"),
            });
        }
    }
    if body.trim().len() < 80 {
        out.push(ReviewItem {
            severity: Severity::Warn,
            rule: "stub/thin-body".into(),
            message: "skill body is very short (< 80 chars); likely incomplete".into(),
        });
    }
}

/// Flag declared MCP tools that don't exist in the registry (reuses the discovery engine).
pub fn check_ssot(candidate: &SkillManifest, out: &mut Vec<ReviewItem>) {
    for c in validate_ssot(std::slice::from_ref(candidate)) {
        out.push(ReviewItem {
            severity: Severity::Error,
            rule: "ssot/unknown-tool".into(),
            message: c.suggested_action,
        });
    }
}

/// Flag the candidate as a near-duplicate of an already-installed skill.
pub fn check_dedup(
    candidate: &SkillManifest,
    installed: &[SkillManifest],
    out: &mut Vec<ReviewItem>,
) {
    if installed.is_empty() {
        return;
    }
    let mut all: Vec<SkillManifest> = installed.to_vec();
    all.push(candidate.clone());
    let opts = DiscoverOptions {
        shingle_k: 2,
        ..DiscoverOptions::default()
    };
    for c in dedup_skills(&all, &opts) {
        if c.members.iter().any(|m| m == &candidate.id) {
            let others: Vec<&String> = c.members.iter().filter(|m| *m != &candidate.id).collect();
            out.push(ReviewItem {
                severity: Severity::Warn,
                rule: "dedup/duplicates-installed".into(),
                message: format!(
                    "near-duplicate of installed skill(s): {others:?} — consider reusing instead of publishing"
                ),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_plugin_types::skill_manifest::SkillCategory;

    fn manifest(name: &str, desc: &str) -> SkillManifest {
        SkillManifest::new(
            "x.test",
            name,
            "0.1.0",
            "tester",
            desc,
            SkillCategory::Unknown,
        )
    }

    #[test]
    fn flags_weak_description() {
        let mut v = Vec::new();
        check_frontmatter(&manifest("good name", "short"), &mut v);
        assert!(v.iter().any(|i| i.rule == "frontmatter/weak-description"));
    }

    #[test]
    fn flags_placeholder_body() {
        let mut v = Vec::new();
        check_stub("This skill will TODO: implement the thing later.", &mut v);
        assert!(v.iter().any(|i| i.rule == "stub/placeholder"));
    }

    #[test]
    fn clean_skill_has_no_findings() {
        let mut v = Vec::new();
        check_frontmatter(
            &manifest(
                "Format Vox",
                "Formats Vox source files using the standard style and reports diffs.",
            ),
            &mut v,
        );
        check_stub(
            "A complete, well-described skill body that explains exactly what to do and how, with enough detail to be useful.",
            &mut v,
        );
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn ssot_flags_phantom_tool() {
        let mut m = manifest(
            "tool skill",
            "Declares a tool that does not exist in the registry at all.",
        );
        m.tools = vec!["vox_totally_made_up_tool".to_string()];
        let mut v = Vec::new();
        check_ssot(&m, &mut v);
        assert!(v.iter().any(|i| i.rule == "ssot/unknown-tool"));
    }

    #[test]
    fn dedup_flags_duplicate_of_installed() {
        let installed = vec![{
            let mut m = manifest(
                "format vox",
                "Formats vox source files with the standard style and reports diffs.",
            );
            m.id = "installed.fmt".into();
            m
        }];
        let mut cand = manifest(
            "format vox",
            "Formats vox source files with the standard style and reports diffs.",
        );
        cand.id = "candidate.fmt".into();
        let mut v = Vec::new();
        check_dedup(&cand, &installed, &mut v);
        assert!(v.iter().any(|i| i.rule == "dedup/duplicates-installed"));
    }
}
