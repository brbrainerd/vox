//! Shared `<!-- ANCHOR: name --> ... <!-- ANCHOR_END: name -->` marker extraction.
//!
//! Used both by the `lint` module's README<->docs drift checks and by
//! `vox-cli-ci`'s `capability-snapshot` command, which pulls the `tier_table`
//! ANCHOR block out of README.md to regenerate `docs/src/reference/shipped-v0.4.md`.

/// Extract the text between the first `start_needle` and the first `end_needle`
/// that appears *after* it. Searching for `end_needle` starting from after
/// `start_idx` (rather than from the start of `content`) means a well-formed
/// document with multiple distinct marked blocks is handled correctly even if
/// an earlier, unrelated `end_needle`-shaped string appears before `start_needle`.
pub fn extract_marked_block(content: &str, start_needle: &str, end_needle: &str) -> Option<String> {
    let start_idx = content.find(start_needle)?;
    let after_start = &content[start_idx + start_needle.len()..];
    let end_idx = after_start.find(end_needle)?;
    Some(after_start[..end_idx].trim().to_string())
}

/// Extract the content of README.md's `<!-- ANCHOR: {name} --> ... <!-- ANCHOR_END: {name} -->`
/// block.
pub fn readme_anchor(readme: &str, name: &str) -> Option<String> {
    let start = format!("<!-- ANCHOR: {name} -->");
    let end = format!("<!-- ANCHOR_END: {name} -->");
    extract_marked_block(readme, &start, &end)
}
