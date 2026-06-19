//! Mines repeated `.vox` code blocks. v1 splits files into blank-line-delimited
//! blocks (language-agnostic, no grammar dependency). AST-based blocking via
//! tree-sitter is a documented phase-2 refinement (see the design spec).

use std::path::Path;

use vox_similarity::{tokenize, Fragment, FragmentKind, LshIndex};

use crate::candidate::{Candidate, CandidateKind, DraftFrontmatter};
use crate::options::DiscoverOptions;

/// Split text into (start_line, block_text) on blank-line boundaries.
pub(crate) fn extract_blocks(text: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 1usize;
    let mut line_no = 0usize;
    for line in text.lines() {
        line_no += 1;
        if line.trim().is_empty() {
            if !cur.trim().is_empty() {
                blocks.push((cur_start, std::mem::take(&mut cur)));
            }
            cur.clear();
            cur_start = line_no + 1;
        } else {
            if cur.is_empty() {
                cur_start = line_no;
            }
            cur.push_str(line);
            cur.push('\n');
        }
    }
    if !cur.trim().is_empty() {
        blocks.push((cur_start, cur));
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_blocks_splits_on_blank_lines() {
        let text = "line a\nline b\n\nline c\n";
        let blocks = extract_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, 1);
        assert!(blocks[0].1.contains("line a"));
        assert_eq!(blocks[1].0, 4);
        assert!(blocks[1].1.contains("line c"));
    }
}
