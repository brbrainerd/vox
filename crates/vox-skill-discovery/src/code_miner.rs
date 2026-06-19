//! Mines repeated `.vox` code blocks. v1 splits files into blank-line-delimited
//! blocks (language-agnostic, no grammar dependency). AST-based blocking via
//! tree-sitter is a documented phase-2 refinement (see the design spec).

use std::path::Path;

use vox_similarity::{tokenize, Fragment, FragmentKind, LshIndex};

use crate::candidate::{Candidate, CandidateKind, DraftFrontmatter};
use crate::options::DiscoverOptions;
use ignore::WalkBuilder;
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

/// Mine repeated `.vox` code blocks under `root`. Returns `RepeatedCode` candidates,
/// one per cluster of `>= min_occurrences` similar blocks.
pub fn mine_repeated_code(root: &Path, opts: &DiscoverOptions) -> Vec<Candidate> {
    let mut index = LshIndex::new(opts.bands, opts.rows);
    for entry in WalkBuilder::new(root).build().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(p) else {
            continue;
        };
        for (start, block) in extract_blocks(&text) {
            if tokenize(&block).len() < opts.min_tokens {
                continue;
            }
            let src = format!("{}:{}", p.display(), start);
            let frag = Fragment::new(
                src.clone(),
                FragmentKind::Code,
                block,
                src,
                opts.shingle_k,
                opts.num_hashes(),
            );
            index.insert(frag);
        }
    }

    let mut candidates = Vec::new();
    for cluster in index.cluster(opts.min_occurrences, opts.min_jaccard) {
        let members: Vec<String> = cluster
            .members
            .iter()
            .map(|&i| index.fragment(i).source_ref.clone())
            .collect();
        let score = vox_similarity::mean_pairwise_jaccard(&index, &cluster.members);
        let stem = stem_of(&members[0]);
        candidates.push(Candidate {
            kind: CandidateKind::RepeatedCode,
            members,
            score,
            suggested_action: "Extract this recurring block into a reusable Vox skill/snippet"
                .to_string(),
            draft_frontmatter: Some(DraftFrontmatter {
                name: format!("{stem}-block"),
                description: "Recurring code block detected across the repository.".to_string(),
                category: "refactor".to_string(),
                tags: vec!["auto-discovered".to_string(), "duplicate".to_string()],
            }),
        });
    }
    candidates
}

/// Best-effort file stem from a "path:line" source ref.
fn stem_of(source_ref: &str) -> String {
    let path = source_ref
        .rsplit_once(':')
        .map(|(p, _)| p)
        .unwrap_or(source_ref);
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vox")
        .to_string()
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

    #[test]
    fn mining_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "ignored/\n").unwrap();
        std::fs::create_dir_all(root.join("ignored")).unwrap();
        let body = "let subtotal = unit_price * quantity\nlet tax = subtotal * tax_rate\nlet total = subtotal + tax\n";
        // two copies in a tracked dir → a candidate; one extra in ignored/ must NOT inflate members
        std::fs::write(root.join("a.vox"), body).unwrap();
        std::fs::write(root.join("b.vox"), body).unwrap();
        std::fs::write(root.join("ignored").join("c.vox"), body).unwrap();
        let opts = DiscoverOptions {
            min_tokens: 5,
            min_occurrences: 2,
            ..DiscoverOptions::default()
        };
        let cands = mine_repeated_code(root, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].members.len(), 2, "ignored/ copy must be excluded");
    }

    #[test]
    fn mine_finds_duplicate_block_across_two_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let body = "let subtotal = unit_price * quantity\nlet tax = subtotal * tax_rate\nlet total = subtotal + tax\nreturn total\n";
        std::fs::write(root.join("a.vox"), body).unwrap();
        std::fs::write(root.join("b.vox"), body).unwrap();
        let opts = DiscoverOptions {
            min_tokens: 5,
            min_occurrences: 2,
            ..DiscoverOptions::default()
        };
        let cands = mine_repeated_code(root, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].kind, CandidateKind::RepeatedCode);
        assert_eq!(cands[0].members.len(), 2);
        assert!(cands[0].score >= 0.9);
    }
}
