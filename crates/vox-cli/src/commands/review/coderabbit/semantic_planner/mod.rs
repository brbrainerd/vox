//! Semantic stacked-PR planner for CodeRabbit — **working-tree diff edition**.
//!
//! Unlike [`super::stack_planner`] (which reads `git ls-files` and groups the whole repo),
//! this planner reads `git diff HEAD --name-status` plus untracked `git status` entries to
//! discover only what has *changed* since the last push, then groups those files by coherent
//! semantic context.
//!
//! # Workflow
//!
//! 1. **Collect** – gather every modified/deleted tracked file and every untracked new file,
//!    filtering out build artifacts, generated HTML, scratch logs, and lock files.
//! 2. **Group** – map each file path to one of the semantic groups in a deterministic,
//!    context-preserving order (e.g. all `crates/vox-typeck/` files go in the typeck PR).
//! 3. **Baseline** – push `refs/heads/cr-baseline-*` to the same commit as `origin/<default_branch>`
//!    (after `git fetch`) so every PR has a real merge base.
//! 4. **Optional `--commit-main`** – legacy path: broad `git add -u` + manifest paths, commit, push default branch.
//! 5. **Worktrees + PR** – for each semantic group, add a git worktree from `origin/<baseline>`,
//!    overlay changed files from the main working tree, commit, push `cr/review-<name>`, open PR **into baseline**.
//!
//! Each PR targets the same baseline branch (independent topology). The main checkout is not switched for chunk work.

mod collector;
mod groups;
mod manifest;
mod rules;
mod submit;
mod types;

