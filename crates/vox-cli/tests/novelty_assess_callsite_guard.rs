//! Architecture guard: novelty scoring must go through `assess_novelty`, not raw
//! `AtomicNoveltyScorer::score` in product crates.

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

const SCAN_ROOTS: &[&str] = &[
    "crates/vox-cli/src",
    "crates/vox-gui/src",
    "crates/vox-orchestrator-mcp/src",
    "crates/vox-publisher/src",
];

const ALLOWED_SCORE_PATH_SUFFIXES: &[&str] =
    &["crates/vox-publisher/src/scientia_novelty_assess.rs"];

const REQUIRED_ASSESS_PATH_SUFFIXES: &[&str] = &[
    "crates/vox-cli/src/commands/db/publication/decision.rs",
    "crates/vox-cli/src/commands/db/publication/discovery.rs",
    "crates/vox-gui/src/commands/scientia_review.rs",
    "crates/vox-orchestrator-mcp/src/scientia_tools/novelty.rs",
];

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn novelty_scorer_score_only_in_assess_seam() {
    let root = workspace_root();
    let needle = "AtomicNoveltyScorer::score";
    let mut violations = Vec::new();

    for scan_root in SCAN_ROOTS {
        let dir = root.join(scan_root);
        let mut files = Vec::new();
        collect_rs_files(&dir, &mut files);
        for path in files {
            let rel = rel_path(&root, &path);
            if ALLOWED_SCORE_PATH_SUFFIXES
                .iter()
                .any(|allowed| rel.ends_with(allowed))
            {
                continue;
            }
            let contents = std::fs::read_to_string(&path).expect("read source");
            let has_score_callsite = contents.lines().any(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("//") && !trimmed.starts_with("*") && trimmed.contains(needle)
            });
            if has_score_callsite {
                violations.push(rel);
            }
        }
    }

    assert!(
        violations.is_empty(),
        "AtomicNoveltyScorer::score must only appear in scientia_novelty_assess.rs; found in:\n{}",
        violations.join("\n")
    );
}

#[test]
fn novelty_assessment_wired_at_product_callsites() {
    let root = workspace_root();
    let needle = "assess_novelty";
    let mut missing = Vec::new();

    for suffix in REQUIRED_ASSESS_PATH_SUFFIXES {
        let path = root.join(suffix);
        let contents = std::fs::read_to_string(&path).expect("read callsite source");
        if !contents.contains(needle) {
            missing.push(*suffix);
        }
    }

    assert!(
        missing.is_empty(),
        "assess_novelty must be called from CLI/GUI/MCP novelty surfaces; missing in:\n{}",
        missing.join("\n")
    );
}
