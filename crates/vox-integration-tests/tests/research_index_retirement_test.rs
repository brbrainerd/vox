#![allow(missing_docs)]

//! Research-index is retired. Discoverability is frontmatter + Starlight sidebar.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn research_index_live_path_is_absent() {
    let live = workspace_root().join("docs/src/architecture/research-index.md");
    assert!(
        !live.exists(),
        "docs/src/architecture/research-index.md must not exist; \
         Starlight sidebar.mjs collectPages() is the browse surface. \
         If you need the last hand-curated snapshot, see \
         docs/src/archive/research-index-hand-curated-retired-2026-09.md \
         (do not ingest archive/ for new work)."
    );
}

#[test]
fn live_policy_does_not_instruct_research_index_updates() {
    let root = workspace_root();
    let policy_files = [
        "AGENTS.md",
        "docs/src/AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "crates/vox-skills/skills/superpowers/deep-research.skill.md",
        "docs/src/.well-known/llms.txt",
        "docs/src/.well-known/llms-full.txt",
        "docs/src/contributors/contributor-hub.md",
        "docs/src/contributors/documentation-governance.md",
    ];
    let banned = [
        "update `docs/src/architecture/research-index.md`",
        "update docs/src/architecture/research-index.md",
        "After writing to `docs/`, update [`docs/src/architecture/research-index.md`]",
        "After creating a new research page, update `docs/src/architecture/research-index.md`",
        "After writing, update `docs/src/architecture/research-index.md`",
        "hand-curated SSOT index",
    ];
    for rel in policy_files {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for needle in banned {
            assert!(
                !text.contains(needle),
                "{rel} still instructs research-index maintenance: {needle:?}"
            );
        }
    }
}
