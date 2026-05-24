use vox_compiler::lexer::lex;
use vox_compiler::parser::{ParseErrorClass, parse};

// TASK-2.6 (commit 080b3f86) restored `actor`, `workflow`, and `activity` as parseable
// bare-keyword blocks; they no longer produce parser-level tombstone errors. ADR-041
// (2026-05-23, supersedes ADR-028) confirms these keywords as public-grammar features
// backed by a real durable runtime — the pipeline-level reservation gate that briefly
// rejected them has also been removed. The acceptance contract now lives in
// `pipeline::tests::test_accept_*_adr041`.
#[test]
#[ignore = "TASK-2.6 / ADR-041: `actor` parses and is a stable grammar feature; see test_accept_*_adr041 — owner: compiler sunset: 2026-12-31"]
fn actor_is_tombstoned() {
    let src = "actor MyActor {}";
    let tokens = lex(src);
    let errs = parse(tokens).expect_err("expected parse failure for actor");
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned));
    assert!(errs[0].message.contains("actor"));
    assert!(errs[0].message.contains("tombstoned"));
}

#[test]
#[ignore = "TASK-2.6 / ADR-041: `workflow` parses and is a stable grammar feature; see test_accept_*_adr041 — owner: compiler sunset: 2026-12-31"]
fn workflow_is_tombstoned() {
    let src = "workflow MyWorkflow {}";
    let tokens = lex(src);
    let errs = parse(tokens).expect_err("expected parse failure for workflow");
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned));
}

#[test]
fn at_component_is_tombstoned() {
    let src = "@component fn Legacy() {}";
    let tokens = lex(src);
    let errs = parse(tokens).expect_err("expected parse failure for @component");
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned));
}

#[test]
fn http_is_tombstoned() {
    let src = "http get \"/\"";
    let tokens = lex(src);
    let errs = parse(tokens).expect_err("expected parse failure for http");
    assert!(errs.iter().any(|e| e.class == ParseErrorClass::Tombstoned));
}
