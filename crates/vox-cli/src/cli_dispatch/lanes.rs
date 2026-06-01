//! Fabrica / `diag` / `ars` lane dispatch helpers.

use crate::Cli;
use crate::cli_args;
use crate::commands;
use crate::latin_cmd;

pub(crate) async fn run_doctor_command(args: &cli_args::DoctorArgs) -> anyhow::Result<()> {
    // Project-health (CR-L7) takes precedence over environment-check flags.
    if let Some(ref project_root) = args.project {
        return commands::diagnostics::doctor::project_check::run(project_root, args.json).await;
    }
    commands::diagnostics::doctor::run(
        args.compile_target.as_deref(),
        args.auto_heal,
        args.test_health,
        args.build_perf,
        args.scope,
        args.json,
        args.probe,
        args.fix_cuda_path,
    )
    .await
}

#[cfg(feature = "stub-check")]
pub(crate) async fn run_stub_check_command(args: &cli_args::StubCheckArgs) -> anyhow::Result<()> {
    // Handle --list-diagnostics before scanning
    if args.list_diagnostics {
        commands::stub_check::list_diagnostics();
        return Ok(());
    }

    // Handle --explain <ID> before scanning
    if let Some(ref id) = args.explain {
        return commands::stub_check::explain_diagnostic(id);
    }

    let scan_root = args
        .path
        .clone()
        .or(args.scan_pos.clone())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Handle --rationale-required before the normal scan
    if args.rationale_required {
        commands::stub_check::check_rationale_required(&scan_root)?;
    }

    commands::stub_check::run(
        &scan_root,
        args.format.as_deref(),
        args.severity.as_deref(),
        args.suggest_fixes,
        args.rules.as_deref(),
        &args.excludes,
        args.langs.as_deref(),
        args.baseline.as_deref(),
        args.save_baseline.as_deref(),
        args.task_list,
        args.import_suppressions,
        args.ingest_findings.as_deref(),
        args.fix_pipeline,
        args.fix_pipeline_apply,
        args.gate.as_deref(),
        args.gate_budget_path.as_deref(),
        args.verify_impacted,
        args.max_escalation,
        args.self_heal_safe_mode,
    )
    .await
}

#[cfg(feature = "script-execution")]
pub(crate) fn script_opts_for_cli(
    args: &cli_args::ScriptArgs,
) -> commands::runtime::run::script::ScriptOpts {
    commands::runtime::run::script::ScriptOpts {
        sandbox: args.sandbox,
        allow_mcp: false,
        no_cache: args.no_cache,
        isolation: args.isolation.clone(),
        trust_class: args.trust_class.clone(),
        wasi_dirs: Vec::new(),
        target_triple: args.target_triple.clone(),
    }
}

#[cfg(feature = "script-execution")]
pub(crate) async fn run_script_subcommand(
    args: &cli_args::ScriptArgs,
    lane: &'static str,
) -> anyhow::Result<()> {
    tracing::info!(
        target: "vox.script",
        path = %args.file.display(),
        lane = lane,
        "script subcommand"
    );
    let opts = script_opts_for_cli(args);
    crate::commands::runtime::run::script::run(&args.file, &args.args, &opts).await
}

#[cfg(feature = "ars")]
pub(crate) async fn run_openclaw_subcommand(
    action: commands::openclaw::OpenClawAction,
) -> anyhow::Result<()> {
    commands::openclaw::run(action, false).await
}

#[cfg(feature = "coderabbit")]
pub(crate) async fn run_review_subcommand(cmd: commands::review::ReviewCli) -> anyhow::Result<()> {
    commands::review::run_coderabbit(cmd).await
}

/// Top-level `vox build` / `check` / … shims that map 1:1 onto [`latin_cmd::FabricaCmd`].
///
/// `Script` is not included: top-level `vox script` uses [`run_script_subcommand`] instead of `fabrica script`.
#[allow(clippy::result_large_err)]
pub(crate) fn cli_top_level_into_fabrica_or_self(
    cli: Cli,
) -> std::result::Result<latin_cmd::FabricaCmd, Cli> {
    use latin_cmd::FabricaCmd;
    match cli {
        Cli::Build { args } => Ok(FabricaCmd::Build(args)),
        Cli::Check { args } => Ok(FabricaCmd::Check(args)),
        Cli::Test { args } => Ok(FabricaCmd::Test(args)),
        Cli::Run { args } => Ok(FabricaCmd::Run(args)),
        Cli::Dev { args } => Ok(FabricaCmd::Dev(args)),
        Cli::BundleApp { args } => Ok(FabricaCmd::Bundle(args)),
        Cli::Compile { args } => Ok(FabricaCmd::Compile(args)),
        Cli::Fmt { args } => Ok(FabricaCmd::Fmt(args)),
        other => Err(other),
    }
}

