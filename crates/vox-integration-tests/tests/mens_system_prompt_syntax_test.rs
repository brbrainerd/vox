//! The MENS system prompt teaches Vox syntax by example. Every fenced code
//! block in it must actually parse against the current grammar — this is
//! the regression guard against the exact defect the audit found (a prompt
//! that taught a dead colon-block dialect for 4+ months undetected).

use std::fs;

fn extract_fenced_blocks(src: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            if in_block {
                blocks.push(std::mem::take(&mut current));
                in_block = false;
            } else {
                in_block = true;
            }
            continue;
        }
        if in_block {
            current.push_str(line);
            current.push('\n');
        }
    }
    blocks
}

#[test]
fn every_fenced_block_in_system_prompt_parses() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../mens/config/system_prompt.txt"
    );
    let src = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let blocks = extract_fenced_blocks(&src);
    assert!(
        blocks.len() >= 3,
        "expected at least 3 fenced code examples (actor, workflow, component), found {}",
        blocks.len()
    );
    for (i, block) in blocks.iter().enumerate() {
        let tokens = vox_compiler::lexer::lex(block);
        let result = vox_compiler::parser::parse_script(tokens);
        assert!(
            result.is_ok(),
            "fenced block {i} failed to parse:\n---\n{block}\n---\nerrors: {:?}",
            result.err()
        );
    }
}

#[test]
fn system_prompt_does_not_mention_retired_syntax() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../mens/config/system_prompt.txt"
    );
    let src = fs::read_to_string(path).unwrap();
    for retired in [
        "ret ",
        "@component fn",
        "@table type",
        "@query fn",
        "@mutation fn",
        "@server fn",
        "@mcp.tool(",
        "@action fn",
        "@agent_def fn",
        "@page fn",
        "@layout fn",
        "@hook fn",
        "@provider fn",
        "@keyframes",
    ] {
        assert!(
            !src.contains(retired),
            "system_prompt.txt still mentions retired syntax: {retired:?}"
        );
    }
}
