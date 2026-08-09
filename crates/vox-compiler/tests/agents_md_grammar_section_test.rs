//! AGENTS.md's Grammar Unification section is the always-loaded agent
//! policy surface. It must not tell agents that retired decorator spellings
//! are canonical.

use std::fs;

#[test]
fn grammar_unification_section_does_not_list_retired_decorators_as_canonical() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../AGENTS.md");
    let src = fs::read_to_string(path).unwrap();
    let section_start = src
        .find("## Grammar Unification")
        .expect("AGENTS.md must have a Grammar Unification section");
    let section_end = src[section_start..]
        .find("\n## ")
        .map(|i| section_start + i)
        .unwrap_or(src.len());
    let section = &src[section_start..section_end];

    for retired in ["`@table`", "`@query`", "`@mutation`", "`@server`"] {
        assert!(
            !section.contains(retired),
            "Grammar Unification section still lists retired decorator {retired} \
             as canonical (it is a hard parse error since 2026-06-30, cd7cc96874)"
        );
    }
    for canonical in ["`table`", "`query`", "`mutation`", "`server`"] {
        assert!(
            section.contains(canonical),
            "Grammar Unification section must list the canonical bare-keyword \
             form {canonical}"
        );
    }
}
