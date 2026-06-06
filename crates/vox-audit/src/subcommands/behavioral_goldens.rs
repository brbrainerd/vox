//! `vox audit --gate behavioral-goldens` — CR-F1 foundation gate.
//!
//! Runs every `examples/golden/*.vox` that carries `// EXPECT:` lines under
//! `vox run --mode interp` and asserts stdout matches the concatenated EXPECT
//! block. Mirrors the landed integration harness
//! `crates/vox-integration-tests/tests/golden_behavioral_gate.rs`; this is the
//! registered-gate form so `vox audit --gate all` and the GA snapshot see it.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};

pub struct BehavioralGoldensSubcommand;

/// Resolve the `vox` binary: `$VOX_BIN` override, else `vox` on PATH.
fn vox_bin() -> String {
    std::env::var("VOX_BIN").unwrap_or_else(|_| "vox".to_string())
}

/// Collect the `// EXPECT:` lines (in source order) joined by newlines.
/// Returns None when the golden declares no expectations.
pub(crate) fn parse_expect(src: &str) -> Option<String> {
    let mut lines = Vec::new();
    for raw in src.lines() {
        let t = raw.trim_start();
        if let Some(rest) = t.strip_prefix("// EXPECT:") {
            lines.push(rest.trim().to_string());
        }
    }
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

impl Subcommand for BehavioralGoldensSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::F1BehavioralGoldens
    }

    fn description(&self) -> &'static str {
        "CR-F1: behavioral goldens — `// EXPECT:` stdout matches `vox run --mode interp`."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let thing = CrlGate::F1BehavioralGoldens.thing_name();
        let golden_dir = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join("examples").join("golden"));

        let entries = match std::fs::read_dir(&golden_dir) {
            Ok(e) => e,
            Err(io) => {
                return RunOutcome {
                    report: AuditReport::infra_error(
                        thing,
                        format!("cannot read golden dir {}: {io}", golden_dir.display()),
                    ),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        let mut total = 0u32;
        let mut passed = 0u32;
        let mut failures: Vec<String> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("vox") {
                continue;
            }
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                // Read failures are a real problem, not a skip (matches the
                // CodeRabbit fix on the integration harness).
                Err(io) => {
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            thing,
                            format!("cannot read {}: {io}", path.display()),
                        ),
                        exit_code: ExitCode::InfrastructureError,
                    };
                }
            };
            let Some(expected) = parse_expect(&src) else {
                continue;
            };
            total += 1;

            let out = std::process::Command::new(vox_bin())
                .args(["run", "--mode", "interp"])
                .arg(&path)
                .output();
            match out {
                Ok(o) => {
                    let got = String::from_utf8_lossy(&o.stdout).trim_end().to_string();
                    if got == expected.trim_end() {
                        passed += 1;
                    } else {
                        failures.push(format!(
                            "{}: expected {:?}, got {:?}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            expected,
                            got
                        ));
                    }
                }
                Err(io) => {
                    // vox binary absent / not executable → infra error, not a
                    // measurement failure.
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            thing,
                            format!("failed to exec `{}`: {io}", vox_bin()),
                        ),
                        exit_code: ExitCode::InfrastructureError,
                    };
                }
            }
        }

        let pass_rate = if total == 0 {
            0.0
        } else {
            passed as f64 / total as f64
        };
        let met = total > 0 && passed == total;
        let mut report = AuditReport::complete(
            thing,
            format!("count:{total}"),
            total,
            Results {
                overall_pass_rate: pass_rate,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold {
            target: args.threshold.unwrap_or(1.0),
            met,
        });
        if !met {
            report.note = Some(if total == 0 {
                "no goldens with `// EXPECT:` lines found".to_string()
            } else {
                format!(
                    "{}/{} behavioral goldens diverged: {}",
                    total - passed,
                    total,
                    failures.join("; ")
                )
            });
        }
        RunOutcome {
            report,
            exit_code: if met {
                ExitCode::Ok
            } else {
                ExitCode::BarMissed
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommonArgs;
    use crate::report::ExitCode;

    #[test]
    fn behavioral_goldens_gate_runs_over_examples() {
        // Requires a `vox` binary on PATH or $VOX_BIN. Degrades to an
        // infrastructure error (never panics / never false-fails) when absent.
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = BehavioralGoldensSubcommand.run(&args);
        assert_eq!(outcome.report.thing, "behavioral-goldens");
        assert!(matches!(
            outcome.exit_code,
            ExitCode::Ok | ExitCode::BarMissed | ExitCode::InfrastructureError
        ));
    }

    #[test]
    fn parse_expect_extracts_lines() {
        let src = "// EXPECT: hello\nfn main() { print(\"hello\") }\n// EXPECT: world\n";
        assert_eq!(parse_expect(src), Some("hello\nworld".to_string()));
        assert_eq!(parse_expect("fn main() {}\n"), None);
    }
}
