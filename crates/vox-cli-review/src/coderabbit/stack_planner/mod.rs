//! Size-first and Semantic PR Stack planner for CodeRabbit.
//!
//! Generates a sequence of dependent PRs (a Stack) to slip past CodeRabbit size limits
//! without sacrificing cross-referencing context.
//! See [`super::limits`] for tier-based constants.

mod heuristics;
mod stack;
mod submit;
mod types;

pub use submit::run_stack_submit;
pub use types::{StackChunk, StackManifest, StackPlanConfig, StackPlanner};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filtration_ignores_locks_and_targets() {
        assert!(StackPlanner::is_ignored("Cargo.lock"));
        assert!(StackPlanner::is_ignored("target/debug/vox.exe"));
        assert!(StackPlanner::is_ignored(".git/config"));
        assert!(StackPlanner::is_ignored(".cursor/workspace.json"));
        assert!(StackPlanner::is_ignored("images/logo.png"));

        // Should NOT ignore code
        assert!(!StackPlanner::is_ignored("crates/vox-cli/src/main.rs"));
        assert!(!StackPlanner::is_ignored("README.md"));
        assert!(!StackPlanner::is_ignored("frontend/package.json"));
    }

    #[test]
    fn test_chunk_assignment_logic() {
        let (order, name) = StackPlanner::get_chunk_id("Cargo.toml");
        assert_eq!(name, "01_scaffold_manifests");
        assert_eq!(order, 5);

        let (_, name) = StackPlanner::get_chunk_id("crates/vox-populi/src/mens/tensor/lora.rs");
        assert_eq!(name, "07_populi_lora_model");

        let (_, name) = StackPlanner::get_chunk_id("crates/vox-cli/src/commands/mens/mod.rs");
        assert_eq!(name, "14_crate_cli_commands");
    }

    #[test]
    fn test_planner_subdivision() {
        let planner = StackPlanner::new(StackPlanConfig {
            max_files_per_pr: 2,
        });

        // 3 rust core files -> should split into 2 batches
        let files = vec![
            "crates/vox-cli/src/main.rs".to_string(),
            "crates/vox-cli/src/lib.rs".to_string(),
            "crates/vox-cli/src/diagnostics.rs".to_string(),
        ];

        let manifest = planner.plan(files);
        assert_eq!(manifest.chunks.len(), 2);
        assert!(manifest.chunks[0].name.starts_with("14_crate_cli_core"));
        assert_eq!(manifest.chunks[0].files.len(), 2);
        assert!(manifest.chunks[1].name.starts_with("14_crate_cli_core"));
        assert_eq!(manifest.chunks[1].files.len(), 1);
    }
}
