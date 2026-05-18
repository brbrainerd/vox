//! `vox audit deploy` — CR-L7 deploy/health integration gate.
//!
//! Per `v1-release-criteria.md` [CR-L7], the full integration test
//! drives `vox new web → vox deploy → vox doctor` on each Marquee
//! fixture inside the [CR-P3] 120-second budget.
//!
//! This subcommand ships the doctor leg today against the real
//! Marquee app fixtures registered in `contracts/marquee/manifest.v1.yaml`
//! with `status: real`. Each fixture must compile clean
//! (`vox_compiler::pipeline::check_file` reports zero error-severity
//! diagnostics for every `.vox` file under the fixture directory) for
//! the gate to pass. The `vox new` / `vox deploy` legs land separately;
//! when they do they front-stack onto this same runner.
//!
//! Replaces the prior `DeployStub` per the no-stub directive.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use serde::Deserialize;
use std::path::Path;
use std::time::Instant;
use vox_compiler::typeck::diagnostics::TypeckSeverity;

const DEFAULT_MARQUEE_MANIFEST: &str = "contracts/marquee/manifest.v1.yaml";

pub struct DeployRunner;

impl Subcommand for DeployRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L7Deploy
    }

    fn description(&self) -> &'static str {
        "CR-L7: doctor-green on every status:real Marquee fixture (deploy leg lands next)."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let manifest_path = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_MARQUEE_MANIFEST));

        let manifest = match MarqueeManifest::load(&manifest_path) {
            Ok(m) => m,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        let real_apps: Vec<&MarqueeApp> = manifest
            .apps
            .iter()
            .filter(|a| a.status == "real")
            .collect();
        if real_apps.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    "no Marquee apps with status:real registered in manifest".to_string(),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                manifest_hash(&manifest_path),
                real_apps.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!(
                "dry-run: {} status:real Marquee fixtures discovered",
                real_apps.len()
            ));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        let started = Instant::now();
        let workspace = workspace_root();
        let mut green = 0u32;
        let mut red_apps: Vec<String> = Vec::new();
        for app in &real_apps {
            let fixture_path = workspace.join(&app.fixture_path);
            if !fixture_path.exists() {
                red_apps.push(format!("{}:missing", app.id));
                continue;
            }
            match doctor_project(&fixture_path) {
                DoctorOutcome::Green => green += 1,
                DoctorOutcome::Red(reason) => red_apps.push(format!("{}:{reason}", app.id)),
            }
        }
        let total = real_apps.len() as u32;
        let pass_rate = if total == 0 {
            0.0
        } else {
            f64::from(green) / f64::from(total)
        };

        let target = args.threshold.unwrap_or(1.0);
        let met = green == total;
        let mut report = AuditReport::complete(
            gate_thing_name(),
            manifest_hash(&manifest_path),
            total,
            Results {
                overall_pass_rate: pass_rate,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold { target, met });
        let duration = started.elapsed().as_secs_f64();
        let mut note = format!(
            "doctor leg: {}/{} fixtures green ({:.2}s); deploy + new legs deferred",
            green, total, duration,
        );
        if !red_apps.is_empty() {
            note.push_str(&format!(" — failing: [{}]", red_apps.join(", ")));
        }
        report.note = Some(note);

        let exit_code = if met {
            ExitCode::Ok
        } else {
            ExitCode::BarMissed
        };
        RunOutcome { report, exit_code }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L7Deploy.thing_name()
}

fn manifest_hash(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        Err(_) => "blake3:unavailable".to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct MarqueeManifest {
    apps: Vec<MarqueeApp>,
}

#[derive(Debug, Deserialize)]
struct MarqueeApp {
    id: String,
    status: String,
    fixture_path: String,
}

impl MarqueeManifest {
    fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        serde_yaml::from_str(&text)
            .map_err(|e| format!("malformed marquee manifest {}: {e}", path.display()))
    }
}

