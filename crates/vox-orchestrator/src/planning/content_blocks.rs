//! Typed content blocks for structured plan rendering.
//!
//! `ContentBlock` is the SSOT for the machine-readable plan payload. Both the
//! MCP `PlanResult` response and the CLI daemon output enrich their payloads
//! with a `content_blocks: Vec<ContentBlock>` field alongside the legacy
//! `plan_md: String` (kept for backward compatibility).
//!
//! ## Why here?
//!
//! `vox-mcp` already depends on `vox-orchestrator` (`Cargo.toml:17`). Defining
//! the type here avoids duplication and ensures both the CLI and the webview
//! receive a single canonical schema.

use serde::{Deserialize, Serialize};

/// A single typed unit of plan output for structured rendering.
///
/// Consumers that speak Markdown may fall back to the sibling `plan_md` field
/// and ignore `content_blocks` entirely — the field is `skip_serializing_if =
/// Vec::is_empty` so wire size is unchanged for existing clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentBlock {
    /// A paragraph of prose text. Render with whitespace breathing room.
    Prose {
        /// The paragraph text (may contain inline Markdown spans).
        text: String,
    },
    /// A fenced code block. CLI renders via `render_code_block`; webview via shiki.
    Code {
        /// Language hint (e.g. `"rust"`, `"vox"`, `"bash"`).
        lang: String,
        /// The raw source text inside the fence (no surrounding backticks).
        text: String,
    },
    /// One row in a structured task list. CLI renders via `OutputTable`.
    TaskItem {
        /// Monotonic 1-based task index.
        id: usize,
        /// Lifecycle status (`pending`, `in_progress`, `completed`, `failed`, …).
        status: String,
        /// Short imperative task description.
        description: String,
        /// Heuristic difficulty 1-10.
        complexity: u8,
    },
    /// A clarifying question from plan adequacy analysis.
    /// CLI renders via `render_human_prompt`; webview renders as an amber callout.
    Question {
        /// The question text.
        text: String,
    },
}

// ── Parser ────────────────────────────────────────────────────────────────────

/// Split a Markdown plan document into typed [`ContentBlock`]s.
///
/// Code fences become [`ContentBlock::Code`], numbered task-list rows become
/// [`ContentBlock::TaskItem`], everything else becomes [`ContentBlock::Prose`].
///
/// The parse is deliberately simple: a single linear scan, no AST. Edge cases
/// in the Markdown are handled gracefully (unclosed fences, malformed numbering)
/// by falling back to `Prose`.
pub fn markdown_to_content_blocks(md: &str) -> Vec<ContentBlock> {
    let mut blocks: Vec<ContentBlock> = Vec::new();
    let mut in_fence = false;
    let mut fence_lang = String::new();
    let mut fence_lines: Vec<&str> = Vec::new();
    let mut prose_lines: Vec<&str> = Vec::new();

    for line in md.lines() {
        let trimmed = line.trim_start();

        // Fence toggle
        if let Some(rest) = trimmed.strip_prefix("```") {
            if !in_fence {
                flush_prose(&mut prose_lines, &mut blocks);
                in_fence = true;
                fence_lang = rest.trim().to_string();
                fence_lines.clear();
            } else {
                in_fence = false;
                let body = fence_lines.join("\n");
                blocks.push(ContentBlock::Code {
                    lang: if fence_lang.is_empty() {
                        "text".to_string()
                    } else {
                        fence_lang.clone()
                    },
                    text: body,
                });
                fence_lines.clear();
                fence_lang.clear();
            }
            continue;
        }

        if in_fence {
            fence_lines.push(line);
            continue;
        }

        // Numbered task-list item: `N. **description** — [files: …] [complexity: K/10]`
        // Pattern: starts with digit(s) followed by `". "`
        if let Some(task) = try_parse_task_item(line) {
            flush_prose(&mut prose_lines, &mut blocks);
            blocks.push(task);
            continue;
        }

        // Everything else: prose
        prose_lines.push(line);
    }

    // Flush any trailing unclosed fence or prose
    if in_fence && !fence_lines.is_empty() {
        blocks.push(ContentBlock::Code {
            lang: if fence_lang.is_empty() {
                "text".to_string()
            } else {
                fence_lang
            },
            text: fence_lines.join("\n"),
        });
    } else {
        flush_prose(&mut prose_lines, &mut blocks);
    }

    blocks
}

