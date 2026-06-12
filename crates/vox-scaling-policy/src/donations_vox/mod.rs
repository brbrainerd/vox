//! Parse, edit, and pretty-print `donations.vox` policy files.
//!
//! The policy file is first-class Vox source. This crate wraps
//! `vox-compiler` parse → `WorkerDonationPolicy` extraction and
//! owns the pretty-print round-trip.
pub mod parse;
pub mod print;

pub use parse::{ParseError, load_policy};
pub use print::pretty_print;

#[cfg(test)]
mod tests {
    use super::*;
    use vox_mesh_types::task::TaskKind;

    const SAMPLE: &str = r#"
// A sample donations.vox policy
slot text_infer { max_concurrent = 2, weight_pct = 60 }
slot embed { max_concurrent = 4, weight_pct = 40 }

let nsfw_allowed = false
let max_job_duration_secs = 7200
let public_mesh_opt_in = true
let min_priority = 5
"#;

    #[test]
    fn round_trip_policy() {
        // Parse the original source
        let policy1 = parse::parse_source(SAMPLE, "<test>").expect("parse failed");

        // Verify parsed values
        assert_eq!(policy1.slots.len(), 2);
        assert_eq!(policy1.slots[0].task_kind, TaskKind::TextInfer);
        assert_eq!(policy1.slots[0].max_concurrent, 2);
        assert_eq!(policy1.slots[0].weight_pct, 60);
        assert_eq!(policy1.slots[1].task_kind, TaskKind::Embed);
        assert_eq!(policy1.slots[1].max_concurrent, 4);
        assert_eq!(policy1.slots[1].weight_pct, 40);
        assert!(!policy1.nsfw_allowed);
        assert_eq!(policy1.max_job_duration_secs, 7200);
        assert!(policy1.public_mesh_opt_in);
        assert_eq!(policy1.min_priority, 5);

        // Pretty-print it
        let printed = pretty_print(&policy1);

        // Re-parse the pretty-printed output
        let policy2 = parse::parse_source(&printed, "<printed>").expect("re-parse failed");

        // Both parsed forms must be equal
        assert_eq!(policy1, policy2);
    }

    #[test]
    fn parse_minimal_policy() {
        let src = "let nsfw_allowed = true\nlet max_job_duration_secs = 100\nlet public_mesh_opt_in = false\nlet min_priority = 1\n";
        let policy = parse::parse_source(src, "<minimal>").expect("parse failed");
        assert!(policy.nsfw_allowed);
        assert_eq!(policy.max_job_duration_secs, 100);
        assert!(!policy.public_mesh_opt_in);
        assert_eq!(policy.min_priority, 1);
        assert!(policy.slots.is_empty());
    }

    #[test]
    fn pretty_print_is_deterministic() {
        let policy = parse::parse_source(SAMPLE, "<test>").expect("parse failed");
        let a = pretty_print(&policy);
        let b = pretty_print(&policy);
        assert_eq!(a, b);
    }
}
