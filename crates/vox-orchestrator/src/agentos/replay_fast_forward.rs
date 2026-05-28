//! Fast-forward replay: read-only turns may serve cached transcripts without re-exec.

use super::mutation_classifier::mutation_kind_for_tool;

/// Read-only tool effects can be replayed from cache without touching the host.
pub fn read_only_fast_forward_eligible(tool: &str) -> bool {
    mutation_kind_for_tool(tool) == "read_only"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_file_eligible() {
        assert!(read_only_fast_forward_eligible("vox_validate_file"));
    }

    #[test]
    fn shell_not_eligible() {
        assert!(!read_only_fast_forward_eligible("vox_run_shell"));
    }
}