/// Flush accumulated prose lines into a single `Prose` block (skipping blank-only runs).
fn flush_prose(lines: &mut Vec<&str>, blocks: &mut Vec<ContentBlock>) {
    let text = lines.join("\n");
    let trimmed = text.trim();
    if !trimmed.is_empty() {
        blocks.push(ContentBlock::Prose {
            text: trimmed.to_string(),
        });
    }
    lines.clear();
}

/// Try to parse a structured numbered task-list row.
///
/// Expected format (from `plan_goal` in `plan.rs`):
/// ```text
/// N. **description** — [files: path1, path2] [complexity: K/10][depends: …]
/// ```
fn try_parse_task_item(line: &str) -> Option<ContentBlock> {
    let trimmed = line.trim();

    // Find leading number + `. `
    let dot_pos = trimmed.find(". ")?;
    let num_str = &trimmed[..dot_pos];
    // Ensure it's all digits
    if !num_str.chars().all(|c| c.is_ascii_digit()) || num_str.is_empty() {
        return None;
    }
    let id: usize = num_str.parse().ok()?;

    let after_num = &trimmed[dot_pos + 2..];

    // Extract description: strip `**…**` markdown bold
    let desc_raw = if let Some(rest) = after_num.strip_prefix("**") {
        let end = rest.find("**").unwrap_or(rest.len());
        &rest[..end]
    } else {
        // Fall back to text before ` — `
        after_num.split(" — ").next().unwrap_or(after_num)
    };

    // Extract complexity from `[complexity: K/10]`
    let complexity: u8 = after_num
        .find("[complexity: ")
        .and_then(|s| {
            let rest = &after_num[s + 13..];
            let end = rest.find('/')?;
            rest[..end].trim().parse().ok()
        })
        .unwrap_or(5);

    Some(ContentBlock::TaskItem {
        id,
        status: "pending".to_string(),
        description: desc_raw.trim().to_string(),
        complexity,
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn task_item_blocks(blocks: &[ContentBlock]) -> Vec<&ContentBlock> {
        blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::TaskItem { .. }))
            .collect()
    }

    fn code_blocks(blocks: &[ContentBlock]) -> Vec<&ContentBlock> {
        blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::Code { .. }))
            .collect()
    }

    #[test]
    fn splits_code_fence_from_prose() {
        let md = "## Plan\n\nSome prose.\n\n```rust\nfn foo() {}\n```\n\nMore prose.";
        let blocks = markdown_to_content_blocks(md);
        assert_eq!(code_blocks(&blocks).len(), 1, "expected one code block");
        let prose: Vec<_> = blocks
            .iter()
            .filter(|b| matches!(b, ContentBlock::Prose { .. }))
            .collect();
        assert!(!prose.is_empty(), "expected prose blocks");
    }

    #[test]
    fn task_item_parsed_from_numbered_list() {
        let md = "1. **Implement auth module** — [files: crates/vox-auth/src/lib.rs] [complexity: 7/10]\n\
                  2. **Write regression tests** — [files: ] [complexity: 4/10]";
        let blocks = markdown_to_content_blocks(md);
        let tasks = task_item_blocks(&blocks);
        assert_eq!(tasks.len(), 2, "expected 2 task items, got {}", tasks.len());
        if let ContentBlock::TaskItem { id, complexity, .. } = tasks[0] {
            assert_eq!(*id, 1);
            assert_eq!(*complexity, 7);
        }
    }

    #[test]
    fn unclosed_fence_does_not_panic() {
        let md = "Some text\n```rust\nfn main() {";
        let blocks = markdown_to_content_blocks(md);
        // Should produce at least one block without panic
        assert!(!blocks.is_empty());
    }

    #[test]
    fn empty_input_produces_no_blocks() {
        assert!(markdown_to_content_blocks("").is_empty());
        assert!(markdown_to_content_blocks("   \n\n   ").is_empty());
    }
}
