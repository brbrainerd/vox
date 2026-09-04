//! Default `vox doctor` checks: optional test-health tools, then full toolchain audit.

mod binary_ssot;
mod build_health;
mod compile_target;
mod freshness;
mod gpu_hardware;
mod gui_sidecar;
mod llm_routing;
mod model_catalog;
mod model_telemetry;
mod secrets;
mod tail;
mod test_health;
pub mod tier_deps;
mod toolchain;
mod vox_ignore;
mod web_frontend;

use super::common::Check;

pub(crate) use build_health::parse_diag_id;

/// Stable diag-id registry, for `--diag` unknown-id error messages.
pub(crate) fn known_diag_ids() -> &'static [&'static str] {
    build_health::KNOWN_DIAGNOSIS_IDS
}

/// Run only the check-set covering `id`. Returns `false` for unregistered ids.
pub(crate) async fn run_diag_check(id: &str, checks: &mut Vec<Check>) -> bool {
    let Some(kind) = build_health::check_kind_for_diag(id) else {
        return false;
    };
    build_health::run_check_for_diag(kind, checks).await;
    true
}

pub async fn run_checks(
    auto_heal: bool,
    test_health: bool,
    compile_target: Option<&str>,
    tier: &str,
    checks: &mut Vec<Check>,
) {
    if let Some(t) = compile_target.filter(|s| !s.is_empty()) {
        compile_target::run(t, checks);
    }
    if test_health::run(test_health, checks).await {
        return;
    }
    toolchain::run(auto_heal, checks).await;
    build_health::run(auto_heal, checks).await;
    freshness::run(checks);
    binary_ssot::run(checks);
    secrets::run(auto_heal, checks).await;
    llm_routing::run(checks).await;
    gpu_hardware::run(checks).await;
    vox_ignore::run(auto_heal, checks).await;
    web_frontend::run(checks).await;
    gui_sidecar::run(checks);
    model_telemetry::run(checks).await;
    model_catalog::run(checks).await;
    tail::run(auto_heal, checks).await;

    // Per-tier runtime-optional dep surfacing (reads distribution SSOT).
    let dep_statuses = tier_deps::check_runtime_optional_deps(tier);
    for s in dep_statuses {
        checks.push(Check::new(
            format!("tier dep: {}", s.name),
            s.present,
            if s.present {
                format!("{} — found", s.name)
            } else {
                s.hint
            },
        ));
    }
}

#[cfg(test)]
mod remediation_tests {
    use super::super::common::Check;

    /// Extract every `vox …` invocation a check's detail text recommends, as the
    /// longest prefix that matches a real catalog entry. Returns the *unmatched*
    /// candidates — a `vox foo bar` whose longest known prefix is nothing.
    fn unknown_vox_invocations(detail: &str, known: &std::collections::HashSet<String>) -> Vec<String> {
        let mut unknown = Vec::new();
        let words: Vec<&str> = detail.split_whitespace().collect();
        for (i, w) in words.iter().enumerate() {
            if w.trim_matches(|c: char| !c.is_ascii_alphanumeric()) != "vox" {
                continue;
            }
            // Only treat this as an invocation when it appears in a command
            // context — after `run:`/`run`, or inside backticks. Prose such as
            // "the vox binary is fine" names no command and must not be linted.
            let in_backticks = w.starts_with('`');
            let after_run = i > 0
                && matches!(
                    words[i - 1].trim_end_matches(':').to_ascii_lowercase().as_str(),
                    "run" | "try"
                );
            if !in_backticks && !after_run {
                continue;
            }
            // Build candidates `vox a`, `vox a b`, … and keep the longest match.
            let mut best: Option<String> = None;
            let mut candidate = String::from("vox");
            for w2 in words.iter().skip(i + 1).take(3) {
                let tok = w2.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-'));
                // Stop at a flag or an obvious non-subcommand token.
                if tok.is_empty() || tok.starts_with('-') {
                    break;
                }
                candidate.push(' ');
                candidate.push_str(tok);
                if known.contains(&candidate) {
                    best = Some(candidate.clone());
                }
            }
            // Only flag when the text named at least one subcommand token and none
            // of the prefixes matched — a bare "vox" in prose is not a claim.
            if best.is_none() && candidate != "vox" {
                unknown.push(candidate);
            }
        }
        unknown
    }

    /// The detector itself must catch the shape that actually shipped, or the
    /// lint above is a green light that proves nothing.
    #[test]
    fn detector_flags_a_nonexistent_subcommand() {
        let known: std::collections::HashSet<String> =
            ["vox doctor", "vox repo init", "vox graph refresh"]
                .into_iter()
                .map(String::from)
                .collect();

        // The two that actually shipped in a doctor remediation.
        assert_eq!(
            unknown_vox_invocations("not registered — run: vox setup", &known),
            vec!["vox setup".to_string()]
        );
        assert!(!unknown_vox_invocations("run: vox login --registry google KEY", &known).is_empty());

        // Real commands, and prose after them, must not be flagged.
        assert!(unknown_vox_invocations("run: vox repo init (writes the catalog)", &known).is_empty());
        assert!(unknown_vox_invocations("see `vox doctor` output", &known).is_empty());
        assert!(unknown_vox_invocations("the vox binary is fine", &known).is_empty());
    }

    /// Every `vox …` command doctor recommends must exist in the clap tree.
    ///
    /// Five dead remediations shipped simultaneously — `vox setup` and
    /// `vox login --registry` (neither exists) and `vox mens pull` (not a
    /// subcommand) among them — because nothing asserted this. A user who follows
    /// doctor's advice and gets "unrecognized subcommand" learns to ignore doctor.
    #[tokio::test]
    async fn every_remediation_names_a_real_vox_command() {
        let mut checks: Vec<Check> = Vec::new();
        super::tail::run(false, &mut checks).await;
        assert!(!checks.is_empty(), "tail::run produced no checks to lint");

        let known: std::collections::HashSet<String> = crate::command_catalog::build_catalog()
            .entries
            .iter()
            .map(|e| e.command.clone())
            .collect();

        let mut failures = Vec::new();
        for c in &checks {
            for bad in unknown_vox_invocations(&c.detail, &known) {
                failures.push(format!("{}: recommends `{bad}`", c.name));
            }
        }
        assert!(
            failures.is_empty(),
            "doctor recommends commands that are not in the clap tree:\n  {}",
            failures.join("\n  ")
        );
    }
}
