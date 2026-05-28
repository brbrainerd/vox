//! Sparse checkpoint eligibility from mutation classification.

use super::mutation_classifier::mutation_kind_for_tool;

/// Whether a durable checkpoint should be taken **before** executing this tool turn.
pub fn should_sparse_checkpoint(tool: &str) -> bool {
    matches!(
        mutation_kind_for_tool(tool),
        "local_mutation" | "external_side_effect"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_skips_checkpoint() {
        assert!(!should_sparse_checkpoint("vox_git_status"));
    }

    #[test]
    fn write_requires_checkpoint() {
        assert!(should_sparse_checkpoint("vox_write_file"));
    }
}