/// Reward events emitted for a fabrica lane command (SP-3 Ludus bus wiring).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FabricaRewardEvents {
    success: &'static str,
    failure: Option<&'static str>,
    capability_id: &'static str,
    command_path: &'static str,
}

/// Stable lane name for a fabrica command (cheap; does not consume `cmd`).
fn fabrica_lane_name(cmd: &latin_cmd::FabricaCmd) -> &'static str {
    use latin_cmd::FabricaCmd;
    match cmd {
        FabricaCmd::Build(_) => "build",
        FabricaCmd::Check(_) => "check",
        FabricaCmd::Test(_) => "test",
        FabricaCmd::Run(_) => "run",
        FabricaCmd::Dev(_) => "dev",
        FabricaCmd::Bundle(_) => "bundle",
        FabricaCmd::Compile(_) => "compile",
        FabricaCmd::Fmt(_) => "fmt",
        #[cfg(feature = "script-execution")]
        FabricaCmd::Script(_) => "script",
    }
}

/// Map a fabrica lane to the Ludus reward events the policy already rewards
/// (`vox_gamify::reward_policy::base_reward`). Lanes with no reward type
/// (`run`/`dev`/`compile`/`script`) return `None`, so no hollow events are emitted.
fn fabrica_reward_events(lane: &str) -> Option<FabricaRewardEvents> {
    Some(match lane {
        "build" => FabricaRewardEvents {
            success: "build_completed",
            failure: Some("build_failed"),
            capability_id: "cli.build",
            command_path: "build",
        },
        "check" => FabricaRewardEvents {
            success: "check_completed",
            failure: Some("check_failed"),
            capability_id: "cli.check",
            command_path: "check",
        },
        "test" => FabricaRewardEvents {
            success: "test_pass",
            failure: Some("test_fail"),
            capability_id: "cli.test",
            command_path: "test",
        },
        "bundle" => FabricaRewardEvents {
            success: "bundle_completed",
            failure: None,
            capability_id: "cli.bundle",
            command_path: "bundle",
        },
        "fmt" => FabricaRewardEvents {
            success: "fmt_completed",
            failure: None,
            capability_id: "cli.fmt",
            command_path: "fmt",
        },
        _ => return None,
    })
}

/// Run a fabrica command, then emit its Ludus reward event (fire-and-forget).
///
/// Emission is non-blocking and self-gating: the shim opens its own DB, checks
/// the gamification config gate, and silently no-ops when disabled — it can
/// never change this command's result, exit code, or latency. Because the GUI
/// shells the `vox` sidecar, GUI-driven commands earn rewards through this same
/// path with no GUI-side code.
pub(crate) async fn run_fabrica_cmd(cmd: latin_cmd::FabricaCmd) -> anyhow::Result<()> {
    let events = fabrica_reward_events(fabrica_lane_name(&cmd));
    let result = run_fabrica_cmd_inner(cmd).await;
    if let Some(ev) = events {
        let success = result.is_ok();
        let event_type = if success {
            Some(ev.success)
        } else {
            ev.failure
        };
        if let Some(event_type) = event_type {
            vox_cli_core::gamify_shim::record_cli_event_fire_and_forget(
                event_type,
                success,
                Some(ev.capability_id),
                Some(ev.command_path),
            );
        }
    }
    result
}

