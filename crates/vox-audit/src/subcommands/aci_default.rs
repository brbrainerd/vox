//! `vox audit aci-default` — CR-L5 ACI envelope default-on gate.
//!
//! Council ratified 2026-05-15 (D5/D20): the
//! `OrchestratorConfig::agentos_aci_envelope_enabled` default flips to `true`
//! in v0.6, NOT v0.5.x. Until that release lands, this audit reports the
//! current default state but treats `false` as the expected baseline (i.e.
//! "bar met" while we are on v0.5.x).
//!
//! Implementation: text-grep over
//! `crates/vox-orchestrator/src/config/impl_default.rs` rather than
//! introspecting `OrchestratorConfig::default()` at runtime — keeps this
//! crate from depending on vox-orchestrator (which is the largest workspace
//! crate and would dominate vox-audit build time).
//!
//! The contract guarantees the line shape; when P1.1 ships, the grep just
//! sees `default_true()` and the audit flips its `pass_rate` accordingly.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};

const DEFAULT_IMPL_RELPATH: &str = "crates/vox-orchestrator/src/config/impl_default.rs";

/// The field whose default we audit.
const FIELD: &str = "agentos_aci_envelope_enabled";

/// As of 2026-05-15 the council ratified D20: v0.5.x stays on `false`; v0.6
/// flips to `true`. The audit reads the workspace `Cargo.toml` version pin
/// to decide which value is "the bar."
const V0_5_X_EXPECTED: bool = false;
const V0_6_PLUS_EXPECTED: bool = true;

pub struct AciDefaultSubcommand;

impl Subcommand for AciDefaultSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::L5AciDefault
    }

    fn description(&self) -> &'static str {
        "CR-L5: `agentos_aci_envelope_enabled` defaults to `true` in v0.6+ (D20-ratified)."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let root = workspace_root();
        let impl_path = root.join(DEFAULT_IMPL_RELPATH);

        if args.dry_run {
            return match std::fs::read_to_string(&impl_path) {
                Ok(_) => RunOutcome {
                    report: AuditReport::complete(
                        gate_thing_name(),
                        "blake3:dry-run-no-hash",
                        0,
                        Results::default(),
                    ),
                    exit_code: ExitCode::Ok,
                },
                Err(err) => RunOutcome {
                    report: AuditReport::infra_error(
                        gate_thing_name(),
                        format!("dry-run could not read {}: {err}", impl_path.display()),
                    ),
                    exit_code: ExitCode::InfrastructureError,
                },
            };
        }

        let impl_text = match std::fs::read_to_string(&impl_path) {
            Ok(t) => t,
            Err(err) => {
                return RunOutcome {
                    report: AuditReport::infra_error(
                        gate_thing_name(),
                        format!("failed to read {}: {err}", impl_path.display()),
                    ),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        let observed = parse_default_value(&impl_text, FIELD);
        let expected = expected_default_for_workspace(&root);
        let met = observed == Some(expected);
        let pass = if met { 1.0 } else { 0.0 };

        let mut report = AuditReport::complete(
            gate_thing_name(),
            content_hash(&impl_text),
            1,
            Results {
                overall_pass_rate: pass,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold {
            target: args.threshold.unwrap_or(1.0),
            met,
        });
        if !met {
            report.note = Some(format!(
                "observed `{FIELD}` default = {:?}; expected {expected} for this workspace version",
                observed,
            ));
        }
        let exit_code = if met {
            ExitCode::Ok
        } else {
            ExitCode::BarMissed
        };
        RunOutcome { report, exit_code }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L5AciDefault.thing_name()
}

fn content_hash(text: &str) -> String {
    format!("blake3:{}", blake3::hash(text.as_bytes()).to_hex())
}

/// Parse the per-field default helper from a snippet like:
///
/// ```text,no_run
///     agentos_aci_envelope_enabled: default_false(),
/// ```
///
/// Returns `Some(true)` for `default_true()`, `Some(false)` for `default_false()`,
/// or `None` if the field is absent or uses an unrecognized helper.
fn parse_default_value(impl_text: &str, field: &str) -> Option<bool> {
    for line in impl_text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(field) {
            continue;
        }
        let rest = trimmed.trim_start_matches(field);
        // Expect: `: default_xxx(),`
        let rest = rest.trim_start();
        if !rest.starts_with(':') {
            continue;
        }
        let rest = rest[1..].trim_start();
        if rest.starts_with("default_true(") {
            return Some(true);
        }
        if rest.starts_with("default_false(") {
            return Some(false);
        }
        return None;
    }
    None
}

/// Read the workspace `Cargo.toml` `[workspace.package].version` pin and decide
/// whether the v0.5.x or v0.6+ expectation applies.
fn expected_default_for_workspace(workspace_root: &std::path::Path) -> bool {
    let cargo_path = workspace_root.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&cargo_path) else {
        // Conservatively assume v0.5.x.
        return V0_5_X_EXPECTED;
    };
    let Some(version) = parse_workspace_version(&text) else {
        return V0_5_X_EXPECTED;
    };
    if is_v0_5_x(&version) {
        V0_5_X_EXPECTED
    } else {
        V0_6_PLUS_EXPECTED
    }
}

fn parse_workspace_version(cargo_toml: &str) -> Option<String> {
    let mut in_workspace_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[workspace.package]" {
            in_workspace_package = true;
            continue;
        }
        if trimmed.starts_with('[') && in_workspace_package {
            break;
        }
        if in_workspace_package && let Some(rest) = trimmed.strip_prefix("version") {
            let rest = rest.trim_start().trim_start_matches('=').trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

fn is_v0_5_x(version: &str) -> bool {
    version.starts_with("0.5.") || version == "0.5"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_false_value() {
        let snippet = "            agentos_aci_envelope_enabled: default_false(),\n";
        assert_eq!(
            parse_default_value(snippet, "agentos_aci_envelope_enabled"),
            Some(false)
        );
    }

    #[test]
    fn parses_default_true_value() {
        let snippet = "    agentos_aci_envelope_enabled: default_true(),";
        assert_eq!(
            parse_default_value(snippet, "agentos_aci_envelope_enabled"),
            Some(true)
        );
    }

    #[test]
    fn parses_returns_none_for_missing_field() {
        let snippet = "    other_field: default_true(),";
        assert_eq!(
            parse_default_value(snippet, "agentos_aci_envelope_enabled"),
            None
        );
    }

    #[test]
    fn workspace_version_parsing() {
        let cargo = r#"[workspace]
members = ["crates/*"]

[workspace.package]
version = "0.5.0"
edition = "2024"
"#;
        assert_eq!(parse_workspace_version(cargo).as_deref(), Some("0.5.0"));
        assert!(is_v0_5_x("0.5.0"));
        assert!(is_v0_5_x("0.5.17"));
        assert!(!is_v0_5_x("0.6.0"));
        assert!(!is_v0_5_x("1.0.0"));
    }

    #[test]
    fn aci_default_runs_against_workspace() {
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = AciDefaultSubcommand.run(&args);
        // On v0.5.x we expect `false`, which is the current state, so bar met.
        // The workspace version pin is `0.5.0` per Cargo.toml; this test will
        // need to flip when v0.6 lands (P1.1) AND the default flip lands.
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "v0.5.x baseline expects default=false; report note: {:?}",
            outcome.report.note
        );
        assert!(!outcome.report.incomplete);
        assert_eq!(outcome.report.thing, "aci-default");
    }

    #[test]
    fn aci_default_dry_run_returns_ok() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = AciDefaultSubcommand.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
    }
}