enum DoctorOutcome {
    Green,
    Red(String),
}

/// Walk `fixture_root` for `.vox` files (mirroring the
/// `vox doctor --project` skip list) and compile-check each. Green iff
/// every file produces zero error-severity diagnostics.
fn doctor_project(fixture_root: &Path) -> DoctorOutcome {
    let mut total = 0u32;
    let mut failing = 0u32;
    let mut first_failure: Option<String> = None;
    for entry in walkdir::WalkDir::new(fixture_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_skipped_dir(e.path()))
        .filter_map(|r| r.ok())
    {
        let p = entry.path();
        if !(p.is_file() && p.extension().is_some_and(|x| x == "vox")) {
            continue;
        }
        total += 1;
        let source = match std::fs::read_to_string(p) {
            Ok(s) => s,
            Err(_) => {
                failing += 1;
                first_failure.get_or_insert_with(|| format!("{}:read-error", p.display()));
                continue;
            }
        };
        let diags = vox_compiler::pipeline::check_file(&source, &p.to_string_lossy());
        if diags
            .iter()
            .any(|d| matches!(d.severity, TypeckSeverity::Error))
        {
            failing += 1;
            first_failure.get_or_insert_with(|| format!("{}:diagnostics", p.display()));
        }
    }
    if total == 0 {
        return DoctorOutcome::Red(format!(
            "no .vox files under {}",
            fixture_root.display()
        ));
    }
    if failing == 0 {
        DoctorOutcome::Green
    } else {
        DoctorOutcome::Red(format!(
            "{failing}/{total}-fail({})",
            first_failure.unwrap_or_else(|| "unknown".into())
        ))
    }
}

fn is_skipped_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name,
        "target" | "node_modules" | ".git" | ".cargo" | "dist" | "build" | "archive"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> CommonArgs {
        CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        }
    }

    #[test]
    fn deploy_runs_against_real_workspace_marquee_manifest_and_passes() {
        let outcome = DeployRunner.run(&args());
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "all status:real marquee fixtures should be doctor-green; note: {:?}",
            outcome.report.note
        );
        assert!(outcome.report.corpus_size >= 1, "at least one real fixture");
        assert_eq!(outcome.report.results.overall_pass_rate, 1.0);
        let threshold = outcome.report.threshold.expect("threshold present");
        assert!(threshold.met);
    }

    #[test]
    fn deploy_dry_run_returns_ok_without_compiling() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = DeployRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("dry-run"));
    }

    #[test]
    fn deploy_with_missing_manifest_returns_infra_error() {
        let args = CommonArgs {
            corpus: Some(std::path::PathBuf::from("this/path/does/not/exist/marquee.yaml")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = DeployRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.incomplete);
    }

    #[test]
    fn deploy_with_broken_fixture_returns_bar_missed() {
        let tmp = tempfile::tempdir().unwrap();
        let fixture = tmp.path().join("bad-fixture");
        std::fs::create_dir_all(fixture.join("src")).unwrap();
        std::fs::write(
            fixture.join("src/main.vox"),
            "this is not valid vox source ###\n",
        )
        .unwrap();
        let manifest_path = tmp.path().join("marquee.yaml");
        // Use absolute path so the runner's `workspace_root().join(...)` works:
        // we override workspace via `corpus` and the runner joins fixture_path
        // onto workspace_root. Simpler: write the absolute path into
        // fixture_path so the workspace_root join is benign.
        let fixture_abs = fixture.display().to_string().replace('\\', "/");
        let yaml = format!(
            "apps:\n  - id: bad\n    status: real\n    fixture_path: \"{}\"\n",
            fixture_abs
        );
        std::fs::write(&manifest_path, yaml).unwrap();
        let args = CommonArgs {
            corpus: Some(manifest_path),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = DeployRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::BarMissed);
        assert_eq!(outcome.report.results.overall_pass_rate, 0.0);
    }
}