async fn run_fabrica_cmd_inner(cmd: latin_cmd::FabricaCmd) -> anyhow::Result<()> {
    use latin_cmd::FabricaCmd;
    match cmd {
        FabricaCmd::Build(a) => {
            commands::build::run(
                &a.file,
                &a.out_dir,
                a.mobile_target.clone(),
                a.build_target.map(Into::into),
                a.scaffold,
                a.emit_ir,
                a.mode,
                vox_codegen::codegen_rust::RustAppShell::default(),
            )
            .await?;
        }
        FabricaCmd::Check(a) => {
            commands::check::run(&a).await?;
        }
        FabricaCmd::Test(a) => {
            commands::test::run(&a).await?;
        }
        FabricaCmd::Run(a) => {
            if let Some(p) = a.port {
                crate::config::set_process_vox_port(p);
            }
            let mut mode = a.mode;
            if a.interp {
                mode = commands::run::RunMode::Interp;
            } else if a.script {
                mode = commands::run::RunMode::Script;
            } else if a.app {
                mode = commands::run::RunMode::App;
            }
            commands::run::run(&a.file, &a.args, mode).await?;
        }
        FabricaCmd::Dev(a) => {
            commands::dev::run(&a.file, &a.out_dir, a.port, a.open, a.build_target).await?;
        }
        FabricaCmd::Bundle(a) => {
            commands::bundle::run(
                &a.file,
                &a.out_dir,
                a.target.as_deref(),
                a.release,
                a.mode,
                vox_codegen::codegen_rust::RustAppShell::default(),
            )
            .await?;
        }
        FabricaCmd::Compile(a) => {
            commands::compile::run(&a).await?;
        }
        FabricaCmd::Fmt(a) => {
            commands::fmt::run(&a.file, a.check)?;
        }
        #[cfg(feature = "script-execution")]
        FabricaCmd::Script(a) => {
            run_script_subcommand(&a, "fabrica").await?;
        }
    }
    Ok(())
}

pub(crate) async fn run_diag_cmd(cmd: latin_cmd::DiagCmd) -> anyhow::Result<()> {
    use latin_cmd::DiagCmd;
    match cmd {
        DiagCmd::Doctor(a) => {
            run_doctor_command(&a).await?;
        }
        #[cfg(any(feature = "codex", feature = "stub-check"))]
        DiagCmd::Architect { cmd } => {
            commands::diagnostics::tools::architect::run(cmd).await?;
        }
        #[cfg(feature = "stub-check")]
        DiagCmd::StubCheck(a) => {
            run_stub_check_command(&a).await?;
        }
    }
    Ok(())
}

pub(crate) async fn run_ars_cmd(cmd: latin_cmd::ArsCmd) -> anyhow::Result<()> {
    use latin_cmd::ArsCmd;
    match cmd {
        ArsCmd::Snippet { cmd } => {
            commands::extras::snippet_cli::run(cmd).await?;
        }
        ArsCmd::Share { cmd } => {
            commands::extras::share_cli::run(cmd).await?;
        }
        #[cfg(feature = "ars")]
        ArsCmd::Skill { cmd } => {
            commands::extras::skill_cmd::run(cmd).await?;
        }
        #[cfg(feature = "ars")]
        ArsCmd::Openclaw { action } => {
            run_openclaw_subcommand(action).await?;
        }
        #[cfg(feature = "extras-ludus")]
        ArsCmd::Ludus { cmd } => {
            commands::extras::ludus_cli::run(cmd).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabrica_reward_events_maps_known_commands() {
        let build = fabrica_reward_events("build").expect("build is rewarded");
        assert_eq!(build.success, "build_completed");
        assert_eq!(build.failure, Some("build_failed"));
        assert_eq!(build.capability_id, "cli.build");
        assert_eq!(build.command_path, "build");

        assert_eq!(
            fabrica_reward_events("check").map(|e| e.success),
            Some("check_completed")
        );
        assert_eq!(
            fabrica_reward_events("test").and_then(|e| e.failure),
            Some("test_fail")
        );

        // Commands the policy rewards on success but not failure → no hollow failure event.
        assert_eq!(fabrica_reward_events("fmt").and_then(|e| e.failure), None);
        assert_eq!(
            fabrica_reward_events("fmt").map(|e| e.success),
            Some("fmt_completed")
        );
        assert_eq!(
            fabrica_reward_events("bundle").and_then(|e| e.failure),
            None
        );
    }

    #[test]
    fn fabrica_reward_events_skips_unrewarded_lanes() {
        // No reward type defined for these lanes yet → emit nothing (not a hollow event).
        for lane in ["run", "dev", "compile", "script", "unknown"] {
            assert!(
                fabrica_reward_events(lane).is_none(),
                "lane `{lane}` should not emit a reward event"
            );
        }
    }
}