pub use collector::{collect_all_files, collect_changed_files};
pub use rules::{SemanticRuleSet, resolve_semantic_rule_set, unassigned_prefix_histogram};
pub use submit::run_semantic_submit;
pub use types::{SemanticChunk, SemanticManifest, SemanticPlanner, SemanticSubmitConfig};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn test_planner(max: usize) -> SemanticPlanner {
        let rs = resolve_semantic_rule_set(&workspace_root(), None, true).expect("rules");
        SemanticPlanner::new(max, rs, false)
    }

    #[test]
    fn is_ignored_build_artifacts() {
        assert!(SemanticPlanner::is_ignored("target/debug/vox"));
        assert!(SemanticPlanner::is_ignored("target-toestub/x"));
        assert!(SemanticPlanner::is_ignored("target-agent/debug/vox-cli"));
        assert!(SemanticPlanner::is_ignored("target-agent2/debug/foo.exe"));
        assert!(SemanticPlanner::is_ignored(
            "target-doc-inv2/.rustc_info.json"
        ));
        assert!(SemanticPlanner::is_ignored(
            "target-ci/debug/build/addr2line/output"
        ));
        assert!(SemanticPlanner::is_ignored("target_debug/vox"));
        assert!(SemanticPlanner::is_ignored("docs/book/index.html"));
        assert!(SemanticPlanner::is_ignored(".vox-research-data/vox.db"));
        assert!(SemanticPlanner::is_ignored("Cargo.lock"));
    }

    #[test]
    fn is_ignored_root_scratch_files() {
        assert!(SemanticPlanner::is_ignored("build_error.log"));
        assert!(SemanticPlanner::is_ignored("check_all.txt"));
        assert!(SemanticPlanner::is_ignored("docs/src/quickstart.txt"));
    }

    #[test]
    fn allow_markdown_prefix_rescues_md_and_txt() {
        let allow = vec![
            "docs/src/quickstart.txt".to_string(),
            "AGENTS.md".to_string(),
        ];
        assert!(!SemanticPlanner::is_ignored_with(
            "docs/src/quickstart.txt",
            &allow
        ));
        assert!(!SemanticPlanner::is_ignored_with("AGENTS.md", &allow));
        assert!(SemanticPlanner::is_ignored_with(
            "docs/src/other.txt",
            &allow
        ));
    }

    #[test]
    fn is_ignored_coderabbit_worktrees() {
        assert!(SemanticPlanner::is_ignored(
            ".coderabbit/worktrees/cr__review-01_scaffold"
        ));
    }

    #[test]
    fn is_ignored_real_files() {
        assert!(!SemanticPlanner::is_ignored("crates/vox-cli/src/main.rs"));
        assert!(SemanticPlanner::is_ignored("AGENTS.md"));
        assert!(!SemanticPlanner::is_ignored("Vox.toml"));
        assert!(SemanticPlanner::is_ignored("docs/src/reference/lexicon.md"));
        assert!(!SemanticPlanner::is_ignored(".github/workflows/ci.yml"));
    }

    #[test]
    fn group_assignment_canonical() {
        let p = test_planner(250);
        let g = |path: &str| p.group_for(path).1;
        assert_eq!(g("AGENTS.md"), "01_scaffold");
        assert_eq!(g("Cargo.toml"), "01_scaffold");
        assert_eq!(g(".github/workflows/ci.yml"), "02_github_agents");
        assert_eq!(g(".agents/workflows/cargo-safety.md"), "02_github_agents");
        assert_eq!(g(".gitignore"), "03_dotfiles_config");
        assert_eq!(
            g(format!("{}/README.md", concat!(".open", "code")).as_str()),
            format!("04_{}_retire", concat!("open", "code")).as_str()
        );
        assert_eq!(g("contracts/api-registry.json"), "05_contracts");
        assert_eq!(g("docs/src/reference/lexicon.md"), "06_docs_src");
        assert_eq!(g("docs/SUMMARY.md"), "07_docs_other");
        assert_eq!(g("frontend/App.tsx"), "08_frontend");
        assert_eq!(g("examples/hello.vox"), "09_examples");
        assert_eq!(
            g("apps/editor/vox-vscode/src/extension.ts"),
            "10_vscode_ext"
        );
        assert_eq!(g("scripts/unlock.ps1"), "11_scripts_xtask");
        assert_eq!(g("mens/data/sft_pairs.jsonl"), "12_populi_ml");
        assert_eq!(g("tests/fixtures/minimal.vox"), "13_tests");
        assert_eq!(g("crates/vox-compiler/src/lib.rs"), "crate_vox_compiler");
        assert_eq!(
            g("crates/vox-primitives/src/lib.rs"),
            "crate_vox_primitives"
        );
        assert_eq!(
            g("crates/vox-actor-runtime/src/lib.rs"),
            "crate_vox_runtime"
        );
        assert_eq!(g("crates/vox-lsp/src/lib.rs"), "crate_vox_lsp");
        assert_eq!(g("crates/vox-mcp/src/lib.rs"), "crate_vox_mcp");
        assert_eq!(g("crates/vox-cli/src/main.rs"), "crate_vox_cli");
        assert_eq!(g("crates/vox-bootstrap/src/main.rs"), "crate_vox_bootstrap");
    }

    #[test]
    fn plan_subdivides_large_groups() {
        let planner = test_planner(3);
        let files: Vec<String> = (0..7)
            .map(|i| format!("crates/vox-cli/src/file{i}.rs"))
            .collect();
        let manifest = planner.plan(files, "cr-baseline");
        assert!(
            manifest
                .chunks
                .iter()
                .any(|c| c.name.starts_with("crate_vox_cli"))
        );
        assert!(manifest.chunks.iter().all(|c| c.files.len() <= 3));
    }

    #[test]
    fn plan_filters_ignored_files() {
        let planner = test_planner(250);
        let files = vec![
            "target/debug/vox".to_string(),
            "build_error.log".to_string(),
            "Cargo.lock".to_string(),
            "crates/vox-cli/src/main.rs".to_string(),
        ];
        let manifest = planner.plan(files, "cr-baseline");
        assert_eq!(manifest.total_files, 1);
        assert_eq!(manifest.chunks[0].files[0], "crates/vox-cli/src/main.rs");
    }
}
