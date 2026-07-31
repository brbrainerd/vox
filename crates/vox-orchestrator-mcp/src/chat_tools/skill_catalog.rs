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
//! Both renderers are pure and content-stable (fixed sort order, fixed caps)
//! so they never bust the DeepSeek/Anthropic prompt-prefix cache that
//! `build_system_prompt` is careful to preserve. Task 3.1: the tier-1 sort
//! ranks by `reliability_scores` (`entity_type = 'skill'`) descending when
//! present, alphabetical otherwise/as tiebreak — never HashMap iteration
//! order — so it stays deterministic given the same input.

/// A tier-1 catalog entry: just the spec-required name + description, plus an
/// optional reliability score (`reliability_scores` `entity_type = 'skill'`).
/// `None` means "no observations recorded yet" — this must NOT be treated as
/// a low/zero score; unproven skills rank after scored ones, not below them.
pub(crate) struct CatalogEntry {
    pub name: String,
    pub description: String,
    pub reliability: Option<f64>,
}

/// Max chars of a description rendered in the tier-1 catalog. Matches the
/// documented SKILL.md authoring standard (1,024 chars) so the "when to use
/// this" half of the description — the part that drives activation — is no
/// longer cut off. Worst case 64 skills × ~1024 chars ≈ 16k tokens of raw
/// text (less after `format!` overhead is amortized); no other documented
/// system-prompt token budget in this codebase caps this section tighter, so
/// this raise is not currently in conflict with anything else found.
const DESC_CAP: usize = 1024;
/// Max bytes of a pinned skill body injected directly.
const PINNED_BODY_CAP: usize = 32 * 1024;

/// Render the `## Skills` system-prompt section. Ranked by reliability
/// (present data first, higher score first) then alphabetically (both as a
/// tiebreak among scored skills and as the sort for all no-data skills), and
/// length-capped so the section is content-stable across turns (cache-safe).
/// Empty input → empty string (no section, no cache churn).
///
/// When `entries.len() > max`, the dropped names are logged via
/// `tracing::info!` — the cap silently dropping a late-ranked skill was the
/// original bug (Task 3.1 / finding F6a); it is no longer silent.
pub(crate) fn render_skill_catalog(entries: &[CatalogEntry], max: usize) -> String {
    if entries.is_empty() || max == 0 {
        return String::new();
    }
    let mut sorted: Vec<&CatalogEntry> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        match (a.reliability, b.reliability) {
            (Some(ra), Some(rb)) => rb
                .partial_cmp(&ra)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });
    if sorted.len() > max {
        let dropped: Vec<&str> = sorted[max..].iter().map(|e| e.name.as_str()).collect();
        tracing::info!(
            dropped_count = dropped.len(),
            dropped_skills = %dropped.join(", "),
            cap = max,
            "skill catalog truncated: skills fell off past the tier-1 cap"
        );
    }
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
            reliability: None,
        }
    }

    fn sr(n: &str, d: &str, r: f64) -> CatalogEntry {
        CatalogEntry {
            name: n.into(),
            description: d.into(),
            reliability: Some(r),
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
        assert!(b < z, "alphabetical fallback for no-data skills");
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
        assert!(!txt.contains(&"x".repeat(1100)), "descriptions truncated");
        // DESC_CAP raised to 1024: a 1000-char description must survive intact.
        let one = render_skill_catalog(&[s("only", &"y".repeat(1000))], 64);
        assert!(one.contains(&"y".repeat(1000)), "1000-char desc not cut");
        assert!(!one.contains(&"y".repeat(1001)));
    }

    #[test]
    fn reliability_ranks_above_no_data_and_higher_above_lower() {
        let txt = render_skill_catalog(
            &[
                s("zzz-no-data", "no reliability yet"),
                sr("low-rel", "scored low", 0.2),
                sr("high-rel", "scored high", 0.9),
                s("aaa-no-data", "no reliability yet either"),
            ],
            64,
        );
        let high = txt.find("high-rel").unwrap();
        let low = txt.find("low-rel").unwrap();
        let aaa = txt.find("aaa-no-data").unwrap();
        let zzz = txt.find("zzz-no-data").unwrap();
        assert!(high < low, "higher reliability ranks first");
        assert!(low < aaa, "any scored skill ranks above any no-data skill");
        assert!(low < zzz, "any scored skill ranks above any no-data skill");
        assert!(
            aaa < zzz,
            "no-data skills fall back to alphabetical among themselves"
        );
    }

    #[test]
    fn reliability_tiebreak_is_alphabetical() {
        let txt = render_skill_catalog(
            &[sr("zeta", "z", 0.5), sr("alpha", "a", 0.5)],
            64,
        );
        let a = txt.find("alpha").unwrap();
        let z = txt.find("zeta").unwrap();
        assert!(a < z, "equal reliability breaks alphabetically");
    }

    #[test]
    fn truncation_drops_lowest_ranked_first() {
        // No-data skills sort last, so with reliability data present and a
        // tight cap, the no-data skill should be the one dropped, not silent
        // — the drop is also observable via render_skill_catalog's tracing
        // (exercised indirectly here: capacity 1 keeps only the scored skill).
        let txt = render_skill_catalog(
            &[sr("kept", "has data", 0.7), s("dropped", "no data")],
            1,
        );
        assert!(txt.contains("kept"));
        assert!(!txt.contains("dropped"));
    }

    #[test]
    fn determinism_same_input_twice_is_byte_identical() {
        let entries = [
            sr("charlie", "c desc", 0.5),
            s("alpha", "a desc"),
            sr("bravo", "b desc", 0.5),
            s("delta", "d desc"),
        ];
        let first = render_skill_catalog(&entries, 64);
        let second = render_skill_catalog(&entries, 64);
        assert_eq!(first, second, "cache-stability: identical input -> identical output");
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
