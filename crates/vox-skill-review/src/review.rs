//! Orchestrates the deterministic review of a candidate SKILL.md.

use vox_plugin_host::skill_parser::parse_skill_md;
use vox_plugin_types::skill_manifest::SkillManifest;

use crate::checks::{check_dedup, check_frontmatter, check_ssot, check_stub};
use crate::model::{ReviewItem, ReviewReport, Severity, Verdict};

/// Propose advisory tags from the manifest category + salient body keywords.
fn suggest_tags(m: &SkillManifest, body: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let cat = m.category.to_string();
    if cat != "Unknown" {
        tags.push(cat.to_lowercase());
    }
    const KEYWORDS: &[&str] = &[
        "test", "git", "deploy", "format", "compile", "search", "review", "doc", "security",
    ];
    let lower = body.to_lowercase();
    for kw in KEYWORDS {
        if lower.contains(kw) && !tags.iter().any(|t| t == kw) {
            tags.push((*kw).to_string());
        }
    }
    tags
}

/// Review a candidate SKILL.md against the installed set. Deterministic + offline.
pub fn review_skill(skill_md: &str, installed: &[SkillManifest]) -> ReviewReport {
    let bundle = match parse_skill_md(skill_md) {
        Ok(b) => b,
        Err(e) => {
            return ReviewReport {
                skill_id: "(unparseable)".into(),
                items: vec![ReviewItem {
                    severity: Severity::Critical,
                    rule: "parse/invalid-skill-md".into(),
                    message: format!("SKILL.md failed to parse: {e}"),
                }],
                suggested_tags: Vec::new(),
                verdict: Verdict::NeedsHuman,
            };
        }
    };
    let m = &bundle.manifest;
    // `skill_md` is a PUBLIC FIELD on VoxSkillBundle (the full SKILL.md text:
    // frontmatter + body). It is NOT a method — `bundle.body()` does not exist.
    let body: &str = &bundle.skill_md;

    let mut items = Vec::new();
    check_frontmatter(m, &mut items);
    check_stub(body, &mut items);
    check_ssot(m, &mut items);
    check_dedup(m, installed, &mut items);

    let verdict = ReviewReport::verdict_for(&items);
    ReviewReport {
        skill_id: m.id.clone(),
        items,
        suggested_tags: suggest_tags(m, body),
        verdict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"---
name: "format-vox"
description: "Formats Vox source files using the standard style and reports the diff."
metadata:
  vox-id: "x.fmt"
  vox-version: "0.1.0"
  vox-category: "refactor"
---
This skill formats Vox source. It runs the formatter, shows a diff, and explains any changes in detail so the user can review them before applying.
"#;

    #[test]
    fn good_skill_passes() {
        let r = review_skill(GOOD, &[]);
        assert_eq!(r.verdict, Verdict::Pass, "{:?}", r.items);
        assert!(r
            .suggested_tags
            .iter()
            .any(|t| t == "refactor" || t == "format"));
    }

    #[test]
    fn placeholder_skill_needs_human() {
        let bad = GOOD.replace("This skill formats Vox source.", "TODO: write this skill.");
        let r = review_skill(&bad, &[]);
        assert_eq!(r.verdict, Verdict::NeedsHuman);
    }
}
