/// Task 2.6 — nucleo command palette fuzzy search.
use vox_term::ui::palette::{Palette, default_commands};

#[test]
fn fuzzy_mdl_matches_model() {
    let palette = Palette::new(default_commands());
    let results = palette.search("mdl");
    assert!(
        results.iter().any(|c| c.name == "/model"),
        "expected /model in results for query 'mdl', got: {results:?}",
        results = results.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}

#[test]
fn empty_query_returns_all() {
    let palette = Palette::new(default_commands());
    let cmds = default_commands();
    let results = palette.search("");
    assert_eq!(results.len(), cmds.len());
}

#[test]
fn no_match_returns_empty() {
    let palette = Palette::new(default_commands());
    let results = palette.search("zzzzzzzzz");
    assert!(
        results.is_empty(),
        "expected no results, got {results:?}",
        results = results.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
}
