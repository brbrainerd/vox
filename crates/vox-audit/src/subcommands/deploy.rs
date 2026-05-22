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
        let doctor_started = Instant::now();
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
        let doctor_seconds = doctor_started.elapsed().as_secs_f64();

        // P2.2: new + deploy legs. Scaffold a fresh `--template web` project,
        // run `vox deploy --dry-run` against it, then re-run the doctor over
        // the scaffolded sources. The trio sum is the canonical CR-P3 budget
        // (120s). We call the library functions directly so the gate stays
        // library-pure (no `vox` binary required).
        let new_deploy_legs = run_new_and_deploy_legs();

        let total = real_apps.len() as u32;
        let pass_rate = if total == 0 {
            0.0
        } else {
            f64::from(green) / f64::from(total)
        };

        let target = args.threshold.unwrap_or(1.0);
        let cr_p3_budget_seconds = 120.0;
        let trio_seconds = new_deploy_legs.new_seconds
            + new_deploy_legs.deploy_seconds
            + new_deploy_legs.doctor_seconds;
        let met = green == total
            && new_deploy_legs.new_ok
            && new_deploy_legs.deploy_ok
            && new_deploy_legs.doctor_ok
            && trio_seconds <= cr_p3_budget_seconds;

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
            "doctor leg: {green}/{total} marquee fixtures green ({doctor_seconds:.2}s). \
             new leg: {new_ok} ({new_seconds:.2}s). \
             deploy --dry-run leg: {deploy_ok} ({deploy_seconds:.2}s). \
             scaffold doctor leg: {scaffold_doctor_ok} ({scaffold_doctor_seconds:.2}s). \
             new+deploy+doctor trio: {trio_seconds:.2}s vs {cr_p3_budget_seconds:.0}s CR-P3 budget. \
             total: {duration:.2}s.",
            new_ok = if new_deploy_legs.new_ok { "ok" } else { "FAIL" },
            new_seconds = new_deploy_legs.new_seconds,
            deploy_ok = if new_deploy_legs.deploy_ok { "ok" } else { "FAIL" },
            deploy_seconds = new_deploy_legs.deploy_seconds,
            scaffold_doctor_ok = if new_deploy_legs.doctor_ok { "ok" } else { "FAIL" },
            scaffold_doctor_seconds = new_deploy_legs.doctor_seconds,
        );
        if !red_apps.is_empty() {
            note.push_str(&format!(" — failing fixtures: [{}]", red_apps.join(", ")));
        }
        if let Some(err) = &new_deploy_legs.error {
            note.push_str(&format!(" — leg error: {err}"));
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

/// Result of running the `vox new` + `vox deploy --dry-run` + scaffold-doctor
/// legs end-to-end in a tempdir. The trio sum is compared against the CR-P3
/// 120-second budget in the caller.
struct NewAndDeployLegs {
    new_ok: bool,
    new_seconds: f64,
    deploy_ok: bool,
    deploy_seconds: f64,
    doctor_ok: bool,
    doctor_seconds: f64,
    error: Option<String>,
}

/// Scaffold a fresh `web` project, run `vox deploy --dry-run` against it,
/// and re-run the doctor over the scaffolded sources. The whole sequence
/// runs in a `tempfile::tempdir()` so it leaves no side-effects.
fn run_new_and_deploy_legs() -> NewAndDeployLegs {
    let mut result = NewAndDeployLegs {
        new_ok: false,
        new_seconds: 0.0,
        deploy_ok: false,
        deploy_seconds: 0.0,
        doctor_ok: false,
        doctor_seconds: 0.0,
        error: None,
    };

    let tmp = match tempfile::tempdir() {
        Ok(t) => t,
        Err(e) => {
            result.error = Some(format!("tempdir: {e}"));
            return result;
        }
    };

    // Leg 1: vox new --template web.
    let new_started = Instant::now();
    let scaffold = vox_project_scaffold::scaffold_vox_project_at(
        tmp.path(),
        "cr-l7-gate",
        "application",
        Some("web"),
    );
    result.new_seconds = new_started.elapsed().as_secs_f64();
    if let Err(e) = scaffold {
        result.error = Some(format!("scaffold: {e}"));
        return result;
    }
    result.new_ok = true;

    // Leg 2: vox deploy --dry-run. Replicates `vox-cli::commands::deploy::run`
    // for the container-target dry-run path. The dry-run short-circuit in
    // execute_container() lands before any docker/podman invocation, so this
    // leg requires no container runtime.
    let deploy_started = Instant::now();
    let manifest_path = tmp.path().join("Vox.toml");
    let deploy_outcome = (|| -> Result<(), String> {
        let manifest = vox_package::VoxManifest::load(&manifest_path)
            .map_err(|e| format!("load Vox.toml: {e}"))?;
        let deploy = manifest
            .deploy
            .as_ref()
            .ok_or_else(|| "no [deploy] section in scaffolded Vox.toml".to_string())?;
        let target_kind = vox_deploy_codegen::resolve_target_kind(None, deploy.target.as_deref());
        if target_kind != "container" {
            return Err(format!(
                "scaffolded web template should default to container; got {target_kind}"
            ));
        }
        let ct = vox_deploy_codegen::build_container_target(
            &manifest.package.name,
            "production",
            deploy.effective_image_name(),
            deploy.effective_registry(),
            deploy
                .container
                .as_ref()
                .and_then(|c| c.dockerfile.as_deref()),
            &deploy
                .container
                .as_ref()
                .map(|c| c.build_args.clone())
                .unwrap_or_default(),
            tmp.path(),
        );
        let target = vox_deploy_codegen::DeployTarget::Container(ct);
        target
            .execute(None, /* dry_run */ true)
            .map_err(|e| format!("dry-run execute: {e}"))?;
        Ok(())
    })();
    result.deploy_seconds = deploy_started.elapsed().as_secs_f64();
    match deploy_outcome {
        Ok(()) => result.deploy_ok = true,
        Err(e) => {
            result.error = Some(e);
            return result;
        }
    }

    // Leg 3: doctor over the scaffolded source.
    let doctor_started = Instant::now();
    let doctor = doctor_project(tmp.path());
    result.doctor_seconds = doctor_started.elapsed().as_secs_f64();
    match doctor {
        DoctorOutcome::Green => result.doctor_ok = true,
        DoctorOutcome::Red(reason) => {
            result.error = Some(format!("scaffold-doctor: {reason}"));
        }
    }
    result
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
