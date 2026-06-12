//! Skill disclosure for the chat system prompt, following the agentskills.io
//! three-tier progressive-disclosure model:
//!
//! * **Tier 1** ([`render_skill_catalog`]): name + description only (~100
//!   tokens/skill), injected for every chat turn so the model knows which
//!   skills exist and when each applies. Works for prompt-only models (MENS)
//!   that cannot call tools.
//! * **Tier 2** (the `vox_skill_use` tool, separate): full SKILL.md body loaded
//!   on demand by tool-calling models.
//! * **Pinned** ([`render_pinned_skill`]): when the user explicitly selects a
//!   skill, its full body is injected directly — no tool round-trip — so even
//!   the prompt-only MENS path honors it.
//!
//! Both renderers are pure and content-stable (alphabetical, fixed caps) so
//! they never bust the DeepSeek/Anthropic prompt-prefix cache that
//! `build_system_prompt` is careful to preserve.

/// A tier-1 catalog entry: just the spec-required name + description.
pub(crate) struct CatalogEntry {
    pub name: String,
    pub description: String,
}

/// Max chars of a description rendered in the tier-1 catalog.
const DESC_CAP: usize = 256;
/// Max bytes of a pinned skill body injected directly.
const PINNED_BODY_CAP: usize = 32 * 1024;

/// Render the `## Skills` system-prompt section. Alphabetical by name and
/// length-capped so the section is content-stable across turns (cache-safe).
/// Empty input → empty string (no section, no cache churn).
pub(crate) fn render_skill_catalog(entries: &[CatalogEntry], max: usize) -> String {
    if entries.is_empty() || max == 0 {
        return String::new();
    }
    let mut sorted: Vec<&CatalogEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    sorted.truncate(max);
    let mut out = String::from(
        "\n\n## Skills\nInstalled skills (name — when to use). To apply one, call the \
         `vox_skill_use` tool with its name to load the full instructions, then follow them. \
         If tools are unavailable, state which skill applies and proceed by its description.\n",
    );
    for e in sorted {
        let mut d = e.description.replace('\n', " ");
        if d.chars().count() > DESC_CAP {
            d = d.chars().take(DESC_CAP).collect::<String>();
            d.push('…');
        }
        out.push_str(&format!("- {} — {}\n", e.name, d));
    }
    out
}

/// Render the `## Active skill` section for a user-pinned skill, injecting the
/// full SKILL.md body so prompt-only models honor it without a tool call.
// Wired into build_system_prompt by Track B3 (pinned-skill injection); tested
// and ready ahead of that wiring.
#[allow(dead_code)]
pub(crate) fn render_pinned_skill(name: &str, body: &str) -> String {
    let mut b = body;
    if b.len() > PINNED_BODY_CAP {
        // Truncate on a char boundary at or below the cap.
        let mut end = PINNED_BODY_CAP;
        while end > 0 && !b.is_char_boundary(end) {
            end -= 1;
        }
        b = &b[..end];
    }
    format!(
        "\n\n## Active skill: {name}\nThe user pinned this skill — follow these instructions \
         for this task:\n\n{b}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: &str, d: &str) -> CatalogEntry {
        CatalogEntry {
            name: n.into(),
            description: d.into(),
        }
    }

    #[test]
    fn renders_alphabetical_capped_catalog() {
        let txt = render_skill_catalog(
            &[
                s("zeta", "Last"),
                s("brainstorming", "Socratic design refinement"),
            ],
            64,
        );
        let b = txt.find("brainstorming").unwrap();
        let z = txt.find("zeta").unwrap();
        assert!(b < z, "alphabetical for prompt-prefix cache stability");
        assert!(txt.contains("## Skills"));
        assert!(txt.contains("vox_skill_use"));
    }

    #[test]
    fn empty_registry_renders_nothing() {
        assert_eq!(render_skill_catalog(&[], 64), "");
        assert_eq!(render_skill_catalog(&[s("a", "b")], 0), "");
    }

    #[test]
    fn caps_entry_count_and_description_length() {
        let many: Vec<CatalogEntry> = (0..100)
            .map(|i| s(&format!("skill-{i:03}"), &"x".repeat(2000)))
            .collect();
        let txt = render_skill_catalog(&many, 10);
        assert_eq!(txt.matches("\n- ").count(), 10);
        assert!(!txt.contains(&"x".repeat(300)), "descriptions truncated");
    }

    #[test]
    fn pinned_skill_section_contains_full_body() {
        let txt = render_pinned_skill("tdd", "# TDD\nRED-GREEN-REFACTOR.");
        assert!(txt.contains("## Active skill: tdd"));
        assert!(txt.contains("RED-GREEN-REFACTOR"));
        assert!(txt.contains("follow these instructions"));
    }

    #[test]
    fn pinned_body_is_capped_on_char_boundary() {
        let body = "é".repeat(40_000); // 80 KB, multibyte — must not panic
        let txt = render_pinned_skill("big", &body);
        assert!(txt.len() < PINNED_BODY_CAP + 256);
    }
}
