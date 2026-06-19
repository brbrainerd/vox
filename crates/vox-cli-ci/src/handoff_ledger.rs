//! `vox ci handoff-ledger` — validates the append-only Antigravity handoff ledger
//! (`docs/superpowers/antigravity-handoff-ledger.md`) against its documented schema.
//! Dependency-free: the ledger entries are semi-structured YAML-ish blocks; we
//! validate key presence and enum values line-by-line (no YAML dependency).

use std::path::Path;

/// A schema violation in a ledger entry.
#[derive(Debug, PartialEq)]
pub struct LedgerViolation {
    pub entry: String, // AGH-NNNN or "(unidentified block)"
    pub reason: String,
}

/// Default ledger path relative to the workspace root.
pub const LEDGER_PATH: &str = "docs/superpowers/antigravity-handoff-ledger.md";

/// The reserved id used by the copy-me schema template block in the ledger header.
/// Real entries never use it, and the lint skips it (otherwise the gate would
/// reject the documentation template itself).
pub(crate) const TEMPLATE_ID: &str = "AGH-NNNN";

/// Top-level keys every entry must declare.
const REQUIRED_KEYS: &[&str] = &[
    "id",
    "date",
    "plan",
    "prompt_version",
    "subsystem",
    "target",
    "outcome",
];

const VALID_OUTCOME: &[&str] = &["green", "partial", "failed"];
const VALID_VERDICT: &[&str] = &["approve", "approve-with-followups", "request-changes"];
/// Fixed failure-category vocabulary (must match the ledger header §C `category` vocab).
const VALID_CATEGORY: &[&str] = &[
    "hallucinated-api",
    "wrong-path",
    "wrong-crate",
    "arch-check-gate",
    "fmt-gate",
    "build-gate",
    "branch-hygiene",
    "scope-creep",
    "already-done",
    "perf",
    "robustness",
    "test-hygiene",
    "unplanned-shared-change",
    "ssot-fork",
    "unit-mismatch",
];

/// Extract the text of each ```yaml fenced block that is a real entry — i.e. it
/// contains an `id: AGH-` line whose value is NOT the `AGH-NNNN` template sentinel.
/// Returns each block's inner text (without the ``` fences).
pub(crate) fn extract_entry_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut cur = String::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if !in_fence && (trimmed == "```yaml" || trimmed == "```yml") {
            in_fence = true;
            cur.clear();
            continue;
        }
        if in_fence && trimmed == "```" {
            in_fence = false;
            let is_entry = cur.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with("id: AGH-") && t.trim() != format!("id: {TEMPLATE_ID}")
            });
            if is_entry {
                blocks.push(std::mem::take(&mut cur));
            }
            cur.clear();
            continue;
        }
        if in_fence {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    blocks
}

/// Return the value of a `key:` line in a block, if present.
fn field<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    block.lines().find_map(|l| {
        let l = l.trim_start();
        l.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))
            .map(|v| v.trim())
    })
}

/// Validate one entry block; push any violations.
pub(crate) fn validate_block(block: &str, out: &mut Vec<LedgerViolation>) {
    let id = field(block, "id")
        .unwrap_or("(unidentified block)")
        .to_string();

    for key in REQUIRED_KEYS {
        if field(block, key).is_none() {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("missing required key `{key}`"),
            });
        }
    }

    if let Some(id_val) = field(block, "id") {
        // AGH-NNNN where NNNN is 4 digits
        let ok = id_val
            .strip_prefix("AGH-")
            .map(|n| n.len() == 4 && n.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false);
        if !ok {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("id `{id_val}` must match AGH-NNNN (4 digits)"),
            });
        }
    }
}

/// Validate enum-valued fields in a block.
pub(crate) fn validate_enums(block: &str, out: &mut Vec<LedgerViolation>) {
    let id = field(block, "id")
        .unwrap_or("(unidentified block)")
        .to_string();

    if let Some(o) = field(block, "outcome") {
        if !VALID_OUTCOME.contains(&o) {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("outcome `{o}` not in {VALID_OUTCOME:?}"),
            });
        }
    }
    if let Some(v) = field(block, "verdict") {
        if !VALID_VERDICT.contains(&v) {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("verdict `{v}` not in {VALID_VERDICT:?}"),
            });
        }
    }
    // every `category: X` line or inline `category: "X"` must be known
    for line in block.lines() {
        if let Some(idx) = line.find("category:") {
            let rest = &line[idx + "category:".len()..];
            let val = rest
                .split(',')
                .next()
                .unwrap_or(rest)
                .trim()
                .trim_matches('}')
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim();
            if !VALID_CATEGORY.contains(&val) {
                out.push(LedgerViolation {
                    entry: id.clone(),
                    reason: format!("category `{val}` not in the fixed vocab"),
                });
            }
        }
    }
}

/// Validate the whole ledger file at `workspace_root/LEDGER_PATH`.
pub fn run(workspace_root: &Path) -> anyhow::Result<Vec<LedgerViolation>> {
    let path = workspace_root.join(LEDGER_PATH);
    let md = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    let blocks = extract_entry_blocks(&md);
    // ids must be unique
    let mut seen = std::collections::HashSet::new();
    for block in &blocks {
        validate_block(block, &mut out);
        validate_enums(block, &mut out);
        if let Some(id) = field(block, "id") {
            if !seen.insert(id.to_string()) {
                out.push(LedgerViolation {
                    entry: id.to_string(),
                    reason: "duplicate id".into(),
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_id_bearing_yaml_blocks() {
        let md = "intro\n```yaml\nid: AGH-0001\noutcome: green\n```\nprose\n```yaml\nkey: not-an-entry\n```\n";
        let blocks = extract_entry_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("AGH-0001"));
    }

    #[test]
    fn skips_the_schema_template_block() {
        // The ledger header contains a copy-me template with `id: AGH-NNNN`;
        // the lint must NOT treat it as a real entry (else it fails on its own doc).
        let md = "```yaml\nid: AGH-NNNN\noutcome: green | partial | failed\n```\n```yaml\nid: AGH-0001\noutcome: green\n```\n";
        let blocks = extract_entry_blocks(md);
        assert_eq!(blocks.len(), 1, "template block must be skipped");
        assert!(blocks[0].contains("AGH-0001"));
    }

    #[test]
    fn flags_missing_required_key() {
        let mut v = Vec::new();
        validate_block("id: AGH-0001\noutcome: green\n", &mut v);
        assert!(
            v.iter()
                .any(|x| x.reason.contains("missing required key `plan`"))
        );
    }

    #[test]
    fn flags_bad_id_format() {
        let mut v = Vec::new();
        validate_block("id: AGH-1\noutcome: green\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("AGH-NNNN")));
    }

    #[test]
    fn flags_bad_outcome() {
        let mut v = Vec::new();
        validate_enums("id: AGH-0001\noutcome: maybe\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("outcome `maybe`")));
    }

    #[test]
    fn flags_unknown_category() {
        let mut v = Vec::new();
        validate_enums(
            "id: AGH-0001\n  - { what: x, category: \"made-up-cat\" }\n",
            &mut v,
        );
        assert!(v.iter().any(|x| x.reason.contains("made-up-cat")));
    }

    #[test]
    fn accepts_known_category_and_outcome() {
        let mut v = Vec::new();
        validate_enums(
            "id: AGH-0001\noutcome: green\n  - { what: x, category: \"perf\" }\n",
            &mut v,
        );
        assert!(v.is_empty());
    }
}
