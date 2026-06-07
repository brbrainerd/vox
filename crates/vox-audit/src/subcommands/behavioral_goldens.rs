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
use std::path::Path;
use std::time::{Duration, Instant};

pub struct BehavioralGoldensSubcommand;

/// Per-golden wall-clock budget for `vox run --mode interp`. A golden that
/// exceeds this is a behavioral failure (e.g. an interpreter regression that
/// loses the step-limit guard), NOT an infra error — so the gate stays
/// bounded even against a broken `vox`. Overridable via `$VOX_GOLDEN_TIMEOUT_SECS`.
fn golden_timeout() -> Duration {
    let secs = std::env::var("VOX_GOLDEN_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

/// Resolve the `vox` binary: `$VOX_BIN` override, else `vox` on PATH.
fn vox_bin() -> String {
    std::env::var("VOX_BIN").unwrap_or_else(|_| "vox".to_string())
}

/// Outcome of running one golden under a bounded timeout.
enum GoldenRun {
    /// Process exited; trimmed stdout captured.
    Done(String),
    /// Exceeded the wall-clock budget; the child was killed.
    TimedOut,
    /// Could not spawn `vox` (binary absent / not executable).
    SpawnErr(String),
}

/// Run `<bin> run --mode interp <path>` with a hard wall-clock cap. stdout is
/// drained on a helper thread so a chatty golden can't fill the pipe and
/// deadlock before exit.
fn run_golden(bin: &str, path: &Path, timeout: Duration) -> GoldenRun {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = match Command::new(bin)
        .args(["run", "--mode", "interp"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return GoldenRun::SpawnErr(e.to_string()),
    };

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let buf = reader.join().unwrap_or_default();
                return GoldenRun::Done(String::from_utf8_lossy(&buf).trim_end().to_string());
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return GoldenRun::TimedOut;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = reader.join();
                return GoldenRun::SpawnErr(e.to_string());
            }
        }
    }
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

            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match run_golden(&vox_bin(), &path, golden_timeout()) {
                GoldenRun::Done(got) => {
                    if got == expected.trim_end() {
                        passed += 1;
                    } else {
                        failures.push(format!("{name}: expected {expected:?}, got {got:?}"));
                    }
                }
                GoldenRun::TimedOut => {
                    // A hang is a behavioral failure, not an infra error — keeps
                    // the gate bounded even against a broken interpreter.
                    failures.push(format!(
                        "{name}: timed out after {}s",
                        golden_timeout().as_secs()
                    ));
                }
                GoldenRun::SpawnErr(io) => {
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
    fn run_golden_missing_binary_is_spawn_err() {
        // A non-existent binary must surface SpawnErr (→ infra error), not hang.
        let out = run_golden(
            "vox-definitely-not-a-real-binary-xyz",
            std::path::Path::new("examples/golden/does-not-matter.vox"),
            std::time::Duration::from_secs(5),
        );
        assert!(matches!(out, GoldenRun::SpawnErr(_)));
    }

    #[test]
    fn parse_expect_extracts_lines() {
        let src = "// EXPECT: hello\nfn main() { print(\"hello\") }\n// EXPECT: world\n";
        assert_eq!(parse_expect(src), Some("hello\nworld".to_string()));
        assert_eq!(parse_expect("fn main() {}\n"), None);
    }
}
