//! CR-META: every `[CR-*]` block in `v1-release-criteria.md` must declare
//! `verify_cmd`, `artifact_path`, and a non-empty `if_failing`. A block is the
//! text from one `[CR-...]` marker up to (but not including) the next, or EOF.

/// Returns `Ok(())` when every criterion block is well-formed, else the list
/// of human-readable violations.
pub fn check_criteria_format(doc: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let blocks = split_blocks(doc);
    if blocks.is_empty() {
        return Err(vec!["no [CR-*] criterion blocks found".to_string()]);
    }
    for (id, body) in blocks {
        for field in ["verify_cmd", "artifact_path", "if_failing"] {
            if !field_present(&body, field) {
                errors.push(format!("[{id}] missing `{field}`"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Field is present iff some line contains a backticked `field` followed
/// (after optional markup/space) by an **explicit** `:` or `·` separator and a
/// non-empty value. The explicit-separator requirement rejects prose mentions
/// like "the `if_failing` pointer" that name the field without declaring it.
fn field_present(body: &str, field: &str) -> bool {
    let needle = format!("`{field}`");
    body.lines().any(|line| {
        let Some(pos) = line.find(&needle) else {
            return false;
        };
        let after = line[pos + needle.len()..].trim_start_matches([' ', '*', '`']);
        let Some(value) = after.strip_prefix(':').or_else(|| after.strip_prefix('·')) else {
            return false;
        };
        !value.trim_start_matches([' ', '*', '`']).trim().is_empty()
    })
}

/// Split into `(id, body)` pairs keyed on `[CR-<id>]` **definition** markers
/// (see [`is_definition_marker`] for what qualifies). The id excludes the
/// surrounding brackets (e.g. `CR-F2`). Prose references such as
/// `Per **[CR-F0]**,` and blockquote notes like `> [CR-P1], ...` are skipped
/// so they don't produce false violations.
fn split_blocks(doc: &str) -> Vec<(String, String)> {
    let mut markers: Vec<(usize, String)> = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = doc[search_from..].find("[CR-") {
        let start = search_from + rel;
        let Some(end_rel) = doc[start..].find(']') else {
            break;
        };
        let id = doc[start + 1..start + end_rel].to_string(); // "CR-F2"
        if is_definition_marker(doc, start) {
            markers.push((start, id));
        }
        search_from = start + end_rel + 1;
    }
    let mut out = Vec::with_capacity(markers.len());
    for i in 0..markers.len() {
        let (start, ref id) = markers[i];
        let end = markers.get(i + 1).map(|(s, _)| *s).unwrap_or(doc.len());
        out.push((id.clone(), doc[start..end].to_string()));
    }
    out
}

/// True when the `[CR-...]` marker at `pos` is a criterion **definition**:
/// it is **bold** (immediately preceded by `**`) AND **line-leading** (only
/// markdown prefix chars — `-`/`#`/`*`/space/tab — precede the bold open on
/// its line). This excludes:
/// - mid-sentence prose refs (`Per **[CR-F0]**,`) — fail line-leading,
/// - blockquote dependency notes (`> [CR-P1], ...`) — fail bold (no `**`).
fn is_definition_marker(doc: &str, pos: usize) -> bool {
    let before = doc[..pos].trim_end_matches([' ', '\t']);
    if !before.ends_with("**") {
        return false;
    }
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    // The chars between line start and the bold open must be markdown prefix.
    let prefix = &before[line_start..before.len() - 2];
    prefix
        .chars()
        .all(|c| matches!(c, '-' | '#' | '*' | '>' | ' ' | '\t'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_blocks_finds_each_marker() {
        let doc = "**[CR-A] a.** x\n**[CR-B] b.** y\n";
        let blocks = split_blocks(doc);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "CR-A");
        assert_eq!(blocks[1].0, "CR-B");
    }

    #[test]
    fn prose_references_are_not_treated_as_definitions() {
        // `[CR-F0]` mid-sentence and `(harness = [CR-L4])` must NOT become
        // blocks. Only the line-leading definition counts.
        let doc = "Per **[CR-F0]**, the snapshot must not be green (see [CR-L4]).\n\
                   - **[CR-F0] Foundation-first ordering.** \
                   `verify_cmd`: `x` `artifact_path`: `y` `if_failing`: z\n";
        let blocks = split_blocks(doc);
        assert_eq!(
            blocks.len(),
            1,
            "exactly one definition block; got {blocks:?}"
        );
        assert_eq!(blocks[0].0, "CR-F0");
        assert!(check_criteria_format(doc).is_ok());
    }

    #[test]
    fn field_present_requires_value() {
        assert!(field_present("`verify_cmd`: `cargo test`", "verify_cmd"));
        assert!(field_present("- `if_failing` · do the thing", "if_failing"));
        assert!(!field_present("`verify_cmd`:\n", "verify_cmd"));
        assert!(!field_present("no field here", "verify_cmd"));
    }

    #[test]
    fn field_present_rejects_prose_mention_without_separator() {
        // A prose reference that names the field but has no `:`/`·` separator
        // must NOT count as a declaration.
        assert!(!field_present(
            "see the `if_failing` pointer for the next fixture to build",
            "if_failing"
        ));
        // And a full block missing `if_failing` (only prose-mentioning it)
        // fails the whole check.
        let doc = "\
**[CR-Z] Missing.** The `if_failing` field is described here in prose.
- `verify_cmd`: `cargo test`
- `artifact_path`: `contracts/reports/z/<UTC>.json`
";
        let errs = check_criteria_format(doc).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| e.contains("CR-Z") && e.contains("if_failing"))
        );
    }
}
