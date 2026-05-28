//! Automated evaluation helpers for MENS-trained emitters (Mn-T12).
//!
//! The production harness will drive `vox check` over emitted `.vox` snippets; this crate holds
//! shared scoring types so CLI and CI can depend on one surface.

/// Normalised compile outcome for a single eval case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileVerdict {
    Pass,
    Fail { diagnostic_summary: String },
}

#[must_use]
pub fn summarize_placeholder() -> CompileVerdict {
    CompileVerdict::Pass
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_ok() {
        assert_eq!(summarize_placeholder(), CompileVerdict::Pass);
    }
}

